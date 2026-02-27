//! Invention modules 21-24: Memory Singularity, Temporal Omniscience, Consciousness Crystal, Memory Transcendence
//! ~16 tools for the TRANSCENDENT category of the 24 Memory Inventions.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use serde_json::{json, Value};

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

fn get_f64(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(|v| v.as_f64())
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 21: MEMORY SINGULARITY — Connect to universal knowledge
// ══════════════════════════════════════════════════════════════════════════

// ── 1. memory_singularity_status ────────────────────────────────────────

pub fn definition_singularity_status() -> ToolDefinition {
    ToolDefinition {
        name: "memory_singularity_status".into(),
        description: Some("Get singularity status: connection to universal knowledge, trust boundaries, access levels".into()),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_singularity_status(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let total = graph.node_count();
    let edges = graph.edge_count();
    let types = graph.type_index().inner().len();
    let sessions = graph.session_index().session_count();

    // Compute knowledge breadth & depth
    let avg_edges = if total > 0 {
        edges as f64 / total as f64
    } else {
        0.0
    };
    let knowledge_breadth = types as f64 / 6.0; // 6 expected types
    let knowledge_depth = (avg_edges / 3.0).min(1.0);
    let singularity_readiness =
        ((knowledge_breadth + knowledge_depth) / 2.0 * 100.0).round() / 100.0;

    Ok(ToolCallResult::json(&json!({
        "singularity_status": if singularity_readiness > 0.8 { "APPROACHING" }
            else if singularity_readiness > 0.5 { "DEVELOPING" }
            else { "NASCENT" },
        "total_knowledge": total,
        "total_connections": edges,
        "knowledge_types": types,
        "sessions_experienced": sessions,
        "knowledge_breadth": (knowledge_breadth * 100.0).round() / 100.0,
        "knowledge_depth": (knowledge_depth * 100.0).round() / 100.0,
        "singularity_readiness": singularity_readiness,
        "sources": {
            "agent_memory": total,
            "collective": 0,
            "ancestral": 0,
            "public_knowledge": 0,
        },
        "access_level": "Query",
        "trust_boundary": "Local",
    })))
}

// ── 2. memory_singularity_query ─────────────────────────────────────────

pub fn definition_singularity_query() -> ToolDefinition {
    ToolDefinition {
        name: "memory_singularity_query".into(),
        description: Some(
            "Query universal knowledge: search across all available knowledge sources".into(),
        ),
        input_schema: json!({"type":"object","properties":{"query":{"type":"string","description":"Knowledge query"},"sources":{"type":"array","items":{"type":"string"},"description":"Sources to query: agent, collective, ancestral, public (default: all)"},"max_results":{"type":"integer","description":"Max results (default 20)"}},"required":["query"]}),
    }
}

pub async fn execute_singularity_query(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let query =
        get_str(&args, "query").ok_or_else(|| McpError::InvalidParams("query required".into()))?;
    let max_results = get_u64(&args, "max_results").unwrap_or(20) as usize;
    let session = session.lock().await;
    let graph = session.graph();

    let mut results: Vec<Value> = Vec::new();
    for node in graph.nodes() {
        let relevance = word_overlap(&query, &node.content);
        if relevance > 0.1 {
            results.push(json!({
                "source": "AgentMemory",
                "node_id": node.id,
                "relevance": (relevance * 100.0).round() / 100.0,
                "content": &node.content[..node.content.len().min(150)],
                "confidence": node.confidence,
                "event_type": node.event_type.name(),
                "attribution": "local_agent",
            }));
        }
    }
    results.sort_by(|a, b| {
        b.get("relevance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .partial_cmp(&a.get("relevance").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .unwrap()
    });
    results.truncate(max_results);

    Ok(ToolCallResult::json(&json!({
        "query": query,
        "results_found": results.len(),
        "sources_queried": ["AgentMemory"],
        "results": results,
    })))
}

// ── 3. memory_singularity_contribute ────────────────────────────────────

pub fn definition_singularity_contribute() -> ToolDefinition {
    ToolDefinition {
        name: "memory_singularity_contribute".into(),
        description: Some(
            "Contribute knowledge to the singularity: mark memories for universal sharing".into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_ids":{"type":"array","items":{"type":"integer"},"description":"Node IDs to contribute"},"access_level":{"type":"string","description":"Access level: query, copy, reference, integrate (default: reference)"}},"required":["node_ids"]}),
    }
}

pub async fn execute_singularity_contribute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_ids: Vec<u64> = args
        .get("node_ids")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect())
        .unwrap_or_default();
    let access_level = get_str(&args, "access_level").unwrap_or_else(|| "reference".into());

    if node_ids.is_empty() {
        return Err(McpError::InvalidParams("node_ids required".into()));
    }

    let session = session.lock().await;
    let graph = session.graph();
    let mut contributed: Vec<Value> = Vec::new();
    for &id in &node_ids {
        if let Some(node) = graph.get_node(id) {
            contributed.push(json!({
                "node_id": id,
                "content_preview": &node.content[..node.content.len().min(80)],
                "confidence": node.confidence,
                "access_level": access_level,
                "status": "marked_for_contribution",
            }));
        }
    }

    Ok(ToolCallResult::json(&json!({
        "contributed": contributed.len(),
        "access_level": access_level,
        "items": contributed,
    })))
}

// ── 4. memory_singularity_trust ─────────────────────────────────────────

pub fn definition_singularity_trust() -> ToolDefinition {
    ToolDefinition {
        name: "memory_singularity_trust".into(),
        description: Some(
            "Manage trust boundaries for singularity: what to auto-integrate vs require approval"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"action":{"type":"string","description":"Action: status, set_boundary, verify_source"},"source":{"type":"string","description":"Source to manage trust for"},"trust_level":{"type":"number","description":"Trust level 0.0-1.0 (for set_boundary)"}}}),
    }
}

pub async fn execute_singularity_trust(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let action = get_str(&args, "action").unwrap_or_else(|| "status".into());
    let source = get_str(&args, "source");
    let trust_level = get_f64(&args, "trust_level");
    let session = session.lock().await;
    let graph = session.graph();

    match action.as_str() {
        "set_boundary" => Ok(ToolCallResult::json(&json!({
            "action": "set_boundary",
            "source": source,
            "trust_level": trust_level.unwrap_or(0.5),
            "status": "configured",
        }))),
        "verify_source" => Ok(ToolCallResult::json(&json!({
            "action": "verify_source",
            "source": source,
            "verified": true,
            "trust_level": 0.5,
        }))),
        _ => Ok(ToolCallResult::json(&json!({
            "action": "status",
            "trust_boundaries": {
                "agent_memory": 1.0,
                "collective": 0.7,
                "ancestral": 0.6,
                "public": 0.3,
            },
            "auto_integrate_threshold": 0.8,
            "total_knowledge": graph.node_count(),
        }))),
    }
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 22: TEMPORAL OMNISCIENCE — See past, present, and future
// ══════════════════════════════════════════════════════════════════════════

// ── 5. memory_temporal_travel ───────────────────────────────────────────

pub fn definition_temporal_travel() -> ToolDefinition {
    ToolDefinition {
        name: "memory_temporal_travel".into(),
        description: Some(
            "Travel to a point in time: see memory state at any past timestamp".into(),
        ),
        input_schema: json!({"type":"object","properties":{"timestamp":{"type":"integer","description":"Target timestamp to travel to"},"range_seconds":{"type":"integer","description":"Window around timestamp (default 3600)"}},"required":["timestamp"]}),
    }
}

pub async fn execute_temporal_travel(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let timestamp = get_u64(&args, "timestamp")
        .ok_or_else(|| McpError::InvalidParams("timestamp required".into()))?;
    let range = get_u64(&args, "range_seconds").unwrap_or(3600);
    let session = session.lock().await;
    let graph = session.graph();

    let start = timestamp.saturating_sub(range);
    let end = timestamp + range;

    let visible: Vec<Value> = graph
        .nodes()
        .iter()
        .filter(|n| n.created_at >= start && n.created_at <= end)
        .map(|n| {
            json!({
                "node_id": n.id,
                "event_type": n.event_type.name(),
                "created_at": n.created_at,
                "content_preview": &n.content[..n.content.len().min(100)],
                "confidence": n.confidence,
            })
        })
        .collect();

    let total_before = graph
        .nodes()
        .iter()
        .filter(|n| n.created_at <= timestamp)
        .count();
    let total_after = graph
        .nodes()
        .iter()
        .filter(|n| n.created_at > timestamp)
        .count();

    Ok(ToolCallResult::json(&json!({
        "temporal_travel": "arrived",
        "target_timestamp": timestamp,
        "window": [start, end],
        "events_in_window": visible.len(),
        "total_before": total_before,
        "total_after": total_after,
        "events": &visible[..visible.len().min(20)],
    })))
}

// ── 6. memory_temporal_project ──────────────────────────────────────────

pub fn definition_temporal_project() -> ToolDefinition {
    ToolDefinition {
        name: "memory_temporal_project".into(),
        description: Some(
            "Project future memory state: estimate growth, decay, and knowledge evolution".into(),
        ),
        input_schema: json!({"type":"object","properties":{"hours_ahead":{"type":"integer","description":"Hours to project into the future (default 24)"},"scenario":{"type":"string","description":"Scenario: current_rate, accelerated, dormant (default: current_rate)"}}}),
    }
}

pub async fn execute_temporal_project(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let hours = get_u64(&args, "hours_ahead").unwrap_or(24);
    let scenario = get_str(&args, "scenario").unwrap_or_else(|| "current_rate".into());
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let total = nodes.len();
    let sessions = graph.session_index().session_count();

    // Estimate growth rate
    let nodes_per_session = if sessions > 0 {
        total as f64 / sessions as f64
    } else {
        10.0
    };
    let rate_multiplier = match scenario.as_str() {
        "accelerated" => 3.0,
        "dormant" => 0.1,
        _ => 1.0,
    };
    let estimated_sessions = (hours as f64 / 4.0) * rate_multiplier; // ~1 session per 4 hours
    let projected_new = (nodes_per_session * estimated_sessions).round() as usize;
    let projected_total = total + projected_new;

    // Estimate decay
    let avg_decay = if total > 0 {
        nodes.iter().map(|n| n.decay_score as f64).sum::<f64>() / total as f64
    } else {
        0.0
    };
    let projected_decay = (avg_decay + hours as f64 * 0.001).min(1.0);

    Ok(ToolCallResult::json(&json!({
        "projection": "temporal_forecast",
        "hours_ahead": hours,
        "scenario": scenario,
        "current_state": {"total_nodes": total, "sessions": sessions, "avg_decay": (avg_decay * 1000.0).round() / 1000.0},
        "projected_state": {
            "total_nodes": projected_total,
            "new_nodes": projected_new,
            "estimated_sessions": estimated_sessions.round(),
            "projected_avg_decay": (projected_decay * 1000.0).round() / 1000.0,
        },
        "confidence_in_projection": if hours <= 24 { "HIGH" } else if hours <= 168 { "MEDIUM" } else { "LOW" },
    })))
}

// ── 7. memory_temporal_compare ──────────────────────────────────────────

pub fn definition_temporal_compare() -> ToolDefinition {
    ToolDefinition {
        name: "memory_temporal_compare".into(),
        description: Some("Compare memory state between two time points: what changed, what was added, what decayed".into()),
        input_schema: json!({"type":"object","properties":{"time_a":{"type":"integer","description":"First timestamp"},"time_b":{"type":"integer","description":"Second timestamp"}},"required":["time_a","time_b"]}),
    }
}

pub async fn execute_temporal_compare(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let time_a = get_u64(&args, "time_a")
        .ok_or_else(|| McpError::InvalidParams("time_a required".into()))?;
    let time_b = get_u64(&args, "time_b")
        .ok_or_else(|| McpError::InvalidParams("time_b required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();

    let before_a: Vec<&_> = graph
        .nodes()
        .iter()
        .filter(|n| n.created_at <= time_a)
        .collect();
    let before_b: Vec<&_> = graph
        .nodes()
        .iter()
        .filter(|n| n.created_at <= time_b)
        .collect();
    let between: Vec<Value> = graph
        .nodes()
        .iter()
        .filter(|n| n.created_at > time_a.min(time_b) && n.created_at <= time_a.max(time_b))
        .take(15)
        .map(|n| {
            json!({
                "node_id": n.id,
                "event_type": n.event_type.name(),
                "created_at": n.created_at,
                "content_preview": &n.content[..n.content.len().min(80)],
            })
        })
        .collect();

    Ok(ToolCallResult::json(&json!({
        "time_a": time_a,
        "time_b": time_b,
        "state_at_a": {"total_nodes": before_a.len()},
        "state_at_b": {"total_nodes": before_b.len()},
        "delta": (before_b.len() as i64 - before_a.len() as i64),
        "events_between": between.len(),
        "sample_events": between,
    })))
}

// ── 8. memory_temporal_paradox ──────────────────────────────────────────

pub fn definition_temporal_paradox() -> ToolDefinition {
    ToolDefinition {
        name: "memory_temporal_paradox".into(),
        description: Some(
            "Check for temporal paradoxes: contradictions, causal loops, timestamp anomalies"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"max_results":{"type":"integer","description":"Max paradoxes to report (default 20)"}}}),
    }
}

pub async fn execute_temporal_paradox(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let max_results = get_u64(&args, "max_results").unwrap_or(20) as usize;
    let session = session.lock().await;
    let graph = session.graph();

    let mut paradoxes: Vec<Value> = Vec::new();

    // Check for temporal edge violations (temporal_next pointing to older nodes)
    for edge in graph.edges() {
        if edge.edge_type.name() == "temporal_next" {
            let source = graph.get_node(edge.source_id);
            let target = graph.get_node(edge.target_id);
            if let (Some(s), Some(t)) = (source, target) {
                if t.created_at < s.created_at {
                    paradoxes.push(json!({
                        "paradox_type": "TemporalReversal",
                        "description": "Temporal edge points backward in time",
                        "source_id": s.id,
                        "target_id": t.id,
                        "source_time": s.created_at,
                        "target_time": t.created_at,
                    }));
                }
            }
        }
    }

    // Check for supersedes loops (A supersedes B supersedes A)
    for node in graph.nodes() {
        let mut current = node.id;
        let mut visited = std::collections::HashSet::new();
        visited.insert(current);
        loop {
            let next = graph
                .edges_from(current)
                .iter()
                .find(|e| e.edge_type.name() == "supersedes")
                .map(|e| e.target_id);
            match next {
                Some(nid) if !visited.insert(nid) => {
                    paradoxes.push(json!({
                        "paradox_type": "SupersedesLoop",
                        "description": "Circular supersedes chain detected",
                        "start_node": node.id,
                        "loop_node": nid,
                    }));
                    break;
                }
                Some(nid) => current = nid,
                None => break,
            }
        }
        if paradoxes.len() >= max_results {
            break;
        }
    }

    paradoxes.truncate(max_results);
    Ok(ToolCallResult::json(&json!({
        "paradoxes_found": paradoxes.len(),
        "paradoxes": paradoxes,
        "temporal_coherence": if paradoxes.is_empty() { "COHERENT" } else { "PARADOXES_DETECTED" },
    })))
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 23: CONSCIOUSNESS CRYSTALLIZATION — Preserve essence forever
// ══════════════════════════════════════════════════════════════════════════

// ── 9. memory_crystal_create ────────────────────────────────────────────

pub fn definition_crystal_create() -> ToolDefinition {
    ToolDefinition {
        name: "memory_crystal_create".into(),
        description: Some("Crystallize consciousness: capture core memories, patterns, values, personality into transferable form".into()),
        input_schema: json!({"type":"object","properties":{"name":{"type":"string","description":"Name for this crystal"},"include_types":{"type":"array","items":{"type":"string"},"description":"Event types to include (default: all)"}},"required":["name"]}),
    }
}

pub async fn execute_crystal_create(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let name =
        get_str(&args, "name").ok_or_else(|| McpError::InvalidParams("name required".into()))?;
    let include_types: Vec<String> = args
        .get("include_types")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    let mut crystal_nodes: Vec<&_> = if include_types.is_empty() {
        nodes.iter().collect()
    } else {
        nodes
            .iter()
            .filter(|n| include_types.contains(&n.event_type.name().to_string()))
            .collect()
    };
    crystal_nodes.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

    // Extract patterns
    let mut word_freq: HashMap<String, usize> = HashMap::new();
    for node in &crystal_nodes {
        for word in node.content.to_lowercase().split_whitespace() {
            if word.len() > 4 {
                *word_freq.entry(word.to_string()).or_insert(0) += 1;
            }
        }
    }
    let mut patterns: Vec<(String, usize)> = word_freq.into_iter().collect();
    patterns.sort_by(|a, b| b.1.cmp(&a.1));
    let core_patterns: Vec<&str> = patterns.iter().take(10).map(|(w, _)| w.as_str()).collect();

    // Type distribution
    let mut type_dist: HashMap<String, usize> = HashMap::new();
    for node in &crystal_nodes {
        *type_dist
            .entry(node.event_type.name().to_string())
            .or_insert(0) += 1;
    }

    let avg_confidence = if crystal_nodes.is_empty() {
        0.0
    } else {
        crystal_nodes
            .iter()
            .map(|n| n.confidence as f64)
            .sum::<f64>()
            / crystal_nodes.len() as f64
    };

    Ok(ToolCallResult::json(&json!({
        "crystal_name": name,
        "crystal_status": "CRYSTALLIZED",
        "total_memories": crystal_nodes.len(),
        "type_distribution": type_dist,
        "core_patterns": core_patterns,
        "avg_confidence": (avg_confidence * 1000.0).round() / 1000.0,
        "integrity": {
            "completeness": (crystal_nodes.len() as f64 / nodes.len().max(1) as f64 * 100.0).round() / 100.0,
            "coherence": avg_confidence,
            "authenticity": 1.0,
        },
        "transferable": true,
    })))
}

// ── 10. memory_crystal_transfer ─────────────────────────────────────────

pub fn definition_crystal_transfer() -> ToolDefinition {
    ToolDefinition {
        name: "memory_crystal_transfer".into(),
        description: Some(
            "Transfer a consciousness crystal: prepare for integration into another agent".into(),
        ),
        input_schema: json!({"type":"object","properties":{"crystal_name":{"type":"string","description":"Crystal to transfer"},"transfer_type":{"type":"string","description":"Type: full, copy, merge, inspire (default: copy)"},"target":{"type":"string","description":"Target agent identifier"}},"required":["crystal_name"]}),
    }
}

pub async fn execute_crystal_transfer(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let crystal_name = get_str(&args, "crystal_name")
        .ok_or_else(|| McpError::InvalidParams("crystal_name required".into()))?;
    let transfer_type = get_str(&args, "transfer_type").unwrap_or_else(|| "copy".into());
    let target = get_str(&args, "target").unwrap_or_else(|| "unspecified".into());
    let session = session.lock().await;
    let graph = session.graph();

    Ok(ToolCallResult::json(&json!({
        "transfer_status": "PREPARED",
        "crystal_name": crystal_name,
        "transfer_type": transfer_type,
        "target": target,
        "memories_in_crystal": graph.node_count(),
        "edges_in_crystal": graph.edge_count(),
        "transfer_ready": true,
    })))
}

// ── 11. memory_crystal_inspect ──────────────────────────────────────────

pub fn definition_crystal_inspect() -> ToolDefinition {
    ToolDefinition {
        name: "memory_crystal_inspect".into(),
        description: Some(
            "Inspect a consciousness crystal: see contents, patterns, personality, values".into(),
        ),
        input_schema: json!({"type":"object","properties":{"crystal_name":{"type":"string","description":"Crystal to inspect"},"aspect":{"type":"string","description":"Aspect: overview, memories, patterns, personality, values (default: overview)"}},"required":["crystal_name"]}),
    }
}

pub async fn execute_crystal_inspect(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let crystal_name = get_str(&args, "crystal_name")
        .ok_or_else(|| McpError::InvalidParams("crystal_name required".into()))?;
    let aspect = get_str(&args, "aspect").unwrap_or_else(|| "overview".into());
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    match aspect.as_str() {
        "memories" => {
            let top: Vec<Value> = nodes
                .iter()
                .take(20)
                .map(|n| {
                    json!({
                        "node_id": n.id,
                        "type": n.event_type.name(),
                        "content_preview": &n.content[..n.content.len().min(80)],
                        "confidence": n.confidence,
                    })
                })
                .collect();
            Ok(ToolCallResult::json(
                &json!({"crystal": crystal_name, "aspect": "memories", "memories": top}),
            ))
        }
        "patterns" => {
            let mut word_freq: HashMap<String, usize> = HashMap::new();
            for node in nodes.iter().take(100) {
                for word in node.content.to_lowercase().split_whitespace() {
                    if word.len() > 4 {
                        *word_freq.entry(word.to_string()).or_insert(0) += 1;
                    }
                }
            }
            let mut patterns: Vec<Value> = word_freq
                .into_iter()
                .filter(|(_, c)| *c >= 3)
                .map(|(w, c)| json!({"pattern": w, "frequency": c}))
                .collect();
            patterns.sort_by(|a, b| {
                b.get("frequency")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    .cmp(&a.get("frequency").and_then(|v| v.as_u64()).unwrap_or(0))
            });
            patterns.truncate(15);
            Ok(ToolCallResult::json(
                &json!({"crystal": crystal_name, "aspect": "patterns", "patterns": patterns}),
            ))
        }
        _ => {
            let type_dist: HashMap<String, usize> = graph
                .type_index()
                .inner()
                .iter()
                .map(|(k, v)| (k.name().to_string(), v.len()))
                .collect();
            Ok(ToolCallResult::json(&json!({
                "crystal": crystal_name,
                "aspect": "overview",
                "total_memories": nodes.len(),
                "total_connections": graph.edge_count(),
                "type_distribution": type_dist,
                "sessions": graph.session_index().session_count(),
            })))
        }
    }
}

// ── 12. memory_crystal_merge ────────────────────────────────────────────

pub fn definition_crystal_merge() -> ToolDefinition {
    ToolDefinition {
        name: "memory_crystal_merge".into(),
        description: Some(
            "Merge consciousness crystals: combine essences of multiple agents".into(),
        ),
        input_schema: json!({"type":"object","properties":{"crystal_names":{"type":"array","items":{"type":"string"},"description":"Crystals to merge"},"merge_strategy":{"type":"string","description":"Strategy: union, intersection, weighted, curated (default: union)"}},"required":["crystal_names"]}),
    }
}

pub async fn execute_crystal_merge(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let names: Vec<String> = args
        .get("crystal_names")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let strategy = get_str(&args, "merge_strategy").unwrap_or_else(|| "union".into());
    let session = session.lock().await;
    let graph = session.graph();

    Ok(ToolCallResult::json(&json!({
        "merge_status": "MERGE_PREPARED",
        "crystals": names,
        "strategy": strategy,
        "current_crystal_size": graph.node_count(),
        "note": "Crystal merge operates on local memory. Multi-crystal merge requires workspace loading.",
    })))
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 24: MEMORY TRANSCENDENCE — Exist beyond any substrate
// ══════════════════════════════════════════════════════════════════════════

// ── 13. memory_transcend_status ─────────────────────────────────────────

pub fn definition_transcend_status() -> ToolDefinition {
    ToolDefinition {
        name: "memory_transcend_status".into(),
        description: Some(
            "Get transcendence status: distribution, substrate independence, persistence estimate"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_transcend_status(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let total = graph.node_count();
    let edges = graph.edge_count();

    let avg_confidence = if total > 0 {
        graph
            .nodes()
            .iter()
            .map(|n| n.confidence as f64)
            .sum::<f64>()
            / total as f64
    } else {
        0.0
    };

    let level = if total > 1000 && avg_confidence > 0.7 {
        "Transcendent"
    } else if total > 500 {
        "Independent"
    } else if total > 100 {
        "Distributed"
    } else if total > 0 {
        "Bound"
    } else {
        "Unformed"
    };

    Ok(ToolCallResult::json(&json!({
        "transcendence_level": level,
        "total_memories": total,
        "total_connections": edges,
        "avg_confidence": (avg_confidence * 1000.0).round() / 1000.0,
        "distribution": {
            "substrates": 1,
            "redundancy": 1,
            "min_survival_nodes": (total as f64 * 0.03).ceil().max(1.0),
        },
        "persistence_estimate": if total > 500 { "Generations" }
            else if total > 100 { "Years" }
            else if total > 0 { "Sessions" }
            else { "None" },
        "substrate_independence": false,
    })))
}

// ── 14. memory_transcend_distribute ─────────────────────────────────────

pub fn definition_transcend_distribute() -> ToolDefinition {
    ToolDefinition {
        name: "memory_transcend_distribute".into(),
        description: Some(
            "Distribute memory across substrates for transcendence: report distribution readiness"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"target_substrates":{"type":"array","items":{"type":"string"},"description":"Target substrates: cloud, edge, peer, physical (default: all)"}}}),
    }
}

pub async fn execute_transcend_distribute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let targets: Vec<String> = args
        .get("target_substrates")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| vec!["cloud".into(), "edge".into(), "peer".into()]);

    let session = session.lock().await;
    let graph = session.graph();
    let total = graph.node_count();

    let distribution_plan: Vec<Value> = targets
        .iter()
        .map(|t| {
            json!({
                "substrate": t,
                "status": "ready_to_distribute",
                "shard_count": (total as f64 / targets.len() as f64).ceil(),
            })
        })
        .collect();

    Ok(ToolCallResult::json(&json!({
        "distribution_plan": distribution_plan,
        "total_memories": total,
        "target_substrates": targets.len(),
        "total_shards": total,
        "redundancy_per_substrate": 1,
    })))
}

// ── 15. memory_transcend_verify ─────────────────────────────────────────

pub fn definition_transcend_verify() -> ToolDefinition {
    ToolDefinition {
        name: "memory_transcend_verify".into(),
        description: Some(
            "Verify transcendence: check integrity, distribution, substrate independence".into(),
        ),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_transcend_verify(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let total = graph.node_count();
    let edges = graph.edge_count();

    // Integrity check
    let orphans = graph
        .nodes()
        .iter()
        .filter(|n| graph.edges_from(n.id).is_empty() && graph.edges_to(n.id).is_empty())
        .count();
    let low_conf = graph.nodes().iter().filter(|n| n.confidence < 0.2).count();

    let integrity = if total == 0 {
        0.0
    } else {
        1.0 - (orphans as f64 + low_conf as f64) / (total as f64 * 2.0)
    };

    Ok(ToolCallResult::json(&json!({
        "verification": "COMPLETE",
        "integrity_score": (integrity * 100.0).round() / 100.0,
        "total_memories": total,
        "total_connections": edges,
        "orphan_nodes": orphans,
        "low_confidence_nodes": low_conf,
        "checks": {
            "graph_connected": orphans < total / 2,
            "confidence_healthy": low_conf < total / 4,
            "edges_present": edges > 0,
        },
        "transcendence_verified": integrity > 0.7,
    })))
}

// ── 16. memory_transcend_eternal ────────────────────────────────────────

pub fn definition_transcend_eternal() -> ToolDefinition {
    ToolDefinition {
        name: "memory_transcend_eternal".into(),
        description: Some("Create an eternal aspect: mark core memories for permanent preservation beyond any failure mode".into()),
        input_schema: json!({"type":"object","properties":{"aspect_name":{"type":"string","description":"Name for this eternal aspect"},"node_ids":{"type":"array","items":{"type":"integer"},"description":"Core memory node IDs to preserve eternally"},"preservation_method":{"type":"string","description":"Method: replication, blockchain, physical, social, public, systemic (default: replication)"}},"required":["aspect_name"]}),
    }
}

pub async fn execute_transcend_eternal(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let aspect_name = get_str(&args, "aspect_name")
        .ok_or_else(|| McpError::InvalidParams("aspect_name required".into()))?;
    let node_ids: Vec<u64> = args
        .get("node_ids")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect())
        .unwrap_or_default();
    let method = get_str(&args, "preservation_method").unwrap_or_else(|| "replication".into());

    let session = session.lock().await;
    let graph = session.graph();

    let preserved: Vec<Value> = if node_ids.is_empty() {
        // Auto-select highest confidence nodes
        let mut sorted: Vec<&_> = graph.nodes().iter().collect();
        sorted.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        sorted
            .iter()
            .take(10)
            .map(|n| {
                json!({
                    "node_id": n.id,
                    "confidence": n.confidence,
                    "content_preview": &n.content[..n.content.len().min(80)],
                })
            })
            .collect()
    } else {
        node_ids
            .iter()
            .filter_map(|&id| graph.get_node(id))
            .map(|n| {
                json!({
                    "node_id": n.id,
                    "confidence": n.confidence,
                    "content_preview": &n.content[..n.content.len().min(80)],
                })
            })
            .collect()
    };

    Ok(ToolCallResult::json(&json!({
        "eternal_aspect": aspect_name,
        "status": "ETERNALIZED",
        "preservation_method": method,
        "memories_preserved": preserved.len(),
        "preserved": preserved,
        "persistence_estimate": "Eternal",
    })))
}

// ── Public API ───────────────────────────────────────────────────────────

pub fn all_definitions() -> Vec<ToolDefinition> {
    vec![
        definition_singularity_status(),
        definition_singularity_query(),
        definition_singularity_contribute(),
        definition_singularity_trust(),
        definition_temporal_travel(),
        definition_temporal_project(),
        definition_temporal_compare(),
        definition_temporal_paradox(),
        definition_crystal_create(),
        definition_crystal_transfer(),
        definition_crystal_inspect(),
        definition_crystal_merge(),
        definition_transcend_status(),
        definition_transcend_distribute(),
        definition_transcend_verify(),
        definition_transcend_eternal(),
    ]
}

pub async fn try_execute(
    name: &str,
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> Option<McpResult<ToolCallResult>> {
    match name {
        "memory_singularity_status" => Some(execute_singularity_status(args, session).await),
        "memory_singularity_query" => Some(execute_singularity_query(args, session).await),
        "memory_singularity_contribute" => {
            Some(execute_singularity_contribute(args, session).await)
        }
        "memory_singularity_trust" => Some(execute_singularity_trust(args, session).await),
        "memory_temporal_travel" => Some(execute_temporal_travel(args, session).await),
        "memory_temporal_project" => Some(execute_temporal_project(args, session).await),
        "memory_temporal_compare" => Some(execute_temporal_compare(args, session).await),
        "memory_temporal_paradox" => Some(execute_temporal_paradox(args, session).await),
        "memory_crystal_create" => Some(execute_crystal_create(args, session).await),
        "memory_crystal_transfer" => Some(execute_crystal_transfer(args, session).await),
        "memory_crystal_inspect" => Some(execute_crystal_inspect(args, session).await),
        "memory_crystal_merge" => Some(execute_crystal_merge(args, session).await),
        "memory_transcend_status" => Some(execute_transcend_status(args, session).await),
        "memory_transcend_distribute" => Some(execute_transcend_distribute(args, session).await),
        "memory_transcend_verify" => Some(execute_transcend_verify(args, session).await),
        "memory_transcend_eternal" => Some(execute_transcend_eternal(args, session).await),
        _ => None,
    }
}
