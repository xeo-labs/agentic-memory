//! CLI command implementations.

use std::path::Path;

use crate::engine::{
    CausalParams, PatternParams, PatternSort, QueryEngine, TraversalParams, WriteEngine,
};
use crate::format::{AmemReader, AmemWriter};
use crate::graph::traversal::TraversalDirection;
use crate::graph::MemoryGraph;
use crate::types::{AmemResult, CognitiveEvent, CognitiveEventBuilder, Edge, EdgeType, EventType};

/// Create a new empty .amem file.
pub fn cmd_create(path: &Path, dimension: usize) -> AmemResult<()> {
    let graph = MemoryGraph::new(dimension);
    let writer = AmemWriter::new(dimension);
    writer.write_to_file(&graph, path)?;
    println!("Created {}", path.display());
    Ok(())
}

/// Display information about an .amem file.
pub fn cmd_info(path: &Path, json: bool) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let file_size = std::fs::metadata(path)?.len();

    if json {
        let info = serde_json::json!({
            "file": path.display().to_string(),
            "version": 1,
            "dimension": graph.dimension(),
            "nodes": graph.node_count(),
            "edges": graph.edge_count(),
            "sessions": graph.session_index().session_count(),
            "file_size": file_size,
            "node_types": {
                "facts": graph.type_index().count(EventType::Fact),
                "decisions": graph.type_index().count(EventType::Decision),
                "inferences": graph.type_index().count(EventType::Inference),
                "corrections": graph.type_index().count(EventType::Correction),
                "skills": graph.type_index().count(EventType::Skill),
                "episodes": graph.type_index().count(EventType::Episode),
            }
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&info).unwrap_or_default()
        );
    } else {
        println!("File: {}", path.display());
        println!("Version: 1");
        println!("Dimension: {}", graph.dimension());
        println!("Nodes: {}", graph.node_count());
        println!("Edges: {}", graph.edge_count());
        println!("Sessions: {}", graph.session_index().session_count());
        println!("File size: {}", format_size(file_size));
        println!("Node types:");
        println!("  Facts: {}", graph.type_index().count(EventType::Fact));
        println!(
            "  Decisions: {}",
            graph.type_index().count(EventType::Decision)
        );
        println!(
            "  Inferences: {}",
            graph.type_index().count(EventType::Inference)
        );
        println!(
            "  Corrections: {}",
            graph.type_index().count(EventType::Correction)
        );
        println!("  Skills: {}", graph.type_index().count(EventType::Skill));
        println!(
            "  Episodes: {}",
            graph.type_index().count(EventType::Episode)
        );
    }
    Ok(())
}

/// Add a cognitive event to the graph.
pub fn cmd_add(
    path: &Path,
    event_type: EventType,
    content: &str,
    session_id: u32,
    confidence: f32,
    supersedes: Option<u64>,
    json: bool,
) -> AmemResult<()> {
    let mut graph = AmemReader::read_from_file(path)?;
    let write_engine = WriteEngine::new(graph.dimension());

    let id = if let Some(old_id) = supersedes {
        write_engine.correct(&mut graph, old_id, content, session_id)?
    } else {
        let event = CognitiveEventBuilder::new(event_type, content)
            .session_id(session_id)
            .confidence(confidence)
            .build();
        graph.add_node(event)?
    };

    let writer = AmemWriter::new(graph.dimension());
    writer.write_to_file(&graph, path)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"id": id, "type": event_type.name()})
        );
    } else {
        println!(
            "Added node {} ({}) to {}",
            id,
            event_type.name(),
            path.display()
        );
    }
    Ok(())
}

/// Add an edge between two nodes.
pub fn cmd_link(
    path: &Path,
    source_id: u64,
    target_id: u64,
    edge_type: EdgeType,
    weight: f32,
    json: bool,
) -> AmemResult<()> {
    let mut graph = AmemReader::read_from_file(path)?;
    let edge = Edge::new(source_id, target_id, edge_type, weight);
    graph.add_edge(edge)?;

    let writer = AmemWriter::new(graph.dimension());
    writer.write_to_file(&graph, path)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"source": source_id, "target": target_id, "type": edge_type.name()})
        );
    } else {
        println!(
            "Linked {} --{}--> {}",
            source_id,
            edge_type.name(),
            target_id
        );
    }
    Ok(())
}

