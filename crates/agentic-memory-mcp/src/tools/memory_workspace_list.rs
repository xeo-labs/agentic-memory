//! Tool: memory_workspace_list — List loaded memory contexts in a workspace.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct ListParams {
    workspace_id: String,
}

/// Return the tool definition for memory_workspace_list.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_workspace_list".to_string(),
        description: Some(
            "List all loaded memory contexts in a workspace, including their roles, \
             paths, labels, and node counts"
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "required": ["workspace_id"],
            "properties": {
                "workspace_id": {
                    "type": "string",
                    "description": "ID of the workspace to list"
                }
            }
        }),
    }
}

/// Execute the memory_workspace_list tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: ListParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let session = session.lock().await;
    let contexts = session.workspace_manager().list(&params.workspace_id)?;

    let items: Vec<Value> = contexts
        .iter()
        .map(|ctx| {
            json!({
                "context_id": ctx.id,
                "role": ctx.role.label(),
                "path": ctx.path,
                "label": ctx.label,
                "node_count": ctx.graph.node_count(),
                "edge_count": ctx.graph.edge_count(),
            })
        })
        .collect();

    Ok(ToolCallResult::json(&json!({
        "workspace_id": params.workspace_id,
        "count": items.len(),
        "contexts": items
    })))
}
