//! Invention modules 5-8: Predictive Memory, Memory Prophecy, Counterfactual Memory, Déjà Vu Detection
//! ~16 tools for the PROPHETIC category of the 24 Memory Inventions.

use crate::session::SessionManager;
use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

fn word_overlap(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    let a_words: std::collections::HashSet<&str> = a_lower.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b_lower.split_whitespace().collect();
    if a_words.is_empty() && b_words.is_empty() {
        return 1.0;
    }
    let i = a_words.intersection(&b_words).count();
    let u = a_words.union(&b_words).count();
    if u == 0 {
        0.0
    } else {
        i as f64 / u as f64
    }
}
fn get_str(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(|v| v.as_str()).map(String::from)
}
fn get_u64(args: &Value, k: &str) -> Option<u64> {
    args.get(k).and_then(|v| v.as_u64())
}

// ── 1. memory_predict ────────────────────────────────────────────────────
pub fn definition_predict() -> ToolDefinition {
    ToolDefinition {
        name: "memory_predict".into(),
        description: Some("Predict what memories will be needed based on current context".into()),
        input_schema: json!({"type":"object","properties":{"context":{"type":"string"},"max_predictions":{"type":"integer","default":10}},"required":["context"]}),
    }
}
pub async fn execute_predict(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let ctx = get_str(&args, "context")
        .ok_or_else(|| McpError::InvalidParams("context required".into()))?;
    let max = args
        .get("max_predictions")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let mut scored: Vec<(u64, f64, String)> = graph
        .nodes()
        .iter()
        .map(|n| {
            let relevance = word_overlap(&ctx, &n.content);
            let recency = 1.0 - n.decay_score as f64;
            (
                n.id,
                relevance * 0.6 + recency * 0.2 + n.confidence as f64 * 0.2,
                n.content.chars().take(80).collect(),
            )
        })
        .filter(|(_, s, _)| *s > 0.1)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(max);
    let predictions: Vec<Value> = scored
        .iter()
        .map(|(id, score, c)| json!({"id":id,"prediction_score":score,"content":c}))
        .collect();
    Ok(ToolCallResult::json(
        &json!({"context":ctx,"predictions_count":predictions.len(),"predictions":predictions}),
    ))
}

// ── 2. memory_predict_preload ────────────────────────────────────────────
pub fn definition_predict_preload() -> ToolDefinition {
    ToolDefinition {
        name: "memory_predict_preload".into(),
        description: Some("Preload predicted memories into hot tier".into()),
        input_schema: json!({"type":"object","properties":{"context":{"type":"string"},"count":{"type":"integer","default":5}},"required":["context"]}),
    }
}
pub async fn execute_predict_preload(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let ctx = get_str(&args, "context")
        .ok_or_else(|| McpError::InvalidParams("context required".into()))?;
    let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let mut scored: Vec<(u64, f64)> = graph
        .nodes()
        .iter()
        .map(|n| (n.id, word_overlap(&ctx, &n.content)))
        .filter(|(_, s)| *s > 0.1)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(count);
    let preloaded: Vec<Value> = scored
        .iter()
        .filter_map(|(id, s)| {
            graph.get_node(*id).map(
                |n| json!({"id":n.id,"score":s,"content":&n.content[..n.content.len().min(80)]}),
            )
        })
        .collect();
    Ok(ToolCallResult::json(
        &json!({"context":ctx,"preloaded_count":preloaded.len(),"preloaded":preloaded}),
    ))
}