/// Get a specific node by ID.
pub fn cmd_get(path: &Path, node_id: u64, json: bool) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let node = graph
        .get_node(node_id)
        .ok_or(crate::types::AmemError::NodeNotFound(node_id))?;

    let edges_out = graph.edges_from(node_id).len();
    let edges_in = graph.edges_to(node_id).len();

    if json {
        let info = serde_json::json!({
            "id": node.id,
            "type": node.event_type.name(),
            "created_at": node.created_at,
            "session_id": node.session_id,
            "confidence": node.confidence,
            "access_count": node.access_count,
            "decay_score": node.decay_score,
            "content": node.content,
            "edges_out": edges_out,
            "edges_in": edges_in,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&info).unwrap_or_default()
        );
    } else {
        println!("Node {}", node.id);
        println!("  Type: {}", node.event_type.name());
        println!("  Created: {}", format_timestamp(node.created_at));
        println!("  Session: {}", node.session_id);
        println!("  Confidence: {:.2}", node.confidence);
        println!("  Access count: {}", node.access_count);
        println!("  Decay score: {:.2}", node.decay_score);
        println!("  Content: {:?}", node.content);
        println!("  Edges out: {}", edges_out);
        println!("  Edges in: {}", edges_in);
    }
    Ok(())
}

/// Run a traversal query.
#[allow(clippy::too_many_arguments)]
pub fn cmd_traverse(
    path: &Path,
    start_id: u64,
    edge_types: Vec<EdgeType>,
    direction: TraversalDirection,
    max_depth: u32,
    max_results: usize,
    min_confidence: f32,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let et = if edge_types.is_empty() {
        vec![
            EdgeType::CausedBy,
            EdgeType::Supports,
            EdgeType::Contradicts,
            EdgeType::Supersedes,
            EdgeType::RelatedTo,
            EdgeType::PartOf,
            EdgeType::TemporalNext,
        ]
    } else {
        edge_types
    };

    let result = query_engine.traverse(
        &graph,
        TraversalParams {
            start_id,
            edge_types: et,
            direction,
            max_depth,
            max_results,
            min_confidence,
        },
    )?;

    if json {
        let nodes_info: Vec<serde_json::Value> = result
            .visited
            .iter()
            .map(|&id| {
                let depth = result.depths.get(&id).copied().unwrap_or(0);
                if let Some(node) = graph.get_node(id) {
                    serde_json::json!({
                        "id": id,
                        "depth": depth,
                        "type": node.event_type.name(),
                        "content": node.content,
                    })
                } else {
                    serde_json::json!({"id": id, "depth": depth})
                }
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&nodes_info).unwrap_or_default()
        );
    } else {
        for &id in &result.visited {
            let depth = result.depths.get(&id).copied().unwrap_or(0);
            let indent = "  ".repeat(depth as usize);
            if let Some(node) = graph.get_node(id) {
                println!(
                    "{}[depth {}] Node {} ({}): {:?}",
                    indent,
                    depth,
                    id,
                    node.event_type.name(),
                    node.content
                );
            }
        }
    }
    Ok(())
}

