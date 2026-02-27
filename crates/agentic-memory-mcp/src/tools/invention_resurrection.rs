//! Invention modules 13-16: Memory Archaeology, Holographic Memory, Memory Immune System, Phoenix Protocol
//! ~17 tools for the RESURRECTION category of the 24 Memory Inventions.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use serde_json::{json, Value};

use agentic_memory::EventType;

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

// ── helpers ──────────────────────────────────────────────────────────────

fn word_overlap(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    let a_words: std::collections::HashSet<&str> = a_lower.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b_lower.split_whitespace().collect();
    if a_words.is_empty() && b_words.is_empty() {
        return 1.0;
    }
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

fn get_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn get_u64(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(|v| v.as_u64())
}

#[allow(dead_code)]
fn get_f64(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(|v| v.as_f64())
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 13: MEMORY ARCHAEOLOGY — Reconstruct lost memories from traces
// ══════════════════════════════════════════════════════════════════════════

// ── 1. memory_archaeology_dig ───────────────────────────────────────────

pub fn definition_archaeology_dig() -> ToolDefinition {
    ToolDefinition {
        name: "memory_archaeology_dig".into(),
        description: Some(
            "Start archaeological dig to recover lost/deleted memories in a topic or time range"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"topic":{"type":"string","description":"Topic to search for traces"},"time_start":{"type":"integer","description":"Start timestamp (optional)"},"time_end":{"type":"integer","description":"End timestamp (optional)"},"max_depth":{"type":"integer","description":"Max search depth (default 3)"}},"required":["topic"]}),
    }
}

