//! Tool: memory_workspace_add — Add an .amem file to a workspace.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::workspace::ContextRole;
use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct AddParams {
    workspace_id: String,
    path: String,
    #[serde(default = "default_role")]
    role: String,
    label: Option<String>,
}

fn default_role() -> String {
    "primary".to_string()
}

/// Return the tool definition for memory_workspace_add.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_workspace_add".to_string(),
        description: Some(
            "Add an .amem memory file to a workspace. Each file becomes a context \
             that can be queried alongside other loaded memories."
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "required": ["workspace_id", "path"],
            "properties": {
                "workspace_id": {
                    "type": "string",
                    "description": "ID of the workspace to add to"
                },
                "path": {
                    "type": "string",
                    "description": "Path to the .amem file"
                },
                "role": {
                    "type": "string",
                    "enum": ["primary", "secondary", "reference", "archive"],
                    "default": "primary",
                    "description": "Role of this context in the workspace"
                },
                "label": {
                    "type": "string",
                    "description": "Optional human-readable label for this context"
                }
            }
        }),
    }
}

/// Execute the memory_workspace_add tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: AddParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let role = ContextRole::from_str(&params.role).ok_or_else(|| {
        McpError::InvalidParams(format!("Invalid role: {}. Use primary/secondary/reference/archive", params.role))
    })?;

    let mut session = session.lock().await;
    let ctx_id = session.workspace_manager_mut().add_context(
        &params.workspace_id,
        &params.path,
        role,
        params.label,
    )?;

    Ok(ToolCallResult::json(&json!({
        "context_id": ctx_id,
        "workspace_id": params.workspace_id,
        "path": params.path,
        "role": role.label(),
        "status": "added"
    })))
}