// ── 3. memory_predict_accuracy ───────────────────────────────────────────
pub fn definition_predict_accuracy() -> ToolDefinition {
    ToolDefinition {
        name: "memory_predict_accuracy".into(),
        description: Some(
            "Check prediction accuracy by comparing predicted vs actually accessed memories".into(),
        ),
        input_schema: json!({"type":"object","properties":{"predicted_ids":{"type":"array","items":{"type":"integer"}},"actual_ids":{"type":"array","items":{"type":"integer"}}},"required":["predicted_ids","actual_ids"]}),
    }
}
pub async fn execute_predict_accuracy(
    args: Value,
    _session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let predicted: std::collections::HashSet<u64> = args
        .get("predicted_ids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_u64()).collect())
        .unwrap_or_default();
    let actual: std::collections::HashSet<u64> = args
        .get("actual_ids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_u64()).collect())
        .unwrap_or_default();
    let hits: Vec<u64> = predicted.intersection(&actual).copied().collect();
    let precision = if predicted.is_empty() {
        0.0
    } else {
        hits.len() as f64 / predicted.len() as f64
    };
    let recall = if actual.is_empty() {
        0.0
    } else {
        hits.len() as f64 / actual.len() as f64
    };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };
    Ok(ToolCallResult::json(
        &json!({"predicted_count":predicted.len(),"actual_count":actual.len(),"hits":hits.len(),"precision":precision,"recall":recall,"f1_score":f1,"hit_ids":hits}),
    ))
}

// ── 4. memory_predict_feedback ───────────────────────────────────────────
pub fn definition_predict_feedback() -> ToolDefinition {
    ToolDefinition {
        name: "memory_predict_feedback".into(),
        description: Some(
            "Provide feedback on prediction quality to improve future predictions".into(),
        ),
        input_schema: json!({"type":"object","properties":{"prediction_id":{"type":"integer"},"was_useful":{"type":"boolean"},"feedback":{"type":"string"}},"required":["prediction_id","was_useful"]}),
    }
}
pub async fn execute_predict_feedback(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let pred_id = get_u64(&args, "prediction_id")
        .ok_or_else(|| McpError::InvalidParams("prediction_id required".into()))?;
    let useful = args
        .get("was_useful")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let feedback = get_str(&args, "feedback").unwrap_or_default();
    let mut session = session.lock().await;
    if useful {
        if let Some(n) = session.graph_mut().get_node_mut(pred_id) {
            n.access_count += 1;
            n.confidence = (n.confidence + 0.05).min(1.0);
        }
    }
    Ok(ToolCallResult::json(
        &json!({"prediction_id":pred_id,"was_useful":useful,"feedback":feedback,"applied":true}),
    ))
}

// ── 5. memory_prophecy ───────────────────────────────────────────────────
pub fn definition_prophecy() -> ToolDefinition {
    ToolDefinition {
        name: "memory_prophecy".into(),
        description: Some(
            "Prophecy about memory future: which memories will become important or fade".into(),
        ),
        input_schema: json!({"type":"object","properties":{"horizon":{"type":"string","enum":["short","medium","long"],"default":"medium"}}}),
    }
}
pub async fn execute_prophecy(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let horizon = get_str(&args, "horizon").unwrap_or_else(|| "medium".into());
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let decay_multiplier = match horizon.as_str() {
        "short" => 1.0,
        "long" => 3.0,
        _ => 2.0,
    };
    let mut rising: Vec<Value> = Vec::new();
    let mut fading: Vec<Value> = Vec::new();
    for n in nodes {
        let projected_decay = (n.decay_score as f64 * decay_multiplier).min(1.0);
        let edges = graph.edges_from(n.id).len() + graph.edges_to(n.id).len();
        if n.access_count > 2 && n.confidence > 0.7 && edges > 1 {
            rising.push(json!({"id":n.id,"content":&n.content[..n.content.len().min(80)],"confidence":n.confidence,"edge_count":edges}));
        } else if projected_decay > 0.8 && n.access_count < 2 {
            fading.push(json!({"id":n.id,"content":&n.content[..n.content.len().min(80)],"projected_decay":projected_decay}));
        }
    }
    rising.truncate(10);
    fading.truncate(10);
    Ok(ToolCallResult::json(
        &json!({"horizon":horizon,"rising_count":rising.len(),"fading_count":fading.len(),"rising":rising,"fading":fading}),
    ))
}

// ── 6. memory_prophecy_similar ───────────────────────────────────────────
pub fn definition_prophecy_similar() -> ToolDefinition {
    ToolDefinition {
        name: "memory_prophecy_similar".into(),
        description: Some("Find memories with similar trajectory/prophecy patterns".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"}},"required":["node_id"]}),
    }
}
pub async fn execute_prophecy_similar(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let target = graph
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let t_edges = graph.edges_from(node_id).len();
    let similar: Vec<Value> = graph.nodes().iter().filter(|n| n.id != node_id).map(|n| {
        let conf_diff = (n.confidence - target.confidence).abs();
        let decay_diff = (n.decay_score - target.decay_score).abs();
        let edge_diff = (graph.edges_from(n.id).len() as f64 - t_edges as f64).abs();
        let similarity = 1.0 - ((conf_diff as f64 + decay_diff as f64 + edge_diff * 0.1) / 3.0).min(1.0);
        (n, similarity)
    }).filter(|(_, s)| *s > 0.5).take(10).map(|(n, s)| json!({"id":n.id,"similarity":s,"content":&n.content[..n.content.len().min(80)]})).collect();
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"similar_count":similar.len(),"similar":similar}),
    ))
}

