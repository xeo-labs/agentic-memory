//! Capture daemon — automatic conversation capture without LLM dependency.
//!
//! Three capture channels feed into the WAL:
//! - Channel A: MCP message stream (tool-based, enhanced Ghost Writer instructions)
//! - Channel B: Client log monitoring (fsnotify, zero LLM dependency)
//! - Channel C: Proxy intercept (optional, advanced)
//!
//! Content-addressed deduplication ensures no duplicate captures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A captured conversation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureEvent {
    pub role: CaptureRole,
    pub content: String,
    pub timestamp: u64,
    pub source: CaptureSource,
    pub session_id: Option<String>,
    pub project_path: Option<String>,
}

/// Who sent the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureRole {
    User,
    Assistant,
    System,
    Tool,
}

/// Which capture channel sourced this event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureSource {
    McpStream,
    ClientLog,
    Proxy,
    Manual,
}

/// Content-addressed deduplication using BLAKE3 hashes.
pub struct ContentDedup {
    /// Recent hashes with 2-second time windows
    recent: Mutex<HashMap<[u8; 32], u64>>,
    /// Maximum entries before cleanup
    max_entries: usize,
}

impl ContentDedup {
    pub fn new(max_entries: usize) -> Self {
        Self {
            recent: Mutex::new(HashMap::with_capacity(max_entries)),
            max_entries,
        }
    }

    /// Check if this content+timestamp combination is a duplicate.
    /// Returns true if duplicate (should be skipped), false if new.
    pub fn is_duplicate(&self, content: &str, timestamp: u64) -> bool {
        // Round timestamp to 2-second window
        let window = timestamp / 2;
        let input = format!("{}:{}", content, window);
        let hash = blake3::hash(input.as_bytes());
        let bytes: [u8; 32] = *hash.as_bytes();

        let mut recent = self.recent.lock().unwrap();

        if recent.contains_key(&bytes) {
            return true;
        }

        // Cleanup if too many entries
        if recent.len() >= self.max_entries {
            // Remove oldest entries (keep most recent half)
            let mut entries: Vec<_> = recent.iter().map(|(k, v)| (*k, *v)).collect();
            entries.sort_by_key(|(_, ts)| *ts);
            let keep_from = entries.len() / 2;
            let to_remove: Vec<[u8; 32]> = entries[..keep_from].iter().map(|(k, _)| *k).collect();
            for key in to_remove {
                recent.remove(&key);
            }
        }

        recent.insert(bytes, timestamp);
        false
    }

    /// Number of entries in the dedup cache.
    pub fn cache_size(&self) -> usize {
        self.recent.lock().unwrap().len()
    }
}

impl Default for ContentDedup {
    fn default() -> Self {
        Self::new(10000)
    }
}

/// Client log monitor — watches conversation log files.
pub struct ClientLogMonitor {
    /// Detected client log directories
    watch_paths: Vec<WatchTarget>,
    /// Content dedup
    dedup: Arc<ContentDedup>,
}

/// A target directory to watch for conversation logs.
#[derive(Debug, Clone)]
pub struct WatchTarget {
    pub client_name: String,
    pub path: PathBuf,
    pub pattern: String,
}

impl ClientLogMonitor {
    pub fn new(dedup: Arc<ContentDedup>) -> Self {
        let watch_paths = Self::detect_clients();
        Self { watch_paths, dedup }
    }

    /// Auto-detect all supported client conversation directories.
    pub fn detect_clients() -> Vec<WatchTarget> {
        let mut targets = Vec::new();

        // Claude Code conversations
        if let Some(home) = dirs::home_dir() {
            let claude_dir = home.join(".claude").join("projects");
            if claude_dir.exists() {
                targets.push(WatchTarget {
                    client_name: "claude-code".to_string(),
                    path: claude_dir,
                    pattern: "*/conversations/*.jsonl".to_string(),
                });
            }

            // Cursor conversations
            let cursor_dir = home.join(".cursor");
            if cursor_dir.exists() {
                targets.push(WatchTarget {
                    client_name: "cursor".to_string(),
                    path: cursor_dir,
                    pattern: "conversations/*.json".to_string(),
                });
            }

            // VS Code / Cody
            let vscode_dir = home.join(".vscode");
            if vscode_dir.exists() {
                targets.push(WatchTarget {
                    client_name: "vscode".to_string(),
                    path: vscode_dir,
                    pattern: "extensions/*/conversations/*.json".to_string(),
                });
            }

            // Windsurf
            let windsurf_dir = home.join(".windsurf");
            if windsurf_dir.exists() {
                targets.push(WatchTarget {
                    client_name: "windsurf".to_string(),
                    path: windsurf_dir,
                    pattern: "conversations/*.json".to_string(),
                });
            }
        }

        targets
    }

