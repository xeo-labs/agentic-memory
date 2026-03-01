//! Tool: memory_workspace_xref — Cross-reference a topic across memory contexts.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct XrefParams {
    workspace_id: String,
    item: String,
}

/// Return the tool definition for memory_workspace_xref.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_workspace_xref".to_string(),
        description: Some(
            "Cross-reference a topic to find which memory contexts contain it and \
             which don't. Quick way to identify knowledge gaps across projects"
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
                    "description": "Topic/concept to cross-reference"
                }
            }
        }),
    }
}

/// Execute the memory_workspace_xref tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: XrefParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let session = session.lock().await;
    let xref = session
        .workspace_manager()
        .cross_reference(&params.workspace_id, &params.item)?;

    Ok(ToolCallResult::json(&json!({
        "item": xref.item,
        "present_in": xref.present_in,
        "absent_from": xref.absent_from,
        "coverage": format!("{}/{}", xref.present_in.len(), xref.present_in.len() + xref.absent_from.len())
    })))
}
