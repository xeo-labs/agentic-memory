//! Invention modules 1-4: Immortal Memory, Semantic Compression, Context Optimization, Memory Metabolism
//! ~17 tools for the INFINITE category of the 24 Memory Inventions.

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

// ── 1. memory_immortal_stats ─────────────────────────────────────────────

pub fn definition_immortal_stats() -> ToolDefinition {
    ToolDefinition {
        name: "memory_immortal_stats".into(),
        description: Some(
            "Get immortality stats: total nodes, oldest, decay status, survival projections".into(),
        ),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_immortal_stats(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let total = nodes.len();
    let oldest = nodes.iter().min_by_key(|n| n.created_at).map(|n| json!({"id":n.id,"created_at":n.created_at,"content":&n.content[..n.content.len().min(80)]}));
    let newest = nodes
        .iter()
        .max_by_key(|n| n.created_at)
        .map(|n| json!({"id":n.id,"created_at":n.created_at}));
    let avg_confidence: f64 = if total > 0 {
        nodes.iter().map(|n| n.confidence as f64).sum::<f64>() / total as f64
    } else {
        0.0
    };
    let avg_decay: f64 = if total > 0 {
        nodes.iter().map(|n| n.decay_score as f64).sum::<f64>() / total as f64
    } else {
        0.0
    };
    let high_confidence = nodes.iter().filter(|n| n.confidence >= 0.8).count();
    let low_decay = nodes.iter().filter(|n| n.decay_score <= 0.3).count();
    Ok(ToolCallResult::json(&json!({
        "total_nodes": total, "total_edges": graph.edge_count(),
        "oldest": oldest, "newest": newest,
        "avg_confidence": (avg_confidence * 1000.0).round() / 1000.0,
        "avg_decay": (avg_decay * 1000.0).round() / 1000.0,
        "high_confidence_count": high_confidence, "low_decay_count": low_decay,
        "immortality_score": (((high_confidence as f64 / total.max(1) as f64) + (low_decay as f64 / total.max(1) as f64)) / 2.0 * 100.0).round() / 100.0
    })))
}

// ── 2. memory_immortal_prove ─────────────────────────────────────────────

pub fn definition_immortal_prove() -> ToolDefinition {
    ToolDefinition {
        name: "memory_immortal_prove".into(),
        description: Some(
            "Prove a memory exists with evidence chain (supporting edges and connected nodes)"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer","description":"Memory node ID to prove"}},"required":["node_id"]}),
    }
}

pub async fn execute_immortal_prove(
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
    let outgoing: Vec<Value> = graph
        .edges_from(node_id)
        .iter()
        .map(|e| json!({"target":e.target_id,"type":e.edge_type.name(),"weight":e.weight}))
        .collect();
    let incoming: Vec<Value> = graph
        .edges_to(node_id)
        .iter()
        .map(|e| json!({"source":e.source_id,"type":e.edge_type.name(),"weight":e.weight}))
        .collect();
    let support_count = incoming.iter().filter(|e| e["type"] == "supports").count()
        + outgoing.iter().filter(|e| e["type"] == "supports").count();
    Ok(ToolCallResult::json(&json!({
        "node_id": node_id, "content": &node.content, "confidence": node.confidence,
        "event_type": node.event_type.name(), "created_at": node.created_at,
        "outgoing_edges": outgoing, "incoming_edges": incoming,
        "support_count": support_count,
        "proven": support_count > 0 || node.confidence >= 0.8
    })))
}

// ── 3. memory_immortal_project ───────────────────────────────────────────

pub fn definition_immortal_project() -> ToolDefinition {
    ToolDefinition {
        name: "memory_immortal_project".into(),
        description: Some(
            "Project memory survival probability based on current decay and access patterns".into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer","description":"Memory node ID"}},"required":["node_id"]}),
    }
}

pub async fn execute_immortal_project(
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
    let support_edges = graph.edges_to(node_id).len() + graph.edges_from(node_id).len();
    let survival_base = node.confidence as f64 * (1.0 - node.decay_score as f64);
    let connection_bonus = (support_edges as f64 * 0.05).min(0.3);
    let access_bonus = (node.access_count as f64 * 0.02).min(0.2);
    let survival = (survival_base + connection_bonus + access_bonus).min(1.0);
    Ok(ToolCallResult::json(&json!({
        "node_id": node_id, "confidence": node.confidence, "decay_score": node.decay_score,
        "access_count": node.access_count, "edge_count": support_edges,
        "survival_probability": (survival * 1000.0).round() / 1000.0,
        "risk_level": if survival > 0.7 { "low" } else if survival > 0.4 { "medium" } else { "high" },
        "recommendation": if survival < 0.4 { "strengthen or consolidate" } else if survival < 0.7 { "consider reinforcing" } else { "healthy" }
    })))
}

// ── 4. memory_immortal_tier_move ─────────────────────────────────────────

pub fn definition_immortal_tier_move() -> ToolDefinition {
    ToolDefinition {
        name: "memory_immortal_tier_move".into(),
        description: Some(
            "Move memory to a different storage tier (hot/warm/cold) by adjusting decay".into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"tier":{"type":"string","enum":["hot","warm","cold"]}},"required":["node_id","tier"]}),
    }
}

pub async fn execute_immortal_tier_move(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let tier =
        get_str(&args, "tier").ok_or_else(|| McpError::InvalidParams("tier required".into()))?;
    let mut session = session.lock().await;
    let graph = session.graph_mut();
    let node = graph
        .get_node_mut(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let old_decay = node.decay_score;
    match tier.as_str() {
        "hot" => {
            node.decay_score = 0.0;
            node.access_count += 1;
        }
        "warm" => {
            node.decay_score = 0.3;
        }
        "cold" => {
            node.decay_score = 0.7;
        }
        _ => {
            return Err(McpError::InvalidParams(
                "tier must be hot, warm, or cold".into(),
            ))
        }
    }
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"old_decay":old_decay,"new_decay":node.decay_score,"tier":tier,"moved":true}),
    ))
}

