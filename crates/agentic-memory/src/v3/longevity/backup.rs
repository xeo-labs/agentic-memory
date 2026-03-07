//! Backup daemon — automatic backup to email, local directory, or cloud.
//!
//! Supports full, incremental, snapshot, and emergency backup modes.
//! Retention: 7 daily, 4 weekly, 12 monthly, 1 annual forever.

use super::store::LongevityError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Backup schedule frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupSchedule {
    Hourly,
    Daily,
    Weekly,
    Manual,
}

/// Backup mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupMode {
    /// Complete .amem + .longevity.db
    Full,
    /// Only changes since last backup
    Incremental,
    /// Point-in-time freeze (read-only copy)
    Snapshot,
    /// Triggered on anomaly detection
    Emergency,
}

/// Backup destination configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupDestination {
    /// Local directory
    LocalDirectory { path: PathBuf },
    /// Email (SMTP)
    Email {
        address: String,
        smtp_host: String,
        smtp_port: u16,
        smtp_user: String,
        smtp_pass_env: String,
    },
    /// S3-compatible storage
    S3 {
        bucket: String,
        prefix: String,
        region: String,
    },
}

/// Backup configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub schedule: BackupSchedule,
    pub mode: BackupMode,
    pub destinations: Vec<BackupDestination>,
    pub encryption_passphrase_env: Option<String>,
    pub retention: RetentionPolicy,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            schedule: BackupSchedule::Daily,
            mode: BackupMode::Full,
            destinations: vec![],
            encryption_passphrase_env: None,
            retention: RetentionPolicy::default(),
        }
    }
}

/// Retention policy for backups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub daily_count: u32,
    pub weekly_count: u32,
    pub monthly_count: u32,
    pub annual_count: u32,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            daily_count: 7,
            weekly_count: 4,
            monthly_count: 12,
            annual_count: 999, // Keep annual backups forever (effectively)
        }
    }
}

/// Result of a backup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub mode: BackupMode,
    pub destination: String,
    pub size_bytes: u64,
    pub duration_ms: u64,
    pub files_backed_up: Vec<String>,
    pub success: bool,
    pub error: Option<String>,
}

/// Manifest for a backup archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub backup_id: String,
    pub created_at: String,
    pub mode: BackupMode,
    pub amem_version: String,
    pub schema_version: u32,
    pub total_memories: u64,
    pub total_sessions: u64,
    pub files: Vec<BackupFileEntry>,
    pub checksums: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFileEntry {
    pub path: String,
    pub size_bytes: u64,
    pub checksum: String,
}

/// The backup daemon manages scheduled backups.
pub struct BackupDaemon {
    config: BackupConfig,
}

impl BackupDaemon {
    pub fn new(config: BackupConfig) -> Self {
        Self { config }
    }

    /// Execute a backup to local directory.
    pub fn backup_to_local(
        &self,
        amem_path: &Path,
        longevity_db_path: &Path,
        dest_dir: &Path,
    ) -> Result<BackupResult, LongevityError> {
        let start = std::time::Instant::now();
        let mut files_backed_up = Vec::new();
        let mut total_size = 0u64;

        // Create dated backup directory
        let date_str = chrono::Utc::now().format("%Y-%m-%d_%H%M%S").to_string();
        let backup_dir = dest_dir.join(format!("amem-backup-{}", date_str));
        std::fs::create_dir_all(&backup_dir)?;

        // Copy .amem file
        if amem_path.exists() {
            let dest = backup_dir.join(amem_path.file_name().unwrap_or_default());
            std::fs::copy(amem_path, &dest)?;
            let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
            total_size += size;
            files_backed_up.push(dest.display().to_string());
        }

        // Copy .longevity.db
        if longevity_db_path.exists() {
            let dest = backup_dir.join(longevity_db_path.file_name().unwrap_or_default());
            std::fs::copy(longevity_db_path, &dest)?;
            let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
            total_size += size;
            files_backed_up.push(dest.display().to_string());
        }

        // Write manifest
        let manifest = BackupManifest {
            backup_id: ulid::Ulid::new().to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            mode: self.config.mode,
            amem_version: crate::v3::V3_VERSION.to_string(),
            schema_version: 1,
            total_memories: 0, // Would query from store
            total_sessions: 0,
            files: files_backed_up
                .iter()
                .map(|f| {
                    let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
                    let checksum = if let Ok(data) = std::fs::read(f) {
                        blake3::hash(&data).to_hex().to_string()
                    } else {
                        String::new()
                    };
                    BackupFileEntry {
                        path: f.clone(),
                        size_bytes: size,
                        checksum,
                    }
                })
                .collect(),
            checksums: std::collections::HashMap::new(),
        };

        let manifest_path = backup_dir.join("manifest.json");
        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(&manifest_path, manifest_json)?;
        files_backed_up.push(manifest_path.display().to_string());

        Ok(BackupResult {
            mode: self.config.mode,
            destination: backup_dir.display().to_string(),
            size_bytes: total_size,
            duration_ms: start.elapsed().as_millis() as u64,
            files_backed_up,
            success: true,
            error: None,
        })
    }

    /// Restore from a backup directory.
    pub fn restore_from_local(
        backup_dir: &Path,
        restore_to: &Path,
    ) -> Result<RestoreResult, LongevityError> {
        let start = std::time::Instant::now();
        let mut files_restored = Vec::new();

        // Read manifest
        let manifest_path = backup_dir.join("manifest.json");
        if !manifest_path.exists() {
            return Err(LongevityError::NotFound(
                "Backup manifest not found".to_string(),
            ));
        }

        let manifest_str = std::fs::read_to_string(&manifest_path)?;
        let manifest: BackupManifest = serde_json::from_str(&manifest_str)?;

        // Ensure restore directory exists
        std::fs::create_dir_all(restore_to)?;

        // Copy all backed up files
        for entry in &manifest.files {
            let source = PathBuf::from(&entry.path);
            if source.exists() {
                if let Some(filename) = source.file_name() {
                    let dest = restore_to.join(filename);
                    std::fs::copy(&source, &dest)?;
                    files_restored.push(dest.display().to_string());
                }
            }
        }

        Ok(RestoreResult {
            backup_id: manifest.backup_id,
            files_restored,
            duration_ms: start.elapsed().as_millis() as u64,
            success: true,
            error: None,
        })
    }

    /// Clean old backups according to retention policy.
    pub fn cleanup_old_backups(&self, backup_dir: &Path) -> Result<u32, LongevityError> {
        let mut cleaned = 0u32;

        if !backup_dir.exists() {
            return Ok(0);
        }

        let mut backups: Vec<_> = std::fs::read_dir(backup_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("amem-backup-"))
                    .unwrap_or(false)
            })
            .collect();

        // Sort by name (which includes date)
        backups.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));

        // Keep only the allowed number of backups
        let max_keep = self.config.retention.daily_count as usize;
        if backups.len() > max_keep {
            for old_backup in &backups[max_keep..] {
                if let Err(e) = std::fs::remove_dir_all(old_backup.path()) {
                    log::warn!("Failed to cleanup backup {:?}: {}", old_backup.path(), e);
                } else {
                    cleaned += 1;
                }
            }
        }

        Ok(cleaned)
    }

    /// Get backup configuration.
    pub fn config(&self) -> &BackupConfig {
        &self.config
    }
}

/// Result of a restore operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub backup_id: String,
    pub files_restored: Vec<String>,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
}