    /// Get the list of detected watch targets.
    pub fn watch_targets(&self) -> &[WatchTarget] {
        &self.watch_paths
    }

    /// Add a custom watch directory.
    pub fn add_watch_path(&mut self, client_name: &str, path: PathBuf, pattern: &str) {
        self.watch_paths.push(WatchTarget {
            client_name: client_name.to_string(),
            path,
            pattern: pattern.to_string(),
        });
    }

    /// Parse a Claude Code conversation JSONL file and extract events.
    pub fn parse_claude_conversation(path: &Path) -> Result<Vec<CaptureEvent>, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let mut events = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                let role = match entry.get("type").and_then(|v| v.as_str()) {
                    Some("human") | Some("user") => CaptureRole::User,
                    Some("assistant") => CaptureRole::Assistant,
                    Some("system") => CaptureRole::System,
                    Some("tool_use") | Some("tool_result") => CaptureRole::Tool,
                    _ => continue,
                };

                let text = entry
                    .get("message")
                    .or_else(|| entry.get("content"))
                    .or_else(|| entry.get("text"))
                    .and_then(|v| match v {
                        serde_json::Value::String(s) => Some(s.clone()),
                        serde_json::Value::Array(arr) => {
                            // Claude format: array of content blocks
                            let texts: Vec<String> = arr
                                .iter()
                                .filter_map(|block| {
                                    block.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                                })
                                .collect();
                            if texts.is_empty() {
                                None
                            } else {
                                Some(texts.join("\n"))
                            }
                        }
                        _ => Some(v.to_string()),
                    });

                if let Some(content) = text {
                    let timestamp = entry
                        .get("timestamp")
                        .and_then(|v| v.as_u64())
                        .unwrap_or_else(|| {
                            chrono::Utc::now().timestamp_millis() as u64
                        });

                    events.push(CaptureEvent {
                        role,
                        content,
                        timestamp,
                        source: CaptureSource::ClientLog,
                        session_id: entry
                            .get("session_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        project_path: None,
                    });
                }
            }
        }

        Ok(events)
    }

    /// Get the dedup cache.
    pub fn dedup(&self) -> &Arc<ContentDedup> {
        &self.dedup
    }
}

/// The capture daemon orchestrates all capture channels.
pub struct CaptureDaemon {
    /// Content deduplication
    dedup: Arc<ContentDedup>,
    /// Client log monitor
    log_monitor: ClientLogMonitor,
    /// Captured events buffer (drained by sync protocol)
    buffer: Arc<Mutex<Vec<CaptureEvent>>>,
    /// Whether the daemon is running
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl CaptureDaemon {
    pub fn new() -> Self {
        let dedup = Arc::new(ContentDedup::default());
        let log_monitor = ClientLogMonitor::new(dedup.clone());

        Self {
            dedup,
            log_monitor,
            buffer: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Capture an event from any source (with dedup).
    pub fn capture(&self, event: CaptureEvent) -> bool {
        if self.dedup.is_duplicate(&event.content, event.timestamp) {
            return false;
        }
        self.buffer.lock().unwrap().push(event);
        true
    }

    /// Drain all buffered events.
    pub fn drain_buffer(&self) -> Vec<CaptureEvent> {
        std::mem::take(&mut *self.buffer.lock().unwrap())
    }

    /// Get the number of buffered events.
    pub fn buffer_size(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }

    /// Get the log monitor.
    pub fn log_monitor(&self) -> &ClientLogMonitor {
        &self.log_monitor
    }

    /// Get mutable reference to log monitor.
    pub fn log_monitor_mut(&mut self) -> &mut ClientLogMonitor {
        &mut self.log_monitor
    }

    /// Check if daemon is running.
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Stop the daemon.
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get stats about the capture daemon.
    pub fn stats(&self) -> CaptureStats {
        CaptureStats {
            buffer_size: self.buffer_size(),
            dedup_cache_size: self.dedup.cache_size(),
            watch_targets: self
                .log_monitor
                .watch_targets()
                .iter()
                .map(|t| format!("{}: {}", t.client_name, t.path.display()))
                .collect(),
            is_running: self.is_running(),
        }
    }
}

impl Default for CaptureDaemon {
    fn default() -> Self {
        Self::new()
    }
}

/// Capture daemon statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureStats {
    pub buffer_size: usize,
    pub dedup_cache_size: usize,
    pub watch_targets: Vec<String>,
    pub is_running: bool,
}
