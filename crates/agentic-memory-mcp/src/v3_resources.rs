//! V3 Resource Gateway — expose memory:// URIs for session context.

use serde_json::json;

use crate::tools::v3_tools::SharedEngine;
use crate::types::{McpError, McpResult, ReadResourceResult, ResourceContent, ResourceDefinition};

/// V3 resource definitions.
pub fn list_v3_resources() -> Vec<ResourceDefinition> {
    vec![
        ResourceDefinition {
            uri: "memory://v3/session/context".to_string(),
            name: "V3 Session Context".to_string(),
            description: Some("Full session context from immortal log".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
        ResourceDefinition {
            uri: "memory://v3/session/decisions".to_string(),
            name: "V3 Decisions".to_string(),
            description: Some("Recent decisions made".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
        ResourceDefinition {
            uri: "memory://v3/session/files".to_string(),
            name: "V3 Files".to_string(),
            description: Some("Files modified in session".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
        ResourceDefinition {
            uri: "memory://v3/session/errors".to_string(),
            name: "V3 Errors".to_string(),
            description: Some("Errors and their resolutions".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
        ResourceDefinition {
            uri: "memory://v3/session/activity".to_string(),
            name: "V3 Activity".to_string(),
            description: Some("Recent messages and activity".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
        ResourceDefinition {
            uri: "memory://v3/stats".to_string(),
            name: "V3 Stats".to_string(),
            description: Some("Memory engine statistics".to_string()),
            mime_type: Some("application/json".to_string()),
        },
    ]
}

/// Read a V3 resource by URI. Returns None if the URI is not a V3 resource.
pub async fn read_v3_resource(
    uri: &str,
    engine: &SharedEngine,
) -> Option<McpResult<ReadResourceResult>> {
    match uri {
        "memory://v3/session/context" => Some(read_session_context(engine).await),
        "memory://v3/session/decisions" => Some(read_decisions(engine).await),
        "memory://v3/session/files" => Some(read_files(engine).await),
        "memory://v3/session/errors" => Some(read_errors(engine).await),
        "memory://v3/session/activity" => Some(read_activity(engine).await),
        "memory://v3/stats" => Some(read_stats(engine).await),
        _ => None,
    }
}

async fn read_session_context(engine: &SharedEngine) -> McpResult<ReadResourceResult> {
    let eng = engine.lock().await;
    let eng = eng
        .as_ref()
        .ok_or_else(|| McpError::InternalError("V3 engine not initialized".to_string()))?;

    let ctx = eng.session_resume();
    let mut md = String::new();

    md.push_str(&format!("# Session {}\n\n", ctx.session_id));
    md.push_str(&format!("**Blocks:** {}\n\n", ctx.block_count));

    if !ctx.decisions.is_empty() {
        md.push_str("## Decisions\n\n");
        for d in &ctx.decisions {
            md.push_str(&format!("- {d}\n"));
        }
        md.push('\n');
    }

    if !ctx.files_touched.is_empty() {
        md.push_str("## Files\n\n");
        md.push_str("| Path | Operation |\n|---|---|\n");
        for (path, op) in &ctx.files_touched {
            md.push_str(&format!("| {path} | {op} |\n"));
        }
        md.push('\n');
    }

    if !ctx.errors_resolved.is_empty() {
        md.push_str("## Errors Resolved\n\n");
        for (err, res) in &ctx.errors_resolved {
            md.push_str(&format!("- **{err}** → {res}\n"));
        }
        md.push('\n');
    }

    if !ctx.recent_messages.is_empty() {
        md.push_str("## Recent Messages\n\n");
        for (role, content) in ctx.recent_messages.iter().take(20) {
            let truncated = if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content.clone()
            };
            md.push_str(&format!("**{role}:** {truncated}\n\n"));
        }
    }

    Ok(ReadResourceResult {
        contents: vec![ResourceContent {
            uri: "memory://v3/session/context".to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(md),
            blob: None,
        }],
    })
}

async fn read_decisions(engine: &SharedEngine) -> McpResult<ReadResourceResult> {
    let eng = engine.lock().await;
    let eng = eng
        .as_ref()
        .ok_or_else(|| McpError::InternalError("V3 engine not initialized".to_string()))?;

    let ctx = eng.session_resume();
    let mut md = String::from("# Decisions\n\n");
    for (i, d) in ctx.decisions.iter().enumerate() {
        md.push_str(&format!("{}. {d}\n", i + 1));
    }

    Ok(ReadResourceResult {
        contents: vec![ResourceContent {
            uri: "memory://v3/session/decisions".to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(md),
            blob: None,
        }],
    })
}

async fn read_files(engine: &SharedEngine) -> McpResult<ReadResourceResult> {
    let eng = engine.lock().await;
    let eng = eng
        .as_ref()
        .ok_or_else(|| McpError::InternalError("V3 engine not initialized".to_string()))?;

    let ctx = eng.session_resume();
    let mut md = String::from("# Files\n\n## Modified\n\n");
    for (path, op) in &ctx.files_touched {
        md.push_str(&format!("- [{op}] `{path}`\n"));
    }

    md.push_str("\n## All Known Files\n\n");
    for path in &ctx.all_known_files {
        md.push_str(&format!("- `{path}`\n"));
    }

    Ok(ReadResourceResult {
        contents: vec![ResourceContent {
            uri: "memory://v3/session/files".to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(md),
            blob: None,
        }],
    })
}

async fn read_errors(engine: &SharedEngine) -> McpResult<ReadResourceResult> {
    let eng = engine.lock().await;
    let eng = eng
        .as_ref()
        .ok_or_else(|| McpError::InternalError("V3 engine not initialized".to_string()))?;

    let ctx = eng.session_resume();
    let mut md = String::from("# Errors Resolved\n\n");
    for (err, res) in &ctx.errors_resolved {
        md.push_str(&format!("### {err}\n**Resolution:** {res}\n\n"));
    }

    Ok(ReadResourceResult {
        contents: vec![ResourceContent {
            uri: "memory://v3/session/errors".to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(md),
            blob: None,
        }],
    })
}

async fn read_activity(engine: &SharedEngine) -> McpResult<ReadResourceResult> {
    let eng = engine.lock().await;
    let eng = eng
        .as_ref()
        .ok_or_else(|| McpError::InternalError("V3 engine not initialized".to_string()))?;

    let ctx = eng.session_resume();
    let mut md = String::from("# Recent Activity\n\n");
    for (role, content) in &ctx.recent_messages {
        let truncated = if content.len() > 300 {
            format!("{}...", &content[..300])
        } else {
            content.clone()
        };
        md.push_str(&format!("**{role}:** {truncated}\n\n---\n\n"));
    }

    Ok(ReadResourceResult {
        contents: vec![ResourceContent {
            uri: "memory://v3/session/activity".to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(md),
            blob: None,
        }],
    })
}

async fn read_stats(engine: &SharedEngine) -> McpResult<ReadResourceResult> {
    let eng = engine.lock().await;
    let eng = eng
        .as_ref()
        .ok_or_else(|| McpError::InternalError("V3 engine not initialized".to_string()))?;

    let stats = eng.stats();

    let stats_json = json!({
        "total_blocks": stats.total_blocks,
        "session_id": stats.session_id,
        "tier_stats": {
            "hot": { "blocks": stats.tier_stats.hot_blocks, "bytes": stats.tier_stats.hot_bytes },
            "warm": { "blocks": stats.tier_stats.warm_blocks, "bytes": stats.tier_stats.warm_bytes },
            "cold": { "blocks": stats.tier_stats.cold_blocks, "bytes": stats.tier_stats.cold_bytes },
            "frozen": { "blocks": stats.tier_stats.frozen_blocks }
        }
    });

    Ok(ReadResourceResult {
        contents: vec![ResourceContent {
            uri: "memory://v3/stats".to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&stats_json).unwrap_or_default()),
            blob: None,
        }],
    })
}
