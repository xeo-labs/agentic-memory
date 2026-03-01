//! Tool: memory_session_resume — Load context from previous sessions.
//!
//! Solves the "bootstrap problem": the agent starts blank each conversation but
//! memories from previous sessions exist in the graph.  This tool retrieves the
//! last session episode, recent decisions, and high-confidence facts so the agent
//! can resume with full context.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use agentic_memory::{EventType, PatternParams, PatternSort};

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct ResumeParams {
    /// Maximum number of recent memories to include (default 15).
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    15
}

/// Return the tool definition for memory_session_resume.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_session_resume".to_string(),
        description: Some(
            "Load context from previous sessions. Call this at the start of every \
             conversation to restore prior context. Returns the last session summary, \
             recent decisions, and key facts"
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "default": 15,
                    "description": "Maximum number of recent memories to load"
                }
            }
        }),
    }
}

/// Execute the memory_session_resume tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: ResumeParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let session = session.lock().await;
    let graph = session.graph();
    let query = session.query_engine();

    // 1. Find the most recent episode (last session summary).
    let episode_pattern = PatternParams {
        event_types: vec![EventType::Episode],
        min_confidence: None,
        max_confidence: None,
        session_ids: vec![],
        created_after: None,
        created_before: None,
        min_decay_score: None,
        max_results: 1,
        sort_by: PatternSort::MostRecent,
    };

    let episodes = query
        .pattern(graph, episode_pattern)
        .map_err(|e| McpError::AgenticMemory(format!("Episode query failed: {e}")))?;

    let last_episode = episodes.first().map(|ep| {
        json!({
            "session_id": ep.session_id,
            "summary": ep.content,
            "created_at": ep.created_at,
        })
    });

    // 2. Find recent decisions (high-value context for resuming).
    let decision_pattern = PatternParams {
        event_types: vec![EventType::Decision],
        min_confidence: Some(0.7),
        max_confidence: None,
        session_ids: vec![],
        created_after: None,
        created_before: None,
        min_decay_score: None,
        max_results: params.limit / 3,
        sort_by: PatternSort::MostRecent,
    };

    let decisions = query
        .pattern(graph, decision_pattern)
        .map_err(|e| McpError::AgenticMemory(format!("Decision query failed: {e}")))?;

    let decision_nodes: Vec<Value> = decisions
        .iter()
        .map(|d| {
            json!({
                "id": d.id,
                "content": d.content,
                "confidence": d.confidence,
                "session_id": d.session_id,
            })
        })
        .collect();

    // 3. Find recent high-confidence facts.
    let fact_pattern = PatternParams {
        event_types: vec![EventType::Fact],
        min_confidence: Some(0.8),
        max_confidence: None,
        session_ids: vec![],
        created_after: None,
        created_before: None,
        min_decay_score: None,
        max_results: params.limit / 3,
        sort_by: PatternSort::MostRecent,
    };

    let facts = query
        .pattern(graph, fact_pattern)
        .map_err(|e| McpError::AgenticMemory(format!("Fact query failed: {e}")))?;

    let fact_nodes: Vec<Value> = facts
        .iter()
        .map(|f| {
            json!({
                "id": f.id,
                "content": f.content,
                "confidence": f.confidence,
                "session_id": f.session_id,
            })
        })
        .collect();

    // 4. Find recent skills / corrections (remaining limit).
    let remaining = params
        .limit
        .saturating_sub(decision_nodes.len() + fact_nodes.len());
    let recent_pattern = PatternParams {
        event_types: vec![EventType::Skill, EventType::Correction],
        min_confidence: None,
        max_confidence: None,
        session_ids: vec![],
        created_after: None,
        created_before: None,
        min_decay_score: None,
        max_results: remaining.max(3),
        sort_by: PatternSort::MostRecent,
    };

    let recent = query
        .pattern(graph, recent_pattern)
        .map_err(|e| McpError::AgenticMemory(format!("Recent query failed: {e}")))?;

    let recent_nodes: Vec<Value> = recent
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "event_type": r.event_type.name(),
                "content": r.content,
                "confidence": r.confidence,
                "session_id": r.session_id,
            })
        })
        .collect();

    // 5. Session gap detection.
    let current_session = session.current_session_id();
    let session_ids = graph.session_index().session_ids();
    let prev_session_id = session_ids
        .iter()
        .filter(|&&s| s < current_session)
        .max()
        .copied();

    let total_loaded = decision_nodes.len() + fact_nodes.len() + recent_nodes.len();

    Ok(ToolCallResult::json(&json!({
        "current_session": current_session,
        "previous_session": prev_session_id,
        "last_episode": last_episode,
        "recent_decisions": decision_nodes,
        "recent_facts": fact_nodes,
        "recent_other": recent_nodes,
        "total_loaded": total_loaded,
        "message": if last_episode.is_some() {
            format!("Resumed with {} memories from previous sessions", total_loaded)
        } else {
            "No previous session context found (first session)".to_string()
        }
    })))
}