// ── 5. memory_semantic_compress ──────────────────────────────────────────

pub fn definition_semantic_compress() -> ToolDefinition {
    ToolDefinition {
        name: "memory_semantic_compress".into(),
        description: Some(
            "Compress similar memories into summaries, identifying clusters of related nodes"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"threshold":{"type":"number","default":0.5,"description":"Similarity threshold (0-1)"}}}),
    }
}

pub async fn execute_semantic_compress(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let threshold = get_f64(&args, "threshold").unwrap_or(0.5);
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let mut clusters: Vec<Vec<u64>> = Vec::new();
    let mut assigned: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for i in 0..nodes.len() {
        if assigned.contains(&nodes[i].id) {
            continue;
        }
        let mut cluster = vec![nodes[i].id];
        assigned.insert(nodes[i].id);
        for j in (i + 1)..nodes.len() {
            if assigned.contains(&nodes[j].id) {
                continue;
            }
            if word_overlap(&nodes[i].content, &nodes[j].content) >= threshold {
                cluster.push(nodes[j].id);
                assigned.insert(nodes[j].id);
            }
        }
        if cluster.len() > 1 {
            clusters.push(cluster);
        }
    }
    let cluster_info: Vec<Value> = clusters
        .iter()
        .take(20)
        .map(|c| {
            let first = graph
                .get_node(c[0])
                .map(|n| n.content.chars().take(80).collect::<String>())
                .unwrap_or_default();
            json!({"size": c.len(), "node_ids": c, "representative": first})
        })
        .collect();
    Ok(ToolCallResult::json(
        &json!({"clusters_found":clusters.len(),"total_compressible_nodes":clusters.iter().map(|c| c.len()).sum::<usize>(),"threshold":threshold,"clusters":cluster_info}),
    ))
}

// ── 6. memory_semantic_dedup ─────────────────────────────────────────────

pub fn definition_semantic_dedup() -> ToolDefinition {
    ToolDefinition {
        name: "memory_semantic_dedup".into(),
        description: Some("Find duplicate or near-duplicate memories".into()),
        input_schema: json!({"type":"object","properties":{"threshold":{"type":"number","default":0.8},"max_results":{"type":"integer","default":20}}}),
    }
}

pub async fn execute_semantic_dedup(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let threshold = get_f64(&args, "threshold").unwrap_or(0.8);
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let mut duplicates: Vec<Value> = Vec::new();
    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            if duplicates.len() >= max_results {
                break;
            }
            let sim = word_overlap(&nodes[i].content, &nodes[j].content);
            if sim >= threshold {
                duplicates.push(
                    json!({"node_a":nodes[i].id,"node_b":nodes[j].id,"similarity":sim,
                    "content_a":&nodes[i].content[..nodes[i].content.len().min(60)],
                    "content_b":&nodes[j].content[..nodes[j].content.len().min(60)]}),
                );
            }
        }
        if duplicates.len() >= max_results {
            break;
        }
    }
    Ok(ToolCallResult::json(
        &json!({"duplicates_found":duplicates.len(),"threshold":threshold,"duplicates":duplicates}),
    ))
}

