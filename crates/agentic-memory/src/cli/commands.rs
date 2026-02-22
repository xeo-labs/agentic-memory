//! CLI command implementations.

use std::path::Path;

use crate::engine::{
    AnalogicalAnchor, AnalogicalParams, BeliefRevisionParams, CausalParams, CentralityAlgorithm,
    CentralityParams, ConsolidationOp, ConsolidationParams, DriftParams, GapDetectionParams,
    GapSeverity, HybridSearchParams, MemoryQualityParams, PatternParams, PatternSort, QueryEngine,
    ShortestPathParams, TextSearchParams, TraversalParams, WriteEngine,
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

/// Graph-wide quality report (confidence, staleness, structural health).
pub fn cmd_quality(
    path: &Path,
    low_confidence: f32,
    stale_decay: f32,
    limit: usize,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();
    let report = query_engine.memory_quality(
        &graph,
        MemoryQualityParams {
            low_confidence_threshold: low_confidence.clamp(0.0, 1.0),
            stale_decay_threshold: stale_decay.clamp(0.0, 1.0),
            max_examples: limit.max(1),
        },
    )?;

    if json {
        let out = serde_json::json!({
            "status": report.status,
            "summary": {
                "nodes": report.node_count,
                "edges": report.edge_count,
                "low_confidence_count": report.low_confidence_count,
                "stale_count": report.stale_count,
                "orphan_count": report.orphan_count,
                "decisions_without_support_count": report.decisions_without_support_count,
                "contradiction_edges": report.contradiction_edges,
                "supersedes_edges": report.supersedes_edges,
            },
            "examples": {
                "low_confidence": report.low_confidence_examples,
                "stale": report.stale_examples,
                "orphan": report.orphan_examples,
                "unsupported_decisions": report.unsupported_decision_examples,
            }
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    } else {
        println!("Memory quality report for {}", path.display());
        println!("  Status: {}", report.status.to_uppercase());
        println!("  Nodes: {}", report.node_count);
        println!("  Edges: {}", report.edge_count);
        println!(
            "  Weak confidence (<{:.2}): {}",
            low_confidence, report.low_confidence_count
        );
        println!("  Stale (<{:.2}): {}", stale_decay, report.stale_count);
        println!("  Orphan nodes: {}", report.orphan_count);
        println!(
            "  Decisions without support edges: {}",
            report.decisions_without_support_count
        );
        println!("  Contradiction edges: {}", report.contradiction_edges);
        println!("  Supersedes edges: {}", report.supersedes_edges);
        if !report.low_confidence_examples.is_empty() {
            println!(
                "  Low-confidence examples: {:?}",
                report.low_confidence_examples
            );
        }
        if !report.unsupported_decision_examples.is_empty() {
            println!(
                "  Unsupported decision examples: {:?}",
                report.unsupported_decision_examples
            );
        }
        println!(
            "  Next: amem runtime-sync {} --workspace . --write-episode",
            path.display()
        );
    }

    Ok(())
}

#[derive(Default)]
struct ArtifactScanReport {
    amem_files: Vec<std::path::PathBuf>,
    acb_files: Vec<std::path::PathBuf>,
    avis_files: Vec<std::path::PathBuf>,
    io_errors: usize,
}

/// Scan a workspace for sister artifacts and optionally write an episode memory.
pub fn cmd_runtime_sync(
    path: &Path,
    workspace: &Path,
    max_depth: u32,
    session_id: u32,
    write_episode: bool,
    json: bool,
) -> AmemResult<()> {
    let mut graph = AmemReader::read_from_file(path)?;
    let report = scan_workspace_artifacts(workspace, max_depth);

    let mut episode_id = None;
    if write_episode {
        let sid = if session_id == 0 {
            graph
                .session_index()
                .session_ids()
                .iter()
                .copied()
                .max()
                .unwrap_or(0)
        } else {
            session_id
        };
        let content = format!(
            "Runtime sync snapshot for workspace {}: amem={} acb={} avis={} (depth={})",
            workspace.display(),
            report.amem_files.len(),
            report.acb_files.len(),
            report.avis_files.len(),
            max_depth
        );
        let event = CognitiveEventBuilder::new(EventType::Episode, content)
            .session_id(sid)
            .confidence(0.95)
            .build();
        let id = graph.add_node(event)?;
        episode_id = Some(id);
        let writer = AmemWriter::new(graph.dimension());
        writer.write_to_file(&graph, path)?;
    }

    if json {
        let out = serde_json::json!({
            "workspace": workspace.display().to_string(),
            "max_depth": max_depth,
            "amem_count": report.amem_files.len(),
            "acb_count": report.acb_files.len(),
            "avis_count": report.avis_files.len(),
            "io_errors": report.io_errors,
            "episode_written": episode_id.is_some(),
            "episode_id": episode_id,
            "sample": {
                "amem": report.amem_files.iter().take(5).map(|p| p.display().to_string()).collect::<Vec<_>>(),
                "acb": report.acb_files.iter().take(5).map(|p| p.display().to_string()).collect::<Vec<_>>(),
                "avis": report.avis_files.iter().take(5).map(|p| p.display().to_string()).collect::<Vec<_>>(),
            }
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    } else {
        println!(
            "Runtime sync scan in {} (depth {})",
            workspace.display(),
            max_depth
        );
        println!("  .amem files: {}", report.amem_files.len());
        println!("  .acb files: {}", report.acb_files.len());
        println!("  .avis files: {}", report.avis_files.len());
        if report.io_errors > 0 {
            println!("  Scan IO errors: {}", report.io_errors);
        }
        if let Some(id) = episode_id {
            println!("  Wrote episode node: {}", id);
        } else {
            println!("  Episode write: skipped");
        }
    }

    Ok(())
}

fn scan_workspace_artifacts(root: &Path, max_depth: u32) -> ArtifactScanReport {
    let mut report = ArtifactScanReport::default();
    scan_dir_recursive(root, 0, max_depth, &mut report);
    report
}

fn scan_dir_recursive(path: &Path, depth: u32, max_depth: u32, report: &mut ArtifactScanReport) {
    if depth > max_depth {
        return;
    }
    let entries = match std::fs::read_dir(path) {
        Ok(v) => v,
        Err(_) => {
            report.io_errors += 1;
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(v) => v,
            Err(_) => {
                report.io_errors += 1;
                continue;
            }
        };
        let p = entry.path();
        if p.is_dir() {
            if should_skip_dir(&p) {
                continue;
            }
            scan_dir_recursive(&p, depth + 1, max_depth, report);
            continue;
        }
        let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        match ext.to_ascii_lowercase().as_str() {
            "amem" => report.amem_files.push(p),
            "acb" => report.acb_files.push(p),
            "avis" => report.avis_files.push(p),
            _ => {}
        }
    }
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    matches!(
        name,
        ".git" | "target" | "node_modules" | ".venv" | ".idea" | ".vscode" | "__pycache__"
    )
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

// ==================== New Query Expansion Commands ====================

/// BM25 text search.
pub fn cmd_text_search(
    path: &Path,
    query: &str,
    event_types: Vec<EventType>,
    session_ids: Vec<u32>,
    limit: usize,
    min_score: f32,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let start = std::time::Instant::now();
    let results = query_engine.text_search(
        &graph,
        graph.term_index(),
        graph.doc_lengths(),
        TextSearchParams {
            query: query.to_string(),
            max_results: limit,
            event_types,
            session_ids,
            min_score,
        },
    )?;
    let elapsed = start.elapsed();

    if json {
        let matches: Vec<serde_json::Value> = results
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let node = graph.get_node(m.node_id);
                serde_json::json!({
                    "rank": i + 1,
                    "node_id": m.node_id,
                    "score": m.score,
                    "matched_terms": m.matched_terms,
                    "type": node.map(|n| n.event_type.name()).unwrap_or("unknown"),
                    "content": node.map(|n| n.content.as_str()).unwrap_or(""),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "query": query,
                "results": matches,
                "total": results.len(),
                "elapsed_ms": elapsed.as_secs_f64() * 1000.0,
            }))
            .unwrap_or_default()
        );
    } else {
        println!("Text search for {:?} in {}:", query, path.display());
        for (i, m) in results.iter().enumerate() {
            if let Some(node) = graph.get_node(m.node_id) {
                let preview = if node.content.len() > 60 {
                    format!("{}...", &node.content[..60])
                } else {
                    node.content.clone()
                };
                println!(
                    "  #{:<3} Node {} ({}) [score: {:.2}]  {:?}",
                    i + 1,
                    m.node_id,
                    node.event_type.name(),
                    m.score,
                    preview
                );
            }
        }
        println!(
            "  {} results ({:.1}ms)",
            results.len(),
            elapsed.as_secs_f64() * 1000.0
        );
    }
    Ok(())
}

/// Hybrid BM25 + vector search.
#[allow(clippy::too_many_arguments)]
pub fn cmd_hybrid_search(
    path: &Path,
    query: &str,
    text_weight: f32,
    vec_weight: f32,
    limit: usize,
    event_types: Vec<EventType>,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let results = query_engine.hybrid_search(
        &graph,
        graph.term_index(),
        graph.doc_lengths(),
        HybridSearchParams {
            query_text: query.to_string(),
            query_vec: None,
            max_results: limit,
            event_types,
            text_weight,
            vector_weight: vec_weight,
            rrf_k: 60,
        },
    )?;

    if json {
        let matches: Vec<serde_json::Value> = results
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let node = graph.get_node(m.node_id);
                serde_json::json!({
                    "rank": i + 1,
                    "node_id": m.node_id,
                    "combined_score": m.combined_score,
                    "text_rank": m.text_rank,
                    "vector_rank": m.vector_rank,
                    "text_score": m.text_score,
                    "vector_similarity": m.vector_similarity,
                    "type": node.map(|n| n.event_type.name()).unwrap_or("unknown"),
                    "content": node.map(|n| n.content.as_str()).unwrap_or(""),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "query": query,
                "results": matches,
                "total": results.len(),
            }))
            .unwrap_or_default()
        );
    } else {
        println!("Hybrid search for {:?}:", query);
        for (i, m) in results.iter().enumerate() {
            if let Some(node) = graph.get_node(m.node_id) {
                let preview = if node.content.len() > 60 {
                    format!("{}...", &node.content[..60])
                } else {
                    node.content.clone()
                };
                println!(
                    "  #{:<3} Node {} ({}) [score: {:.4}]  {:?}",
                    i + 1,
                    m.node_id,
                    node.event_type.name(),
                    m.combined_score,
                    preview
                );
            }
        }
        println!("  {} results", results.len());
    }
    Ok(())
}

/// Centrality analysis.
#[allow(clippy::too_many_arguments)]
pub fn cmd_centrality(
    path: &Path,
    algorithm: &str,
    damping: f32,
    edge_types: Vec<EdgeType>,
    event_types: Vec<EventType>,
    limit: usize,
    iterations: u32,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let algo = match algorithm {
        "degree" => CentralityAlgorithm::Degree,
        "betweenness" => CentralityAlgorithm::Betweenness,
        _ => CentralityAlgorithm::PageRank { damping },
    };

    let result = query_engine.centrality(
        &graph,
        CentralityParams {
            algorithm: algo,
            max_iterations: iterations,
            tolerance: 1e-6,
            top_k: limit,
            event_types,
            edge_types,
        },
    )?;

    if json {
        let scores: Vec<serde_json::Value> = result
            .scores
            .iter()
            .enumerate()
            .map(|(i, (id, score))| {
                let node = graph.get_node(*id);
                serde_json::json!({
                    "rank": i + 1,
                    "node_id": id,
                    "score": score,
                    "type": node.map(|n| n.event_type.name()).unwrap_or("unknown"),
                    "content": node.map(|n| n.content.as_str()).unwrap_or(""),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "algorithm": algorithm,
                "converged": result.converged,
                "iterations": result.iterations,
                "scores": scores,
            }))
            .unwrap_or_default()
        );
    } else {
        let algo_name = match algorithm {
            "degree" => "Degree",
            "betweenness" => "Betweenness",
            _ => "PageRank",
        };
        println!(
            "{} centrality (converged: {}, iterations: {}):",
            algo_name, result.converged, result.iterations
        );
        for (i, (id, score)) in result.scores.iter().enumerate() {
            if let Some(node) = graph.get_node(*id) {
                let preview = if node.content.len() > 50 {
                    format!("{}...", &node.content[..50])
                } else {
                    node.content.clone()
                };
                println!(
                    "  #{:<3} Node {} ({}) [score: {:.6}]  {:?}",
                    i + 1,
                    id,
                    node.event_type.name(),
                    score,
                    preview
                );
            }
        }
    }
    Ok(())
}

/// Shortest path.
#[allow(clippy::too_many_arguments)]
pub fn cmd_path(
    path: &Path,
    source_id: u64,
    target_id: u64,
    edge_types: Vec<EdgeType>,
    direction: TraversalDirection,
    max_depth: u32,
    weighted: bool,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let result = query_engine.shortest_path(
        &graph,
        ShortestPathParams {
            source_id,
            target_id,
            edge_types,
            direction,
            max_depth,
            weighted,
        },
    )?;

    if json {
        let path_info: Vec<serde_json::Value> = result
            .path
            .iter()
            .map(|&id| {
                let node = graph.get_node(id);
                serde_json::json!({
                    "node_id": id,
                    "type": node.map(|n| n.event_type.name()).unwrap_or("unknown"),
                    "content": node.map(|n| n.content.as_str()).unwrap_or(""),
                })
            })
            .collect();
        let edges_info: Vec<serde_json::Value> = result
            .edges
            .iter()
            .map(|e| {
                serde_json::json!({
                    "source_id": e.source_id,
                    "target_id": e.target_id,
                    "edge_type": e.edge_type.name(),
                    "weight": e.weight,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "found": result.found,
                "cost": result.cost,
                "path": path_info,
                "edges": edges_info,
            }))
            .unwrap_or_default()
        );
    } else if result.found {
        println!(
            "Path from node {} to node {} ({} hops, cost: {:.2}):",
            source_id,
            target_id,
            result.path.len().saturating_sub(1),
            result.cost
        );
        // Print path as chain
        let mut parts: Vec<String> = Vec::new();
        for (i, &id) in result.path.iter().enumerate() {
            if let Some(node) = graph.get_node(id) {
                let label = format!("Node {} ({})", id, node.event_type.name());
                if i < result.edges.len() {
                    parts.push(format!(
                        "{} --[{}]-->",
                        label,
                        result.edges[i].edge_type.name()
                    ));
                } else {
                    parts.push(label);
                }
            }
        }
        println!("  {}", parts.join(" "));
    } else {
        println!(
            "No path found from node {} to node {}",
            source_id, target_id
        );
    }
    Ok(())
}

/// Belief revision.
pub fn cmd_revise(
    path: &Path,
    hypothesis: &str,
    threshold: f32,
    max_depth: u32,
    confidence: f32,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let report = query_engine.belief_revision(
        &graph,
        BeliefRevisionParams {
            hypothesis: hypothesis.to_string(),
            hypothesis_vec: None,
            contradiction_threshold: threshold,
            max_depth,
            hypothesis_confidence: confidence,
        },
    )?;

    if json {
        let contradicted: Vec<serde_json::Value> = report
            .contradicted
            .iter()
            .map(|c| {
                let node = graph.get_node(c.node_id);
                serde_json::json!({
                    "node_id": c.node_id,
                    "strength": c.contradiction_strength,
                    "reason": c.reason,
                    "type": node.map(|n| n.event_type.name()).unwrap_or("unknown"),
                    "content": node.map(|n| n.content.as_str()).unwrap_or(""),
                })
            })
            .collect();
        let weakened: Vec<serde_json::Value> = report
            .weakened
            .iter()
            .map(|w| {
                serde_json::json!({
                    "node_id": w.node_id,
                    "original_confidence": w.original_confidence,
                    "revised_confidence": w.revised_confidence,
                    "depth": w.depth,
                })
            })
            .collect();
        let cascade: Vec<serde_json::Value> = report
            .cascade
            .iter()
            .map(|s| {
                serde_json::json!({
                    "node_id": s.node_id,
                    "via_edge": s.via_edge.name(),
                    "from_node": s.from_node,
                    "depth": s.depth,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "hypothesis": hypothesis,
                "contradicted": contradicted,
                "weakened": weakened,
                "invalidated_decisions": report.invalidated_decisions,
                "total_affected": report.total_affected,
                "cascade": cascade,
            }))
            .unwrap_or_default()
        );
    } else {
        println!("Belief revision: {:?}\n", hypothesis);
        if report.contradicted.is_empty() {
            println!("  No contradictions found.");
        } else {
            println!("Directly contradicted:");
            for c in &report.contradicted {
                if let Some(node) = graph.get_node(c.node_id) {
                    println!(
                        "  X Node {} ({}): {:?} [score: {:.2}]",
                        c.node_id,
                        node.event_type.name(),
                        node.content,
                        c.contradiction_strength
                    );
                }
            }
        }
        if !report.weakened.is_empty() {
            println!("\nCascade effects:");
            for w in &report.weakened {
                if let Some(node) = graph.get_node(w.node_id) {
                    let action = if node.event_type == EventType::Decision {
                        "INVALIDATED"
                    } else {
                        "weakened"
                    };
                    println!(
                        "  ! Node {} ({}): {} ({:.2} -> {:.2})",
                        w.node_id,
                        node.event_type.name(),
                        action,
                        w.original_confidence,
                        w.revised_confidence
                    );
                }
            }
        }
        println!(
            "\nTotal affected: {} nodes ({} decisions)",
            report.total_affected,
            report.invalidated_decisions.len()
        );
    }
    Ok(())
}

/// Gap detection.
#[allow(clippy::too_many_arguments)]
pub fn cmd_gaps(
    path: &Path,
    threshold: f32,
    min_support: u32,
    limit: usize,
    sort: &str,
    session_range: Option<(u32, u32)>,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let sort_by = match sort {
        "recent" => GapSeverity::MostRecent,
        "confidence" => GapSeverity::LowestConfidence,
        _ => GapSeverity::HighestImpact,
    };

    let report = query_engine.gap_detection(
        &graph,
        GapDetectionParams {
            confidence_threshold: threshold,
            min_support_count: min_support,
            max_results: limit,
            session_range,
            sort_by,
        },
    )?;

    if json {
        let gaps: Vec<serde_json::Value> = report
            .gaps
            .iter()
            .map(|g| {
                let node = graph.get_node(g.node_id);
                serde_json::json!({
                    "node_id": g.node_id,
                    "gap_type": format!("{:?}", g.gap_type),
                    "severity": g.severity,
                    "description": g.description,
                    "downstream_count": g.downstream_count,
                    "type": node.map(|n| n.event_type.name()).unwrap_or("unknown"),
                    "content": node.map(|n| n.content.as_str()).unwrap_or(""),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "gaps": gaps,
                "health_score": report.summary.health_score,
                "summary": {
                    "total_gaps": report.summary.total_gaps,
                    "unjustified_decisions": report.summary.unjustified_decisions,
                    "single_source_inferences": report.summary.single_source_inferences,
                    "low_confidence_foundations": report.summary.low_confidence_foundations,
                    "unstable_knowledge": report.summary.unstable_knowledge,
                    "stale_evidence": report.summary.stale_evidence,
                }
            }))
            .unwrap_or_default()
        );
    } else {
        println!("Reasoning gaps in {}:\n", path.display());
        for g in &report.gaps {
            let severity_marker = if g.severity > 0.8 {
                "CRITICAL"
            } else if g.severity > 0.5 {
                "WARNING"
            } else {
                "INFO"
            };
            if let Some(node) = graph.get_node(g.node_id) {
                println!(
                    "  {}: Node {} ({}) -- {:?}",
                    severity_marker,
                    g.node_id,
                    node.event_type.name(),
                    g.gap_type
                );
                let preview = if node.content.len() > 60 {
                    format!("{}...", &node.content[..60])
                } else {
                    node.content.clone()
                };
                println!("     {:?}", preview);
                println!(
                    "     Severity: {:.2} | {} downstream dependents",
                    g.severity, g.downstream_count
                );
                println!();
            }
        }
        println!(
            "Health score: {:.2} / 1.00  |  {} gaps found",
            report.summary.health_score, report.summary.total_gaps
        );
    }
    Ok(())
}

/// Analogical query.
#[allow(clippy::too_many_arguments)]
pub fn cmd_analogy(
    path: &Path,
    description: &str,
    limit: usize,
    min_similarity: f32,
    exclude_sessions: Vec<u32>,
    depth: u32,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    // Find the best matching node to use as anchor
    let tokenizer = crate::engine::Tokenizer::new();
    let query_terms: std::collections::HashSet<String> =
        tokenizer.tokenize(description).into_iter().collect();

    // Find the most relevant node as the anchor center
    let mut best_id = None;
    let mut best_score = -1.0f32;
    for node in graph.nodes() {
        let node_terms: std::collections::HashSet<String> =
            tokenizer.tokenize(&node.content).into_iter().collect();
        let overlap = query_terms.intersection(&node_terms).count();
        let score = if query_terms.is_empty() {
            0.0
        } else {
            overlap as f32 / query_terms.len() as f32
        };
        if score > best_score {
            best_score = score;
            best_id = Some(node.id);
        }
    }

    let anchor = match best_id {
        Some(id) => AnalogicalAnchor::Node(id),
        None => {
            println!("No matching nodes found for the description.");
            return Ok(());
        }
    };

    let results = query_engine.analogical(
        &graph,
        AnalogicalParams {
            anchor,
            context_depth: depth,
            max_results: limit,
            min_similarity,
            exclude_sessions,
        },
    )?;

    if json {
        let analogies: Vec<serde_json::Value> = results
            .iter()
            .map(|a| {
                let node = graph.get_node(a.center_id);
                serde_json::json!({
                    "center_id": a.center_id,
                    "structural_similarity": a.structural_similarity,
                    "content_similarity": a.content_similarity,
                    "combined_score": a.combined_score,
                    "subgraph_nodes": a.subgraph_nodes,
                    "type": node.map(|n| n.event_type.name()).unwrap_or("unknown"),
                    "content": node.map(|n| n.content.as_str()).unwrap_or(""),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "description": description,
                "analogies": analogies,
            }))
            .unwrap_or_default()
        );
    } else {
        println!("Analogies for {:?}:\n", description);
        for (i, a) in results.iter().enumerate() {
            if let Some(node) = graph.get_node(a.center_id) {
                println!(
                    "  #{} Node {} ({}) [combined: {:.3}]",
                    i + 1,
                    a.center_id,
                    node.event_type.name(),
                    a.combined_score
                );
                println!(
                    "     Structural: {:.3} | Content: {:.3} | Subgraph: {} nodes",
                    a.structural_similarity,
                    a.content_similarity,
                    a.subgraph_nodes.len()
                );
            }
        }
        if results.is_empty() {
            println!("  No analogies found.");
        }
    }
    Ok(())
}

/// Consolidation.
#[allow(clippy::too_many_arguments)]
pub fn cmd_consolidate(
    path: &Path,
    deduplicate: bool,
    link_contradictions: bool,
    promote_inferences: bool,
    prune: bool,
    compress_episodes: bool,
    all: bool,
    threshold: f32,
    confirm: bool,
    backup: Option<std::path::PathBuf>,
    json: bool,
) -> AmemResult<()> {
    let mut graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let dry_run = !confirm;

    // Build operations list
    let mut ops = Vec::new();
    if deduplicate || all {
        ops.push(ConsolidationOp::DeduplicateFacts { threshold });
    }
    if link_contradictions || all {
        ops.push(ConsolidationOp::LinkContradictions {
            threshold: threshold.min(0.8),
        });
    }
    if promote_inferences || all {
        ops.push(ConsolidationOp::PromoteInferences {
            min_access: 3,
            min_confidence: 0.8,
        });
    }
    if prune || all {
        ops.push(ConsolidationOp::PruneOrphans { max_decay: 0.1 });
    }
    if compress_episodes || all {
        ops.push(ConsolidationOp::CompressEpisodes { group_size: 3 });
    }

    if ops.is_empty() {
        eprintln!("No operations specified. Use --deduplicate, --link-contradictions, --promote-inferences, --prune, --compress-episodes, or --all");
        return Ok(());
    }

    // If not dry-run, create backup first
    let backup_path = if !dry_run {
        let bp = backup.unwrap_or_else(|| {
            let mut p = path.to_path_buf();
            let name = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            p.set_file_name(format!("{}.pre-consolidation.amem", name));
            p
        });
        std::fs::copy(path, &bp)?;
        Some(bp)
    } else {
        None
    };

    let report = query_engine.consolidate(
        &mut graph,
        ConsolidationParams {
            session_range: None,
            operations: ops,
            dry_run,
            backup_path: backup_path.clone(),
        },
    )?;

    // Write back if not dry-run
    if !dry_run {
        let writer = AmemWriter::new(graph.dimension());
        writer.write_to_file(&graph, path)?;
    }

    if json {
        let actions: Vec<serde_json::Value> = report
            .actions
            .iter()
            .map(|a| {
                serde_json::json!({
                    "operation": a.operation,
                    "description": a.description,
                    "affected_nodes": a.affected_nodes,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "dry_run": dry_run,
                "deduplicated": report.deduplicated,
                "contradictions_linked": report.contradictions_linked,
                "inferences_promoted": report.inferences_promoted,
                "backup_path": backup_path.map(|p| p.display().to_string()),
                "actions": actions,
            }))
            .unwrap_or_default()
        );
    } else {
        if dry_run {
            println!("Consolidation DRY RUN (use --confirm to apply):\n");
        } else {
            println!("Consolidation applied:\n");
            if let Some(bp) = &backup_path {
                println!("  Backup: {}", bp.display());
            }
        }
        for a in &report.actions {
            println!("  [{}] {}", a.operation, a.description);
        }
        println!();
        println!("  Deduplicated: {}", report.deduplicated);
        println!("  Contradictions linked: {}", report.contradictions_linked);
        println!("  Inferences promoted: {}", report.inferences_promoted);
    }
    Ok(())
}

/// Drift detection.
pub fn cmd_drift(
    path: &Path,
    topic: &str,
    limit: usize,
    min_relevance: f32,
    json: bool,
) -> AmemResult<()> {
    let graph = AmemReader::read_from_file(path)?;
    let query_engine = QueryEngine::new();

    let report = query_engine.drift_detection(
        &graph,
        DriftParams {
            topic: topic.to_string(),
            topic_vec: None,
            max_results: limit,
            min_relevance,
        },
    )?;

    if json {
        let timelines: Vec<serde_json::Value> = report
            .timelines
            .iter()
            .map(|t| {
                let snapshots: Vec<serde_json::Value> = t
                    .snapshots
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "node_id": s.node_id,
                            "session_id": s.session_id,
                            "confidence": s.confidence,
                            "content_preview": s.content_preview,
                            "change_type": format!("{:?}", s.change_type),
                        })
                    })
                    .collect();
                serde_json::json!({
                    "snapshots": snapshots,
                    "change_count": t.change_count,
                    "correction_count": t.correction_count,
                    "contradiction_count": t.contradiction_count,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "topic": topic,
                "timelines": timelines,
                "stability": report.stability,
                "likely_to_change": report.likely_to_change,
            }))
            .unwrap_or_default()
        );
    } else {
        println!("Drift analysis for {:?}:\n", topic);
        for (i, t) in report.timelines.iter().enumerate() {
            println!(
                "Timeline {} ({} changes, stability: {:.1}):",
                i + 1,
                t.change_count,
                report.stability
            );
            for s in &t.snapshots {
                let change = format!("{:?}", s.change_type).to_uppercase();
                println!(
                    "  Session {:>3}: {:<12} {:?}  [{:.2}]",
                    s.session_id, change, s.content_preview, s.confidence
                );
            }
            println!();
        }
        if report.timelines.is_empty() {
            println!("  No relevant nodes found for this topic.");
        } else {
            let prediction = if report.likely_to_change {
                "LIKELY TO CHANGE"
            } else {
                "STABLE"
            };
            println!(
                "Overall stability: {:.2} | Prediction: {}",
                report.stability, prediction
            );
        }
    }
    Ok(())
}
