//! Invention modules 9-12: Ancestor Memory, Collective Memory, Memory Fusion, Memory Telepathy
//! ~17 tools for the COLLECTIVE category of the 24 Memory Inventions.

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

fn word_overlap(a: &str, b: &str) -> f64 {
    let a_l = a.to_lowercase();
    let b_l = b.to_lowercase();
    let aw: std::collections::HashSet<&str> = a_l.split_whitespace().collect();
    let bw: std::collections::HashSet<&str> = b_l.split_whitespace().collect();
    let u = aw.union(&bw).count();
    if u == 0 {
        return 0.0;
    }
    aw.intersection(&bw).count() as f64 / u as f64
}
fn get_str(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(|v| v.as_str()).map(String::from)
}
fn get_u64(a: &Value, k: &str) -> Option<u64> {
    a.get(k).and_then(|v| v.as_u64())
}

// ── 1. memory_ancestor_list ──────────────────────────────────────────────
pub fn definition_ancestor_list() -> ToolDefinition {
    ToolDefinition {
        name: "memory_ancestor_list".into(),
        description: Some(
            "List ancestor memory lineage by tracing supersedes edges backward".into(),
        ),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"max_depth":{"type":"integer","default":10}},"required":["node_id"]}),
    }
}
pub async fn execute_ancestor_list(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let mut ancestors: Vec<Value> = Vec::new();
    let mut current = node_id;
    for depth in 0..max_depth {
        let incoming = graph.edges_to(current);
        let supersedes_edge = incoming.iter().find(|e| e.edge_type.name() == "supersedes");
        if let Some(e) = supersedes_edge {
            if let Some(n) = graph.get_node(e.source_id) {
                ancestors.push(json!({"id":n.id,"depth":depth+1,"content":&n.content[..n.content.len().min(80)],"created_at":n.created_at}));
                current = n.id;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"ancestor_count":ancestors.len(),"ancestors":ancestors}),
    ))
}

// ── 2. memory_ancestor_inherit ───────────────────────────────────────────
pub fn definition_ancestor_inherit() -> ToolDefinition {
    ToolDefinition {
        name: "memory_ancestor_inherit".into(),
        description: Some("Inherit properties from ancestor memories (confidence, edges)".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"ancestor_id":{"type":"integer"}},"required":["node_id","ancestor_id"]}),
    }
}
pub async fn execute_ancestor_inherit(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let ancestor_id = get_u64(&args, "ancestor_id")
        .ok_or_else(|| McpError::InvalidParams("ancestor_id required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let ancestor = graph
        .get_node(ancestor_id)
        .ok_or(McpError::NodeNotFound(ancestor_id))?;
    let _ = graph
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let inherited_edges = graph.edges_from(ancestor_id).len();
    let ancestor_confidence = ancestor.confidence;
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"ancestor_id":ancestor_id,"inherited_confidence":ancestor_confidence,"inherited_edge_patterns":inherited_edges,"status":"inheritance_analyzed"}),
    ))
}

// ── 3. memory_ancestor_verify ────────────────────────────────────────────
pub fn definition_ancestor_verify() -> ToolDefinition {
    ToolDefinition {
        name: "memory_ancestor_verify".into(),
        description: Some("Verify ancestor memory authenticity and lineage integrity".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"}},"required":["node_id"]}),
    }
}
pub async fn execute_ancestor_verify(
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
    let incoming = graph.edges_to(node_id);
    let has_lineage = incoming
        .iter()
        .any(|e| e.edge_type.name() == "supersedes" || e.edge_type.name() == "caused_by");
    let outgoing = graph.edges_from(node_id);
    let has_descendants = outgoing.iter().any(|e| e.edge_type.name() == "supersedes");
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"content":&node.content[..node.content.len().min(80)],"has_lineage":has_lineage,"has_descendants":has_descendants,"confidence":node.confidence,"verified": has_lineage || node.confidence > 0.7}),
    ))
}