/// Pattern search.
#[allow(clippy::too_many_arguments)]
pub fn cmd_search(
    path: &Path,
    event_types: Vec<EventType>,
    session_ids: Vec<u32>,
    min_confidence: Option<f32>,
    max_confidence: Option<f32>,
    created_after: Option<u64>,
    created_before: Option<u64>,
    sort_by: PatternSort,
    limit: usize,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let results = query_engine.pattern(
        &graph,
        PatternParams {
            event_types,
            min_confidence,
            max_confidence,
            session_ids,
            created_after,
            created_before,
            min_decay_score: None,
            max_results: limit,
            sort_by,
        },
    )?;

    if json {
        let nodes: Vec<serde_json::Value> = results
            .iter()
            .map(|node| {
                serde_json::json!({
                    "id": node.id,
                    "type": node.event_type.name(),
                    "confidence": node.confidence,
                    "content": node.content,
                    "session_id": node.session_id,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&nodes).unwrap_or_default()
        );
    } else {
        for node in &results {
            println!(
                "Node {} ({}, confidence: {:.2}): {:?}",
                node.id,
                node.event_type.name(),
                node.confidence,
                node.content
            );
        }
        println!("\n{} results", results.len());
    }
    Ok(())
}

/// Causal impact analysis.
pub fn cmd_impact(path: &Path, node_id: u64, max_depth: u32, json: bool) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let result = query_engine.causal(
        &graph,
        CausalParams {
            node_id,
            max_depth,
            dependency_types: vec![EdgeType::CausedBy, EdgeType::Supports],
        },
    )?;

    if json {
        let info = serde_json::json!({
            "root_id": result.root_id,
            "direct_dependents": result.dependency_tree.get(&node_id).map(|v| v.len()).unwrap_or(0),
            "total_dependents": result.dependents.len(),
            "affected_decisions": result.affected_decisions,
            "affected_inferences": result.affected_inferences,
            "dependents": result.dependents,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&info).unwrap_or_default()
        );
    } else {
        println!("Impact analysis for node {}", node_id);
        let direct = result
            .dependency_tree
            .get(&node_id)
            .map(|v| v.len())
            .unwrap_or(0);
        println!("  Direct dependents: {}", direct);
        println!("  Total dependents: {}", result.dependents.len());
        println!("  Affected decisions: {}", result.affected_decisions);
        println!("  Affected inferences: {}", result.affected_inferences);

        if !result.dependents.is_empty() {
            println!("\nDependency tree:");
            print_dependency_tree(&graph, &result.dependency_tree, node_id, 1);
        }
    }
    Ok(())
}

fn print_dependency_tree(
    graph: &MemoryGraph,
    tree: &std::collections::HashMap<u64, Vec<(u64, EdgeType)>>,
    node_id: u64,
    depth: usize,
) {
    if let Some(deps) = tree.get(&node_id) {
        for (dep_id, edge_type) in deps {
            let indent = "  ".repeat(depth);
            if let Some(node) = graph.get_node(*dep_id) {
                println!(
                    "{}<- Node {} ({}, {})",
                    indent,
                    dep_id,
                    node.event_type.name(),
                    edge_type.name()
                );
            }
            print_dependency_tree(graph, tree, *dep_id, depth + 1);
        }
    }
}

/// Resolve a node through SUPERSEDES chains.
pub fn cmd_resolve(path: &Path, node_id: u64, json: bool) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let resolved = query_engine.resolve(&graph, node_id)?;

    if json {
        let info = serde_json::json!({
            "original_id": node_id,
            "resolved_id": resolved.id,
            "type": resolved.event_type.name(),
            "content": resolved.content,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&info).unwrap_or_default()
        );
    } else {
        if resolved.id != node_id {
            // Show chain
            let mut chain = vec![node_id];
            let mut current = node_id;
            for _ in 0..100 {
                let mut next = None;
                for edge in graph.edges_to(current) {
                    if edge.edge_type == EdgeType::Supersedes {
                        next = Some(edge.source_id);
                        break;
                    }
                }
                match next {
                    Some(n) => {
                        chain.push(n);
                        current = n;
                    }
                    None => break,
                }
            }
            let chain_str: Vec<String> = chain.iter().map(|id| format!("Node {}", id)).collect();
            println!("{} (current)", chain_str.join(" -> superseded by -> "));
        } else {
            println!("Node {} is already the current version", node_id);
        }
        println!("\nCurrent version:");
        println!("  Node {}", resolved.id);
        println!("  Type: {}", resolved.event_type.name());
        println!("  Content: {:?}", resolved.content);
    }
    Ok(())
}

