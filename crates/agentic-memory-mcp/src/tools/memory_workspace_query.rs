//! Tool: memory_workspace_query — Query across all memory contexts in a workspace.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct QueryParams {
    workspace_id: String,
    query: String,
    #[serde(default = "default_max_per_context")]
    max_per_context: usize,
}

fn default_max_per_context() -> usize {
    10
}

/// Return the tool definition for memory_workspace_query.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_workspace_query".to_string(),
        description: Some(
            "Search across all memory contexts in a workspace. Returns matches from \
             each loaded context, enabling cross-project knowledge discovery"
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "required": ["workspace_id", "query"],
            "properties": {
                "workspace_id": {
                    "type": "string",
                    "description": "ID of the workspace to query"
                },
                "query": {
                    "type": "string",
                    "description": "Text query to search across all contexts"
                },
                "max_per_context": {
                    "type": "integer",
                    "default": 10,
                    "description": "Maximum matches per context"
                }
            }
        }),
    }
}

/// Execute the memory_workspace_query tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: QueryParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let session = session.lock().await;
    let results = session.workspace_manager().query_all(
        &params.workspace_id,
        &params.query,
        params.max_per_context,
    )?;

    let context_results: Vec<Value> = results
        .iter()
        .map(|cr| {
            let matches: Vec<Value> = cr
                .matches
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
                "context_id": cr.context_id,
                "context_role": cr.context_role.label(),
                "match_count": cr.matches.len(),
                "matches": matches,
            })
        })
        .collect();

    let total_matches: usize = results.iter().map(|cr| cr.matches.len()).sum();

    Ok(ToolCallResult::json(&json!({
        "workspace_id": params.workspace_id,
        "query": params.query,
        "total_matches": total_matches,
        "contexts_searched": context_results.len(),
        "results": context_results
    })))
}
