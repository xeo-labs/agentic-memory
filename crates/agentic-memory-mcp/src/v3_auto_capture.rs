//! V3 Auto-Capture Middleware — intercepts tool calls and captures to the immortal log.

use serde_json::Value;
use tokio::sync::Mutex;

use agentic_memory::v3::FileOperation;

use crate::tools::v3_tools::SharedEngine;

/// Configuration for auto-capture behavior.
#[derive(Debug, Clone)]
pub struct AutoCaptureConfig {
    /// Whether auto-capture is enabled.
    pub enabled: bool,
    /// Capture messages automatically.
    pub capture_messages: bool,
    /// Capture tool calls automatically.
    pub capture_tools: bool,
    /// Capture file operations automatically.
    pub capture_files: bool,
    /// Create checkpoint every N tool calls.
    pub checkpoint_interval: Option<u32>,
    /// Context pressure threshold (0.0-1.0).
    pub pressure_threshold: f32,
}

impl Default for AutoCaptureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_messages: true,
            capture_tools: true,
            capture_files: true,
            checkpoint_interval: Some(50),
            pressure_threshold: 0.75,
        }
    }
}

/// V3 Auto-Capture Middleware.
pub struct AutoCaptureMiddleware {
    engine: SharedEngine,
    config: AutoCaptureConfig,
    tool_call_count: Mutex<u32>,
}

impl AutoCaptureMiddleware {
    /// Create new middleware with the given engine and config.
    pub fn new(engine: SharedEngine, config: AutoCaptureConfig) -> Self {
        Self {
            engine,
            config,
            tool_call_count: Mutex::new(0),
        }
    }

    /// Create middleware with default config.
    pub fn with_defaults(engine: SharedEngine) -> Self {
        Self::new(engine, AutoCaptureConfig::default())
    }

    /// Called before tool execution. Captures the tool call to the V3 log.
    pub async fn on_tool_call(&self, tool_name: &str, input: &Value) {
        if !self.config.enabled || !self.config.capture_tools {
            return;
        }

        // Don't capture V3 capture tools themselves (avoid infinite loop)
        if tool_name.starts_with("memory_capture_") || tool_name.starts_with("memory_v3_") {
            return;
        }

        // Detect file operations from common tool patterns
        if self.config.capture_files {
            self.detect_and_capture_file_op(tool_name, input).await;
        }
    }

    /// Called after tool execution. Captures result to the V3 log.
    pub async fn on_tool_result(
        &self,
        tool_name: &str,
        input: Value,
        output: Value,
        duration_ms: u64,
        success: bool,
    ) {
        if !self.config.enabled || !self.config.capture_tools {
            return;
        }

        // Don't capture V3 tools themselves
        if tool_name.starts_with("memory_capture_") || tool_name.starts_with("memory_v3_") {
            return;
        }

        let eng = self.engine.lock().await;
        if let Some(engine) = eng.as_ref() {
            if let Err(e) = engine.capture_tool_call(
                tool_name,
                input,
                Some(output),
                Some(duration_ms),
                success,
            ) {
                tracing::warn!("V3 auto-capture failed for tool {}: {}", tool_name, e);
            }
        }

        // Check if we should create a checkpoint
        drop(eng);
        self.maybe_checkpoint().await;
    }

    /// Called when context pressure exceeds threshold.
    pub async fn on_context_pressure(&self, current_tokens: u32, max_tokens: u32) {
        if !self.config.enabled {
            return;
        }

        let pressure = current_tokens as f32 / max_tokens.max(1) as f32;
        if pressure < self.config.pressure_threshold {
            return;
        }

        let eng = self.engine.lock().await;
        if let Some(engine) = eng.as_ref() {
            if let Err(e) = engine.capture_boundary(
                agentic_memory::v3::BoundaryType::ContextPressure,
                current_tokens,
                max_tokens,
                &format!("Context pressure: {:.0}%", pressure * 100.0),
                None,
            ) {
                tracing::warn!("V3 auto-capture context pressure failed: {}", e);
            }
        }
    }

    /// Called before compaction occurs.
    pub async fn on_pre_compaction(&self, context_tokens: u32, summary: &str) {
        if !self.config.enabled {
            return;
        }

        let eng = self.engine.lock().await;
        if let Some(engine) = eng.as_ref() {
            if let Err(e) = engine.capture_boundary(
                agentic_memory::v3::BoundaryType::Compaction,
                context_tokens,
                0,
                summary,
                None,
            ) {
                tracing::warn!("V3 auto-capture compaction boundary failed: {}", e);
            }
        }
    }

    /// Detect file operations from tool input and capture them.
    async fn detect_and_capture_file_op(&self, tool_name: &str, input: &Value) {
        let (path, op) = match tool_name {
            "create_file" | "file_create" | "Write" => {
                let path = input
                    .get("file_path")
                    .or_else(|| input.get("path"))
                    .and_then(|v| v.as_str());
                (path, Some(FileOperation::Create))
            }
            "str_replace" | "edit_file" | "Edit" => {
                let path = input
                    .get("file_path")
                    .or_else(|| input.get("path"))
                    .and_then(|v| v.as_str());
                (path, Some(FileOperation::Update))
            }
            "view" | "read_file" | "Read" => {
                let path = input
                    .get("file_path")
                    .or_else(|| input.get("path"))
                    .and_then(|v| v.as_str());
                (path, Some(FileOperation::Read))
            }
            "bash_tool" | "Bash" | "bash" => {
                // Detect rm commands in bash
                if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                    if cmd.contains("rm ") || cmd.contains("rm\t") {
                        // Extract first argument after rm
                        let parts: Vec<&str> = cmd.split_whitespace().collect();
                        if let Some(pos) = parts.iter().position(|&p| p == "rm") {
                            let path = parts.get(pos + 1).copied();
                            (path, Some(FileOperation::Delete))
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            }
            _ => (None, None),
        };

        if let (Some(path), Some(op)) = (path, op) {
            let eng = self.engine.lock().await;
            if let Some(engine) = eng.as_ref() {
                if let Err(e) = engine.capture_file_operation(path, op, None, None, None) {
                    tracing::warn!("V3 auto-capture file op failed for {}: {}", path, e);
                }
            }
        }
    }

    /// Create a checkpoint if the interval has been reached.
    async fn maybe_checkpoint(&self) {
        let interval = match self.config.checkpoint_interval {
            Some(i) => i,
            None => return,
        };

        let mut count = self.tool_call_count.lock().await;
        *count += 1;

        if *count >= interval {
            *count = 0;
            drop(count);

            let eng = self.engine.lock().await;
            if let Some(engine) = eng.as_ref() {
                if let Err(e) = engine.capture_checkpoint(
                    vec![],
                    &format!("Auto-checkpoint after {} tool calls", interval),
                    vec![],
                ) {
                    tracing::warn!("V3 auto-checkpoint failed: {}", e);
                }
            }
        }
    }
}
