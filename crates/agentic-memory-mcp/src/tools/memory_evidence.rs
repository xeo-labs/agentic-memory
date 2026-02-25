//! Tool: memory_evidence — Get detailed evidence for a claim from memory.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use agentic_memory::TextSearchParams;

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct EvidenceParams {
    query: String,
    #[serde(default = "default_max")]
    max_results: usize,
}

fn default_max() -> usize {
    10
}

/// Return the tool definition for memory_evidence.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_evidence".to_string(),
        description: Some(
            "Get detailed evidence for a claim from stored memories. Returns matching \
             memory nodes with full content, timestamps, sessions, and relationships."
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query to search evidence for"
                },
                "max_results": {
                    "type": "integer",
                    "default": 10,
                    "description": "Maximum number of evidence items to return"
                }
            }
        }),
    }
}

/// Execute the memory_evidence tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: EvidenceParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    if params.query.trim().is_empty() {
        return Ok(ToolCallResult::json(&json!({
            "count": 0,
            "evidence": []
        })));
    }

    let session = session.lock().await;
    let graph = session.graph();

    let results = session
        .query_engine()
        .text_search(
            graph,
            graph.term_index.as_ref(),
            graph.doc_lengths.as_ref(),
            TextSearchParams {
                query: params.query.clone(),
                max_results: params.max_results,
                event_types: Vec::new(),
                session_ids: Vec::new(),
                min_score: 0.0,
            },
        )
        .map_err(|e| McpError::AgenticMemory(format!("Evidence search failed: {e}")))?;

    let evidence: Vec<Value> = results
        .iter()
        .filter_map(|m| {
            graph.get_node(m.node_id).map(|node| {
                // Get related edges
                let outgoing: Vec<Value> = graph
                    .edges_from(node.id)
                    .iter()
                    .map(|e| {
                        json!({
                            "target_id": e.target_id,
                            "edge_type": e.edge_type.name(),
                            "weight": e.weight,
                        })
                    })
                    .collect();

                let incoming: Vec<Value> = graph
                    .edges_to(node.id)
                    .iter()
                    .map(|e| {
                        json!({
                            "source_id": e.source_id,
                            "edge_type": e.edge_type.name(),
                            "weight": e.weight,
                        })
                    })
                    .collect();

                json!({
                    "node_id": node.id,
                    "event_type": node.event_type.name(),
                    "content": node.content,
                    "confidence": node.confidence,
                    "session_id": node.session_id,
                    "created_at": node.created_at,
                    "last_accessed": node.last_accessed,
                    "access_count": node.access_count,
                    "decay_score": node.decay_score,
                    "score": m.score,
                    "matched_terms": m.matched_terms,
                    "outgoing_edges": outgoing,
                    "incoming_edges": incoming,
                    "source": format!("session:{}", node.session_id),
                })
            })
        })
        .collect();

    Ok(ToolCallResult::json(&json!({
        "query": params.query,
        "count": evidence.len(),
        "evidence": evidence
    })))
}
