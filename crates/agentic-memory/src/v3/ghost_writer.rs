//! The Ghost Writer: Automatic sync to ALL AI coding assistants.
//!
//! Supports:
//! - Claude Code  (~/.claude/memory)
//! - Cursor       (~/.cursor/memory)
//! - Windsurf     (~/.windsurf/memory)
//! - Cody         (~/.sourcegraph/cody/memory)
//!
//! Zero user configuration. Zero tool calls. Just works.
//! **We build for ALL AI agents. Not just Claude.**

use super::edge_cases;
use super::engine::{MemoryEngineV3, SessionResumeResult};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const START_MARKER: &str = "<!-- AGENTIC_MEMORY_V3_START -->";
const END_MARKER: &str = "<!-- AGENTIC_MEMORY_V3_END -->";

// ═══════════════════════════════════════════════════════════════════
// Multi-client support
// ═══════════════════════════════════════════════════════════════════

/// Supported AI coding assistants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientType {
    /// Claude Code (Anthropic)
    Claude,
    /// Cursor (AI-first IDE)
    Cursor,
    /// Windsurf (Codeium)
    Windsurf,
    /// Cody (Sourcegraph)
    Cody,
}

impl ClientType {
    /// The filename to write in the client's memory directory.
    pub fn memory_filename(&self) -> &'static str {
        match self {
            ClientType::Claude => "V3_CONTEXT.md",
            ClientType::Cursor => "agentic-memory.md",
            ClientType::Windsurf => "agentic-memory.md",
            ClientType::Cody => "agentic-memory.md",
        }
    }

    /// Human-readable name for logging.
    pub fn display_name(&self) -> &'static str {
        match self {
            ClientType::Claude => "Claude Code",
            ClientType::Cursor => "Cursor",
            ClientType::Windsurf => "Windsurf",
            ClientType::Cody => "Cody",
        }
    }

    /// Return all known client types.
    pub fn all() -> &'static [ClientType] {
        &[
            ClientType::Claude,
            ClientType::Cursor,
            ClientType::Windsurf,
            ClientType::Cody,
        ]
    }
}

/// A detected client with its memory directory.
#[derive(Debug, Clone)]
pub struct DetectedClient {
    pub client_type: ClientType,
    pub memory_dir: PathBuf,
}

/// The Ghost Writer daemon.
/// Runs in background, syncs context to ALL detected AI coding assistants.
pub struct GhostWriter {
    /// Our V3 engine
    engine: Arc<MemoryEngineV3>,

    /// Claude Code's memory directory (auto-detected, re-checked periodically)
    /// Kept for backward compat — also used as the primary detect target
    claude_memory_dir: Mutex<Option<PathBuf>>,

    /// ALL detected client memory directories
    detected_clients: Mutex<Vec<DetectedClient>>,

    /// Sync interval
    sync_interval: Duration,

    /// Running flag
    running: Arc<AtomicBool>,

    /// Last sync time
    last_sync: Mutex<Option<chrono::DateTime<Utc>>>,

    /// Detection interval for re-checking client directories
    detection_interval: Duration,
}

impl GhostWriter {
    /// Create and AUTO-START the ghost writer (syncs to ALL detected clients)
    pub fn spawn(engine: Arc<MemoryEngineV3>) -> Arc<Self> {
        let clients = Self::detect_all_memory_dirs();
        let claude_dir = clients
            .iter()
            .find(|c| c.client_type == ClientType::Claude)
            .map(|c| c.memory_dir.clone());

        for c in &clients {
            log::info!("{} detected at {:?}", c.client_type.display_name(), c.memory_dir);
        }

        let writer = Arc::new(Self {
            engine,
            claude_memory_dir: Mutex::new(claude_dir),
            detected_clients: Mutex::new(clients),
            sync_interval: Duration::from_secs(5),
            running: Arc::new(AtomicBool::new(true)),
            last_sync: Mutex::new(None),
            detection_interval: Duration::from_secs(300), // Re-check every 5 min
        });

        writer.clone().start_background_sync();
        writer
    }