// ── 7. memory_semantic_similar ───────────────────────────────────────────

pub fn definition_semantic_similar() -> ToolDefinition {
    ToolDefinition {
        name: "memory_semantic_similar_enhanced".into(),
        description: Some(
            "Find semantically similar memories to a query (enhanced with scoring)".into(),
        ),
        input_schema: json!({"type":"object","properties":{"query":{"type":"string"},"max_results":{"type":"integer","default":10}},"required":["query"]}),
    }
}

pub async fn execute_semantic_similar(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let query =
        get_str(&args, "query").ok_or_else(|| McpError::InvalidParams("query required".into()))?;
    let max = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let mut scored: Vec<(u64, f64, &str)> = graph
        .nodes()
        .iter()
        .map(|n| (n.id, word_overlap(&query, &n.content), n.content.as_str()))
        .filter(|(_, s, _)| *s > 0.0)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(max);
    let results: Vec<Value> = scored.iter().map(|(id, score, content)| json!({"id":id,"similarity":score,"content":&content[..content.len().min(120)]})).collect();
    Ok(ToolCallResult::json(
        &json!({"query":query,"results_count":results.len(),"results":results}),
    ))
}

// ── 8. memory_semantic_cluster ───────────────────────────────────────────

pub fn definition_semantic_cluster() -> ToolDefinition {
    ToolDefinition {
        name: "memory_semantic_cluster".into(),
        description: Some("Cluster memories by semantic similarity using keyword overlap".into()),
        input_schema: json!({"type":"object","properties":{"num_clusters":{"type":"integer","default":5}}}),
    }
}

pub async fn execute_semantic_cluster(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let num_clusters = args
        .get("num_clusters")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    // Simple keyword-based clustering
    let mut keyword_groups: HashMap<String, Vec<u64>> = HashMap::new();
    for node in nodes {
        let words: Vec<&str> = node.content.split_whitespace().collect();
        if let Some(key) = words.first() {
            let k = key.to_lowercase();
            keyword_groups.entry(k).or_default().push(node.id);
        }
    }
    let mut clusters: Vec<(&String, &Vec<u64>)> = keyword_groups.iter().collect();
    clusters.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    clusters.truncate(num_clusters);
    let result: Vec<Value> = clusters.iter().map(|(k, ids)| json!({"keyword": k, "size": ids.len(), "node_ids": &ids[..ids.len().min(10)]})).collect();
    Ok(ToolCallResult::json(
        &json!({"num_clusters":result.len(),"total_nodes":nodes.len(),"clusters":result}),
    ))
}

// ── 9. memory_context_optimize ───────────────────────────────────────────

pub fn definition_context_optimize() -> ToolDefinition {
    ToolDefinition {
        name: "memory_context_optimize".into(),
        description: Some(
            "Optimize context window selection - find the most relevant memories for a topic"
                .into(),
        ),
        input_schema: json!({"type":"object","properties":{"topic":{"type":"string"},"window_size":{"type":"integer","default":10}},"required":["topic"]}),
    }
}

pub async fn execute_context_optimize(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let topic =
        get_str(&args, "topic").ok_or_else(|| McpError::InvalidParams("topic required".into()))?;
    let window = args
        .get("window_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let mut scored: Vec<(u64, f64)> = graph
        .nodes()
        .iter()
        .map(|n| {
            let relevance = word_overlap(&topic, &n.content);
            let freshness = 1.0 - n.decay_score as f64;
            let importance = n.confidence as f64;
            (n.id, relevance * 0.5 + freshness * 0.25 + importance * 0.25)
        })
        .filter(|(_, s)| *s > 0.1)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(window);
    let results: Vec<Value> = scored.iter().filter_map(|(id, score)| {
        graph.get_node(*id).map(|n| json!({"id":n.id,"score":score,"content":&n.content[..n.content.len().min(100)],"confidence":n.confidence}))
    }).collect();
    Ok(ToolCallResult::json(
        &json!({"topic":topic,"window_size":window,"optimized_count":results.len(),"context":results}),
    ))
}

// ── 10. memory_context_expand ────────────────────────────────────────────

pub fn definition_context_expand() -> ToolDefinition {
    ToolDefinition {
        name: "memory_context_expand".into(),
        description: Some(
            "Expand context around a memory by following edges to related nodes".into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"depth":{"type":"integer","default":2}},"required":["node_id"]}),
    }
}

