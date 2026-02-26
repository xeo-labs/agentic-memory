//! Hooks for integrating with Claude Code's session management.
//! These functions are called by Claude Code at key moments.

use super::block::*;
use super::engine::{MemoryEngineV3, SessionResumeResult};

/// Claude Code integration hooks
pub struct ClaudeHooks;

impl ClaudeHooks {
    /// Hook called at the START of every Claude Code message
    pub fn on_message_start(
        engine: &MemoryEngineV3,
        role: &str,
        content: &str,
        _context_tokens: u32,
    ) {
        let _ = match role {
            "user" => engine.capture_user_message(content, Some(estimate_tokens(content))),
            "assistant" => {
                engine.capture_assistant_message(content, Some(estimate_tokens(content)))
            }
            _ => engine.capture_user_message(content, Some(estimate_tokens(content))),
        };
    }

    /// Hook called BEFORE every tool execution
    pub fn on_tool_start(_engine: &MemoryEngineV3, _tool_name: &str, _input: &serde_json::Value) {
        // We'll capture the full tool call when it completes
        // This is just for tracking that a tool started
    }

    /// Hook called AFTER every tool execution
    pub fn on_tool_complete(
        engine: &MemoryEngineV3,
        tool_name: &str,
        input: serde_json::Value,
        output: serde_json::Value,
        duration_ms: u64,
        success: bool,
    ) {
        let _ = engine.capture_tool_call(
            tool_name,
            input.clone(),
            Some(output),
            Some(duration_ms),
            success,
        );

        // Special handling for file tools
        if tool_name == "create_file" || tool_name == "str_replace" || tool_name == "view" {
            if let Some(path) = input.get("path").and_then(|p| p.as_str()) {
                let op = match tool_name {
                    "create_file" => FileOperation::Create,
                    "str_replace" => FileOperation::Update,
                    "view" => FileOperation::Read,
                    _ => FileOperation::Read,
                };
                let _ = engine.capture_file_operation(path, op, None, None, None);
            }
        }
    }

    /// Hook called when Claude Code detects context pressure
    pub fn on_context_pressure(engine: &MemoryEngineV3, current_tokens: u32, max_tokens: u32) {
        if current_tokens as f32 / max_tokens as f32 > 0.8 {
            // Context is getting full, capture checkpoint
            let _ = engine.capture_checkpoint(
                vec![], // Would be populated by Claude Code
                "Context pressure detected",
                vec![],
            );
        }
    }

    /// Hook called BEFORE compaction happens
    /// THIS IS THE CRITICAL MOMENT — capture everything before it's lost
    pub fn on_pre_compaction(
        engine: &MemoryEngineV3,
        context_tokens_before: u32,
        summary: &str,
        active_files: Vec<String>,
        pending_tasks: Vec<String>,
        working_context: &str,
    ) {
        // Capture full checkpoint
        let _ = engine.capture_checkpoint(active_files.clone(), working_context, pending_tasks);

        // Capture the boundary event
        let _ = engine.capture_boundary(
            BoundaryType::Compaction,
            context_tokens_before,
            0, // Will be filled after compaction
            summary,
            Some(&format!("Active files: {:?}", active_files)),
        );
    }

    /// Hook called AFTER compaction completes
    pub fn on_post_compaction(_engine: &MemoryEngineV3, _context_tokens_after: u32) {
        // In practice, we'd update the last boundary block
        // with the post-compaction token count
    }

    /// Hook called at SESSION END
    pub fn on_session_end(engine: &MemoryEngineV3, summary: &str) {
        let _ = engine.capture_boundary(BoundaryType::SessionEnd, 0, 0, summary, None);
    }

    /// Hook called at SESSION START (resume)
    pub fn on_session_start(engine: &MemoryEngineV3) -> SessionResumeResult {
        // Mark new session
        let _ = engine.capture_boundary(
            BoundaryType::SessionStart,
            0,
            0,
            "New session started",
            None,
        );

        // Return full context for Claude Code to use
        engine.session_resume()
    }
}

/// Helper: estimate tokens from text
fn estimate_tokens(text: &str) -> u32 {
    // Rough estimate: 4 characters per token
    (text.len() / 4) as u32 + 1
}