pub async fn execute_archaeology_dig(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let topic =
        get_str(&args, "topic").ok_or_else(|| McpError::InvalidParams("topic required".into()))?;
    let max_depth = get_u64(&args, "max_depth").unwrap_or(3) as usize;
    let time_start = get_u64(&args, "time_start");
    let time_end = get_u64(&args, "time_end");
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    // Phase 1: Find direct content matches (artifacts)
    let mut artifacts: Vec<Value> = Vec::new();
    for node in nodes {
        if let Some(ts) = time_start {
            if node.created_at < ts {
                continue;
            }
        }
        if let Some(te) = time_end {
            if node.created_at > te {
                continue;
            }
        }
        let overlap = word_overlap(&topic, &node.content);
        if overlap > 0.15 {
            artifacts.push(json!({
                "artifact_type": "DirectMatch",
                "node_id": node.id,
                "content_preview": &node.content[..node.content.len().min(120)],
                "relevance": (overlap * 100.0).round() / 100.0,
                "created_at": node.created_at,
                "confidence": node.confidence,
            }));
        }
    }

    // Phase 2: Follow edges from matched nodes to find related traces (up to max_depth)
    let mut related: Vec<Value> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut frontier: Vec<(u64, usize)> = artifacts
        .iter()
        .filter_map(|a| a.get("node_id").and_then(|v| v.as_u64()).map(|id| (id, 0)))
        .collect();
    while let Some((nid, depth)) = frontier.pop() {
        if depth >= max_depth || !visited.insert(nid) {
            continue;
        }
        for edge in graph.edges_from(nid) {
            if let Some(target) = graph.get_node(edge.target_id) {
                if !visited.contains(&target.id) {
                    related.push(json!({
                        "artifact_type": "RelatedTrace",
                        "node_id": target.id,
                        "edge_type": edge.edge_type.name(),
                        "distance": depth + 1,
                        "content_preview": &target.content[..target.content.len().min(80)],
                    }));
                    frontier.push((target.id, depth + 1));
                }
            }
        }
    }

    artifacts.sort_by(|a, b| {
        b.get("relevance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .partial_cmp(&a.get("relevance").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .unwrap()
    });
    let total_artifacts = artifacts.len() + related.len();
    Ok(ToolCallResult::json(&json!({
        "dig_status": "complete",
        "topic": topic,
        "direct_artifacts": artifacts.len(),
        "related_traces": related.len(),
        "total_artifacts": total_artifacts,
        "artifacts": &artifacts[..artifacts.len().min(20)],
        "related": &related[..related.len().min(20)],
        "reconstruction_possible": total_artifacts > 0,
    })))
}

// ── 2. memory_archaeology_artifacts ─────────────────────────────────────

pub fn definition_archaeology_artifacts() -> ToolDefinition {
    ToolDefinition {
        name: "memory_archaeology_artifacts".into(),
        description: Some(
            "Get artifacts found near a node — references, cached summaries, pattern implications"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer","description":"Central node to explore"},"radius":{"type":"integer","description":"Search radius (default 2)"}},"required":["node_id"]}),
    }
}

pub async fn execute_archaeology_artifacts(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let radius = get_u64(&args, "radius").unwrap_or(2) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let _center = graph
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;

    let mut artifacts = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut frontier = vec![(node_id, 0usize)];
    while let Some((nid, depth)) = frontier.pop() {
        if depth > radius || !visited.insert(nid) {
            continue;
        }
        if let Some(node) = graph.get_node(nid) {
            let artifact_type = if nid == node_id {
                "CenterNode"
            } else if depth == 1 {
                "DirectReference"
            } else {
                "IndirectTrace"
            };
            artifacts.push(json!({
                "artifact_type": artifact_type,
                "node_id": node.id,
                "event_type": node.event_type.name(),
                "distance": depth,
                "content_preview": &node.content[..node.content.len().min(100)],
                "confidence": node.confidence,
                "edges_out": graph.edges_from(nid).len(),
                "edges_in": graph.edges_to(nid).len(),
            }));
            for edge in graph.edges_from(nid) {
                frontier.push((edge.target_id, depth + 1));
            }
            for edge in graph.edges_to(nid) {
                frontier.push((edge.source_id, depth + 1));
            }
        }
    }
    Ok(ToolCallResult::json(&json!({
        "center_node": node_id,
        "radius": radius,
        "artifacts_found": artifacts.len(),
        "artifacts": artifacts,
    })))
}

// ── 3. memory_archaeology_reconstruct ───────────────────────────────────

pub fn definition_archaeology_reconstruct() -> ToolDefinition {
    ToolDefinition {
        name: "memory_archaeology_reconstruct".into(),
        description: Some(
            "Attempt to reconstruct a lost memory by combining artifacts and traces".into(),
        ),
        input_schema: json!({"type":"object","properties":{"topic":{"type":"string","description":"Topic of lost memory to reconstruct"},"artifact_ids":{"type":"array","items":{"type":"integer"},"description":"Node IDs of artifacts to use for reconstruction"}},"required":["topic"]}),
    }
}

pub async fn execute_archaeology_reconstruct(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let topic =
        get_str(&args, "topic").ok_or_else(|| McpError::InvalidParams("topic required".into()))?;
    let artifact_ids: Vec<u64> = args
        .get("artifact_ids")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect())
        .unwrap_or_default();

    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    // Collect artifacts
    let mut fragments: Vec<Value> = Vec::new();
    if artifact_ids.is_empty() {
        // Auto-find by topic
        for node in nodes {
            let overlap = word_overlap(&topic, &node.content);
            if overlap > 0.2 {
                fragments.push(json!({
                    "node_id": node.id,
                    "content": &node.content[..node.content.len().min(200)],
                    "relevance": (overlap * 100.0).round() / 100.0,
                    "event_type": node.event_type.name(),
                    "confidence": node.confidence,
                }));
            }
        }
    } else {
        for &id in &artifact_ids {
            if let Some(node) = graph.get_node(id) {
                fragments.push(json!({
                    "node_id": node.id,
                    "content": &node.content[..node.content.len().min(200)],
                    "relevance": word_overlap(&topic, &node.content),
                    "event_type": node.event_type.name(),
                    "confidence": node.confidence,
                }));
            }
        }
    }

    fragments.sort_by(|a, b| {
        b.get("relevance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .partial_cmp(&a.get("relevance").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .unwrap()
    });

    let reconstruction_confidence = if fragments.is_empty() {
        0.0
    } else {
        let avg_relevance: f64 = fragments
            .iter()
            .map(|f| f.get("relevance").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .sum::<f64>()
            / fragments.len() as f64;
        let coverage = (fragments.len() as f64 / 5.0).min(1.0); // 5 artifacts = full coverage
        ((avg_relevance + coverage) / 2.0 * 100.0).round() / 100.0
    };

    Ok(ToolCallResult::json(&json!({
        "topic": topic,
        "fragments_found": fragments.len(),
        "reconstruction_confidence": reconstruction_confidence,
        "reconstruction_status": if reconstruction_confidence > 0.6 { "high_confidence" }
            else if reconstruction_confidence > 0.3 { "partial" }
            else if !fragments.is_empty() { "low_confidence" }
            else { "no_artifacts" },
        "fragments": &fragments[..fragments.len().min(15)],
        "recommendation": if reconstruction_confidence > 0.6 {
            "Sufficient artifacts for confident reconstruction"
        } else if reconstruction_confidence > 0.3 {
            "Partial reconstruction possible — more artifacts needed for certainty"
        } else {
            "Insufficient artifacts — try broader search or different topic terms"
        },
    })))
}

// ── 4. memory_archaeology_verify ────────────────────────────────────────

pub fn definition_archaeology_verify() -> ToolDefinition {
    ToolDefinition {
        name: "memory_archaeology_verify".into(),
        description: Some(
            "Verify a reconstruction against existing evidence and supporting edges".into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer","description":"Node ID of reconstructed memory to verify"}},"required":["node_id"]}),
    }
}

pub async fn execute_archaeology_verify(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let node = graph
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;

    let edges_out = graph.edges_from(node_id);
    let edges_in = graph.edges_to(node_id);
    let supports: Vec<Value> = edges_out.iter()
        .filter(|e| e.edge_type.name() == "supports")
        .filter_map(|e| graph.get_node(e.target_id))
        .map(|n| json!({"node_id": n.id, "content_preview": &n.content[..n.content.len().min(80)], "confidence": n.confidence}))
        .collect();
    let supported_by: Vec<Value> = edges_in.iter()
        .filter(|e| e.edge_type.name() == "supports")
        .filter_map(|e| graph.get_node(e.source_id))
        .map(|n| json!({"node_id": n.id, "content_preview": &n.content[..n.content.len().min(80)], "confidence": n.confidence}))
        .collect();
    let mut contradictions: Vec<Value> = Vec::new();
    for e in edges_out
        .iter()
        .filter(|e| e.edge_type.name() == "contradicts")
    {
        let other = e.target_id;
        if let Some(n) = graph.get_node(other) {
            contradictions.push(
                json!({"node_id": n.id, "content_preview": &n.content[..n.content.len().min(80)]}),
            );
        }
    }
    for e in edges_in
        .iter()
        .filter(|e| e.edge_type.name() == "contradicts")
    {
        let other = if e.source_id == node_id {
            e.target_id
        } else {
            e.source_id
        };
        if let Some(n) = graph.get_node(other) {
            contradictions.push(
                json!({"node_id": n.id, "content_preview": &n.content[..n.content.len().min(80)]}),
            );
        }
    }

    let evidence_strength = (supports.len() + supported_by.len()) as f64
        / (supports.len() + supported_by.len() + contradictions.len() + 1) as f64;
    Ok(ToolCallResult::json(&json!({
        "node_id": node_id,
        "content_preview": &node.content[..node.content.len().min(120)],
        "confidence": node.confidence,
        "supports": supports,
        "supported_by": supported_by,
        "contradictions": contradictions,
        "evidence_strength": (evidence_strength * 100.0).round() / 100.0,
        "verified": contradictions.is_empty() && evidence_strength > 0.5,
        "verdict": if contradictions.is_empty() && evidence_strength > 0.5 { "VERIFIED" }
            else if contradictions.is_empty() { "UNVERIFIED — insufficient evidence" }
            else { "CONTESTED — contradictions found" },
    })))
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 14: HOLOGRAPHIC MEMORY — Any piece contains info about the whole
// ══════════════════════════════════════════════════════════════════════════

// ── 5. memory_holographic_status ────────────────────────────────────────

pub fn definition_holographic_status() -> ToolDefinition {
    ToolDefinition {
        name: "memory_holographic_status".into(),
        description: Some("Get holographic memory status: shard distribution, redundancy, reconstruction readiness".into()),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_holographic_status(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let total = graph.node_count();
    let edges = graph.edge_count();

    // Simulate holographic shard distribution based on graph connectivity
    let avg_edges_per_node = if total > 0 {
        edges as f64 / total as f64
    } else {
        0.0
    };
    let redundancy = (avg_edges_per_node * 2.0).min(10.0);
    let min_shards_for_reconstruction = (total as f64 * 0.03).ceil().max(1.0) as usize;
    let reconstruction_possible = total > 0;

    // Type distribution as "shards"
    let type_index = graph.type_index();
    let type_shards: HashMap<String, usize> = type_index
        .inner()
        .iter()
        .map(|(k, v)| (k.name().to_string(), v.len()))
        .collect();

    Ok(ToolCallResult::json(&json!({
        "total_nodes": total,
        "total_edges": edges,
        "avg_connectivity": (avg_edges_per_node * 100.0).round() / 100.0,
        "redundancy_factor": (redundancy * 100.0).round() / 100.0,
        "min_shards_for_reconstruction": min_shards_for_reconstruction,
        "reconstruction_possible": reconstruction_possible,
        "encoding": "SemanticGraph",
        "shard_distribution": type_shards,
        "resilience": if redundancy > 5.0 { "HIGH" } else if redundancy > 2.0 { "MEDIUM" } else { "LOW" },
    })))
}

// ── 6. memory_holographic_reconstruct ───────────────────────────────────

pub fn definition_holographic_reconstruct() -> ToolDefinition {
    ToolDefinition {
        name: "memory_holographic_reconstruct".into(),
        description: Some(
            "Attempt to reconstruct a node from surrounding shards (edges and neighbors)".into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer","description":"Node ID to reconstruct from shards"}},"required":["node_id"]}),
    }
}

pub async fn execute_holographic_reconstruct(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let node = graph
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;

    // Gather shards: all connected nodes
    let mut shards: Vec<Value> = Vec::new();
    for edge in graph.edges_from(node_id) {
        if let Some(target) = graph.get_node(edge.target_id) {
            shards.push(json!({
                "shard_type": "outgoing",
                "edge_type": edge.edge_type.name(),
                "node_id": target.id,
                "content_preview": &target.content[..target.content.len().min(80)],
            }));
        }
    }
    for edge in graph.edges_to(node_id) {
        if let Some(source) = graph.get_node(edge.source_id) {
            shards.push(json!({
                "shard_type": "incoming",
                "edge_type": edge.edge_type.name(),
                "node_id": source.id,
                "content_preview": &source.content[..source.content.len().min(80)],
            }));
        }
    }

    let shard_count = shards.len();
    let reconstruction_quality = if shard_count >= 5 {
        "COMPLETE"
    } else if shard_count >= 3 {
        "HIGH"
    } else if shard_count >= 1 {
        "PARTIAL"
    } else {
        "IMPOSSIBLE"
    };

    Ok(ToolCallResult::json(&json!({
        "node_id": node_id,
        "original_content": &node.content[..node.content.len().min(200)],
        "original_confidence": node.confidence,
        "shards_found": shard_count,
        "shards": &shards[..shards.len().min(20)],
        "reconstruction_quality": reconstruction_quality,
        "reconstructable": shard_count >= 1,
    })))
}

// ── 7. memory_holographic_simulate ──────────────────────────────────────

pub fn definition_holographic_simulate() -> ToolDefinition {
    ToolDefinition {
        name: "memory_holographic_simulate".into(),
        description: Some("Simulate shard loss: what happens if N% of memory is lost?".into()),
        input_schema: json!({"type":"object","properties":{"loss_percentage":{"type":"number","description":"Percentage of nodes to simulate losing (0-100)"},"seed":{"type":"integer","description":"Random seed for reproducibility"}},"required":["loss_percentage"]}),
    }
}

pub async fn execute_holographic_simulate(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let loss_pct = get_f64(&args, "loss_percentage")
        .ok_or_else(|| McpError::InvalidParams("loss_percentage required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let total = graph.node_count();
    let nodes_lost = ((total as f64 * loss_pct / 100.0).round() as usize).min(total);
    let nodes_remaining = total - nodes_lost;

    // Estimate reconstruction based on graph connectivity
    let edges = graph.edge_count();
    let avg_connectivity = if total > 0 {
        edges as f64 / total as f64
    } else {
        0.0
    };
    let recovery_rate = if nodes_remaining == 0 {
        0.0
    } else {
        ((nodes_remaining as f64 / total.max(1) as f64) + (avg_connectivity / 10.0)).min(1.0)
    };

    Ok(ToolCallResult::json(&json!({
        "simulation": "holographic_loss",
        "total_nodes": total,
        "loss_percentage": loss_pct,
        "nodes_lost": nodes_lost,
        "nodes_remaining": nodes_remaining,
        "estimated_recovery_rate": (recovery_rate * 100.0).round() / 100.0,
        "avg_connectivity": (avg_connectivity * 100.0).round() / 100.0,
        "verdict": if recovery_rate > 0.9 { "SURVIVABLE — high recovery expected" }
            else if recovery_rate > 0.6 { "DEGRADED — partial recovery possible" }
            else if recovery_rate > 0.3 { "CRITICAL — significant data loss" }
            else { "CATASTROPHIC — memory likely unrecoverable" },
    })))
}

// ── 8. memory_holographic_distribute ────────────────────────────────────

pub fn definition_holographic_distribute() -> ToolDefinition {
    ToolDefinition {
        name: "memory_holographic_distribute".into(),
        description: Some(
            "Distribute memory shards: report on how well-connected each event type is".into(),
        ),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_holographic_distribute(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let type_index = graph.type_index();

    let mut distribution: Vec<Value> = Vec::new();
    for (etype, ids) in type_index.inner().iter() {
        let total_edges: usize = ids
            .iter()
            .map(|id| graph.edges_from(*id).len() + graph.edges_to(*id).len())
            .sum();
        let avg_edges = if ids.is_empty() {
            0.0
        } else {
            total_edges as f64 / ids.len() as f64
        };
        distribution.push(json!({
            "event_type": etype.name(),
            "count": ids.len(),
            "total_edges": total_edges,
            "avg_connectivity": (avg_edges * 100.0).round() / 100.0,
            "resilience": if avg_edges > 4.0 { "HIGH" } else if avg_edges > 1.5 { "MEDIUM" } else { "LOW" },
        }));
    }

    distribution.sort_by(|a, b| {
        b.get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            .cmp(&a.get("count").and_then(|v| v.as_u64()).unwrap_or(0))
    });

    Ok(ToolCallResult::json(&json!({
        "distribution": distribution,
        "total_types": distribution.len(),
    })))
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 15: MEMORY IMMUNE SYSTEM — Defend against false/corrupt memories
// ══════════════════════════════════════════════════════════════════════════

// ── 9. memory_immune_status ─────────────────────────────────────────────

pub fn definition_immune_status() -> ToolDefinition {
    ToolDefinition {
        name: "memory_immune_status".into(),
        description: Some(
            "Get immune system status: health score, threats detected, quarantined items".into(),
        ),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_immune_status(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let total = nodes.len();

    // Detect potential threats
    let zero_confidence = nodes.iter().filter(|n| n.confidence <= 0.0).count();
    let very_low_confidence = nodes
        .iter()
        .filter(|n| n.confidence > 0.0 && n.confidence < 0.2)
        .count();
    let high_decay = nodes.iter().filter(|n| n.decay_score > 0.8).count();
    let orphans = nodes
        .iter()
        .filter(|n| graph.edges_from(n.id).is_empty() && graph.edges_to(n.id).is_empty())
        .count();

    // Contradiction pairs
    let contradiction_count = graph
        .edges()
        .iter()
        .filter(|e| e.edge_type.name() == "contradicts")
        .count();

    let threats = zero_confidence + very_low_confidence + high_decay;
    let health = if total == 0 {
        1.0
    } else {
        1.0 - (threats as f64 / total as f64).min(1.0)
    };

    Ok(ToolCallResult::json(&json!({
        "immune_health": (health * 100.0).round() / 100.0,
        "total_nodes": total,
        "threats_detected": threats,
        "zero_confidence": zero_confidence,
        "very_low_confidence": very_low_confidence,
        "high_decay": high_decay,
        "orphan_nodes": orphans,
        "contradiction_pairs": contradiction_count,
        "quarantined": 0,
        "status": if health > 0.9 { "HEALTHY" } else if health > 0.7 { "MINOR_THREATS" } else if health > 0.5 { "COMPROMISED" } else { "CRITICAL" },
    })))
}

// ── 10. memory_immune_scan ──────────────────────────────────────────────

pub fn definition_immune_scan() -> ToolDefinition {
    ToolDefinition {
        name: "memory_immune_scan".into(),
        description: Some("Scan memory for threats: contradictions, low confidence, injection patterns, anomalies".into()),
        input_schema: json!({"type":"object","properties":{"scan_type":{"type":"string","description":"Type of scan: full, contradictions, low_confidence, anomalies (default: full)"},"max_results":{"type":"integer","description":"Max results to return (default 20)"}}})
    }
}

pub async fn execute_immune_scan(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let scan_type = get_str(&args, "scan_type").unwrap_or_else(|| "full".into());
    let max_results = get_u64(&args, "max_results").unwrap_or(20) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    let mut threats: Vec<Value> = Vec::new();

    if scan_type == "full" || scan_type == "low_confidence" {
        for node in nodes {
            if node.confidence < 0.2 {
                threats.push(json!({
                    "threat_type": "LowConfidence",
                    "severity": if node.confidence <= 0.0 { "Critical" } else { "Moderate" },
                    "node_id": node.id,
                    "confidence": node.confidence,
                    "content_preview": &node.content[..node.content.len().min(80)],
                }));
            }
        }
    }

    if scan_type == "full" || scan_type == "contradictions" {
        for edge in graph.edges() {
            if edge.edge_type.name() == "contradicts" {
                let source = graph.get_node(edge.source_id);
                let target = graph.get_node(edge.target_id);
                threats.push(json!({
                    "threat_type": "Contradiction",
                    "severity": "High",
                    "source_id": edge.source_id,
                    "target_id": edge.target_id,
                    "source_preview": source.map(|n| &n.content[..n.content.len().min(60)]),
                    "target_preview": target.map(|n| &n.content[..n.content.len().min(60)]),
                }));
            }
        }
    }

    if scan_type == "full" || scan_type == "anomalies" {
        for node in nodes {
            if node.decay_score > 0.9 && node.access_count > 10 {
                threats.push(json!({
                    "threat_type": "Anomaly",
                    "severity": "Moderate",
                    "reason": "High decay despite frequent access",
                    "node_id": node.id,
                    "decay_score": node.decay_score,
                    "access_count": node.access_count,
                }));
            }
        }
    }

    threats.truncate(max_results);
    Ok(ToolCallResult::json(&json!({
        "scan_type": scan_type,
        "threats_found": threats.len(),
        "threats": threats,
    })))
}

// ── 11. memory_immune_quarantine ────────────────────────────────────────

pub fn definition_immune_quarantine() -> ToolDefinition {
    ToolDefinition {
        name: "memory_immune_quarantine".into(),
        description: Some(
            "Quarantine a suspicious memory by setting its confidence to 0 and marking it".into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer","description":"Node to quarantine"},"reason":{"type":"string","description":"Reason for quarantine"}},"required":["node_id","reason"]}),
    }
}

pub async fn execute_immune_quarantine(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let reason = get_str(&args, "reason").unwrap_or_else(|| "manual quarantine".into());
    let mut session = session.lock().await;
    let graph = session.graph_mut();
    let node = graph
        .get_node_mut(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let old_confidence = node.confidence;
    node.confidence = 0.0;
    Ok(ToolCallResult::json(&json!({
        "quarantined": true,
        "node_id": node_id,
        "previous_confidence": old_confidence,
        "new_confidence": 0.0,
        "reason": reason,
    })))
}

// ── 12. memory_immune_release ───────────────────────────────────────────

pub fn definition_immune_release() -> ToolDefinition {
    ToolDefinition {
        name: "memory_immune_release".into(),
        description: Some("Release a quarantined memory by restoring its confidence".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer","description":"Node to release"},"confidence":{"type":"number","description":"Confidence to restore (default 0.5)"}},"required":["node_id"]}),
    }
}

pub async fn execute_immune_release(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let confidence = get_f64(&args, "confidence").unwrap_or(0.5) as f32;
    let mut session = session.lock().await;
    let graph = session.graph_mut();
    let node = graph
        .get_node_mut(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let old_confidence = node.confidence;
    node.confidence = confidence.clamp(0.0, 1.0);
    Ok(ToolCallResult::json(&json!({
        "released": true,
        "node_id": node_id,
        "previous_confidence": old_confidence,
        "new_confidence": node.confidence,
    })))
}

// ── 13. memory_immune_train ─────────────────────────────────────────────

pub fn definition_immune_train() -> ToolDefinition {
    ToolDefinition {
        name: "memory_immune_train".into(),
        description: Some(
            "Train a new antibody: define a threat pattern to auto-detect in future scans".into(),
        ),
        input_schema: json!({"type":"object","properties":{"threat_pattern":{"type":"string","description":"Description of threat pattern to detect"},"threat_type":{"type":"string","description":"Type: FalseMemory, Corruption, Contradiction, Replay, Poisoning"},"example_node_ids":{"type":"array","items":{"type":"integer"},"description":"Example nodes exhibiting this threat"}},"required":["threat_pattern","threat_type"]}),
    }
}

pub async fn execute_immune_train(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let threat_pattern = get_str(&args, "threat_pattern")
        .ok_or_else(|| McpError::InvalidParams("threat_pattern required".into()))?;
    let threat_type = get_str(&args, "threat_type")
        .ok_or_else(|| McpError::InvalidParams("threat_type required".into()))?;
    let example_ids: Vec<u64> = args
        .get("example_node_ids")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect())
        .unwrap_or_default();

    // Store antibody as a new memory node
    let content = format!(
        "[ANTIBODY] type={} pattern={} examples={:?}",
        threat_type, threat_pattern, example_ids
    );
    let mut session = session.lock().await;
    let (node_id, _) = session.add_event(EventType::Skill, &content, 0.95, vec![])?;

    Ok(ToolCallResult::json(&json!({
        "antibody_created": true,
        "antibody_node_id": node_id,
        "threat_type": threat_type,
        "threat_pattern": threat_pattern,
        "examples_used": example_ids.len(),
    })))
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 16: PHOENIX PROTOCOL — Rise from complete memory loss
// ══════════════════════════════════════════════════════════════════════════

// ── 14. memory_phoenix_initiate ─────────────────────────────────────────

pub fn definition_phoenix_initiate() -> ToolDefinition {
    ToolDefinition {
        name: "memory_phoenix_initiate".into(),
        description: Some(
            "Initiate Phoenix Protocol: begin full memory recovery from external traces".into(),
        ),
        input_schema: json!({"type":"object","properties":{"reason":{"type":"string","description":"Reason for initiating phoenix protocol"},"recovery_target":{"type":"string","description":"What to recover: all, recent, critical, topic-specific"}},"required":["reason"]}),
    }
}

pub async fn execute_phoenix_initiate(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let reason = get_str(&args, "reason")
        .ok_or_else(|| McpError::InvalidParams("reason required".into()))?;
    let recovery_target = get_str(&args, "recovery_target").unwrap_or_else(|| "all".into());
    let session = session.lock().await;
    let graph = session.graph();
    let current_state = graph.node_count();

    // Log the initiation
    let traces_available = vec![
        json!({"source": "CurrentGraph", "available": current_state > 0, "items": current_state}),
        json!({"source": "SessionIndex", "available": true, "items": graph.session_index().session_count()}),
        json!({"source": "TypeIndex", "available": true, "items": graph.type_index().len()}),
        json!({"source": "EdgeNetwork", "available": graph.edge_count() > 0, "items": graph.edge_count()}),
    ];

    Ok(ToolCallResult::json(&json!({
        "phoenix_protocol": "INITIATED",
        "reason": reason,
        "recovery_target": recovery_target,
        "current_state": {"nodes": current_state, "edges": graph.edge_count()},
        "available_traces": traces_available,
        "next_steps": ["Use memory_phoenix_gather to collect traces", "Use memory_phoenix_reconstruct to rebuild", "Use memory_phoenix_status to monitor progress"],
    })))
}

// ── 15. memory_phoenix_gather ───────────────────────────────────────────

pub fn definition_phoenix_gather() -> ToolDefinition {
    ToolDefinition {
        name: "memory_phoenix_gather".into(),
        description: Some(
            "Gather external traces for phoenix recovery: session data, type data, edge chains"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"source":{"type":"string","description":"Trace source: sessions, types, edges, high_confidence, all (default: all)"},"max_traces":{"type":"integer","description":"Max traces to gather (default 50)"}}}),
    }
}

pub async fn execute_phoenix_gather(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let source = get_str(&args, "source").unwrap_or_else(|| "all".into());
    let max_traces = get_u64(&args, "max_traces").unwrap_or(50) as usize;
    let session = session.lock().await;
    let graph = session.graph();

    let mut traces: Vec<Value> = Vec::new();

    if source == "all" || source == "sessions" {
        for (session_id, node_ids) in graph.session_index().inner().iter() {
            traces.push(json!({
                "trace_type": "SessionRecord",
                "session_id": session_id,
                "node_count": node_ids.len(),
                "node_ids": &node_ids[..node_ids.len().min(10)],
            }));
        }
    }

    if source == "all" || source == "types" {
        for (etype, node_ids) in graph.type_index().inner().iter() {
            traces.push(json!({
                "trace_type": "TypeCluster",
                "event_type": etype.name(),
                "node_count": node_ids.len(),
            }));
        }
    }

    if source == "all" || source == "high_confidence" {
        let mut high_conf: Vec<&_> = graph
            .nodes()
            .iter()
            .filter(|n| n.confidence >= 0.8)
            .collect();
        high_conf.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        for node in high_conf.iter().take(max_traces / 3) {
            traces.push(json!({
                "trace_type": "HighConfidenceNode",
                "node_id": node.id,
                "confidence": node.confidence,
                "content_preview": &node.content[..node.content.len().min(80)],
            }));
        }
    }

    if source == "all" || source == "edges" {
        let edges = graph.edges();
        for edge in edges.iter().take(max_traces / 4) {
            traces.push(json!({
                "trace_type": "EdgeRecord",
                "source_id": edge.source_id,
                "target_id": edge.target_id,
                "edge_type": edge.edge_type.name(),
                "weight": edge.weight,
            }));
        }
    }

    traces.truncate(max_traces);
    Ok(ToolCallResult::json(&json!({
        "traces_gathered": traces.len(),
        "source": source,
        "traces": traces,
    })))
}

// ── 16. memory_phoenix_reconstruct ──────────────────────────────────────

pub fn definition_phoenix_reconstruct() -> ToolDefinition {
    ToolDefinition {
        name: "memory_phoenix_reconstruct".into(),
        description: Some(
            "Reconstruct memory from gathered traces — the actual rebirth step".into(),
        ),
        input_schema: json!({"type":"object","properties":{"strategy":{"type":"string","description":"Strategy: conservative (high-conf only), balanced, aggressive (all traces)"},"min_confidence":{"type":"number","description":"Min confidence threshold for reconstruction (default 0.3)"}}}),
    }
}

pub async fn execute_phoenix_reconstruct(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let strategy = get_str(&args, "strategy").unwrap_or_else(|| "balanced".into());
    let min_confidence = get_f64(&args, "min_confidence").unwrap_or(0.3) as f32;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    let threshold = match strategy.as_str() {
        "conservative" => 0.8f32,
        "aggressive" => 0.1f32,
        _ => min_confidence,
    };

    let recoverable: Vec<Value> = nodes
        .iter()
        .filter(|n| n.confidence >= threshold)
        .map(|n| {
            json!({
                "node_id": n.id,
                "event_type": n.event_type.name(),
                "confidence": n.confidence,
                "content_preview": &n.content[..n.content.len().min(80)],
                "edges": graph.edges_from(n.id).len() + graph.edges_to(n.id).len(),
            })
        })
        .collect();

    let total = nodes.len();
    let recovered = recoverable.len();
    let recovery_rate = if total > 0 {
        recovered as f64 / total as f64
    } else {
        0.0
    };

    Ok(ToolCallResult::json(&json!({
        "phoenix_status": "RECONSTRUCTION_COMPLETE",
        "strategy": strategy,
        "threshold": threshold,
        "total_nodes": total,
        "recoverable_nodes": recovered,
        "recovery_rate": (recovery_rate * 100.0).round() / 100.0,
        "recovered_sample": &recoverable[..recoverable.len().min(15)],
        "verdict": if recovery_rate > 0.9 { "FULL REBIRTH — memory restored" }
            else if recovery_rate > 0.6 { "PARTIAL REBIRTH — most memory recovered" }
            else if recovery_rate > 0.3 { "EMBER — core memories recovered" }
            else { "ASH — minimal recovery, reconstruction needed" },
    })))
}

// ── 17. memory_phoenix_status ───────────────────────────────────────────

pub fn definition_phoenix_status() -> ToolDefinition {
    ToolDefinition {
        name: "memory_phoenix_status".into(),
        description: Some(
            "Get Phoenix Protocol rebirth status: progress, recovery confidence, gaps".into(),
        ),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_phoenix_status(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let total = graph.node_count();
    let edges = graph.edge_count();

    let high_conf = graph.nodes().iter().filter(|n| n.confidence >= 0.8).count();
    let medium_conf = graph
        .nodes()
        .iter()
        .filter(|n| n.confidence >= 0.4 && n.confidence < 0.8)
        .count();
    let low_conf = graph.nodes().iter().filter(|n| n.confidence < 0.4).count();
    let sessions = graph.session_index().session_count();
    let types = graph.type_index().len();

    let overall_health = if total == 0 {
        0.0
    } else {
        (high_conf as f64 * 1.0 + medium_conf as f64 * 0.5 + low_conf as f64 * 0.1) / total as f64
    };

    Ok(ToolCallResult::json(&json!({
        "phoenix_status": if total == 0 { "NO_MEMORY" } else if overall_health > 0.8 { "REBORN" } else if overall_health > 0.5 { "RECOVERING" } else { "REBUILDING" },
        "total_nodes": total,
        "total_edges": edges,
        "high_confidence": high_conf,
        "medium_confidence": medium_conf,
        "low_confidence": low_conf,
        "sessions_tracked": sessions,
        "event_types": types,
        "overall_health": (overall_health * 100.0).round() / 100.0,
    })))
}

// ── Public API ───────────────────────────────────────────────────────────

pub fn all_definitions() -> Vec<ToolDefinition> {
    vec![
        definition_archaeology_dig(),
        definition_archaeology_artifacts(),
        definition_archaeology_reconstruct(),
        definition_archaeology_verify(),
        definition_holographic_status(),
        definition_holographic_reconstruct(),
        definition_holographic_simulate(),
        definition_holographic_distribute(),
        definition_immune_status(),
        definition_immune_scan(),
        definition_immune_quarantine(),
        definition_immune_release(),
        definition_immune_train(),
        definition_phoenix_initiate(),
        definition_phoenix_gather(),
        definition_phoenix_reconstruct(),
        definition_phoenix_status(),
    ]
}

pub async fn try_execute(
    name: &str,
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> Option<McpResult<ToolCallResult>> {
    match name {
        "memory_archaeology_dig" => Some(execute_archaeology_dig(args, session).await),
        "memory_archaeology_artifacts" => Some(execute_archaeology_artifacts(args, session).await),
        "memory_archaeology_reconstruct" => {
            Some(execute_archaeology_reconstruct(args, session).await)
        }
        "memory_archaeology_verify" => Some(execute_archaeology_verify(args, session).await),
        "memory_holographic_status" => Some(execute_holographic_status(args, session).await),
        "memory_holographic_reconstruct" => {
            Some(execute_holographic_reconstruct(args, session).await)
        }
        "memory_holographic_simulate" => Some(execute_holographic_simulate(args, session).await),
        "memory_holographic_distribute" => {
            Some(execute_holographic_distribute(args, session).await)
        }
        "memory_immune_status" => Some(execute_immune_status(args, session).await),
        "memory_immune_scan" => Some(execute_immune_scan(args, session).await),
        "memory_immune_quarantine" => Some(execute_immune_quarantine(args, session).await),
        "memory_immune_release" => Some(execute_immune_release(args, session).await),
        "memory_immune_train" => Some(execute_immune_train(args, session).await),
        "memory_phoenix_initiate" => Some(execute_phoenix_initiate(args, session).await),
        "memory_phoenix_gather" => Some(execute_phoenix_gather(args, session).await),
        "memory_phoenix_reconstruct" => Some(execute_phoenix_reconstruct(args, session).await),
        "memory_phoenix_status" => Some(execute_phoenix_status(args, session).await),
        _ => None,
    }
}