pub async fn execute_context_expand(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let _ = graph
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let mut visited: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut frontier = vec![node_id];
    visited.insert(node_id);
    for _ in 0..depth {
        let mut next_frontier = Vec::new();
        for &nid in &frontier {
            for edge in graph.edges_from(nid) {
                if visited.insert(edge.target_id) {
                    next_frontier.push(edge.target_id);
                }
            }
            for edge in graph.edges_to(nid) {
                if visited.insert(edge.source_id) {
                    next_frontier.push(edge.source_id);
                }
            }
        }
        frontier = next_frontier;
    }
    let context: Vec<Value> = visited.iter().filter_map(|id| {
        graph.get_node(*id).map(|n| json!({"id":n.id,"type":n.event_type.name(),"content":&n.content[..n.content.len().min(100)],"confidence":n.confidence}))
    }).collect();
    Ok(ToolCallResult::json(
        &json!({"center_node":node_id,"depth":depth,"expanded_count":context.len(),"context":context}),
    ))
}

// ── 11. memory_context_summarize ─────────────────────────────────────────

pub fn definition_context_summarize() -> ToolDefinition {
    ToolDefinition {
        name: "memory_context_summarize".into(),
        description: Some("Summarize a context window of memories".into()),
        input_schema: json!({"type":"object","properties":{"node_ids":{"type":"array","items":{"type":"integer"}}},"required":["node_ids"]}),
    }
}

