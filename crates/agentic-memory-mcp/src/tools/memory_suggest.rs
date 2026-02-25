//! Tool: memory_suggest — Find similar memories for corrections/suggestions.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use agentic_memory::TextSearchParams;

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct SuggestParams {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    5
}

/// Return the tool definition for memory_suggest.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_suggest".to_string(),
        description: Some(
            "Find similar memories when a claim doesn't match exactly. Useful for \
             correcting misremembered facts or finding related knowledge."
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query to find suggestions for"
                },
                "limit": {
                    "type": "integer",
                    "default": 5,
                    "description": "Maximum number of suggestions"
                }
            }
        }),
    }
}

/// Execute the memory_suggest tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: SuggestParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    if params.query.trim().is_empty() {
        return Ok(ToolCallResult::json(&json!({
            "query": params.query,
            "count": 0,
            "suggestions": []
        })));
    }

    let session = session.lock().await;
    let graph = session.graph();

    // Use text search with low threshold to catch partial matches
    let results = session
        .query_engine()
        .text_search(
            graph,
            graph.term_index.as_ref(),
            graph.doc_lengths.as_ref(),
            TextSearchParams {
                query: params.query.clone(),
                max_results: params.limit * 2,
                event_types: Vec::new(),
                session_ids: Vec::new(),
                min_score: 0.0,
            },
        )
        .map_err(|e| McpError::AgenticMemory(format!("Suggest search failed: {e}")))?;

    let mut suggestions: Vec<Value> = results
        .iter()
        .filter_map(|m| {
            graph.get_node(m.node_id).map(|node| {
                json!({
                    "node_id": node.id,
                    "event_type": node.event_type.name(),
                    "content": node.content,
                    "confidence": node.confidence,
                    "relevance_score": m.score,
                    "matched_terms": m.matched_terms,
                    "session_id": node.session_id,
                })
            })
        })
        .collect();

    // Also add word-overlap suggestions from content scanning
    if suggestions.len() < params.limit {
        let query_lower = params.query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        let existing_ids: Vec<u64> = results.iter().map(|m| m.node_id).collect();

        let mut extra: Vec<(f32, Value)> = Vec::new();
        for node in graph.nodes() {
            if existing_ids.contains(&node.id) {
                continue;
            }
            let content_lower = node.content.to_lowercase();
            let overlap = query_words
                .iter()
                .filter(|w| content_lower.contains(**w))
                .count();
            if overlap > 0 {
                let score = overlap as f32 / query_words.len().max(1) as f32;
                extra.push((
                    score,
                    json!({
                        "node_id": node.id,
                        "event_type": node.event_type.name(),
                        "content": node.content,
                        "confidence": node.confidence,
                        "relevance_score": score,
                        "matched_terms": [],
                        "session_id": node.session_id,
                    }),
                ));
            }
        }

        extra.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, val) in extra.into_iter().take(params.limit - suggestions.len()) {
            suggestions.push(val);
        }
    }

    suggestions.truncate(params.limit);

    Ok(ToolCallResult::json(&json!({
        "query": params.query,
        "count": suggestions.len(),
        "suggestions": suggestions
    })))
}
