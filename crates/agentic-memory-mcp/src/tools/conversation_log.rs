//! Tool: conversation_log — Log user prompts and agent responses into the conversation thread.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use agentic_memory::EventType;

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct ConversationLogParams {
    #[serde(default)]
    user_message: Option<String>,
    #[serde(default)]
    agent_response: Option<String>,
    #[serde(default)]
    topic: Option<String>,
}

/// Return the tool definition for conversation_log.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "conversation_log".to_string(),
        description: Some(
            "Log a user prompt and/or agent response into the conversation thread. \
             Call this to record what the user said and what you decided to do. \
             Entries are automatically linked into the session's temporal chain"
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "user_message": {
                    "type": "string",
                    "description": "What the user said or asked"
                },
                "agent_response": {
                    "type": "string",
                    "description": "Summary of the agent's response or action taken"
                },
                "topic": {
                    "type": "string",
                    "description": "Optional topic or category (e.g., 'project-setup', 'debugging')"
                }
            }
        }),
    }
}

/// Execute the conversation_log tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: ConversationLogParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    if params.user_message.is_none() && params.agent_response.is_none() {
        return Err(McpError::InvalidParams(
            "At least one of 'user_message' or 'agent_response' must be provided".to_string(),
        ));
    }

    // Build content with a recognizable prefix.
    let mut parts = Vec::new();
    if let Some(ref topic) = params.topic {
        parts.push(format!("topic={topic}"));
    }
    if let Some(ref msg) = params.user_message {
        parts.push(format!("user: {msg}"));
    }
    if let Some(ref resp) = params.agent_response {
        parts.push(format!("agent: {resp}"));
    }
    let content = format!("[conversation] {}", parts.join(" | "));

    let mut session = session.lock().await;

    let prev_id = session.last_temporal_node_id();
    let (node_id, _) = session.add_event(EventType::Fact, &content, 0.85, vec![])?;

    // Link into the temporal chain.
    let mut temporal_edges = 0;
    if let Some(prev) = prev_id {
        if session.link_temporal(prev, node_id).is_ok() {
            temporal_edges = 1;
        }
    }
    session.advance_temporal_chain(node_id);

    Ok(ToolCallResult::json(&json!({
        "node_id": node_id,
        "edges_created": temporal_edges,
        "message": "Conversation logged and linked to temporal chain"
    })))
}