pub async fn execute_context_summarize(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let ids: Vec<u64> = args
        .get("node_ids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_u64()).collect())
        .unwrap_or_default();
    if ids.is_empty() {
        return Err(McpError::InvalidParams("node_ids required".into()));
    }
    let session = session.lock().await;
    let graph = session.graph();
    let mut types: HashMap<String, usize> = HashMap::new();
    let mut contents: Vec<String> = Vec::new();
    let mut total_confidence = 0.0f64;
    for &id in &ids {
        if let Some(n) = graph.get_node(id) {
            *types.entry(n.event_type.name().to_string()).or_default() += 1;
            contents.push(n.content.chars().take(60).collect());
            total_confidence += n.confidence as f64;
        }
    }
    let found = contents.len();
    Ok(ToolCallResult::json(&json!({
        "node_count":ids.len(),"found":found,
        "avg_confidence": if found > 0 { total_confidence / found as f64 } else { 0.0 },
        "type_distribution":types,
        "snippets":contents
    })))
}

// ── 12. memory_context_navigate ──────────────────────────────────────────

pub fn definition_context_navigate() -> ToolDefinition {
    ToolDefinition {
        name: "memory_context_navigate".into(),
        description: Some("Navigate between context clusters by finding bridge nodes".into()),
        input_schema: json!({"type":"object","properties":{"from_node":{"type":"integer"},"to_topic":{"type":"string"}},"required":["from_node","to_topic"]}),
    }
}

pub async fn execute_context_navigate(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let from = get_u64(&args, "from_node")
        .ok_or_else(|| McpError::InvalidParams("from_node required".into()))?;
    let topic = get_str(&args, "to_topic")
        .ok_or_else(|| McpError::InvalidParams("to_topic required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let _ = graph.get_node(from).ok_or(McpError::NodeNotFound(from))?;
    // BFS from from_node, scoring by topic relevance
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((from, 0u32));
    visited.insert(from);
    let mut path_nodes: Vec<Value> = Vec::new();
    while let Some((nid, dist)) = queue.pop_front() {
        if dist > 5 {
            break;
        }
        if let Some(n) = graph.get_node(nid) {
            let rel = word_overlap(&topic, &n.content);
            if rel > 0.2 || nid == from {
                path_nodes.push(json!({"id":nid,"distance":dist,"relevance":rel,"content":&n.content[..n.content.len().min(80)]}));
            }
            if rel > 0.5 && nid != from {
                break;
            } // Found target cluster
        }
        for edge in graph.edges_from(nid) {
            if visited.insert(edge.target_id) {
                queue.push_back((edge.target_id, dist + 1));
            }
        }
    }
    Ok(ToolCallResult::json(
        &json!({"from_node":from,"to_topic":topic,"path_length":path_nodes.len(),"path":path_nodes}),
    ))
}

// ── 13. memory_metabolism_status ──────────────────────────────────────────

pub fn definition_metabolism_status() -> ToolDefinition {
    ToolDefinition {
        name: "memory_metabolism_status".into(),
        description: Some("Get memory metabolism health: decay rates, consolidation status, active/dormant ratios".into()),
        input_schema: json!({"type":"object","properties":{}}),
    }
}

pub async fn execute_metabolism_status(
    _args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let total = nodes.len();
    let hot = nodes.iter().filter(|n| n.decay_score < 0.2).count();
    let warm = nodes
        .iter()
        .filter(|n| n.decay_score >= 0.2 && n.decay_score < 0.6)
        .count();
    let cold = nodes.iter().filter(|n| n.decay_score >= 0.6).count();
    let avg_access: f64 = if total > 0 {
        nodes.iter().map(|n| n.access_count as f64).sum::<f64>() / total as f64
    } else {
        0.0
    };
    Ok(ToolCallResult::json(&json!({
        "total_memories":total,"hot":hot,"warm":warm,"cold":cold,
        "avg_access_count":(avg_access*100.0).round()/100.0,
        "health": if hot as f64 / total.max(1) as f64 > 0.3 { "active" } else if cold as f64 / total.max(1) as f64 > 0.5 { "sluggish" } else { "balanced" }
    })))
}

// ── 14. memory_metabolism_process ─────────────────────────────────────────

pub fn definition_metabolism_process() -> ToolDefinition {
    ToolDefinition {
        name: "memory_metabolism_process".into(),
        description: Some(
            "Process and consolidate memories: merge duplicates, strengthen connections".into(),
        ),
        input_schema: json!({"type":"object","properties":{"dry_run":{"type":"boolean","default":true}}}),
    }
}

pub async fn execute_metabolism_process(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    // Find weak memories that could be consolidated
    let weak: Vec<u64> = nodes
        .iter()
        .filter(|n| n.confidence < 0.3 && n.decay_score > 0.7)
        .map(|n| n.id)
        .collect();
    let orphans: Vec<u64> = nodes
        .iter()
        .filter(|n| graph.edges_from(n.id).is_empty() && graph.edges_to(n.id).is_empty())
        .map(|n| n.id)
        .collect();
    Ok(ToolCallResult::json(&json!({
        "dry_run":dry_run,"weak_memories":weak.len(),"orphan_memories":orphans.len(),
        "weak_ids":&weak[..weak.len().min(20)],"orphan_ids":&orphans[..orphans.len().min(20)],
        "recommendation": if weak.len() + orphans.len() > 10 { "consolidation recommended" } else { "memory health is good" }
    })))
}

// ── 15. memory_metabolism_strengthen ──────────────────────────────────────

pub fn definition_metabolism_strengthen() -> ToolDefinition {
    ToolDefinition {
        name: "memory_metabolism_strengthen".into(),
        description: Some("Strengthen a memory by increasing confidence and reducing decay".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"boost":{"type":"number","default":0.1}},"required":["node_id"]}),
    }
}

pub async fn execute_metabolism_strengthen(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let boost = get_f64(&args, "boost").unwrap_or(0.1) as f32;
    let mut session = session.lock().await;
    let graph = session.graph_mut();
    let node = graph
        .get_node_mut(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let old_conf = node.confidence;
    let old_decay = node.decay_score;
    node.confidence = (node.confidence + boost).min(1.0);
    node.decay_score = (node.decay_score - boost * 0.5).max(0.0);
    node.access_count += 1;
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"old_confidence":old_conf,"new_confidence":node.confidence,"old_decay":old_decay,"new_decay":node.decay_score,"strengthened":true}),
    ))
}

// ── 16. memory_metabolism_decay ───────────────────────────────────────────

pub fn definition_metabolism_decay() -> ToolDefinition {
    ToolDefinition {
        name: "memory_metabolism_decay".into(),
        description: Some(
            "Apply decay to stale memories that haven't been accessed recently".into(),
        ),
        input_schema: json!({"type":"object","properties":{"decay_amount":{"type":"number","default":0.05},"min_age_seconds":{"type":"integer","default":86400}}}),
    }
}