    /// Spawn only if ANY AI client is detected; returns None if none installed
    pub fn spawn_if_available(engine: Arc<MemoryEngineV3>) -> Option<Arc<Self>> {
        let clients = Self::detect_all_memory_dirs();
        if clients.is_empty() {
            log::info!("No AI coding assistants detected. Ghost writer disabled. Memory still works via MCP tools.");
            return None;
        }

        let claude_dir = clients
            .iter()
            .find(|c| c.client_type == ClientType::Claude)
            .map(|c| c.memory_dir.clone());

        for c in &clients {
            log::info!("{} detected at {:?}", c.client_type.display_name(), c.memory_dir);
        }

        let writer = Arc::new(Self {
            engine,
            claude_memory_dir: Mutex::new(claude_dir),
            detected_clients: Mutex::new(clients),
            sync_interval: Duration::from_secs(5),
            running: Arc::new(AtomicBool::new(true)),
            last_sync: Mutex::new(None),
            detection_interval: Duration::from_secs(300),
        });
        writer.clone().start_background_sync();
        Some(writer)
    }

    /// Create without auto-start (for testing)
    pub fn new(engine: Arc<MemoryEngineV3>) -> Self {
        let clients = Self::detect_all_memory_dirs();
        let claude_dir = clients
            .iter()
            .find(|c| c.client_type == ClientType::Claude)
            .map(|c| c.memory_dir.clone());

        Self {
            engine,
            claude_memory_dir: Mutex::new(claude_dir),
            detected_clients: Mutex::new(clients),
            sync_interval: Duration::from_secs(5),
            running: Arc::new(AtomicBool::new(false)),
            last_sync: Mutex::new(None),
            detection_interval: Duration::from_secs(300),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Multi-client detection
    // ═══════════════════════════════════════════════════════════════

    /// Detect ALL AI coding assistant memory directories.
    /// Returns every client whose config directory exists or can be created.
    pub fn detect_all_memory_dirs() -> Vec<DetectedClient> {
        let mut dirs = Vec::new();

        if let Some(home) = dirs::home_dir() {
            // Claude Code: ~/.claude/memory
            let claude = home.join(".claude").join("memory");
            if Self::create_if_parent_exists(&claude) {
                dirs.push(DetectedClient {
                    client_type: ClientType::Claude,
                    memory_dir: claude,
                });
            }

            // Cursor: ~/.cursor/memory
            let cursor = home.join(".cursor").join("memory");
            if Self::create_if_parent_exists(&cursor) {
                dirs.push(DetectedClient {
                    client_type: ClientType::Cursor,
                    memory_dir: cursor,
                });
            }

            // Windsurf: ~/.windsurf/memory
            let windsurf = home.join(".windsurf").join("memory");
            if Self::create_if_parent_exists(&windsurf) {
                dirs.push(DetectedClient {
                    client_type: ClientType::Windsurf,
                    memory_dir: windsurf,
                });
            }

            // Cody: ~/.sourcegraph/cody/memory
            let cody = home.join(".sourcegraph").join("cody").join("memory");
            if Self::create_if_parent_exists(&cody) {
                dirs.push(DetectedClient {
                    client_type: ClientType::Cody,
                    memory_dir: cody,
                });
            }
        }

        // Also check env overrides
        if let Ok(dir) = std::env::var("CLAUDE_MEMORY_DIR") {
            let path = PathBuf::from(dir);
            if std::fs::create_dir_all(&path).is_ok() {
                // Avoid duplicate if already detected
                if !dirs.iter().any(|d| d.memory_dir == path) {
                    dirs.push(DetectedClient {
                        client_type: ClientType::Claude,
                        memory_dir: path,
                    });
                }
            }
        }

        dirs
    }

    /// Create the memory directory if its parent directory already exists
    /// (i.e., the client is installed). Returns true if the dir now exists.
    fn create_if_parent_exists(memory_dir: &Path) -> bool {
        if memory_dir.exists() {
            return true;
        }
        // Only create if the parent (client's config dir) already exists
        if let Some(parent) = memory_dir.parent() {
            if parent.exists() {
                return std::fs::create_dir_all(memory_dir).is_ok();
            }
        }
        false
    }

    /// Sync context to ALL detected clients at once.
    pub fn sync_to_all_clients(&self) {
        let context = self.engine.session_resume();
        let clients = self.detected_clients.lock().unwrap().clone();

        for detected in &clients {
            let filename = detected.client_type.memory_filename();
            let target = detected.memory_dir.join(filename);
            let markdown = Self::format_for_client(&context, detected.client_type);

            if edge_cases::safe_write_to_claude(&target, &markdown).is_ok() {
                log::debug!(
                    "Synced to {} at {:?}",
                    detected.client_type.display_name(),
                    target
                );
            }
        }

        *self.last_sync.lock().unwrap() = Some(Utc::now());
    }

    /// Format context for a specific client.
    /// Claude gets the full format. Other clients get a streamlined markdown.
    pub fn format_for_client(context: &SessionResumeResult, client: ClientType) -> String {
        match client {
            ClientType::Claude => Self::format_as_claude_memory(context),
            _ => Self::format_as_generic_memory(context, client),
        }
    }

    /// Format context as generic AI assistant markdown (Cursor, Windsurf, Cody).
    fn format_as_generic_memory(context: &SessionResumeResult, client: ClientType) -> String {
        let mut md = String::new();

        md.push_str("# AgenticMemory V3 Context\n\n");
        md.push_str(&format!(
            "> Auto-synced for {} at {}\n\n",
            client.display_name(),
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        md.push_str(&format!(
            "**Session:** `{}` | **Blocks:** {}\n\n",
            context.session_id, context.block_count
        ));

        // Decisions first (most actionable)
        if !context.decisions.is_empty() {
            md.push_str("## Decisions\n\n");
            for (i, d) in context.decisions.iter().enumerate() {
                md.push_str(&format!("{}. {}\n", i + 1, d));
            }
            md.push('\n');
        }

        // Files
        if !context.files_touched.is_empty() {
            md.push_str("## Files Modified\n\n");
            for (path, op) in context.files_touched.iter().take(30) {
                md.push_str(&format!("- `{}` ({})\n", path, op));
            }
            md.push('\n');
        }

        // Errors
        if !context.errors_resolved.is_empty() {
            md.push_str("## Errors Resolved\n\n");
            for (err, res) in &context.errors_resolved {
                md.push_str(&format!("- **{}** → {}\n", err, res));
            }
            md.push('\n');
        }

        md.push_str("---\n");
        md.push_str("_Auto-generated by AgenticMemory V3. Do not edit._\n");

        md
    }

    /// Get all currently detected clients.
    pub fn detected_clients(&self) -> Vec<DetectedClient> {
        self.detected_clients.lock().unwrap().clone()
    }

    /// Auto-detect Claude Code's memory directory (convenience wrapper for tests).
    #[cfg(test)]
    fn detect_claude_memory_dir() -> Option<PathBuf> {
        // First try the multi-client detection
        let clients = Self::detect_all_memory_dirs();
        if let Some(claude) = clients.iter().find(|c| c.client_type == ClientType::Claude) {
            return Some(claude.memory_dir.clone());
        }

        // Check CLAUDE_MEMORY_DIR env var (direct override)
        if let Ok(dir) = std::env::var("CLAUDE_MEMORY_DIR") {
            let path = PathBuf::from(dir);
            if std::fs::create_dir_all(&path).is_ok() {
                return Some(path);
            }
        }

        None
    }

    /// Start background sync thread with periodic re-detection
    fn start_background_sync(self: Arc<Self>) {
        let writer = self.clone();

        std::thread::Builder::new()
            .name("ghost-writer".to_string())
            .spawn(move || {
                let mut last_detection = Instant::now();

                while writer.running.load(Ordering::SeqCst) {
                    // Periodic re-detection of ALL client directories
                    if last_detection.elapsed() > writer.detection_interval {
                        let new_clients = Self::detect_all_memory_dirs();
                        let current_count = writer.detected_clients.lock().unwrap().len();
                        if new_clients.len() != current_count {
                            log::info!(
                                "Client detection changed: {} → {} clients",
                                current_count,
                                new_clients.len()
                            );
                        }
                        // Update Claude dir too
                        let claude_dir = new_clients
                            .iter()
                            .find(|c| c.client_type == ClientType::Claude)
                            .map(|c| c.memory_dir.clone());
                        *writer.claude_memory_dir.lock().unwrap() = claude_dir;
                        *writer.detected_clients.lock().unwrap() = new_clients;
                        last_detection = Instant::now();
                    }

                    writer.sync_once();
                    std::thread::sleep(writer.sync_interval);
                }
            })
            .expect("Failed to spawn ghost writer thread");
    }

    /// Perform one sync cycle — writes to ALL detected clients.
    pub fn sync_once(&self) {
        // Sync to ALL clients (Claude, Cursor, Windsurf, Cody)
        self.sync_to_all_clients();

        // Additionally: merge into Claude's MEMORY.md if it exists
        let claude_dir = self.claude_memory_dir.lock().unwrap().clone();
        if let Some(dir) = claude_dir {
            let memory_file = dir.join("MEMORY.md");
            if memory_file.exists() {
                let context = self.engine.session_resume();
                Self::merge_into_memory_md(&memory_file, &context);
            }
        }
    }

    /// Stop the ghost writer
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get last sync time
    pub fn last_sync_time(&self) -> Option<chrono::DateTime<Utc>> {
        *self.last_sync.lock().unwrap()
    }

    /// Get detected Claude memory directory
    pub fn get_claude_memory_dir(&self) -> Option<PathBuf> {
        self.claude_memory_dir.lock().unwrap().clone()
    }

    /// Format context as Claude-compatible markdown
    pub fn format_as_claude_memory(context: &SessionResumeResult) -> String {
        let mut md = String::new();

        md.push_str("# AgenticMemory V3 Context\n\n");
        md.push_str(&format!(
            "> Auto-synced by Ghost Writer at {}\n\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Session info
        md.push_str(&format!("**Session:** `{}`\n\n", context.session_id));

        // Recent decisions (most important — shown first)
        if !context.decisions.is_empty() {
            md.push_str("## Recent Decisions\n\n");
            for (i, decision) in context.decisions.iter().enumerate() {
                md.push_str(&format!("{}. {}\n", i + 1, decision));
            }
            md.push('\n');
        }

        // Files touched
        if !context.files_touched.is_empty() {
            md.push_str("## Files Modified\n\n");
            md.push_str("| File | Operation |\n");
            md.push_str("|------|----------|\n");
            for (path, op) in context.files_touched.iter().take(20) {
                md.push_str(&format!("| `{}` | {} |\n", path, op));
            }
            if context.files_touched.len() > 20 {
                md.push_str(&format!(
                    "\n_...and {} more files_\n",
                    context.files_touched.len() - 20
                ));
            }
            md.push('\n');
        }

        // Errors resolved
        if !context.errors_resolved.is_empty() {
            md.push_str("## Errors Resolved\n\n");
            for (error, resolution) in &context.errors_resolved {
                md.push_str(&format!("- **{}**\n  -> {}\n", error, resolution));
            }
            md.push('\n');
        }

        // Recent activity summary
        if !context.recent_messages.is_empty() {
            md.push_str("## Recent Activity\n\n");
            for (role, msg) in context.recent_messages.iter().take(10) {
                let preview = if msg.len() > 150 {
                    format!("{}...", &msg[..150])
                } else {
                    msg.clone()
                };
                md.push_str(&format!("- **[{}]** {}\n", role, preview));
            }
            md.push('\n');
        }

        // All known files (collapsed reference)
        if !context.all_known_files.is_empty() {
            md.push_str("<details>\n<summary>All Known Files (");
            md.push_str(&context.all_known_files.len().to_string());
            md.push_str(")</summary>\n\n");
            for file in &context.all_known_files {
                md.push_str(&format!("- `{}`\n", file));
            }
            md.push_str("\n</details>\n\n");
        }

        md.push_str("---\n");
        md.push_str(
            "_This file is auto-generated by AgenticMemory V3. Do not edit manually._\n",
        );

        md
    }

    /// Merge our context into existing MEMORY.md (preserves user sections, uses atomic write)
    fn merge_into_memory_md(memory_file: &Path, context: &SessionResumeResult) {
        let existing = match std::fs::read_to_string(memory_file) {
            Ok(content) => content,
            Err(_) => return,
        };

        let our_section = Self::format_memory_md_section(context);

        let new_content = if existing.contains(START_MARKER) && existing.contains(END_MARKER) {
            // Replace existing section
            if let (Some(start), Some(end)) =
                (existing.find(START_MARKER), existing.find(END_MARKER))
            {
                let before = &existing[..start];
                let after = &existing[end + END_MARKER.len()..];
                format!("{}{}{}", before, our_section, after)
            } else {
                return;
            }
        } else {
            // Append our section
            format!("{}\n\n{}", existing.trim(), our_section)
        };

        // Preserve any user-defined sections (marked with <!-- USER_START/END -->)
        let final_content = edge_cases::merge_preserving_user_sections(
            &existing,
            &new_content,
        );

        // Use atomic write for safety
        let _ = edge_cases::safe_write_to_claude(memory_file, &final_content);
    }

    /// Format our section for MEMORY.md
    fn format_memory_md_section(context: &SessionResumeResult) -> String {
        let mut section = String::new();

        section.push_str(START_MARKER);
        section.push_str("\n## AgenticMemory V3 Session Context\n\n");
        section.push_str(&format!(
            "_Last updated: {}_\n\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Compact format for MEMORY.md
        if !context.decisions.is_empty() {
            section.push_str("**Recent Decisions:**\n");
            for decision in context.decisions.iter().take(5) {
                section.push_str(&format!("- {}\n", decision));
            }
            section.push('\n');
        }

        if !context.files_touched.is_empty() {
            let files: Vec<_> = context
                .files_touched
                .iter()
                .take(10)
                .map(|(p, _)| format!("`{}`", p))
                .collect();
            section.push_str(&format!("**Files:** {}\n\n", files.join(", ")));
        }

        section.push_str(END_MARKER);
        section.push('\n');

        section
    }
}

impl Drop for GhostWriter {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> SessionResumeResult {
        SessionResumeResult {
            session_id: "test-session".to_string(),
            block_count: 10,
            recent_messages: vec![
                ("user".to_string(), "Hello".to_string()),
                ("assistant".to_string(), "Hi there!".to_string()),
            ],
            files_touched: vec![
                ("/src/main.rs".to_string(), "create".to_string()),
                ("/src/lib.rs".to_string(), "update".to_string()),
            ],
            decisions: vec![
                "Use Rust for performance".to_string(),
                "Implement V3 architecture".to_string(),
            ],
            errors_resolved: vec![(
                "missing dep".to_string(),
                "added to Cargo.toml".to_string(),
            )],
            all_known_files: vec!["/src/main.rs".to_string()],
        }
    }

    #[test]
    fn test_format_as_claude_memory() {
        let context = sample_context();
        let markdown = GhostWriter::format_as_claude_memory(&context);

        assert!(markdown.contains("AgenticMemory V3 Context"));
        assert!(markdown.contains("Use Rust for performance"));
        assert!(markdown.contains("/src/main.rs"));
    }

    #[test]
    fn test_format_for_cursor() {
        let context = sample_context();
        let markdown = GhostWriter::format_for_client(&context, ClientType::Cursor);

        assert!(markdown.contains("AgenticMemory V3 Context"));
        assert!(markdown.contains("Cursor"));
        assert!(markdown.contains("Use Rust for performance"));
        assert!(markdown.contains("/src/main.rs"));
    }

    #[test]
    fn test_format_for_windsurf() {
        let context = sample_context();
        let markdown = GhostWriter::format_for_client(&context, ClientType::Windsurf);

        assert!(markdown.contains("Windsurf"));
        assert!(markdown.contains("Decisions"));
    }

    #[test]
    fn test_format_for_cody() {
        let context = sample_context();
        let markdown = GhostWriter::format_for_client(&context, ClientType::Cody);

        assert!(markdown.contains("Cody"));
        assert!(markdown.contains("Decisions"));
    }

    #[test]
    fn test_client_type_filenames() {
        assert_eq!(ClientType::Claude.memory_filename(), "V3_CONTEXT.md");
        assert_eq!(ClientType::Cursor.memory_filename(), "agentic-memory.md");
        assert_eq!(ClientType::Windsurf.memory_filename(), "agentic-memory.md");
        assert_eq!(ClientType::Cody.memory_filename(), "agentic-memory.md");
    }

    #[test]
    fn test_client_type_all() {
        let all = ClientType::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&ClientType::Claude));
        assert!(all.contains(&ClientType::Cursor));
        assert!(all.contains(&ClientType::Windsurf));
        assert!(all.contains(&ClientType::Cody));
    }

    #[test]
    fn test_detect_claude_memory_dir_with_env() {
        let dir = tempfile::TempDir::new().unwrap();
        std::env::set_var("CLAUDE_MEMORY_DIR", dir.path().to_str().unwrap());

        let detected = GhostWriter::detect_claude_memory_dir();
        assert!(detected.is_some());

        std::env::remove_var("CLAUDE_MEMORY_DIR");
    }

    #[test]
    fn test_create_if_parent_exists() {
        let dir = tempfile::TempDir::new().unwrap();
        let memory_dir = dir.path().join("memory");

        // Parent exists, so this should succeed
        assert!(GhostWriter::create_if_parent_exists(&memory_dir));
        assert!(memory_dir.exists());
    }

    #[test]
    fn test_create_if_parent_missing() {
        let memory_dir = PathBuf::from("/tmp/nonexistent_ghost_test_dir/also_missing/memory");

        // Parent doesn't exist, should fail
        assert!(!GhostWriter::create_if_parent_exists(&memory_dir));
    }
}
