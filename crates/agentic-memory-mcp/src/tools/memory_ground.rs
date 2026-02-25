//! Tool: memory_ground — Verify a claim has memory backing (anti-hallucination).

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use agentic_memory::TextSearchParams;

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct GroundParams {
    claim: String,
    #[serde(default = "default_threshold")]
    threshold: f32,
}

fn default_threshold() -> f32 {
    0.3
}

/// Return the tool definition for memory_ground.
pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_ground".to_string(),
        description: Some(
            "Verify a claim has memory backing. Returns verified/partial/ungrounded status \
             to prevent hallucination about what was previously remembered."
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "required": ["claim"],
            "properties": {
                "claim": {
                    "type": "string",
                    "description": "The claim to verify against stored memories"
                },
                "threshold": {
                    "type": "number",
                    "default": 0.3,
                    "description": "Minimum BM25 score to consider a match (0.0-10.0)"
                }
            }
        }),
    }
}

/// Execute the memory_ground tool.
pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: GroundParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    if params.claim.trim().is_empty() {
        return Ok(ToolCallResult::json(&json!({
            "status": "ungrounded",
            "claim": params.claim,
            "reason": "Empty claim",
            "suggestions": []
        })));
    }

    let session = session.lock().await;
    let graph = session.graph();

    // Use BM25 text search to find matching memories
    let results = session
        .query_engine()
        .text_search(
            graph,
            graph.term_index.as_ref(),
            graph.doc_lengths.as_ref(),
            TextSearchParams {
                query: params.claim.clone(),
                max_results: 10,
                event_types: Vec::new(),
                session_ids: Vec::new(),
                min_score: 0.0,
            },
        )
        .map_err(|e| McpError::AgenticMemory(format!("Grounding search failed: {e}")))?;

    let threshold = params.threshold;

    // Classify results
    let strong: Vec<&agentic_memory::TextMatch> =
        results.iter().filter(|m| m.score >= threshold).collect();

    if strong.is_empty() {
        // Try fuzzy suggestions
        let suggestions = suggest_similar_content(graph, &params.claim);
        return Ok(ToolCallResult::json(&json!({
            "status": "ungrounded",
            "claim": params.claim,
            "reason": "No memory nodes match this claim",
            "suggestions": suggestions
        })));
    }

    // Build evidence from strong matches
    let evidence: Vec<Value> = strong
        .iter()
        .filter_map(|m| {
            graph.get_node(m.node_id).map(|node| {
                json!({
                    "node_id": node.id,
                    "event_type": node.event_type.name(),
                    "content": node.content,
                    "confidence": node.confidence,
                    "session_id": node.session_id,
                    "created_at": node.created_at,
                    "score": m.score,
                    "matched_terms": m.matched_terms,
                })
            })
        })
        .collect();

    let avg_score: f32 = strong.iter().map(|m| m.score).sum::<f32>() / strong.len() as f32;
    let confidence = (avg_score / (avg_score + 1.0)).min(1.0);

    Ok(ToolCallResult::json(&json!({
        "status": "verified",
        "claim": params.claim,
        "confidence": confidence,
        "evidence_count": evidence.len(),
        "evidence": evidence
    })))
}

/// Find memory content that is similar to the query (for suggestions).
fn suggest_similar_content(graph: &agentic_memory::MemoryGraph, query: &str) -> Vec<String> {
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let mut suggestions: Vec<(f32, String)> = Vec::new();

    for node in graph.nodes() {
        let content_lower = node.content.to_lowercase();
        // Count overlapping words
        let overlap = query_words
            .iter()
            .filter(|w| content_lower.contains(**w))
            .count();
        if overlap > 0 {
            let score = overlap as f32 / query_words.len().max(1) as f32;
            let preview = if node.content.len() > 80 {
                format!("{}...", &node.content[..80])
            } else {
                node.content.clone()
            };
            suggestions.push((score, preview));
        }
    }

    suggestions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    suggestions.truncate(5);
    suggestions.into_iter().map(|(_, s)| s).collect()
}
