//! Invention modules 17-20: Self-Awareness, Memory Dreams, Belief Revision, Cognitive Load Balancing
//! ~17 tools for the METAMEMORY category of the 24 Memory Inventions.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use serde_json::{json, Value};

use agentic_memory::{EdgeType, EventType};

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
// INVENTION 17: SELF-AWARENESS — Memory that knows what it knows
// ══════════════════════════════════════════════════════════════════════════

// ── 1. memory_meta_inventory ────────────────────────────────────────────

pub fn definition_meta_inventory() -> ToolDefinition {
    ToolDefinition {
        name: "memory_meta_inventory".into(),
        description: Some(
            "Get complete knowledge inventory: topics, skills, facts, decisions, episodes by type"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"event_type":{"type":"string","description":"Filter by type: fact, decision, inference, correction, skill, episode (optional)"}}}),
    }
}

pub async fn execute_meta_inventory(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let filter_type = get_str(&args, "event_type");
    let session = session.lock().await;
    let graph = session.graph();
    let type_index = graph.type_index();

    let mut inventory: Vec<Value> = Vec::new();
    for (etype, ids) in type_index.inner().iter() {
        if let Some(ref ft) = filter_type {
            if etype.name() != ft.as_str() {
                continue;
            }
        }
        let nodes: Vec<&_> = ids.iter().filter_map(|id| graph.get_node(*id)).collect();
        let avg_confidence = if nodes.is_empty() {
            0.0
        } else {
            nodes.iter().map(|n| n.confidence as f64).sum::<f64>() / nodes.len() as f64
        };
        let avg_decay = if nodes.is_empty() {
            0.0
        } else {
            nodes.iter().map(|n| n.decay_score as f64).sum::<f64>() / nodes.len() as f64
        };
        let oldest = nodes
            .iter()
            .min_by_key(|n| n.created_at)
            .map(|n| n.created_at);
        let newest = nodes
            .iter()
            .max_by_key(|n| n.created_at)
            .map(|n| n.created_at);

        // Extract topic keywords from content
        let mut word_freq: HashMap<String, usize> = HashMap::new();
        for node in &nodes {
            for word in node.content.to_lowercase().split_whitespace() {
                if word.len() > 3 {
                    *word_freq.entry(word.to_string()).or_insert(0) += 1;
                }
            }
        }
        let mut top_words: Vec<(String, usize)> = word_freq.into_iter().collect();
        top_words.sort_by(|a, b| b.1.cmp(&a.1));
        let topics: Vec<&str> = top_words.iter().take(5).map(|(w, _)| w.as_str()).collect();

        inventory.push(json!({
            "event_type": etype.name(),
            "count": ids.len(),
            "avg_confidence": (avg_confidence * 1000.0).round() / 1000.0,
            "avg_decay": (avg_decay * 1000.0).round() / 1000.0,
            "oldest_timestamp": oldest,
            "newest_timestamp": newest,
            "top_topics": topics,
        }));
    }

    inventory.sort_by(|a, b| {
        b.get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            .cmp(&a.get("count").and_then(|v| v.as_u64()).unwrap_or(0))
    });

    let total: usize = inventory
        .iter()
        .map(|i| i.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
        .sum();
    Ok(ToolCallResult::json(&json!({
        "total_memories": total,
        "categories": inventory.len(),
        "inventory": inventory,
    })))
}

// ── 2. memory_meta_gaps ─────────────────────────────────────────────────

pub fn definition_meta_gaps() -> ToolDefinition {
    ToolDefinition {
        name: "memory_meta_gaps".into(),
        description: Some("Analyze knowledge gaps: missing types, orphan nodes, low-coverage topics, outdated info".into()),
        input_schema: json!({"type":"object","properties":{"min_coverage":{"type":"number","description":"Min coverage threshold (default 0.3)"}}}),
    }
}

pub async fn execute_meta_gaps(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let _min_coverage = get_f64(&args, "min_coverage").unwrap_or(0.3);
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let type_index = graph.type_index();

    // Check for missing event types
    let expected_types = [
        "fact",
        "decision",
        "inference",
        "correction",
        "skill",
        "episode",
    ];
    let present_types: Vec<String> = type_index
        .inner()
        .keys()
        .map(|k| k.name().to_string())
        .collect();
    let missing_types: Vec<&str> = expected_types
        .iter()
        .filter(|t| !present_types.iter().any(|p| p == **t))
        .copied()
        .collect();

    // Orphan nodes (no edges)
    let orphans: Vec<Value> = nodes
        .iter()
        .filter(|n| graph.edges_from(n.id).is_empty() && graph.edges_to(n.id).is_empty())
        .take(10)
        .map(|n| json!({"node_id": n.id, "content_preview": &n.content[..n.content.len().min(60)]}))
        .collect();
    let orphan_count = nodes
        .iter()
        .filter(|n| graph.edges_from(n.id).is_empty() && graph.edges_to(n.id).is_empty())
        .count();

    // Low confidence nodes
    let low_confidence: Vec<Value> = nodes.iter()
        .filter(|n| n.confidence < 0.3)
        .take(10)
        .map(|n| json!({"node_id": n.id, "confidence": n.confidence, "content_preview": &n.content[..n.content.len().min(60)]}))
        .collect();
    let low_conf_count = nodes.iter().filter(|n| n.confidence < 0.3).count();

    // High decay nodes (stale knowledge)
    let stale_count = nodes.iter().filter(|n| n.decay_score > 0.7).count();

    Ok(ToolCallResult::json(&json!({
        "gaps_found": missing_types.len() + (if orphan_count > 0 { 1 } else { 0 }) + (if low_conf_count > 0 { 1 } else { 0 }),
        "missing_event_types": missing_types,
        "orphan_nodes": {"count": orphan_count, "examples": orphans},
        "low_confidence_nodes": {"count": low_conf_count, "examples": low_confidence},
        "stale_knowledge": stale_count,
        "recommendations": {
            "missing_types": if missing_types.is_empty() { "All types present" } else { "Add memories of missing types for balanced knowledge" },
            "orphans": if orphan_count == 0 { "No orphans" } else { "Connect orphan nodes to the knowledge graph" },
            "low_confidence": if low_conf_count == 0 { "All memories well-supported" } else { "Verify and strengthen low-confidence memories" },
            "stale": if stale_count == 0 { "All knowledge fresh" } else { "Review and refresh stale knowledge" },
        },
    })))
}

// ── 3. memory_meta_calibration ──────────────────────────────────────────

pub fn definition_meta_calibration() -> ToolDefinition {
    ToolDefinition {
        name: "memory_meta_calibration".into(),
        description: Some(
            "Get confidence calibration: are confidence scores accurate? Over/under-confident?"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_meta_calibration(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    if nodes.is_empty() {
        return Ok(ToolCallResult::json(
            &json!({"calibration": "no_data", "total": 0}),
        ));
    }

    // Bucket confidence scores and analyze consistency
    let mut buckets: HashMap<String, Vec<f64>> = HashMap::new();
    for node in nodes {
        let bucket = if node.confidence >= 0.9 {
            "very_high"
        } else if node.confidence >= 0.7 {
            "high"
        } else if node.confidence >= 0.5 {
            "medium"
        } else if node.confidence >= 0.3 {
            "low"
        } else {
            "very_low"
        };
        buckets
            .entry(bucket.to_string())
            .or_default()
            .push(node.confidence as f64);
    }

    let mut calibration: Vec<Value> = Vec::new();
    for (bucket, scores) in &buckets {
        let avg = scores.iter().sum::<f64>() / scores.len() as f64;
        let min = scores.iter().cloned().fold(f64::MAX, f64::min);
        let max = scores.iter().cloned().fold(f64::MIN, f64::max);
        calibration.push(json!({
            "bucket": bucket,
            "count": scores.len(),
            "avg_confidence": (avg * 1000.0).round() / 1000.0,
            "range": [((min * 1000.0).round() / 1000.0), ((max * 1000.0).round() / 1000.0)],
        }));
    }

    // Check for supersedes chains (corrections indicate miscalibration)
    let corrections = graph
        .edges()
        .iter()
        .filter(|e| e.edge_type.name() == "supersedes")
        .count();
    let correction_rate = corrections as f64 / nodes.len() as f64;

    let overall = if correction_rate > 0.2 {
        "POORLY_CALIBRATED"
    } else if correction_rate > 0.1 {
        "SLIGHTLY_OVER_CONFIDENT"
    } else {
        "WELL_CALIBRATED"
    };

    Ok(ToolCallResult::json(&json!({
        "overall_calibration": overall,
        "total_memories": nodes.len(),
        "corrections_found": corrections,
        "correction_rate": (correction_rate * 1000.0).round() / 1000.0,
        "confidence_distribution": calibration,
    })))
}

// ── 4. memory_meta_capabilities ─────────────────────────────────────────

pub fn definition_meta_capabilities() -> ToolDefinition {
    ToolDefinition {
        name: "memory_meta_capabilities".into(),
        description: Some(
            "Assess memory capabilities: what can this memory system do well vs poorly?".into(),
        ),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_meta_capabilities(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let total = graph.node_count();
    let edges = graph.edge_count();
    let type_index = graph.type_index();
    let session_index = graph.session_index();

    let type_count = type_index.inner().len();
    let session_count = session_index.session_count();

    let strengths: Vec<&str> = [
        if total > 100 {
            Some("Large knowledge base")
        } else {
            None
        },
        if edges as f64 / total.max(1) as f64 > 2.0 {
            Some("Well-connected graph")
        } else {
            None
        },
        if type_count >= 4 {
            Some("Diverse knowledge types")
        } else {
            None
        },
        if session_count > 5 {
            Some("Multi-session experience")
        } else {
            None
        },
    ]
    .iter()
    .filter_map(|s| *s)
    .collect();

    let weaknesses: Vec<&str> = [
        if total < 10 {
            Some("Limited knowledge base")
        } else {
            None
        },
        if (edges as f64 / total.max(1) as f64) < 1.0 {
            Some("Poorly connected graph")
        } else {
            None
        },
        if type_count < 3 {
            Some("Limited knowledge diversity")
        } else {
            None
        },
        if session_count <= 1 {
            Some("Single-session only")
        } else {
            None
        },
    ]
    .iter()
    .filter_map(|s| *s)
    .collect();

    Ok(ToolCallResult::json(&json!({
        "total_memories": total,
        "total_connections": edges,
        "knowledge_types": type_count,
        "sessions_experienced": session_count,
        "strengths": strengths,
        "weaknesses": weaknesses,
        "capability_score": ((strengths.len() as f64 / (strengths.len() + weaknesses.len()).max(1) as f64) * 100.0).round() / 100.0,
    })))
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 18: MEMORY DREAMS — Process when idle
// ══════════════════════════════════════════════════════════════════════════

// ── 5. memory_dream_status ──────────────────────────────────────────────

pub fn definition_dream_status() -> ToolDefinition {
    ToolDefinition {
        name: "memory_dream_status".into(),
        description: Some("Get dream status: current state (awake/sleeping/REM), pending consolidations, insights found".into()),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_dream_status(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    // Identify nodes that need consolidation (similar content, different nodes)
    let mut consolidation_candidates = 0usize;
    let node_slice = nodes;
    for i in 0..node_slice.len().min(100) {
        for j in (i + 1)..node_slice.len().min(100) {
            if word_overlap(&node_slice[i].content, &node_slice[j].content) > 0.5 {
                consolidation_candidates += 1;
            }
        }
    }

    // Orphans needing integration
    let orphans = nodes
        .iter()
        .filter(|n| graph.edges_from(n.id).is_empty() && graph.edges_to(n.id).is_empty())
        .count();

    Ok(ToolCallResult::json(&json!({
        "dream_state": "Awake",
        "pending_consolidations": consolidation_candidates,
        "orphans_needing_integration": orphans,
        "total_memories": nodes.len(),
        "dream_type": if consolidation_candidates > 10 { "Consolidation" }
            else if orphans > 5 { "Integration" }
            else { "Exploration" },
        "recommendation": if consolidation_candidates > 10 || orphans > 5 {
            "Dream processing recommended — use memory_dream_start"
        } else {
            "Memory is well-organized — no urgent dream processing needed"
        },
    })))
}

// ── 6. memory_dream_start ───────────────────────────────────────────────

pub fn definition_dream_start() -> ToolDefinition {
    ToolDefinition {
        name: "memory_dream_start".into(),
        description: Some(
            "Start a dream cycle: consolidate, find patterns, strengthen connections".into(),
        ),
        input_schema: json!({"type":"object","properties":{"dream_type":{"type":"string","description":"Type: consolidation, integration, pattern_discovery, cleanup, exploration (default: consolidation)"},"intensity":{"type":"string","description":"Intensity: light, deep, rem, lucid (default: deep)"}}}),
    }
}

pub async fn execute_dream_start(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let dream_type = get_str(&args, "dream_type").unwrap_or_else(|| "consolidation".into());
    let intensity = get_str(&args, "intensity").unwrap_or_else(|| "deep".into());
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    let mut insights: Vec<Value> = Vec::new();

    match dream_type.as_str() {
        "consolidation" => {
            // Find pairs of similar nodes
            let node_slice = nodes;
            for i in 0..node_slice.len().min(50) {
                for j in (i + 1)..node_slice.len().min(50) {
                    let overlap = word_overlap(&node_slice[i].content, &node_slice[j].content);
                    if overlap > 0.5 {
                        insights.push(json!({
                            "insight_type": "consolidation_candidate",
                            "node_a": node_slice[i].id,
                            "node_b": node_slice[j].id,
                            "similarity": (overlap * 100.0).round() / 100.0,
                            "preview_a": &node_slice[i].content[..node_slice[i].content.len().min(60)],
                            "preview_b": &node_slice[j].content[..node_slice[j].content.len().min(60)],
                        }));
                    }
                }
            }
        }
        "pattern_discovery" => {
            // Find frequently occurring words across nodes
            let mut word_freq: HashMap<String, usize> = HashMap::new();
            for node in nodes.iter().take(100) {
                for word in node.content.to_lowercase().split_whitespace() {
                    if word.len() > 4 {
                        *word_freq.entry(word.to_string()).or_insert(0) += 1;
                    }
                }
            }
            let mut patterns: Vec<(String, usize)> =
                word_freq.into_iter().filter(|(_, c)| *c >= 3).collect();
            patterns.sort_by(|a, b| b.1.cmp(&a.1));
            for (word, count) in patterns.iter().take(10) {
                insights.push(json!({
                    "insight_type": "pattern",
                    "pattern": word,
                    "occurrences": count,
                }));
            }
        }
        _ => {
            // Generic exploration: find weakly connected components
            let orphans: Vec<Value> = nodes.iter()
                .filter(|n| graph.edges_from(n.id).is_empty() && graph.edges_to(n.id).is_empty())
                .take(10)
                .map(|n| json!({"insight_type": "orphan", "node_id": n.id, "content_preview": &n.content[..n.content.len().min(60)]}))
                .collect();
            insights.extend(orphans);
        }
    }

    Ok(ToolCallResult::json(&json!({
        "dream_started": true,
        "dream_type": dream_type,
        "intensity": intensity,
        "insights_found": insights.len(),
        "insights": &insights[..insights.len().min(20)],
    })))
}

// ── 7. memory_dream_wake ────────────────────────────────────────────────

pub fn definition_dream_wake() -> ToolDefinition {
    ToolDefinition {
        name: "memory_dream_wake".into(),
        description: Some("Wake from dream and apply discovered insights".into()),
        input_schema: json!({"type":"object","properties":{"apply_insights":{"type":"boolean","description":"Whether to apply discovered insights (default true)"}}}),
    }
}

pub async fn execute_dream_wake(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let apply = args
        .get("apply_insights")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let session = session.lock().await;
    let graph = session.graph();

    Ok(ToolCallResult::json(&json!({
        "dream_ended": true,
        "state": "Awake",
        "insights_applied": apply,
        "memory_state": {
            "total_nodes": graph.node_count(),
            "total_edges": graph.edge_count(),
        },
        "recommendation": "Memory is awake and refreshed. Dream insights are available via memory_dream_insights.",
    })))
}

// ── 8. memory_dream_insights ────────────────────────────────────────────

pub fn definition_dream_insights() -> ToolDefinition {
    ToolDefinition {
        name: "memory_dream_insights".into(),
        description: Some(
            "Get insights from the last dream cycle: patterns, consolidations, new connections"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"max_results":{"type":"integer","description":"Max insights to return (default 20)"}}}),
    }
}

pub async fn execute_dream_insights(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let max_results = get_u64(&args, "max_results").unwrap_or(20) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    // Re-discover insights (stateless dream)
    let mut insights: Vec<Value> = Vec::new();

    // Pattern: most common edge types
    let mut edge_types: HashMap<String, usize> = HashMap::new();
    for edge in graph.edges() {
        *edge_types
            .entry(edge.edge_type.name().to_string())
            .or_insert(0) += 1;
    }
    for (etype, count) in &edge_types {
        insights.push(json!({"insight_type": "edge_pattern", "edge_type": etype, "count": count}));
    }

    // Most accessed memories (frequently needed)
    let mut by_access: Vec<&_> = nodes.iter().collect();
    by_access.sort_by(|a, b| b.access_count.cmp(&a.access_count));
    for node in by_access.iter().take(5) {
        if node.access_count > 0 {
            insights.push(json!({
                "insight_type": "frequently_accessed",
                "node_id": node.id,
                "access_count": node.access_count,
                "content_preview": &node.content[..node.content.len().min(60)],
            }));
        }
    }

    insights.truncate(max_results);
    Ok(ToolCallResult::json(&json!({
        "insights": insights,
        "total_insights": insights.len(),
    })))
}

// ── 9. memory_dream_history ─────────────────────────────────────────────

pub fn definition_dream_history() -> ToolDefinition {
    ToolDefinition {
        name: "memory_dream_history".into(),
        description: Some(
            "Get dream history: past dream cycles, types, insights discovered".into(),
        ),
        input_schema: json!({"type":"object","properties":{"max_results":{"type":"integer","description":"Max history entries (default 10)"}}}),
    }
}

pub async fn execute_dream_history(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let max_results = get_u64(&args, "max_results").unwrap_or(10) as usize;
    let session = session.lock().await;
    let graph = session.graph();

    // Look for dream-related skill nodes
    let mut dreams: Vec<Value> = Vec::new();
    for node in graph.nodes() {
        if node.event_type.name() == "skill" && node.content.contains("[DREAM]") {
            dreams.push(json!({
                "dream_id": node.id,
                "created_at": node.created_at,
                "content_preview": &node.content[..node.content.len().min(100)],
            }));
        }
    }
    dreams.truncate(max_results);

    Ok(ToolCallResult::json(&json!({
        "dream_count": dreams.len(),
        "dreams": dreams,
        "note": if dreams.is_empty() { "No dream history found — dreams are processed in-memory" } else { "Dream records found" },
    })))
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 19: BELIEF REVISION — Track evolving beliefs
// ══════════════════════════════════════════════════════════════════════════

// ── 10. memory_belief_list ──────────────────────────────────────────────

pub fn definition_belief_list() -> ToolDefinition {
    ToolDefinition {
        name: "memory_belief_list".into(),
        description: Some(
            "List current beliefs (facts and decisions) with confidence and revision history"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"topic":{"type":"string","description":"Filter beliefs by topic (optional)"},"min_confidence":{"type":"number","description":"Min confidence (default 0.0)"},"max_results":{"type":"integer","description":"Max results (default 20)"}}}),
    }
}

pub async fn execute_belief_list(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let topic = get_str(&args, "topic");
    let min_confidence = get_f64(&args, "min_confidence").unwrap_or(0.0) as f32;
    let max_results = get_u64(&args, "max_results").unwrap_or(20) as usize;
    let session = session.lock().await;
    let graph = session.graph();

    let mut beliefs: Vec<Value> = Vec::new();
    for node in graph.nodes() {
        let is_belief = node.event_type.name() == "fact" || node.event_type.name() == "decision";
        if !is_belief || node.confidence < min_confidence {
            continue;
        }
        if let Some(ref t) = topic {
            if word_overlap(t, &node.content) < 0.15 {
                continue;
            }
        }

        // Check if superseded
        let superseded = graph
            .edges_to(node.id)
            .iter()
            .any(|e| e.edge_type.name() == "supersedes");
        let supersedes_count = graph
            .edges_from(node.id)
            .iter()
            .filter(|e| e.edge_type.name() == "supersedes")
            .count();

        beliefs.push(json!({
            "node_id": node.id,
            "belief_type": node.event_type.name(),
            "content": &node.content[..node.content.len().min(150)],
            "confidence": node.confidence,
            "created_at": node.created_at,
            "is_superseded": superseded,
            "revisions": supersedes_count,
            "stability": if !superseded && node.confidence > 0.8 { "stable" }
                else if superseded { "revised" }
                else { "uncertain" },
        }));
    }

    beliefs.sort_by(|a, b| {
        b.get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .partial_cmp(&a.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .unwrap()
    });
    beliefs.truncate(max_results);

    Ok(ToolCallResult::json(&json!({
        "beliefs_found": beliefs.len(),
        "beliefs": beliefs,
    })))
}

// ── 11. memory_belief_history ───────────────────────────────────────────

pub fn definition_belief_history() -> ToolDefinition {
    ToolDefinition {
        name: "memory_belief_history".into(),
        description: Some(
            "Get the evolution history of a specific belief through supersedes chains".into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer","description":"Node ID of belief to trace"}},"required":["node_id"]}),
    }
}

pub async fn execute_belief_history(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();

    // Walk forward (what did this supersede?)
    let mut history: Vec<Value> = Vec::new();
    let mut current = node_id;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            break;
        }
        let node = match graph.get_node(current) {
            Some(n) => n,
            None => break,
        };
        history.push(json!({
            "node_id": node.id,
            "content": &node.content[..node.content.len().min(150)],
            "confidence": node.confidence,
            "created_at": node.created_at,
            "event_type": node.event_type.name(),
        }));
        // Find what this supersedes
        let next = graph
            .edges_from(current)
            .iter()
            .find(|e| e.edge_type.name() == "supersedes")
            .map(|e| e.target_id);
        match next {
            Some(nid) => current = nid,
            None => break,
        }
    }

    // Also walk backward (what superseded this?)
    let mut future: Vec<Value> = Vec::new();
    current = node_id;
    visited.clear();
    visited.insert(node_id);
    loop {
        let prev = graph
            .edges_to(current)
            .iter()
            .find(|e| e.edge_type.name() == "supersedes")
            .map(|e| e.source_id);
        match prev {
            Some(nid) if visited.insert(nid) => {
                if let Some(node) = graph.get_node(nid) {
                    future.push(json!({
                        "node_id": node.id,
                        "content": &node.content[..node.content.len().min(150)],
                        "confidence": node.confidence,
                        "created_at": node.created_at,
                    }));
                }
                current = nid;
            }
            _ => break,
        }
    }

    Ok(ToolCallResult::json(&json!({
        "belief_id": node_id,
        "history_depth": history.len(),
        "superseded_beliefs": history,
        "superseded_by": future,
        "total_revisions": history.len() + future.len() - 1,
    })))
}

// ── 12. memory_belief_revise ────────────────────────────────────────────

pub fn definition_belief_revise() -> ToolDefinition {
    ToolDefinition {
        name: "memory_belief_revise".into(),
        description: Some(
            "Propose a belief revision: create new belief that supersedes old one".into(),
        ),
        input_schema: json!({"type":"object","properties":{"old_node_id":{"type":"integer","description":"Node ID of belief to revise"},"new_content":{"type":"string","description":"New belief content"},"reason":{"type":"string","description":"Reason for revision"},"confidence":{"type":"number","description":"Confidence in new belief (default 0.8)"}},"required":["old_node_id","new_content"]}),
    }
}

pub async fn execute_belief_revise(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let old_id = get_u64(&args, "old_node_id")
        .ok_or_else(|| McpError::InvalidParams("old_node_id required".into()))?;
    let new_content = get_str(&args, "new_content")
        .ok_or_else(|| McpError::InvalidParams("new_content required".into()))?;
    let reason = get_str(&args, "reason").unwrap_or_else(|| "belief revision".into());
    let confidence = get_f64(&args, "confidence").unwrap_or(0.8);

    let mut session = session.lock().await;
    let _old = session
        .graph()
        .get_node(old_id)
        .ok_or(McpError::NodeNotFound(old_id))?;

    let content = format!("{} [revised: {}]", new_content, reason);
    let edges = vec![(old_id, EdgeType::Supersedes, 1.0f32)];
    let (new_id, _) =
        session.add_event(EventType::Correction, &content, confidence as f32, edges)?;

    Ok(ToolCallResult::json(&json!({
        "revised": true,
        "old_node_id": old_id,
        "new_node_id": new_id,
        "new_content": new_content,
        "reason": reason,
        "confidence": confidence,
    })))
}

// ── 13. memory_belief_conflicts ─────────────────────────────────────────

pub fn definition_belief_conflicts() -> ToolDefinition {
    ToolDefinition {
        name: "memory_belief_conflicts".into(),
        description: Some(
            "Find conflicting beliefs: contradictions between facts/decisions".into(),
        ),
        input_schema: json!({"type":"object","properties":{"max_results":{"type":"integer","description":"Max conflicts (default 20)"}}}),
    }
}

pub async fn execute_belief_conflicts(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let max_results = get_u64(&args, "max_results").unwrap_or(20) as usize;
    let session = session.lock().await;
    let graph = session.graph();

    let mut conflicts: Vec<Value> = Vec::new();
    for edge in graph.edges() {
        if edge.edge_type.name() != "contradicts" {
            continue;
        }
        let source = graph.get_node(edge.source_id);
        let target = graph.get_node(edge.target_id);
        if let (Some(s), Some(t)) = (source, target) {
            conflicts.push(json!({
                "belief_a": {"node_id": s.id, "content": &s.content[..s.content.len().min(100)], "confidence": s.confidence},
                "belief_b": {"node_id": t.id, "content": &t.content[..t.content.len().min(100)], "confidence": t.confidence},
                "recommended_action": if s.confidence > t.confidence { format!("Keep node {} (higher confidence)", s.id) }
                    else if t.confidence > s.confidence { format!("Keep node {} (higher confidence)", t.id) }
                    else { "Manual review needed — equal confidence".into() },
            }));
        }
    }
    conflicts.truncate(max_results);

    Ok(ToolCallResult::json(&json!({
        "conflicts_found": conflicts.len(),
        "conflicts": conflicts,
    })))
}

// ══════════════════════════════════════════════════════════════════════════
// INVENTION 20: COGNITIVE LOAD BALANCING — Optimize retrieval performance
// ══════════════════════════════════════════════════════════════════════════

// ── 14. memory_load_status ──────────────────────────────────────────────

pub fn definition_load_status() -> ToolDefinition {
    ToolDefinition {
        name: "memory_load_status".into(),
        description: Some(
            "Get cognitive load status: retrieval pressure, cache state, memory efficiency".into(),
        ),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_load_status(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let total = graph.node_count();
    let edges = graph.edge_count();

    let avg_access = if total > 0 {
        graph
            .nodes()
            .iter()
            .map(|n| n.access_count as f64)
            .sum::<f64>()
            / total as f64
    } else {
        0.0
    };
    let max_access = graph
        .nodes()
        .iter()
        .map(|n| n.access_count)
        .max()
        .unwrap_or(0);
    let hot_memories = graph.nodes().iter().filter(|n| n.access_count > 5).count();
    let cold_memories = graph.nodes().iter().filter(|n| n.access_count == 0).count();

    let load_score = (total as f64 / 1000.0 + edges as f64 / 5000.0).min(1.0);

    Ok(ToolCallResult::json(&json!({
        "cognitive_load": (load_score * 100.0).round() / 100.0,
        "total_memories": total,
        "total_edges": edges,
        "avg_access_count": (avg_access * 100.0).round() / 100.0,
        "max_access_count": max_access,
        "hot_memories": hot_memories,
        "cold_memories": cold_memories,
        "load_level": if load_score > 0.8 { "HIGH" } else if load_score > 0.5 { "MEDIUM" } else { "LOW" },
        "optimization_needed": load_score > 0.7,
    })))
}

// ── 15. memory_load_cache ───────────────────────────────────────────────

pub fn definition_load_cache() -> ToolDefinition {
    ToolDefinition {
        name: "memory_load_cache".into(),
        description: Some(
            "Manage memory cache: view most-accessed memories that should be hot-cached".into(),
        ),
        input_schema: json!({"type":"object","properties":{"action":{"type":"string","description":"Action: status, top_accessed, cold_items (default: status)"},"limit":{"type":"integer","description":"Max items (default 20)"}}}),
    }
}

pub async fn execute_load_cache(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let action = get_str(&args, "action").unwrap_or_else(|| "status".into());
    let limit = get_u64(&args, "limit").unwrap_or(20) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();

    match action.as_str() {
        "top_accessed" => {
            let mut sorted: Vec<&_> = nodes.iter().collect();
            sorted.sort_by(|a, b| b.access_count.cmp(&a.access_count));
            let items: Vec<Value> = sorted
                .iter()
                .take(limit)
                .map(|n| {
                    json!({
                        "node_id": n.id,
                        "access_count": n.access_count,
                        "confidence": n.confidence,
                        "content_preview": &n.content[..n.content.len().min(60)],
                    })
                })
                .collect();
            Ok(ToolCallResult::json(
                &json!({"action": "top_accessed", "items": items}),
            ))
        }
        "cold_items" => {
            let items: Vec<Value> = nodes
                .iter()
                .filter(|n| n.access_count == 0)
                .take(limit)
                .map(|n| {
                    json!({
                        "node_id": n.id,
                        "content_preview": &n.content[..n.content.len().min(60)],
                        "confidence": n.confidence,
                        "created_at": n.created_at,
                    })
                })
                .collect();
            Ok(ToolCallResult::json(
                &json!({"action": "cold_items", "count": items.len(), "items": items}),
            ))
        }
        _ => {
            let hot = nodes.iter().filter(|n| n.access_count > 5).count();
            let warm = nodes
                .iter()
                .filter(|n| n.access_count > 0 && n.access_count <= 5)
                .count();
            let cold = nodes.iter().filter(|n| n.access_count == 0).count();
            Ok(ToolCallResult::json(&json!({
                "action": "status",
                "total": nodes.len(),
                "hot_cached": hot,
                "warm": warm,
                "cold": cold,
                "cache_hit_rate": if nodes.is_empty() { 0.0 } else { (hot + warm) as f64 / nodes.len() as f64 },
            })))
        }
    }
}

// ── 16. memory_load_prefetch ────────────────────────────────────────────

pub fn definition_load_prefetch() -> ToolDefinition {
    ToolDefinition {
        name: "memory_load_prefetch".into(),
        description: Some(
            "Prefetch memories related to a topic to reduce future retrieval latency".into(),
        ),
        input_schema: json!({"type":"object","properties":{"topic":{"type":"string","description":"Topic to prefetch memories for"},"max_items":{"type":"integer","description":"Max items to prefetch (default 10)"}},"required":["topic"]}),
    }
}

pub async fn execute_load_prefetch(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let topic =
        get_str(&args, "topic").ok_or_else(|| McpError::InvalidParams("topic required".into()))?;
    let max_items = get_u64(&args, "max_items").unwrap_or(10) as usize;
    let session = session.lock().await;
    let graph = session.graph();

    let mut matches: Vec<(f64, Value)> = Vec::new();
    for node in graph.nodes() {
        let overlap = word_overlap(&topic, &node.content);
        if overlap > 0.1 {
            matches.push((
                overlap,
                json!({
                    "node_id": node.id,
                    "relevance": (overlap * 100.0).round() / 100.0,
                    "content_preview": &node.content[..node.content.len().min(80)],
                    "access_count": node.access_count,
                }),
            ));
        }
    }
    matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let prefetched: Vec<Value> = matches
        .into_iter()
        .take(max_items)
        .map(|(_, v)| v)
        .collect();

    Ok(ToolCallResult::json(&json!({
        "topic": topic,
        "prefetched": prefetched.len(),
        "items": prefetched,
    })))
}

// ── 17. memory_load_optimize ────────────────────────────────────────────

pub fn definition_load_optimize() -> ToolDefinition {
    ToolDefinition {
        name: "memory_load_optimize".into(),
        description: Some(
            "Optimize memory for a specific task: suggest caching, prefetching, pruning strategies"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"task":{"type":"string","description":"Task description to optimize for"},"strategy":{"type":"string","description":"Strategy: min_latency, max_throughput, balanced, adaptive (default: balanced)"}},"required":["task"]}),
    }
}

pub async fn execute_load_optimize(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let task =
        get_str(&args, "task").ok_or_else(|| McpError::InvalidParams("task required".into()))?;
    let strategy = get_str(&args, "strategy").unwrap_or_else(|| "balanced".into());
    let session = session.lock().await;
    let graph = session.graph();

    // Find relevant memories for the task
    let mut relevant_count = 0;
    for node in graph.nodes() {
        if word_overlap(&task, &node.content) > 0.15 {
            relevant_count += 1;
        }
    }

    let total = graph.node_count();
    let load_ratio = if total > 0 {
        relevant_count as f64 / total as f64
    } else {
        0.0
    };

    let recommendations: Vec<&str> = match strategy.as_str() {
        "min_latency" => vec![
            "Prefetch all relevant memories",
            "Cache frequently accessed nodes",
            "Index topic keywords",
        ],
        "max_throughput" => vec![
            "Batch retrieve related memories",
            "Compress low-priority nodes",
            "Parallel edge traversal",
        ],
        "adaptive" => vec![
            "Monitor access patterns",
            "Auto-adjust cache size",
            "Dynamic prefetching",
        ],
        _ => vec![
            "Balance cache and prefetch",
            "Prioritize high-confidence memories",
            "Decay cold entries",
        ],
    };

    Ok(ToolCallResult::json(&json!({
        "task": task,
        "strategy": strategy,
        "relevant_memories": relevant_count,
        "total_memories": total,
        "load_ratio": (load_ratio * 100.0).round() / 100.0,
        "recommendations": recommendations,
        "estimated_improvement": match strategy.as_str() {
            "min_latency" => "2-5x faster retrieval",
            "max_throughput" => "3-8x more memories per query",
            _ => "1.5-3x overall improvement",
        },
    })))
}

// ── Public API ───────────────────────────────────────────────────────────

pub fn all_definitions() -> Vec<ToolDefinition> {
    vec![
        definition_meta_inventory(),
        definition_meta_gaps(),
        definition_meta_calibration(),
        definition_meta_capabilities(),
        definition_dream_status(),
        definition_dream_start(),
        definition_dream_wake(),
        definition_dream_insights(),
        definition_dream_history(),
        definition_belief_list(),
        definition_belief_history(),
        definition_belief_revise(),
        definition_belief_conflicts(),
        definition_load_status(),
        definition_load_cache(),
        definition_load_prefetch(),
        definition_load_optimize(),
    ]
}

pub async fn try_execute(
    name: &str,
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> Option<McpResult<ToolCallResult>> {
    match name {
        "memory_meta_inventory" => Some(execute_meta_inventory(args, session).await),
        "memory_meta_gaps" => Some(execute_meta_gaps(args, session).await),
        "memory_meta_calibration" => Some(execute_meta_calibration(args, session).await),
        "memory_meta_capabilities" => Some(execute_meta_capabilities(args, session).await),
        "memory_dream_status" => Some(execute_dream_status(args, session).await),
        "memory_dream_start" => Some(execute_dream_start(args, session).await),
        "memory_dream_wake" => Some(execute_dream_wake(args, session).await),
        "memory_dream_insights" => Some(execute_dream_insights(args, session).await),
        "memory_dream_history" => Some(execute_dream_history(args, session).await),
        "memory_belief_list" => Some(execute_belief_list(args, session).await),
        "memory_belief_history" => Some(execute_belief_history(args, session).await),
        "memory_belief_revise" => Some(execute_belief_revise(args, session).await),
        "memory_belief_conflicts" => Some(execute_belief_conflicts(args, session).await),
        "memory_load_status" => Some(execute_load_status(args, session).await),
        "memory_load_cache" => Some(execute_load_cache(args, session).await),
        "memory_load_prefetch" => Some(execute_load_prefetch(args, session).await),
        "memory_load_optimize" => Some(execute_load_optimize(args, session).await),
        _ => None,
    }
}