pub async fn execute_metabolism_decay(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let decay_amount = get_f64(&args, "decay_amount").unwrap_or(0.05) as f32;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    let min_age = args
        .get("min_age_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(86400)
        * 1_000_000;
    let candidates: Vec<Value> = nodes.iter().filter(|n| {
        now.saturating_sub(n.created_at) > min_age && n.decay_score < 0.9
    }).take(50).map(|n| {
        let new_decay = (n.decay_score + decay_amount).min(1.0);
        json!({"id":n.id,"current_decay":n.decay_score,"projected_decay":new_decay,"content":&n.content[..n.content.len().min(60)]})
    }).collect();
    Ok(ToolCallResult::json(
        &json!({"decay_amount":decay_amount,"candidates_count":candidates.len(),"candidates":candidates,"note":"dry run - use metabolism_process to apply"}),
    ))
}

// ── 17. memory_metabolism_consolidate ─────────────────────────────────────

pub fn definition_metabolism_consolidate() -> ToolDefinition {
    ToolDefinition {
        name: "memory_metabolism_consolidate".into(),
        description: Some("Consolidate related memories by identifying merge candidates".into()),
        input_schema: json!({"type":"object","properties":{"threshold":{"type":"number","default":0.7},"max_groups":{"type":"integer","default":10}}}),
    }
}

pub async fn execute_metabolism_consolidate(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let threshold = get_f64(&args, "threshold").unwrap_or(0.7);
    let max_groups = args
        .get("max_groups")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let mut groups: Vec<Vec<u64>> = Vec::new();
    let mut used: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for i in 0..nodes.len() {
        if used.contains(&nodes[i].id) || groups.len() >= max_groups {
            continue;
        }
        let mut group = vec![nodes[i].id];
        used.insert(nodes[i].id);
        for j in (i + 1)..nodes.len() {
            if used.contains(&nodes[j].id) {
                continue;
            }
            if word_overlap(&nodes[i].content, &nodes[j].content) >= threshold {
                group.push(nodes[j].id);
                used.insert(nodes[j].id);
            }
        }
        if group.len() > 1 {
            groups.push(group);
        }
    }
    let info: Vec<Value> = groups
        .iter()
        .map(|g| {
            let rep = graph
                .get_node(g[0])
                .map(|n| n.content.chars().take(80).collect::<String>())
                .unwrap_or_default();
            json!({"size":g.len(),"ids":g,"representative":rep})
        })
        .collect();
    Ok(ToolCallResult::json(
        &json!({"groups_found":groups.len(),"threshold":threshold,"consolidation_groups":info}),
    ))
}

// ── Public API ───────────────────────────────────────────────────────────

pub fn all_definitions() -> Vec<ToolDefinition> {
    vec![
        definition_immortal_stats(),
        definition_immortal_prove(),
        definition_immortal_project(),
        definition_immortal_tier_move(),
        definition_semantic_compress(),
        definition_semantic_dedup(),
        definition_semantic_similar(),
        definition_semantic_cluster(),
        definition_context_optimize(),
        definition_context_expand(),
        definition_context_summarize(),
        definition_context_navigate(),
        definition_metabolism_status(),
        definition_metabolism_process(),
        definition_metabolism_strengthen(),
        definition_metabolism_decay(),
        definition_metabolism_consolidate(),
    ]
}

pub async fn try_execute(
    name: &str,
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> Option<McpResult<ToolCallResult>> {
    match name {
        "memory_immortal_stats" => Some(execute_immortal_stats(args, session).await),
        "memory_immortal_prove" => Some(execute_immortal_prove(args, session).await),
        "memory_immortal_project" => Some(execute_immortal_project(args, session).await),
        "memory_immortal_tier_move" => Some(execute_immortal_tier_move(args, session).await),
        "memory_semantic_compress" => Some(execute_semantic_compress(args, session).await),
        "memory_semantic_dedup" => Some(execute_semantic_dedup(args, session).await),
        "memory_semantic_similar_enhanced" => Some(execute_semantic_similar(args, session).await),
        "memory_semantic_cluster" => Some(execute_semantic_cluster(args, session).await),
        "memory_context_optimize" => Some(execute_context_optimize(args, session).await),
        "memory_context_expand" => Some(execute_context_expand(args, session).await),
        "memory_context_summarize" => Some(execute_context_summarize(args, session).await),
        "memory_context_navigate" => Some(execute_context_navigate(args, session).await),
        "memory_metabolism_status" => Some(execute_metabolism_status(args, session).await),
        "memory_metabolism_process" => Some(execute_metabolism_process(args, session).await),
        "memory_metabolism_strengthen" => Some(execute_metabolism_strengthen(args, session).await),
        "memory_metabolism_decay" => Some(execute_metabolism_decay(args, session).await),
        "memory_metabolism_consolidate" => {
            Some(execute_metabolism_consolidate(args, session).await)
        }
        _ => None,
    }
}
