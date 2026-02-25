//! Tool: memory_workspace_create — Create a multi-memory workspace.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct CreateParams {
    name: String,
}

/// Return the tool definition for memory_workspace_create.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_workspace_create".to_string(),
        description: Some(
            "Create a multi-memory workspace for loading and querying multiple .amem files \
             simultaneously. Use this to compare memories across projects or time periods."
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name for the workspace"
                }
            }
        }),
    }
}

/// Execute the memory_workspace_create tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: CreateParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let mut session = session.lock().await;
    let id = session.workspace_manager_mut().create(&params.name);

    Ok(ToolCallResult::json(&json!({
        "workspace_id": id,
        "name": params.name,
        "status": "created"
    })))
}
