//! Tool: memory_quality — Graph quality and reliability summary.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use agentic_memory::{MemoryQualityParams, QueryEngine};

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

#[derive(Debug, Deserialize)]
struct QualityParams {
    #[serde(default = "default_low_conf")]
    low_confidence_threshold: f32,
    #[serde(default = "default_stale_decay")]
    stale_decay_threshold: f32,
    #[serde(default = "default_max_examples")]
    max_examples: usize,
}

fn default_low_conf() -> f32 {
    0.45
}
fn default_stale_decay() -> f32 {
    0.20
}
fn default_max_examples() -> usize {
    20
}

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "memory_quality".to_string(),
        description: Some(
            "Evaluate memory reliability: confidence, staleness, orphan nodes, and unsupported decisions"
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "low_confidence_threshold": { "type": "number", "default": 0.45 },
                "stale_decay_threshold": { "type": "number", "default": 0.20 },
                "max_examples": { "type": "integer", "default": 20 }
            }
        }),
    }
}

pub async fn execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let params: QualityParams = serde_json::from_value(args)
        .map_err(|e| McpError::InvalidParams(format!("invalid params: {e}")))?;

    let session = session.lock().await;
    let graph = session.graph();
    let qe = QueryEngine::new();
    let report = qe
        .memory_quality(
            graph,
            MemoryQualityParams {
                low_confidence_threshold: params.low_confidence_threshold.clamp(0.0, 1.0),
                stale_decay_threshold: params.stale_decay_threshold.clamp(0.0, 1.0),
                max_examples: params.max_examples.max(1),
            },
        )
        .map_err(|e| McpError::AgenticMemory(format!("memory quality failed: {e}")))?;

    Ok(ToolCallResult::json(&json!({
        "status": report.status,
        "summary": {
            "nodes": report.node_count,
            "edges": report.edge_count,
            "low_confidence_count": report.low_confidence_count,
            "stale_count": report.stale_count,
            "orphan_count": report.orphan_count,
            "decisions_without_support_count": report.decisions_without_support_count,
            "contradiction_edges": report.contradiction_edges,
            "supersedes_edges": report.supersedes_edges
        },
        "examples": {
            "low_confidence": report.low_confidence_examples,
            "stale": report.stale_examples,
            "orphan": report.orphan_examples,
            "unsupported_decisions": report.unsupported_decision_examples
        }
    })))
}