/// List sessions.
pub fn cmd_sessions(path: &Path, limit: usize, json: bool) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let session_ids = graph.session_index().session_ids();

    if json {
        let sessions: Vec<serde_json::Value> = session_ids
            .iter()
            .rev()
            .take(limit)
            .map(|&sid| {
                serde_json::json!({
                    "session_id": sid,
                    "node_count": graph.session_index().node_count(sid),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&sessions).unwrap_or_default()
        );
    } else {
        println!("Sessions in {}:", path.display());
        for &sid in session_ids.iter().rev().take(limit) {
            let count = graph.session_index().node_count(sid);
            println!("  Session {}: {} nodes", sid, count);
        }
        println!("  Total: {} sessions", session_ids.len());
    }
    Ok(())
}

/// Export graph as JSON.
pub fn cmd_export(
    path: &Path,
    nodes_only: bool,
    session: Option<u32>,
    pretty: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;

    let nodes: Vec<&CognitiveEvent> = if let Some(sid) = session {
        let ids = graph.session_index().get_session(sid);
        ids.iter().filter_map(|&id| graph.get_node(id)).collect()
    } else {
        graph.nodes().iter().collect()
    };

    let nodes_json: Vec<serde_json::Value> = nodes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "event_type": n.event_type.name(),
                "created_at": n.created_at,
                "session_id": n.session_id,
                "confidence": n.confidence,
                "access_count": n.access_count,
                "last_accessed": n.last_accessed,
                "decay_score": n.decay_score,
                "content": n.content,
            })
        })
        .collect();

    let output = if nodes_only {
        serde_json::json!({"nodes": nodes_json})
    } else {
        let edges_json: Vec<serde_json::Value> = graph
            .edges()
            .iter()
            .map(|e| {
                serde_json::json!({
                    "source_id": e.source_id,
                    "target_id": e.target_id,
                    "edge_type": e.edge_type.name(),
                    "weight": e.weight,
                    "created_at": e.created_at,
                })
            })
            .collect();
        serde_json::json!({"nodes": nodes_json, "edges": edges_json})
    };

    if pretty {
        println!(
            "{}",
            serde_json::to_string_pretty(&output).unwrap_or_default()
        );
    } else {
        println!("{}", serde_json::to_string(&output).unwrap_or_default());
    }
    Ok(())
}

/// Import nodes and edges from JSON.
pub fn cmd_import(path: &Path, json_path: &Path) -> AmemResult<()> {
    let mut graph = AmemReader::read_from_file(path)?;
    let json_data = std::fs::read_to_string(json_path)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_data)
        .map_err(|e| crate::types::AmemError::Compression(e.to_string()))?;

    let mut added_nodes = 0;
    let mut added_edges = 0;

    if let Some(nodes) = parsed.get("nodes").and_then(|v| v.as_array()) {
        for node_val in nodes {
            let event_type = node_val
                .get("event_type")
                .and_then(|v| v.as_str())
                .and_then(EventType::from_name)
                .unwrap_or(EventType::Fact);
            let content = node_val
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let session_id = node_val
                .get("session_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let confidence = node_val
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0) as f32;

            let event = CognitiveEventBuilder::new(event_type, content)
                .session_id(session_id)
                .confidence(confidence)
                .build();
            graph.add_node(event)?;
            added_nodes += 1;
        }
    }

    if let Some(edges) = parsed.get("edges").and_then(|v| v.as_array()) {
        for edge_val in edges {
            let source_id = edge_val
                .get("source_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let target_id = edge_val
                .get("target_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let edge_type = edge_val
                .get("edge_type")
                .and_then(|v| v.as_str())
                .and_then(EdgeType::from_name)
                .unwrap_or(EdgeType::RelatedTo);
            let weight = edge_val
                .get("weight")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0) as f32;

            let edge = Edge::new(source_id, target_id, edge_type, weight);
            if graph.add_edge(edge).is_ok() {
                added_edges += 1;
            }
        }
    }

    let writer = AmemWriter::new(graph.dimension());
    writer.write_to_file(&graph, path)?;

    println!("Imported {} nodes and {} edges", added_nodes, added_edges);
    Ok(())
}

