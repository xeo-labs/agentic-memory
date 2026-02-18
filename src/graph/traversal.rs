//! Graph traversal algorithms (BFS).

use std::collections::{HashMap, HashSet, VecDeque};

use crate::types::{AmemError, AmemResult, Edge, EdgeType};

use super::MemoryGraph;

/// Direction for graph traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    /// Follow outgoing edges (source -> target).
    Forward,
    /// Follow incoming edges (target <- source).
    Backward,
    /// Follow edges in both directions.
    Both,
}

/// BFS traversal from a starting node, following specific edge types.
#[allow(clippy::type_complexity)]
pub fn bfs_traverse(
    graph: &MemoryGraph,
    start_id: u64,
    edge_types: &[EdgeType],
    direction: TraversalDirection,
    max_depth: u32,
    max_results: usize,
    min_confidence: f32,
) -> AmemResult<(Vec<u64>, Vec<Edge>, HashMap<u64, u32>)> {
    if graph.get_node(start_id).is_none() {
        return Err(AmemError::NodeNotFound(start_id));
    }

    let edge_set: HashSet<EdgeType> = edge_types.iter().copied().collect();
    let mut visited: HashSet<u64> = HashSet::new();
    let mut visited_order: Vec<u64> = Vec::new();
    let mut edges_traversed: Vec<Edge> = Vec::new();
    let mut depths: HashMap<u64, u32> = HashMap::new();
    let mut queue: VecDeque<(u64, u32)> = VecDeque::new();

    visited.insert(start_id);
    visited_order.push(start_id);
    depths.insert(start_id, 0);
    queue.push_back((start_id, 0));

    while let Some((current_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if visited_order.len() >= max_results {
            break;
        }

        let mut neighbors: Vec<(u64, Edge)> = Vec::new();

        // Forward: follow outgoing edges
        if direction == TraversalDirection::Forward || direction == TraversalDirection::Both {
            for edge in graph.edges_from(current_id) {
                if edge_set.contains(&edge.edge_type) {
                    neighbors.push((edge.target_id, *edge));
                }
            }
        }

        // Backward: follow incoming edges
        if direction == TraversalDirection::Backward || direction == TraversalDirection::Both {
            for edge in graph.edges_to(current_id) {
                if edge_set.contains(&edge.edge_type) {
                    neighbors.push((edge.source_id, *edge));
                }
            }
        }

        for (neighbor_id, edge) in neighbors {
            if visited.contains(&neighbor_id) {
                continue;
            }
            if visited_order.len() >= max_results {
                break;
            }

            // Check confidence threshold
            if let Some(node) = graph.get_node(neighbor_id) {
                if node.confidence < min_confidence {
                    continue;
                }
            } else {
                continue;
            }

            visited.insert(neighbor_id);
            visited_order.push(neighbor_id);
            depths.insert(neighbor_id, depth + 1);
            edges_traversed.push(edge);
            queue.push_back((neighbor_id, depth + 1));
        }
    }

    Ok((visited_order, edges_traversed, depths))
}