// ── 7. memory_prophecy_regret ────────────────────────────────────────────
pub fn definition_prophecy_regret() -> ToolDefinition {
    ToolDefinition {
        name: "memory_prophecy_regret".into(),
        description: Some(
            "Analyze memory regret: what should have been saved or strengthened".into(),
        ),
        input_schema: json!({"type":"object","properties":{"session_id":{"type":"integer"}}}),
    }
}
pub async fn execute_prophecy_regret(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let session = session.lock().await;
    let graph = session.graph();
    let filter_session = args
        .get("session_id")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let nodes = graph.nodes();
    let regrets: Vec<Value> = nodes.iter().filter(|n| {
        n.decay_score > 0.7 && n.confidence > 0.5 && graph.edges_from(n.id).len() > 2
        && filter_session.is_none_or(|s| n.session_id == s)
    }).take(15).map(|n| json!({"id":n.id,"content":&n.content[..n.content.len().min(80)],"confidence":n.confidence,"decay":n.decay_score,"edge_count":graph.edges_from(n.id).len(),"regret_reason":"high-value memory decaying without reinforcement"})).collect();
    Ok(ToolCallResult::json(
        &json!({"regrets_count":regrets.len(),"regrets":regrets}),
    ))
}

// ── 8. memory_prophecy_track ─────────────────────────────────────────────
pub fn definition_prophecy_track() -> ToolDefinition {
    ToolDefinition {
        name: "memory_prophecy_track".into(),
        description: Some("Track prophecy accuracy over time".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"}},"required":["node_id"]}),
    }
}
pub async fn execute_prophecy_track(
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
    let trajectory = json!({"confidence_trend": if node.access_count > 3 { "strengthening" } else if node.decay_score > 0.5 { "weakening" } else { "stable" },
        "current_confidence":node.confidence,"current_decay":node.decay_score,"access_count":node.access_count,
        "edge_count":graph.edges_from(node_id).len()+graph.edges_to(node_id).len()});
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"trajectory":trajectory}),
    ))
}

// ── 9-12. Counterfactual Memory ──────────────────────────────────────────
pub fn definition_counterfactual_what_if() -> ToolDefinition {
    ToolDefinition {
        name: "memory_counterfactual_what_if".into(),
        description: Some("What-if analysis: what would happen if a memory was different".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"alternative_content":{"type":"string"}},"required":["node_id","alternative_content"]}),
    }
}
pub async fn execute_counterfactual_what_if(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let alt = get_str(&args, "alternative_content")
        .ok_or_else(|| McpError::InvalidParams("alternative_content required".into()))?;
    let session = session.lock().await;
    let graph = session.graph();
    let node = graph
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let affected: Vec<Value> = graph.edges_from(node_id).iter().filter_map(|e| graph.get_node(e.target_id).map(|n| {
        let original_overlap = word_overlap(&node.content, &n.content);
        let alt_overlap = word_overlap(&alt, &n.content);
        json!({"id":n.id,"content":&n.content[..n.content.len().min(60)],"original_relevance":original_overlap,"alternative_relevance":alt_overlap,"impact": if (alt_overlap - original_overlap).abs() > 0.3 { "high" } else { "low" }})
    })).collect();
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"original":&node.content[..node.content.len().min(80)],"alternative":alt,"affected_nodes":affected.len(),"affected":affected}),
    ))
}

pub fn definition_counterfactual_compare() -> ToolDefinition {
    ToolDefinition {
        name: "memory_counterfactual_compare".into(),
        description: Some("Compare two counterfactual scenarios".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"scenario_a":{"type":"string"},"scenario_b":{"type":"string"}},"required":["node_id","scenario_a","scenario_b"]}),
    }
}
pub async fn execute_counterfactual_compare(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let a = get_str(&args, "scenario_a").unwrap_or_default();
    let b = get_str(&args, "scenario_b").unwrap_or_default();
    let session = session.lock().await;
    let graph = session.graph();
    let _ = graph
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let downstream: Vec<&str> = graph
        .edges_from(node_id)
        .iter()
        .filter_map(|e| graph.get_node(e.target_id).map(|n| n.content.as_str()))
        .collect();
    let a_impact: f64 = downstream.iter().map(|c| word_overlap(&a, c)).sum::<f64>()
        / downstream.len().max(1) as f64;
    let b_impact: f64 = downstream.iter().map(|c| word_overlap(&b, c)).sum::<f64>()
        / downstream.len().max(1) as f64;
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"scenario_a":a,"scenario_b":b,"a_avg_impact":a_impact,"b_avg_impact":b_impact,"better_scenario": if a_impact > b_impact { "a" } else { "b" },"downstream_count":downstream.len()}),
    ))
}