/// Run decay calculations.
pub fn cmd_decay(path: &Path, threshold: f32, json: bool) -> AmemResult<()> {
    let mut graph = AmemReader::read_from_file(path)?;
    let write_engine = WriteEngine::new(graph.dimension());
    let current_time = crate::types::now_micros();
    let report = write_engine.run_decay(&mut graph, current_time)?;

    let writer = AmemWriter::new(graph.dimension());
    writer.write_to_file(&graph, path)?;

    let low: Vec<u64> = report
        .low_importance_nodes
        .iter()
        .filter(|&&id| {
            graph
                .get_node(id)
                .map(|n| n.decay_score < threshold)
                .unwrap_or(false)
        })
        .copied()
        .collect();

    if json {
        let info = serde_json::json!({
            "nodes_decayed": report.nodes_decayed,
            "low_importance_count": low.len(),
            "low_importance_nodes": low,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&info).unwrap_or_default()
        );
    } else {
        println!("Decay complete:");
        println!("  Nodes updated: {}", report.nodes_decayed);
        println!(
            "  Low importance (below {}): {} nodes",
            threshold,
            low.len()
        );
    }
    Ok(())
}

/// Detailed statistics.
pub fn cmd_stats(path: &Path, json: bool) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let file_size = std::fs::metadata(path)?.len();

    let node_count = graph.node_count();
    let edge_count = graph.edge_count();
    let avg_edges = if node_count > 0 {
        edge_count as f64 / node_count as f64
    } else {
        0.0
    };
    let max_edges = graph
        .nodes()
        .iter()
        .map(|n| graph.edges_from(n.id).len())
        .max()
        .unwrap_or(0);
    let session_count = graph.session_index().session_count();
    let avg_nodes_per_session = if session_count > 0 {
        node_count as f64 / session_count as f64
    } else {
        0.0
    };

    // Confidence distribution
    let mut conf_buckets = [0usize; 5];
    for node in graph.nodes() {
        let bucket = ((node.confidence * 5.0).floor() as usize).min(4);
        conf_buckets[bucket] += 1;
    }

    if json {
        let info = serde_json::json!({
            "nodes": node_count,
            "edges": edge_count,
            "avg_edges_per_node": avg_edges,
            "max_edges_per_node": max_edges,
            "sessions": session_count,
            "file_size": file_size,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&info).unwrap_or_default()
        );
    } else {
        println!("Graph Statistics:");
        println!("  Nodes: {}", node_count);
        println!("  Edges: {}", edge_count);
        println!("  Avg edges per node: {:.2}", avg_edges);
        println!("  Max edges per node: {}", max_edges);
        println!("  Sessions: {}", session_count);
        println!("  Avg nodes per session: {:.0}", avg_nodes_per_session);
        println!();
        println!("  Confidence distribution:");
        println!("    0.0-0.2: {} nodes", conf_buckets[0]);
        println!("    0.2-0.4: {} nodes", conf_buckets[1]);
        println!("    0.4-0.6: {} nodes", conf_buckets[2]);
        println!("    0.6-0.8: {} nodes", conf_buckets[3]);
        println!("    0.8-1.0: {} nodes", conf_buckets[4]);
        println!();
        println!("  Edge type distribution:");
        for et_val in 0u8..=6 {
            if let Some(et) = EdgeType::from_u8(et_val) {
                let count = graph.edges().iter().filter(|e| e.edge_type == et).count();
                if count > 0 {
                    println!("    {}: {}", et.name(), count);
                }
            }
        }
    }
    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_timestamp(micros: u64) -> String {
    let secs = (micros / 1_000_000) as i64;
    let dt = chrono::DateTime::from_timestamp(secs, 0);
    match dt {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => format!("{} us", micros),
    }
}