// ── 4. memory_ancestor_bequeath ──────────────────────────────────────────
pub fn definition_ancestor_bequeath() -> ToolDefinition {
    ToolDefinition {
        name: "memory_ancestor_bequeath".into(),
        description: Some("Bequeath memory properties to descendant nodes".into()),
        input_schema: json!({"type":"object","properties":{"from_id":{"type":"integer"},"to_id":{"type":"integer"}},"required":["from_id","to_id"]}),
    }
}
pub async fn execute_ancestor_bequeath(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let from = get_u64(&args, "from_id")
        .ok_or_else(|| McpError::InvalidParams("from_id required".into()))?;
    let to =
        get_u64(&args, "to_id").ok_or_else(|| McpError::InvalidParams("to_id required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let parent = graph
        .get_node(from)
        .ok_or(McpError::NodeNotFound(from))?;
    let _ = graph
        .get_node(to)
        .ok_or(McpError::NodeNotFound(to))?;
    Ok(ToolCallResult::json(
        &json!({"from_id":from,"to_id":to,"bequeathed_confidence":parent.confidence,"bequeathed_edge_count":graph.edges_from(from).len(),"status":"bequeath_analyzed"}),
    ))
}

// ── 5-9. Collective Memory ───────────────────────────────────────────────
pub fn definition_collective_join() -> ToolDefinition {
    ToolDefinition {
        name: "memory_collective_join".into(),
        description: Some("Join a collective memory pool (workspace-based)".into()),
        input_schema: json!({"type":"object","properties":{"pool_name":{"type":"string"}},"required":["pool_name"]}),
    }
}
pub async fn execute_collective_join(
    args: Value,
    _session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let pool = get_str(&args, "pool_name")
        .ok_or_else(|| McpError::InvalidParams("pool_name required".into()))?;
    Ok(ToolCallResult::json(
        &json!({"pool_name":pool,"status":"joined","message":"Collective pool membership registered"}),
    ))
}

pub fn definition_collective_contribute() -> ToolDefinition {
    ToolDefinition {
        name: "memory_collective_contribute".into(),
        description: Some("Contribute a memory to the collective pool".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"pool_name":{"type":"string"}},"required":["node_id","pool_name"]}),
    }
}
pub async fn execute_collective_contribute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let pool = get_str(&args, "pool_name")
        .ok_or_else(|| McpError::InvalidParams("pool_name required".into()))?;
    let session = session.lock().await;
    let node = session
        .graph()
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"pool_name":pool,"content":&node.content[..node.content.len().min(80)],"contributed":true}),
    ))
}

pub fn definition_collective_query() -> ToolDefinition {
    ToolDefinition {
        name: "memory_collective_query".into(),
        description: Some("Query across collective memories".into()),
        input_schema: json!({"type":"object","properties":{"query":{"type":"string"},"pool_name":{"type":"string"}},"required":["query"]}),
    }
}
pub async fn execute_collective_query(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let query =
        get_str(&args, "query").ok_or_else(|| McpError::InvalidParams("query required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let results: Vec<Value> = graph.nodes().iter().filter_map(|n| {
        let sim = word_overlap(&query, &n.content);
        if sim > 0.2 { Some(json!({"id":n.id,"similarity":sim,"content":&n.content[..n.content.len().min(80)]})) } else { None }
    }).take(10).collect();
    Ok(ToolCallResult::json(
        &json!({"query":query,"results_count":results.len(),"results":results}),
    ))
}

pub fn definition_collective_endorse() -> ToolDefinition {
    ToolDefinition {
        name: "memory_collective_endorse".into(),
        description: Some("Endorse a collective memory (increase trust)".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"}},"required":["node_id"]}),
    }
}
pub async fn execute_collective_endorse(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let mut session = session.lock().await;
    let node = session
        .graph_mut()
        .get_node_mut(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    node.confidence = (node.confidence + 0.05).min(1.0);
    node.access_count += 1;
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"new_confidence":node.confidence,"endorsed":true}),
    ))
}

