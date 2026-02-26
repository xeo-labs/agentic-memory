//! V3 Greeting — inject session context into MCP server info.

use agentic_memory::v3::{EngineStats, MemoryEngineV3, SessionResumeResult};

/// Format the session context as a greeting string for the LLM client.
pub fn format_greeting_context(context: &SessionResumeResult) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "Session {} | {} blocks in immortal log",
        &context.session_id[..8.min(context.session_id.len())],
        context.block_count
    ));

    if !context.decisions.is_empty() {
        lines.push(String::new());
        lines.push("Recent decisions:".to_string());
        for (i, d) in context.decisions.iter().take(5).enumerate() {
            lines.push(format!("  {}. {}", i + 1, d));
        }
    }

    if !context.files_touched.is_empty() {
        lines.push(String::new());
        lines.push("Files modified:".to_string());
        for (path, op) in context.files_touched.iter().take(10) {
            lines.push(format!("  [{op}] {path}"));
        }
    }

    if !context.errors_resolved.is_empty() {
        lines.push(String::new());
        lines.push(format!("{} errors resolved", context.errors_resolved.len()));
    }

    lines.join("\n")
}

/// Compact one-line context summary.
pub fn compact_context_string(context: &SessionResumeResult) -> String {
    format!(
        "{} blocks | {} files | {} decisions | {} errors fixed",
        context.block_count,
        context.files_touched.len(),
        context.decisions.len(),
        context.errors_resolved.len()
    )
}

/// Build a server description string with V3 context injected.
pub fn server_description_with_context(engine: &MemoryEngineV3) -> String {
    let context = engine.session_resume();

    if context.block_count == 0 {
        return format!("AgenticMemory V3 Immortal Architecture — fresh session (0 blocks)");
    }

    format!(
        "AgenticMemory V3 — {}\n\n{}",
        compact_context_string(&context),
        format_greeting_context(&context)
    )
}

/// Format tier stats for the greeting.
pub fn format_tier_stats(stats: &EngineStats) -> String {
    format!(
        "Storage: {} blocks total | hot={} warm={} cold={} frozen={}",
        stats.total_blocks,
        stats.tier_stats.hot_blocks,
        stats.tier_stats.warm_blocks,
        stats.tier_stats.cold_blocks,
        stats.tier_stats.frozen_blocks
    )
}