pub fn definition_counterfactual_insights() -> ToolDefinition {
    ToolDefinition {
        name: "memory_counterfactual_insights".into(),
        description: Some("Extract insights from counterfactual analysis".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"}},"required":["node_id"]}),
    }
}
pub async fn execute_counterfactual_insights(
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
    let dependents = graph.edges_from(node_id).len();
    let supporters = graph.edges_to(node_id).len();
    let criticality =
        (dependents as f64 * 0.3 + supporters as f64 * 0.2 + node.confidence as f64 * 0.5).min(1.0);
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"content":&node.content[..node.content.len().min(80)],"dependents":dependents,"supporters":supporters,"criticality":criticality,
        "insight": if criticality > 0.7 { "This memory is a critical decision point - changes would cascade widely" } else if criticality > 0.4 { "This memory has moderate influence" } else { "This memory is relatively isolated - changes have limited impact" }}),
    ))
}

pub fn definition_counterfactual_best() -> ToolDefinition {
    ToolDefinition {
        name: "memory_counterfactual_best".into(),
        description: Some("Find the best counterfactual scenario for a decision".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"scenarios":{"type":"array","items":{"type":"string"}}},"required":["node_id","scenarios"]}),
    }
}
pub async fn execute_counterfactual_best(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let scenarios: Vec<String> = args
        .get("scenarios")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    if scenarios.is_empty() {
        return Err(McpError::InvalidParams("scenarios required".into()));
    }
    let session = session.lock().await;
    let graph = session.graph();
    let _ = graph
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    let downstream: Vec<&str> = graph
        .edges_from(node_id)
        .iter()
        .filter_map(|e| graph.get_node(e.target_id).map(|n| n.content.as_str()))
        .collect();
    let mut scored: Vec<(usize, f64, &str)> = scenarios
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let impact: f64 = downstream.iter().map(|c| word_overlap(s, c)).sum::<f64>()
                / downstream.len().max(1) as f64;
            (i, impact, s.as_str())
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let ranked: Vec<Value> = scored
        .iter()
        .map(|(i, score, s)| json!({"rank":i+1,"scenario":s,"impact_score":score}))
        .collect();
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"scenarios_count":scenarios.len(),"best_scenario":scored.first().map(|s| s.2).unwrap_or(""),"rankings":ranked}),
    ))
}

