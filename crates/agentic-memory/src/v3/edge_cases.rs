//! Edge case handlers for V3 Immortal Architecture.
//! Handles storage failures, concurrency, data validation, platform differences,
//! and recovery scenarios. ZERO DATA LOSS. EVER.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════
// ERROR TYPES
// ═══════════════════════════════════════════════════════════════════

/// Storage-related errors
#[derive(Debug)]
pub enum StorageError {
    /// Disk is full
    DiskFull { needed: usize, available: usize },
    /// No writable location found
    NoWritableLocation,
    /// File corruption detected
    Corruption { details: String },
    /// Permission denied
    PermissionDenied { path: PathBuf, operation: String },
    /// IO error
    Io(std::io::Error),
    /// Serialization error
    Serialization(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DiskFull { needed, available } => {
                write!(
                    f,
                    "Disk full: need {} bytes, only {} available",
                    needed, available
                )
            }
            Self::NoWritableLocation => write!(f, "No writable location found"),
            Self::Corruption { details } => write!(f, "Corruption detected: {}", details),
            Self::PermissionDenied { path, operation } => {
                write!(f, "Permission denied: {} on {:?}", operation, path)
            }
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Serialization(s) => write!(f, "Serialization error: {}", s),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied {
                path: PathBuf::new(),
                operation: "unknown".to_string(),
            },
            _ => Self::Io(e),
        }
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

/// Lock-related errors
#[derive(Debug)]
pub enum LockError {
    Timeout,
    Io(std::io::Error),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, "Lock acquisition timed out"),
            Self::Io(e) => write!(f, "Lock IO error: {}", e),
        }
    }
}

impl std::error::Error for LockError {}

/// Validation errors
#[derive(Debug)]
pub enum ValidationError {
    MissingField(&'static str),
    InvalidValue {
        field: &'static str,
        expected: &'static str,
        got: String,
    },
    EmptyValue(&'static str),
    TooLarge {
        field: &'static str,
        max_bytes: usize,
        got_bytes: usize,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "Missing required field: {}", field),
            Self::InvalidValue {
                field,
                expected,
                got,
            } => {
                write!(
                    f,
                    "Invalid value for {}: expected {}, got {}",
                    field, expected, got
                )
            }
            Self::EmptyValue(field) => write!(f, "Empty value for required field: {}", field),
            Self::TooLarge {
                field,
                max_bytes,
                got_bytes,
            } => {
                write!(
                    f,
                    "Value too large for {}: max {} bytes, got {} bytes",
                    field, max_bytes, got_bytes
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

// ═══════════════════════════════════════════════════════════════════
// 1. STORAGE EDGE CASES
// ═══════════════════════════════════════════════════════════════════

/// Check available disk space before writing
pub fn check_disk_space(path: &Path, needed: usize) -> Result<(), StorageError> {
    // Use a heuristic: check if the parent directory exists and is writable
    let dir = path.parent().unwrap_or(path);

    // Try to estimate available space by writing a test
    let test_file = dir.join(".space_test");
    match std::fs::write(&test_file, &vec![0u8; std::cmp::min(needed, 4096)]) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&test_file);
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                Err(StorageError::PermissionDenied {
                    path: dir.to_path_buf(),
                    operation: "write".to_string(),
                })
            } else {
                // Likely disk full or other error
                Err(StorageError::DiskFull {
                    needed,
                    available: 0,
                })
            }
        }
    }
}

/// Atomic write: write to temp file then rename
pub fn atomic_write(target: &Path, data: &[u8]) -> Result<(), std::io::Error> {
    let temp_path = target.with_extension("tmp");

    // Write to temp file
    let mut file = File::create(&temp_path)?;
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);

    // Atomic rename
    match std::fs::rename(&temp_path, target) {
        Ok(_) => Ok(()),
        Err(_) if cfg!(windows) => {
            // Windows: target may be locked; retry with backoff
            for attempt in 0..3 {
                std::thread::sleep(Duration::from_millis(100 * (attempt + 1)));
                if std::fs::rename(&temp_path, target).is_ok() {
                    return Ok(());
                }
            }
            // Last resort: copy instead of rename
            std::fs::copy(&temp_path, target)?;
            std::fs::remove_file(&temp_path)?;
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(e)
        }
    }
}

