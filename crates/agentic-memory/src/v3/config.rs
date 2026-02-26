//! Complete V3 configuration with TOML persistence.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Complete V3 configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryV3Config {
    /// Data directory
    pub data_dir: PathBuf,

    /// Storage configuration
    pub storage: StorageConfig,

    /// Index configuration
    pub indexes: IndexConfig,

    /// Embedding configuration
    pub embeddings: EmbeddingConfig,

    /// Encryption configuration
    pub encryption: EncryptionConfig,

    /// Auto-capture configuration
    pub auto_capture: AutoCaptureConfig,

    /// Performance configuration
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Hot tier max size (bytes)
    pub hot_max_bytes: usize,

    /// Warm tier max size (bytes)
    pub warm_max_bytes: usize,

    /// Cold tier compression level
    pub cold_compression: String,

    /// Frozen tier compression level
    pub frozen_compression: String,

    /// Auto-archive after days
    pub archive_after_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// Enable temporal index
    pub temporal: bool,

    /// Enable semantic index
    pub semantic: bool,

    /// Enable causal index
    pub causal: bool,

    /// Enable entity index
    pub entity: bool,

    /// Enable procedural index
    pub procedural: bool,

    /// Rebuild indexes on startup
    pub rebuild_on_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Embedding provider: "none", "tfidf", "local", "api"
    pub provider: String,

    /// Embedding dimension
    pub dimension: usize,

    /// For API provider: endpoint URL
    pub api_url: Option<String>,

    /// For API provider: API key (from env var)
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Enable encryption
    pub enabled: bool,

    /// Key derivation: "password" or "keyfile"
    pub key_source: String,

    /// For keyfile: path to key file
    pub keyfile_path: Option<PathBuf>,

    /// For password: env var name containing password
    pub password_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCaptureConfig {
    /// Enable auto-capture
    pub enabled: bool,

    /// Capture messages
    pub messages: bool,

    /// Capture tool calls
    pub tools: bool,

    /// Capture file operations
    pub files: bool,

    /// Checkpoint interval (blocks)
    pub checkpoint_interval: u32,

    /// Context pressure threshold (0.0 - 1.0)
    pub pressure_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Memory map threshold (use mmap above this size)
    pub mmap_threshold_bytes: usize,

    /// Index cache size (entries)
    pub index_cache_size: usize,

    /// Write buffer size (bytes)
    pub write_buffer_size: usize,

    /// Background thread count
    pub background_threads: usize,
}

impl Default for MemoryV3Config {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from(".agentic/memory"),
            storage: StorageConfig {
                hot_max_bytes: 10 * 1024 * 1024,
                warm_max_bytes: 100 * 1024 * 1024,
                cold_compression: "default".to_string(),
                frozen_compression: "best".to_string(),
                archive_after_days: 365,
            },
            indexes: IndexConfig {
                temporal: true,
                semantic: true,
                causal: true,
                entity: true,
                procedural: true,
                rebuild_on_start: false,
            },
            embeddings: EmbeddingConfig {
                provider: "tfidf".to_string(),
                dimension: 384,
                api_url: None,
                api_key_env: None,
            },
            encryption: EncryptionConfig {
                enabled: false,
                key_source: "password".to_string(),
                keyfile_path: None,
                password_env: None,
            },
            auto_capture: AutoCaptureConfig {
                enabled: true,
                messages: true,
                tools: true,
                files: true,
                checkpoint_interval: 50,
                pressure_threshold: 0.75,
            },
            performance: PerformanceConfig {
                mmap_threshold_bytes: 1024 * 1024,
                index_cache_size: 10000,
                write_buffer_size: 64 * 1024,
                background_threads: 2,
            },
        }
    }
}

impl MemoryV3Config {
    /// Load from TOML file
    pub fn load(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save to TOML file
    pub fn save(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load from default location or create default
    pub fn load_or_default() -> Self {
        let default_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agentic")
            .join("memory.toml");

        Self::load(&default_path).unwrap_or_default()
    }
}
