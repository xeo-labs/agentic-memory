//! Tool: memory_workspace_compare — Compare a topic across memory contexts.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct CompareParams {
    workspace_id: String,
    item: String,
    #[serde(default = "default_max")]
    max_per_context: usize,
}

fn default_max() -> usize {
    5
}

/// Return the tool definition for memory_workspace_compare.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_workspace_compare".to_string(),
        description: Some(
            "Compare how a topic appears across different memory contexts. Shows where \
             a concept exists, what's different, and where it's missing."
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "required": ["workspace_id", "item"],
            "properties": {
                "workspace_id": {
                    "type": "string",
                    "description": "ID of the workspace"
                },
                "item": {
                    "type": "string",
                    "description": "Topic/concept to compare across contexts"
                },
                "max_per_context": {
                    "type": "integer",
                    "default": 5,
                    "description": "Maximum matches per context for comparison"
                }
            }
        }),
    }
}

/// Execute the memory_workspace_compare tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: CompareParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let session = session.lock().await;
    let comparison = session.workspace_manager().compare(
        &params.workspace_id,
        &params.item,
        params.max_per_context,
    )?;

    let per_context: Vec<Value> = comparison
        .matches_per_context
        .iter()
        .map(|(label, matches)| {
            let match_items: Vec<Value> = matches
                .iter()
                .map(|m| {
                    json!({
                        "node_id": m.node_id,
                        "content": m.content,
                        "event_type": m.event_type,
                        "confidence": m.confidence,
                        "score": m.score,
                    })
                })
                .collect();

            json!({
                "context": label,
                "matches": match_items,
            })
        })
        .collect();

    Ok(ToolCallResult::json(&json!({
        "item": comparison.item,
        "found_in": comparison.found_in,
        "missing_from": comparison.missing_from,
        "details": per_context
    })))
}
