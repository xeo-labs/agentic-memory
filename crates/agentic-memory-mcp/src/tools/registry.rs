//! Tool registration and dispatch.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde_json::Value;

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

use super::{
    conversation_log, memory_add, memory_causal, memory_context, memory_correct, memory_evidence,
    memory_ground, memory_quality, memory_query, memory_resolve, memory_similar, memory_stats,
    memory_suggest, memory_temporal, memory_traverse, memory_workspace_add,
    memory_workspace_compare, memory_workspace_create, memory_workspace_list,
    memory_workspace_query, memory_workspace_xref, session_end, session_start,
};

/// Registry of all available MCP tools.
pub struct ToolRegistry;

impl ToolRegistry {
    /// List all available tool definitions.
    pub fn list_tools() -> Vec<ToolDefinition> {
        vec![
            conversation_log::definition(),
            memory_add::definition(),
            memory_query::definition(),
            memory_quality::definition(),
            memory_traverse::definition(),
            memory_correct::definition(),
            memory_resolve::definition(),
            memory_context::definition(),
            memory_similar::definition(),
            memory_causal::definition(),
            memory_temporal::definition(),
            memory_stats::definition(),
            // V2: Grounding (anti-hallucination)
            memory_ground::definition(),
            memory_evidence::definition(),
            memory_suggest::definition(),
            // V2: Multi-context workspaces
            memory_workspace_create::definition(),
            memory_workspace_add::definition(),
            memory_workspace_list::definition(),
            memory_workspace_query::definition(),
            memory_workspace_compare::definition(),
            memory_workspace_xref::definition(),
            // Session lifecycle
            session_start::definition(),
            session_end::definition(),
        ]
    }

    /// Dispatch a tool call to the appropriate handler.
    pub async fn call(
        name: &str,
        arguments: Option<Value>,
        session: &Arc<Mutex<SessionManager>>,
    ) -> McpResult<ToolCallResult> {
        let args = arguments.unwrap_or(Value::Object(serde_json::Map::new()));

        match name {
            "conversation_log" => conversation_log::execute(args, session).await,
            "memory_add" => memory_add::execute(args, session).await,
            "memory_query" => memory_query::execute(args, session).await,
            "memory_quality" => memory_quality::execute(args, session).await,
            "memory_traverse" => memory_traverse::execute(args, session).await,
            "memory_correct" => memory_correct::execute(args, session).await,
            "memory_resolve" => memory_resolve::execute(args, session).await,
            "memory_context" => memory_context::execute(args, session).await,
            "memory_similar" => memory_similar::execute(args, session).await,
            "memory_causal" => memory_causal::execute(args, session).await,
            "memory_temporal" => memory_temporal::execute(args, session).await,
            "memory_stats" => memory_stats::execute(args, session).await,
            // V2: Grounding
            "memory_ground" => memory_ground::execute(args, session).await,
            "memory_evidence" => memory_evidence::execute(args, session).await,
            "memory_suggest" => memory_suggest::execute(args, session).await,
            // V2: Workspaces
            "memory_workspace_create" => memory_workspace_create::execute(args, session).await,
            "memory_workspace_add" => memory_workspace_add::execute(args, session).await,
            "memory_workspace_list" => memory_workspace_list::execute(args, session).await,
            "memory_workspace_query" => memory_workspace_query::execute(args, session).await,
            "memory_workspace_compare" => memory_workspace_compare::execute(args, session).await,
            "memory_workspace_xref" => memory_workspace_xref::execute(args, session).await,
            // Session
            "session_start" => session_start::execute(args, session).await,
            "session_end" => session_end::execute(args, session).await,
            _ => Err(McpError::ToolNotFound(name.to_string())),
        }
    }
}