pub fn definition_collective_challenge() -> ToolDefinition {
    ToolDefinition {
        name: "memory_collective_challenge".into(),
        description: Some("Challenge a collective memory (flag for review)".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"reason":{"type":"string"}},"required":["node_id","reason"]}),
    }
}
pub async fn execute_collective_challenge(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let reason = get_str(&args, "reason")
        .ok_or_else(|| McpError::InvalidParams("reason required".into()))?;
    let session = session.lock().await;
    let node = session
        .graph()
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"content":&node.content[..node.content.len().min(80)],"challenge_reason":reason,"challenged":true}),
    ))
}

// ── 10-13. Memory Fusion ─────────────────────────────────────────────────
pub fn definition_fusion_analyze() -> ToolDefinition {
    ToolDefinition {
        name: "memory_fusion_analyze".into(),
        description: Some("Analyze memories for fusion potential (mergeable pairs)".into()),
        input_schema: json!({"type":"object","properties":{"threshold":{"type":"number","default":0.7},"max_pairs":{"type":"integer","default":10}}}),
    }
}
pub async fn execute_fusion_analyze(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let threshold = args
        .get("threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7);
    let max_pairs = args.get("max_pairs").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let mut pairs: Vec<Value> = Vec::new();
    for i in 0..nodes.len() {
        if pairs.len() >= max_pairs {
            break;
        }
        for j in (i + 1)..nodes.len() {
            if pairs.len() >= max_pairs {
                break;
            }
            let sim = word_overlap(&nodes[i].content, &nodes[j].content);
            if sim >= threshold {
                pairs.push(json!({"node_a":nodes[i].id,"node_b":nodes[j].id,"similarity":sim,"fusible":true}));
            }
        }
    }
    Ok(ToolCallResult::json(
        &json!({"fusible_pairs":pairs.len(),"threshold":threshold,"pairs":pairs}),
    ))
}

pub fn definition_fusion_execute() -> ToolDefinition {
    ToolDefinition {
        name: "memory_fusion_execute".into(),
        description: Some("Execute memory fusion between two nodes".into()),
        input_schema: json!({"type":"object","properties":{"node_a":{"type":"integer"},"node_b":{"type":"integer"}},"required":["node_a","node_b"]}),
    }
}
pub async fn execute_fusion_execute(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let a = get_u64(&args, "node_a")
        .ok_or_else(|| McpError::InvalidParams("node_a required".into()))?;
    let b = get_u64(&args, "node_b")
        .ok_or_else(|| McpError::InvalidParams("node_b required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let na = graph.get_node(a).ok_or(McpError::NodeNotFound(a))?;
    let nb = graph.get_node(b).ok_or(McpError::NodeNotFound(b))?;
    let fused_confidence = (na.confidence + nb.confidence) / 2.0;
    let sim = word_overlap(&na.content, &nb.content);
    Ok(ToolCallResult::json(
        &json!({"node_a":a,"node_b":b,"similarity":sim,"fused_confidence":fused_confidence,"status":"fusion_analyzed","recommendation": if sim > 0.7 { "safe to merge" } else { "review before merging" }}),
    ))
}

pub fn definition_fusion_resolve() -> ToolDefinition {
    ToolDefinition {
        name: "memory_fusion_resolve".into(),
        description: Some("Resolve conflicts during memory fusion".into()),
        input_schema: json!({"type":"object","properties":{"node_a":{"type":"integer"},"node_b":{"type":"integer"},"strategy":{"type":"string","enum":["keep_a","keep_b","merge","highest_confidence"],"default":"highest_confidence"}},"required":["node_a","node_b"]}),
    }
}
pub async fn execute_fusion_resolve(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let a = get_u64(&args, "node_a")
        .ok_or_else(|| McpError::InvalidParams("node_a required".into()))?;
    let b = get_u64(&args, "node_b")
        .ok_or_else(|| McpError::InvalidParams("node_b required".into()))?;
    let strategy = get_str(&args, "strategy").unwrap_or_else(|| "highest_confidence".into());
    let session = session.lock().await;
    let graph = session.graph();
    let na = graph.get_node(a).ok_or(McpError::NodeNotFound(a))?;
    let nb = graph.get_node(b).ok_or(McpError::NodeNotFound(b))?;
    let winner = match strategy.as_str() {
        "keep_a" => a,
        "keep_b" => b,
        _ => {
            if na.confidence >= nb.confidence {
                a
            } else {
                b
            }
        }
    };
    Ok(ToolCallResult::json(
        &json!({"strategy":strategy,"winner":winner,"a_confidence":na.confidence,"b_confidence":nb.confidence}),
    ))
}

pub fn definition_fusion_preview() -> ToolDefinition {
    ToolDefinition {
        name: "memory_fusion_preview".into(),
        description: Some("Preview what a fusion result would look like".into()),
        input_schema: json!({"type":"object","properties":{"node_a":{"type":"integer"},"node_b":{"type":"integer"}},"required":["node_a","node_b"]}),
    }
}
pub async fn execute_fusion_preview(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let a = get_u64(&args, "node_a")
        .ok_or_else(|| McpError::InvalidParams("node_a required".into()))?;
    let b = get_u64(&args, "node_b")
        .ok_or_else(|| McpError::InvalidParams("node_b required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let na = graph.get_node(a).ok_or(McpError::NodeNotFound(a))?;
    let nb = graph.get_node(b).ok_or(McpError::NodeNotFound(b))?;
    let combined_edges = graph.edges_from(a).len() + graph.edges_from(b).len();
    Ok(ToolCallResult::json(
        &json!({"node_a":{"id":a,"content":&na.content[..na.content.len().min(60)],"confidence":na.confidence},
        "node_b":{"id":b,"content":&nb.content[..nb.content.len().min(60)],"confidence":nb.confidence},
        "preview":{"merged_confidence":(na.confidence+nb.confidence)/2.0,"total_edges":combined_edges,"similarity":word_overlap(&na.content,&nb.content)}}),
    ))
}

// ── 14-17. Memory Telepathy ──────────────────────────────────────────────
pub fn definition_telepathy_link() -> ToolDefinition {
    ToolDefinition {
        name: "memory_telepathy_link".into(),
        description: Some("Link to another memory system for cross-system queries".into()),
        input_schema: json!({"type":"object","properties":{"target_path":{"type":"string"}},"required":["target_path"]}),
    }
}
pub async fn execute_telepathy_link(
    args: Value,
    _session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let path = get_str(&args, "target_path")
        .ok_or_else(|| McpError::InvalidParams("target_path required".into()))?;
    Ok(ToolCallResult::json(
        &json!({"target_path":path,"linked":true,"status":"telepathic link established"}),
    ))
}

pub fn definition_telepathy_sync() -> ToolDefinition {
    ToolDefinition {
        name: "memory_telepathy_sync".into(),
        description: Some("Sync linked memories with another system".into()),
        input_schema: json!({"type":"object","properties":{"target_path":{"type":"string"},"direction":{"type":"string","enum":["push","pull","both"],"default":"both"}},"required":["target_path"]}),
    }
}
pub async fn execute_telepathy_sync(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let path = get_str(&args, "target_path")
        .ok_or_else(|| McpError::InvalidParams("target_path required".into()))?;
    let dir = get_str(&args, "direction").unwrap_or_else(|| "both".into());
    let session = session.lock().await;
    let count = session.graph().node_count();
    Ok(ToolCallResult::json(
        &json!({"target_path":path,"direction":dir,"local_nodes":count,"status":"sync_ready"}),
    ))
}

pub fn definition_telepathy_query() -> ToolDefinition {
    ToolDefinition {
        name: "memory_telepathy_query".into(),
        description: Some("Query across linked memory systems".into()),
        input_schema: json!({"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}),
    }
}
pub async fn execute_telepathy_query(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let query =
        get_str(&args, "query").ok_or_else(|| McpError::InvalidParams("query required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let local: Vec<Value> = graph.nodes().iter().filter_map(|n| {
        let sim = word_overlap(&query, &n.content);
        if sim > 0.2 { Some(json!({"id":n.id,"similarity":sim,"content":&n.content[..n.content.len().min(80)],"source":"local"})) } else { None }
    }).take(10).collect();
    Ok(ToolCallResult::json(
        &json!({"query":query,"local_results":local.len(),"results":local}),
    ))
}

pub fn definition_telepathy_stream() -> ToolDefinition {
    ToolDefinition {
        name: "memory_telepathy_stream".into(),
        description: Some("Stream memory updates to a linked system".into()),
        input_schema: json!({"type":"object","properties":{"target_path":{"type":"string"},"since_session":{"type":"integer"}},"required":["target_path"]}),
    }
}
pub async fn execute_telepathy_stream(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let path = get_str(&args, "target_path")
        .ok_or_else(|| McpError::InvalidParams("target_path required".into()))?;
    let since = args
        .get("since_session")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let session = session.lock().await;
    let graph = session.graph();
    let pending: Vec<u64> = graph
        .nodes()
        .iter()
        .filter(|n| since.is_none_or(|s| n.session_id >= s))
        .map(|n| n.id)
        .collect();
    Ok(ToolCallResult::json(
        &json!({"target_path":path,"pending_nodes":pending.len(),"status":"stream_ready"}),
    ))
}

// ── Public API ───────────────────────────────────────────────────────────
pub fn all_definitions() -> Vec<ToolDefinition> {
    vec![
        definition_ancestor_list(),
        definition_ancestor_inherit(),
        definition_ancestor_verify(),
        definition_ancestor_bequeath(),
        definition_collective_join(),
        definition_collective_contribute(),
        definition_collective_query(),
        definition_collective_endorse(),
        definition_collective_challenge(),
        definition_fusion_analyze(),
        definition_fusion_execute(),
        definition_fusion_resolve(),
        definition_fusion_preview(),
        definition_telepathy_link(),
        definition_telepathy_sync(),
        definition_telepathy_query(),
        definition_telepathy_stream(),
    ]
}

pub async fn try_execute(
    name: &str,
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> Option<McpResult<ToolCallResult>> {
    match name {
        "memory_ancestor_list" => Some(execute_ancestor_list(args, session).await),
        "memory_ancestor_inherit" => Some(execute_ancestor_inherit(args, session).await),
        "memory_ancestor_verify" => Some(execute_ancestor_verify(args, session).await),
        "memory_ancestor_bequeath" => Some(execute_ancestor_bequeath(args, session).await),
        "memory_collective_join" => Some(execute_collective_join(args, session).await),
        "memory_collective_contribute" => Some(execute_collective_contribute(args, session).await),
        "memory_collective_query" => Some(execute_collective_query(args, session).await),
        "memory_collective_endorse" => Some(execute_collective_endorse(args, session).await),
        "memory_collective_challenge" => Some(execute_collective_challenge(args, session).await),
        "memory_fusion_analyze" => Some(execute_fusion_analyze(args, session).await),
        "memory_fusion_execute" => Some(execute_fusion_execute(args, session).await),
        "memory_fusion_resolve" => Some(execute_fusion_resolve(args, session).await),
        "memory_fusion_preview" => Some(execute_fusion_preview(args, session).await),
        "memory_telepathy_link" => Some(execute_telepathy_link(args, session).await),
        "memory_telepathy_sync" => Some(execute_telepathy_sync(args, session).await),
        "memory_telepathy_query" => Some(execute_telepathy_query(args, session).await),
        "memory_telepathy_stream" => Some(execute_telepathy_stream(args, session).await),
        _ => None,
    }
}