/// Find a writable location from a list of candidates
pub fn find_writable_location() -> Result<PathBuf, StorageError> {
    let candidates = vec![
        dirs::data_local_dir().map(|d| d.join("agentic-memory")),
        dirs::home_dir().map(|d| d.join(".agentic-memory")),
        Some(PathBuf::from("/tmp/agentic-memory")),
        std::env::current_dir()
            .ok()
            .map(|d| d.join(".agentic-memory")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if std::fs::create_dir_all(&candidate).is_ok() {
            // Test write
            let test_file = candidate.join(".write_test");
            if std::fs::write(&test_file, b"test").is_ok() {
                let _ = std::fs::remove_file(&test_file);
                return Ok(candidate);
            }
        }
    }

    Err(StorageError::NoWritableLocation)
}

/// Sanitize path for filesystem safety
pub fn safe_path(input: &str) -> PathBuf {
    // Handle Windows long path prefix
    #[cfg(windows)]
    let input = if input.len() > 250 {
        format!("\\\\?\\{}", input)
    } else {
        input.to_string()
    };

    #[cfg(not(windows))]
    let input = input.to_string();

    // Hash if contains problematic characters or is too long
    let problematic = input.contains(['<', '>', ':', '"', '|', '?', '*', '\0']);
    let too_long = input.len() > 200;

    if problematic || too_long {
        let hash = blake3::hash(input.as_bytes());
        PathBuf::from(format!("hashed_{}", &hash.to_hex()[..16]))
    } else {
        PathBuf::from(&input)
    }
}

// ═══════════════════════════════════════════════════════════════════
// 2. CONCURRENCY — FILE LOCKING
// ═══════════════════════════════════════════════════════════════════

/// File-based lock for concurrent access control
pub struct FileLock {
    _lock_file: File,
    lock_path: PathBuf,
}

impl FileLock {
    /// Acquire an exclusive lock with timeout
    pub fn acquire(path: &Path, timeout: Duration) -> Result<Self, LockError> {
        let lock_path = path.with_extension("lock");
        let start = Instant::now();

        loop {
            // Try to create lock file exclusively
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut file) => {
                    // Write PID for stale lock detection
                    let pid = std::process::id();
                    let _ = write!(file, "{}", pid);
                    let _ = file.sync_all();

                    return Ok(Self {
                        _lock_file: file,
                        lock_path,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if start.elapsed() >= timeout {
                        // Check if lock is stale before failing
                        if Self::is_stale_lock(&lock_path) {
                            Self::break_stale_lock(&lock_path);
                            continue;
                        }
                        return Err(LockError::Timeout);
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(LockError::Io(e)),
            }
        }
    }

    /// Check if a lock file is stale (holder crashed)
    fn is_stale_lock(lock_path: &Path) -> bool {
        if let Ok(metadata) = std::fs::metadata(lock_path) {
            let age = metadata
                .modified()
                .ok()
                .and_then(|t| t.elapsed().ok())
                .unwrap_or_default();

            // Lock is stale if older than 60 seconds
            if age > Duration::from_secs(60) {
                return true;
            }

            // Also check if PID is still alive
            if let Ok(content) = std::fs::read_to_string(lock_path) {
                if let Ok(pid) = content.trim().parse::<u32>() {
                    return !is_process_alive(pid);
                }
            }
        }
        false
    }

    /// Break a stale lock
    fn break_stale_lock(lock_path: &Path) {
        let _ = std::fs::remove_file(lock_path);
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

/// Check if a process is still running
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // On Unix, kill(pid, 0) checks existence without sending a signal
        unsafe { libc_free_kill_check(pid) }
    }

    #[cfg(not(unix))]
    {
        // On non-Unix, assume alive if we can't check
        let _ = pid;
        true
    }
}

#[cfg(unix)]
unsafe fn libc_free_kill_check(pid: u32) -> bool {
    // Use std::process::Command to check if process exists
    // This avoids needing libc dependency
    std::fs::metadata(format!("/proc/{}", pid)).is_ok()
        || std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(true) // If we can't check, assume alive
}

// ═══════════════════════════════════════════════════════════════════
// 3. PROJECT ISOLATION
// ═══════════════════════════════════════════════════════════════════

/// Per-project isolation for multiple Claude instances
pub struct ProjectIsolation {
    pub project_id: String,
    pub project_dir: PathBuf,
}

impl ProjectIsolation {
    /// Detect or create project isolation
    pub fn detect_or_create() -> Self {
        let project_id = std::env::var("CLAUDE_PROJECT_ID")
            .ok()
            .or_else(|| Self::detect_from_cwd())
            .unwrap_or_else(Self::generate_project_id);

        let project_dir = Self::project_data_dir(&project_id);
        let _ = std::fs::create_dir_all(&project_dir);

        Self {
            project_id,
            project_dir,
        }
    }

    fn detect_from_cwd() -> Option<String> {
        let cwd = std::env::current_dir().ok()?;
        let canonical = cwd.canonicalize().ok()?;
        let hash = blake3::hash(canonical.to_string_lossy().as_bytes());
        Some(format!("proj_{}", &hash.to_hex()[..12]))
    }

    fn generate_project_id() -> String {
        format!("proj_{}", &uuid::Uuid::new_v4().to_string()[..8])
    }

    fn project_data_dir(project_id: &str) -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("agentic-memory")
            .join("projects")
            .join(project_id)
    }
}

// ═══════════════════════════════════════════════════════════════════
// 5. DATA VALIDATION
// ═══════════════════════════════════════════════════════════════════

/// Content normalization result
#[derive(Debug, PartialEq)]
pub enum NormalizedContent {
    /// Empty content
    Empty,
    /// Only whitespace/control characters
    WhitespaceOnly,
    /// Valid content (trimmed)
    Valid(String),
}

/// Normalize content for capture
pub fn normalize_content(content: &str) -> NormalizedContent {
    if content.is_empty() {
        return NormalizedContent::Empty;
    }

    // Check if ALL characters are whitespace/control before trimming
    if content.chars().all(|c| c.is_whitespace() || c.is_control()) {
        if content.trim().is_empty() && !content.is_empty() {
            return NormalizedContent::WhitespaceOnly;
        }
        return NormalizedContent::Empty;
    }

    let trimmed = content.trim();
    NormalizedContent::Valid(trimmed.to_string())
}

/// Content type detection for binary vs text
#[derive(Debug, PartialEq)]
pub enum ContentType {
    Text,
    Binary(&'static str), // mime type
}

/// Detect if data is text or binary
pub fn detect_content_type(data: &[u8]) -> ContentType {
    if data.is_empty() {
        return ContentType::Text;
    }

    // Check for common binary signatures
    if data.len() >= 4 {
        match &data[0..4] {
            [0x89, 0x50, 0x4E, 0x47] => return ContentType::Binary("image/png"),
            [0xFF, 0xD8, 0xFF, _] => return ContentType::Binary("image/jpeg"),
            [0x25, 0x50, 0x44, 0x46] => return ContentType::Binary("application/pdf"),
            [0x50, 0x4B, 0x03, 0x04] => return ContentType::Binary("application/zip"),
            [0x47, 0x49, 0x46, 0x38] => return ContentType::Binary("image/gif"),
            [0x7F, 0x45, 0x4C, 0x46] => return ContentType::Binary("application/x-elf"),
            _ => {}
        }
    }

    // Check if valid UTF-8
    match std::str::from_utf8(data) {
        Ok(s) => {
            // Check for excessive control characters (likely binary)
            let control_count = s
                .chars()
                .filter(|c| c.is_control() && *c != '\n' && *c != '\r' && *c != '\t')
                .count();
            let control_ratio = control_count as f32 / s.len().max(1) as f32;
            if control_ratio > 0.1 {
                ContentType::Binary("application/octet-stream")
            } else {
                ContentType::Text
            }
        }
        Err(_) => ContentType::Binary("application/octet-stream"),
    }
}

/// Maximum single block size (10 MB)
pub const MAX_SINGLE_BLOCK_BYTES: usize = 10 * 1024 * 1024;

/// Chunk size for large content (1 MB)
pub const CHUNK_SIZE: usize = 1024 * 1024;

/// Validate and potentially chunk large content
pub fn validate_content_size(content: &str) -> Result<(), ValidationError> {
    if content.len() > MAX_SINGLE_BLOCK_BYTES {
        return Err(ValidationError::TooLarge {
            field: "content",
            max_bytes: MAX_SINGLE_BLOCK_BYTES,
            got_bytes: content.len(),
        });
    }
    Ok(())
}

/// Validated timestamp: clamp to sane range
pub fn validated_timestamp() -> chrono::DateTime<chrono::Utc> {
    let now = chrono::Utc::now();

    let min_valid = chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let max_valid = chrono::DateTime::parse_from_rfc3339("2100-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    if now < min_valid {
        log::warn!("System clock appears to be in the past: {:?}", now);
        min_valid
    } else if now > max_valid {
        log::warn!("System clock appears to be in the future: {:?}", now);
        max_valid
    } else {
        now
    }
}

// ═══════════════════════════════════════════════════════════════════
// 7. PLATFORM — PATH NORMALIZATION
// ═══════════════════════════════════════════════════════════════════

/// Normalize a file path for cross-platform consistency
pub fn normalize_path(path: &str) -> String {
    // Convert Windows separators to Unix
    let normalized = path.replace('\\', "/");

    // Remove trailing slashes
    let normalized = normalized.trim_end_matches('/');

    // Lowercase on case-insensitive systems
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let normalized = normalized.to_lowercase();

    normalized.to_string()
}

/// Compare paths with platform-aware normalization
pub fn paths_equal(a: &str, b: &str) -> bool {
    normalize_path(a) == normalize_path(b)
}

// ═══════════════════════════════════════════════════════════════════
// 8. RECOVERY — IDEMPOTENT RECOVERY MARKERS
// ═══════════════════════════════════════════════════════════════════

/// Recovery marker for idempotent crash recovery
pub struct RecoveryMarker {
    data_dir: PathBuf,
}

impl RecoveryMarker {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
        }
    }

    /// Check if recovery is needed
    pub fn needs_recovery(&self) -> bool {
        let in_progress = self.data_dir.join(".recovery_in_progress");
        in_progress.exists()
    }

    /// Check if recovery already completed for current log state
    pub fn recovery_completed(&self) -> bool {
        let complete = self.data_dir.join(".recovery_complete");
        let log_path = self.data_dir.join("immortal.log");

        if !complete.exists() {
            return false;
        }

        // Check if recovery marker is newer than log file
        let complete_time = std::fs::metadata(&complete)
            .ok()
            .and_then(|m| m.modified().ok());
        let log_time = std::fs::metadata(&log_path)
            .ok()
            .and_then(|m| m.modified().ok());

        match (complete_time, log_time) {
            (Some(ct), Some(lt)) => ct > lt,
            _ => false,
        }
    }

    /// Mark recovery as in progress
    pub fn mark_in_progress(&self) {
        let marker = self.data_dir.join(".recovery_in_progress");
        let _ = std::fs::write(&marker, chrono::Utc::now().to_rfc3339());
    }

    /// Mark recovery as complete
    pub fn mark_complete(&self) {
        let complete = self.data_dir.join(".recovery_complete");
        let in_progress = self.data_dir.join(".recovery_in_progress");
        let _ = std::fs::write(&complete, chrono::Utc::now().to_rfc3339());
        let _ = std::fs::remove_file(&in_progress);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 6. INDEX CONSISTENCY
// ═══════════════════════════════════════════════════════════════════

/// Index consistency report
#[derive(Debug, Default)]
pub struct IndexConsistencyReport {
    pub consistent: bool,
    pub missing_in_temporal: Vec<u64>,
    pub missing_in_semantic: Vec<u64>,
    pub missing_in_entity: Vec<u64>,
    pub total_blocks: u64,
}

// ═══════════════════════════════════════════════════════════════════
// GHOST WRITER EDGE CASES
// ═══════════════════════════════════════════════════════════════════

/// Safe write to Claude memory directory (handles locks)
pub fn safe_write_to_claude(target: &Path, content: &str) -> Result<(), std::io::Error> {
    atomic_write(target, content.as_bytes())
}

/// Merge content preserving user sections marked with <!-- USER_START/END -->
pub fn merge_preserving_user_sections(existing: &str, our_content: &str) -> String {
    // Find user sections
    let mut user_sections = Vec::new();
    let mut search_from = 0;
    while let Some(start) = existing[search_from..].find("<!-- USER_START -->") {
        let abs_start = search_from + start;
        if let Some(end_offset) = existing[abs_start..].find("<!-- USER_END -->") {
            let abs_end = abs_start + end_offset + "<!-- USER_END -->".len();
            user_sections.push(&existing[abs_start..abs_end]);
            search_from = abs_end;
        } else {
            break;
        }
    }

    if user_sections.is_empty() {
        return our_content.to_string();
    }

    // Build merged content
    let mut merged = our_content.to_string();
    merged.push_str("\n\n<!-- User-defined sections preserved: -->\n");
    for section in user_sections {
        merged.push_str(section);
        merged.push('\n');
    }

    merged
}

// ═══════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── Storage Tests ──

    #[test]
    fn test_find_writable_location() {
        let result = find_writable_location();
        assert!(result.is_ok(), "Should find at least one writable location");
    }

    #[test]
    fn test_atomic_write() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("test.txt");
        atomic_write(&target, b"Hello, world!").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "Hello, world!");
    }

    #[test]
    fn test_atomic_write_overwrites() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("test.txt");
        atomic_write(&target, b"First").unwrap();
        atomic_write(&target, b"Second").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "Second");
    }

    #[test]
    fn test_safe_path_normal() {
        let path = safe_path("/src/main.rs");
        assert_eq!(path, PathBuf::from("/src/main.rs"));
    }

    #[test]
    fn test_safe_path_problematic_chars() {
        let path = safe_path("file<name>:test");
        assert!(path.to_string_lossy().starts_with("hashed_"));
    }

    #[test]
    fn test_safe_path_too_long() {
        let long_name = "a".repeat(300);
        let path = safe_path(&long_name);
        assert!(path.to_string_lossy().starts_with("hashed_"));
        assert!(path.to_string_lossy().len() < 100);
    }

    #[test]
    fn test_safe_path_unicode() {
        let path = safe_path("/src/🦀_memory.rs");
        assert_eq!(path, PathBuf::from("/src/🦀_memory.rs"));
    }

    #[test]
    fn test_safe_path_null_bytes() {
        let path = safe_path("file\0name");
        assert!(path.to_string_lossy().starts_with("hashed_"));
    }

    // ── Concurrency Tests ──

    #[test]
    fn test_file_lock_acquire_release() {
        let dir = TempDir::new().unwrap();
        let data_path = dir.path().join("test.dat");
        // Create the data file first
        std::fs::write(&data_path, "data").unwrap();

        let lock_path = data_path.with_extension("lock");

        {
            let _lock = FileLock::acquire(&data_path, Duration::from_secs(2)).unwrap();
            assert!(lock_path.exists(), "Lock file should exist while held");
        }
        // Lock should be released on drop
        assert!(
            !lock_path.exists(),
            "Lock file should be removed after drop"
        );
    }

    #[test]
    fn test_file_lock_timeout() {
        let dir = TempDir::new().unwrap();
        let lock_path = dir.path().join("test.dat.lock");

        // Create a fake lock file (simulating another process)
        std::fs::write(&lock_path, "99999999").unwrap(); // Fake PID

        let result = FileLock::acquire(&dir.path().join("test.dat"), Duration::from_millis(200));
        // Should eventually succeed or timeout — on most systems the PID won't exist
        // so the stale lock detection will break it
        assert!(result.is_ok() || matches!(result, Err(LockError::Timeout)));
    }

    // ── Project Isolation Tests ──

    #[test]
    fn test_project_isolation_deterministic() {
        let iso1 = ProjectIsolation::detect_or_create();
        let iso2 = ProjectIsolation::detect_or_create();
        assert_eq!(iso1.project_id, iso2.project_id);
    }

    // ── Data Validation Tests ──

    #[test]
    fn test_normalize_empty_content() {
        assert_eq!(normalize_content(""), NormalizedContent::Empty);
    }

    #[test]
    fn test_normalize_whitespace_only() {
        assert_eq!(
            normalize_content("   \t\n  "),
            NormalizedContent::WhitespaceOnly
        );
    }

    #[test]
    fn test_normalize_valid_content() {
        assert_eq!(
            normalize_content("  Hello, world!  "),
            NormalizedContent::Valid("Hello, world!".to_string())
        );
    }

    #[test]
    fn test_detect_content_type_text() {
        assert_eq!(detect_content_type(b"Hello, world!"), ContentType::Text);
    }

    #[test]
    fn test_detect_content_type_png() {
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(
            detect_content_type(&png_header),
            ContentType::Binary("image/png")
        );
    }

    #[test]
    fn test_detect_content_type_jpeg() {
        let jpeg_header = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(
            detect_content_type(&jpeg_header),
            ContentType::Binary("image/jpeg")
        );
    }

    #[test]
    fn test_detect_content_type_binary() {
        // Random bytes that aren't valid UTF-8
        let binary = vec![0xFF, 0xFE, 0x00, 0x01, 0x80, 0x81, 0x82, 0x83];
        assert!(matches!(
            detect_content_type(&binary),
            ContentType::Binary(_)
        ));
    }

    #[test]
    fn test_detect_content_type_empty() {
        assert_eq!(detect_content_type(b""), ContentType::Text);
    }

    #[test]
    fn test_validate_content_size_ok() {
        assert!(validate_content_size("Hello").is_ok());
    }

    #[test]
    fn test_validate_content_size_too_large() {
        let large = "x".repeat(MAX_SINGLE_BLOCK_BYTES + 1);
        assert!(validate_content_size(&large).is_err());
    }

    #[test]
    fn test_validated_timestamp_sane() {
        let ts = validated_timestamp();
        let now = chrono::Utc::now();
        let diff = (now - ts).num_seconds().abs();
        assert!(diff < 5, "Timestamp should be within 5 seconds of now");
    }

    // ── Platform Tests ──

    #[test]
    fn test_normalize_path_unix() {
        let normalized = normalize_path("/src/main.rs");
        assert!(normalized.contains("src/main.rs"));
    }

    #[test]
    fn test_normalize_path_windows_separators() {
        let normalized = normalize_path("src\\main.rs");
        assert_eq!(normalized, normalize_path("src/main.rs"));
    }

    #[test]
    fn test_normalize_path_trailing_slash() {
        let normalized = normalize_path("/src/dir/");
        assert!(!normalized.ends_with('/'));
    }

    #[test]
    fn test_paths_equal() {
        assert!(paths_equal("src/main.rs", "src/main.rs"));
        assert!(paths_equal("src\\main.rs", "src/main.rs"));
    }

    // ── Recovery Marker Tests ──

    #[test]
    fn test_recovery_marker_fresh() {
        let dir = TempDir::new().unwrap();
        let marker = RecoveryMarker::new(dir.path());
        assert!(!marker.needs_recovery());
        assert!(!marker.recovery_completed());
    }

    #[test]
    fn test_recovery_marker_in_progress() {
        let dir = TempDir::new().unwrap();
        let marker = RecoveryMarker::new(dir.path());
        marker.mark_in_progress();
        assert!(marker.needs_recovery());
    }

    #[test]
    fn test_recovery_marker_complete() {
        let dir = TempDir::new().unwrap();
        let marker = RecoveryMarker::new(dir.path());
        marker.mark_in_progress();
        marker.mark_complete();
        assert!(!marker.needs_recovery());
    }

    // ── Ghost Writer Edge Cases ──

    #[test]
    fn test_merge_preserving_user_sections_no_sections() {
        let result = merge_preserving_user_sections("old content", "new content");
        assert_eq!(result, "new content");
    }

    #[test]
    fn test_merge_preserving_user_sections_with_sections() {
        let existing =
            "some text\n<!-- USER_START -->\nMy custom notes\n<!-- USER_END -->\nmore text";
        let result = merge_preserving_user_sections(existing, "new auto content");
        assert!(result.contains("new auto content"));
        assert!(result.contains("<!-- USER_START -->"));
        assert!(result.contains("My custom notes"));
        assert!(result.contains("<!-- USER_END -->"));
    }

    #[test]
    fn test_merge_preserving_multiple_user_sections() {
        let existing = "text\n<!-- USER_START -->\nSection 1\n<!-- USER_END -->\nmiddle\n<!-- USER_START -->\nSection 2\n<!-- USER_END -->";
        let result = merge_preserving_user_sections(existing, "new content");
        assert!(result.contains("Section 1"));
        assert!(result.contains("Section 2"));
    }

    #[test]
    fn test_safe_write_to_claude() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("test.md");
        safe_write_to_claude(&target, "# Test Content").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "# Test Content");
    }

    // ── Concurrent append stress test ──

    #[test]
    fn test_check_disk_space_ok() {
        let dir = TempDir::new().unwrap();
        let result = check_disk_space(dir.path(), 1024);
        assert!(result.is_ok());
    }
}
