//! Tool: memory_correct — Record a correction to a previous belief.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CorrectParams {
    old_node_id: u64,
    new_content: String,
    #[serde(default = "default_confidence")]
    confidence: f32,
    reason: Option<String>,
}

fn default_confidence() -> f32 {
    0.95
}

/// Return the tool definition for memory_correct.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_correct".to_string(),
        description: Some(
            "Record a correction to a previous belief, creating a new node that supersedes the old one"
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "old_node_id": { "type": "integer", "description": "ID of the node being corrected" },
                "new_content": { "type": "string", "description": "The correct information" },
                "confidence": { "type": "number", "default": 0.95 },
                "reason": { "type": "string", "description": "Optional explanation for the correction" }
            },
            "required": ["old_node_id", "new_content"]
        }),
    }
}

/// Execute the memory_correct tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: CorrectParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let mut session = session.lock().await;

    // Verify the old node exists
    if session.graph().get_node(params.old_node_id).is_none() {
        return Err(McpError::NodeNotFound(params.old_node_id));
    }

    let new_id = session.correct_node(params.old_node_id, &params.new_content)?;

    Ok(ToolCallResult::json(&json!({
        "new_node_id": new_id,
        "old_node_id": params.old_node_id,
        "supersedes": true,
        "reason": params.reason,
    })))
}