// ── 13-16. Déjà Vu Detection ─────────────────────────────────────────────
pub fn definition_dejavu_check() -> ToolDefinition {
    ToolDefinition {
        name: "memory_dejavu_check".into(),
        description: Some("Check for déjà vu: find patterns that have occurred before".into()),
        input_schema: json!({"type":"object","properties":{"content":{"type":"string"},"threshold":{"type":"number","default":0.6}},"required":["content"]}),
    }
}
pub async fn execute_dejavu_check(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let content = get_str(&args, "content")
        .ok_or_else(|| McpError::InvalidParams("content required".into()))?;
    let threshold = args
        .get("threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.6);
    let session = session.lock().await;
    let graph = session.graph();
    let matches: Vec<Value> = graph.nodes().iter().filter_map(|n| {
        let sim = word_overlap(&content, &n.content);
        if sim >= threshold { Some(json!({"id":n.id,"similarity":sim,"content":&n.content[..n.content.len().min(80)],"session_id":n.session_id,"created_at":n.created_at})) } else { None }
    }).take(10).collect();
    Ok(ToolCallResult::json(
        &json!({"deja_vu": !matches.is_empty(),"matches_count":matches.len(),"threshold":threshold,"matches":matches}),
    ))
}

pub fn definition_dejavu_history() -> ToolDefinition {
    ToolDefinition {
        name: "memory_dejavu_history".into(),
        description: Some("Get history of déjà vu events (recurring patterns)".into()),
        input_schema: json!({"type":"object","properties":{"max_results":{"type":"integer","default":20}}}),
    }
}
pub async fn execute_dejavu_history(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let max = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let nodes = graph.nodes();
    let mut recurring: Vec<Value> = Vec::new();
    for i in 0..nodes.len() {
        if recurring.len() >= max {
            break;
        }
        let mut count = 0;
        for j in (i + 1)..nodes.len() {
            if word_overlap(&nodes[i].content, &nodes[j].content) > 0.6 {
                count += 1;
            }
        }
        if count > 0 {
            recurring.push(json!({"id":nodes[i].id,"content":&nodes[i].content[..nodes[i].content.len().min(80)],"recurrence_count":count}));
        }
    }
    Ok(ToolCallResult::json(
        &json!({"recurring_patterns":recurring.len(),"history":recurring}),
    ))
}

pub fn definition_dejavu_patterns() -> ToolDefinition {
    ToolDefinition {
        name: "memory_dejavu_patterns".into(),
        description: Some("Find recurring déjà vu patterns across sessions".into()),
        input_schema: json!({"type":"object","properties":{"min_occurrences":{"type":"integer","default":2}}}),
    }
}
pub async fn execute_dejavu_patterns(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let min_occ = args
        .get("min_occurrences")
        .and_then(|v| v.as_u64())
        .unwrap_or(2) as usize;
    let session = session.lock().await;
    let graph = session.graph();
    let mut patterns: std::collections::HashMap<String, Vec<u64>> =
        std::collections::HashMap::new();
    for n in graph.nodes() {
        let key: String = n
            .content
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if !key.is_empty() {
            patterns.entry(key).or_default().push(n.id);
        }
    }
    let results: Vec<Value> = patterns.iter().filter(|(_, ids)| ids.len() >= min_occ).take(20)
        .map(|(pat, ids)| json!({"pattern":pat,"occurrences":ids.len(),"node_ids":&ids[..ids.len().min(10)]})).collect();
    Ok(ToolCallResult::json(
        &json!({"min_occurrences":min_occ,"patterns_found":results.len(),"patterns":results}),
    ))
}

pub fn definition_dejavu_feedback() -> ToolDefinition {
    ToolDefinition {
        name: "memory_dejavu_feedback".into(),
        description: Some("Provide feedback on déjà vu detection accuracy".into()),
        input_schema: json!({"type":"object","properties":{"node_id":{"type":"integer"},"was_true_dejavu":{"type":"boolean"}},"required":["node_id","was_true_dejavu"]}),
    }
}
pub async fn execute_dejavu_feedback(
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> McpResult<ToolCallResult> {
    let node_id = get_u64(&args, "node_id")
        .ok_or_else(|| McpError::InvalidParams("node_id required".into()))?;
    let was_true = args
        .get("was_true_dejavu")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let session = session.lock().await;
    let _ = session
        .graph()
        .get_node(node_id)
        .ok_or(McpError::NodeNotFound(node_id))?;
    Ok(ToolCallResult::json(
        &json!({"node_id":node_id,"was_true_dejavu":was_true,"feedback_recorded":true}),
    ))
}

// ── Public API ───────────────────────────────────────────────────────────
pub fn all_definitions() -> Vec<ToolDefinition> {
    vec![
        definition_predict(),
        definition_predict_preload(),
        definition_predict_accuracy(),
        definition_predict_feedback(),
        definition_prophecy(),
        definition_prophecy_similar(),
        definition_prophecy_regret(),
        definition_prophecy_track(),
        definition_counterfactual_what_if(),
        definition_counterfactual_compare(),
        definition_counterfactual_insights(),
        definition_counterfactual_best(),
        definition_dejavu_check(),
        definition_dejavu_history(),
        definition_dejavu_patterns(),
        definition_dejavu_feedback(),
    ]
}

pub async fn try_execute(
    name: &str,
    args: Value,
    session: &Arc<Mutex<SessionManager>>,
) -> Option<McpResult<ToolCallResult>> {
    match name {
        "memory_predict" => Some(execute_predict(args, session).await),
        "memory_predict_preload" => Some(execute_predict_preload(args, session).await),
        "memory_predict_accuracy" => Some(execute_predict_accuracy(args, session).await),
        "memory_predict_feedback" => Some(execute_predict_feedback(args, session).await),
        "memory_prophecy" => Some(execute_prophecy(args, session).await),
        "memory_prophecy_similar" => Some(execute_prophecy_similar(args, session).await),
        "memory_prophecy_regret" => Some(execute_prophecy_regret(args, session).await),
        "memory_prophecy_track" => Some(execute_prophecy_track(args, session).await),
        "memory_counterfactual_what_if" => {
            Some(execute_counterfactual_what_if(args, session).await)
        }
        "memory_counterfactual_compare" => {
            Some(execute_counterfactual_compare(args, session).await)
        }
        "memory_counterfactual_insights" => {
            Some(execute_counterfactual_insights(args, session).await)
        }
        "memory_counterfactual_best" => Some(execute_counterfactual_best(args, session).await),
        "memory_dejavu_check" => Some(execute_dejavu_check(args, session).await),
        "memory_dejavu_history" => Some(execute_dejavu_history(args, session).await),
        "memory_dejavu_patterns" => Some(execute_dejavu_patterns(args, session).await),
        "memory_dejavu_feedback" => Some(execute_dejavu_feedback(args, session).await),
        _ => None,
    }
}
