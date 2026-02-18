//! Query executor — all query types.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::graph::traversal::{bfs_traverse, TraversalDirection};
use crate::graph::MemoryGraph;
use crate::index::cosine_similarity;
use crate::types::{AmemError, AmemResult, CognitiveEvent, Edge, EdgeType, EventType};

/// Parameters for a traversal query.
pub struct TraversalParams {
    /// Starting node ID.
    pub start_id: u64,
    /// Which edge types to follow.
    pub edge_types: Vec<EdgeType>,
    /// Direction of traversal.
    pub direction: TraversalDirection,
    /// Maximum depth (number of hops).
    pub max_depth: u32,
    /// Maximum number of nodes to return.
    pub max_results: usize,
    /// Minimum confidence threshold for visited nodes.
    pub min_confidence: f32,
}

/// Result of a traversal query.
pub struct TraversalResult {
    /// Ordered list of visited node IDs (BFS order).
    pub visited: Vec<u64>,
    /// The edges that were traversed.
    pub edges_traversed: Vec<Edge>,
    /// Depth at which each node was found.
    pub depths: HashMap<u64, u32>,
}

/// Sort order for pattern queries.
#[derive(Debug, Clone, Copy)]
pub enum PatternSort {
    /// Most recent first.
    MostRecent,
    /// Highest confidence first.
    HighestConfidence,
    /// Most accessed first.
    MostAccessed,
    /// Highest decay score first.
    MostImportant,
}

/// Parameters for a pattern query.
pub struct PatternParams {
    /// Filter by event type(s). Empty = all types.
    pub event_types: Vec<EventType>,
    /// Minimum confidence (inclusive).
    pub min_confidence: Option<f32>,
    /// Maximum confidence (inclusive).
    pub max_confidence: Option<f32>,
    /// Filter by session ID(s). Empty = all sessions.
    pub session_ids: Vec<u32>,
    /// Filter by creation time: after this timestamp.
    pub created_after: Option<u64>,
    /// Filter by creation time: before this timestamp.
    pub created_before: Option<u64>,
    /// Filter by minimum decay score.
    pub min_decay_score: Option<f32>,
    /// Maximum number of results.
    pub max_results: usize,
    /// Sort order.
    pub sort_by: PatternSort,
}

/// Time range for temporal queries.
pub enum TimeRange {
    /// All nodes created in this timestamp range.
    TimeWindow { start: u64, end: u64 },
    /// All nodes from this session.
    Session(u32),
    /// All nodes from these sessions.
    Sessions(Vec<u32>),
}

/// Parameters for a temporal query.
pub struct TemporalParams {
    /// First time range.
    pub range_a: TimeRange,
    /// Second time range.
    pub range_b: TimeRange,
}

/// Result of a temporal query.
pub struct TemporalResult {
    /// Nodes that exist in range_b but not range_a (new knowledge).
    pub added: Vec<u64>,
    /// Nodes in range_a that were superseded by nodes in range_b.
    pub corrected: Vec<(u64, u64)>,
    /// Nodes in range_a with no corresponding update in range_b (unchanged).
    pub unchanged: Vec<u64>,
    /// Nodes only in range_a that have low decay scores (potentially stale).
    pub potentially_stale: Vec<u64>,
}

/// Parameters for a causal (impact) query.
pub struct CausalParams {
    /// The node to analyze impact for.
    pub node_id: u64,
    /// Maximum depth to traverse.
    pub max_depth: u32,
    /// Which dependency edge types to follow.
    pub dependency_types: Vec<EdgeType>,
}

/// Result of a causal query.
pub struct CausalResult {
    /// The root node being analyzed.
    pub root_id: u64,
    /// All nodes that directly or indirectly depend on the root.
    pub dependents: Vec<u64>,
    /// The dependency tree: node_id -> list of (dependent_id, edge_type).
    pub dependency_tree: HashMap<u64, Vec<(u64, EdgeType)>>,
    /// Total number of decisions that depend on this node.
    pub affected_decisions: usize,
    /// Total number of inferences that depend on this node.
    pub affected_inferences: usize,
}

/// Parameters for a similarity query.
pub struct SimilarityParams {
    /// Query vector (must match graph dimension).
    pub query_vec: Vec<f32>,
    /// Maximum number of results.
    pub top_k: usize,
    /// Minimum similarity threshold.
    pub min_similarity: f32,
    /// Filter by event type(s). Empty = all types.
    pub event_types: Vec<EventType>,
    /// Exclude nodes with zero vectors.
    pub skip_zero_vectors: bool,
}

/// A match from a similarity search.
pub struct SimilarityMatchResult {
    /// The node ID.
    pub node_id: u64,
    /// The similarity score.
    pub similarity: f32,
}

/// A subgraph extracted around a center node.
pub struct SubGraph {
    /// All nodes in the subgraph.
    pub nodes: Vec<CognitiveEvent>,
    /// All edges in the subgraph.
    pub edges: Vec<Edge>,
    /// The center node ID.
    pub center_id: u64,
}

/// The query engine supports all query operations.
pub struct QueryEngine;

impl QueryEngine {
    /// Create a new query engine.
    pub fn new() -> Self {
        Self
    }

    /// Traverse from a starting node following specific edge types.
    pub fn traverse(
        &self,
        graph: &MemoryGraph,
        params: TraversalParams,
    ) -> AmemResult<TraversalResult> {
        let (visited, edges_traversed, depths) = bfs_traverse(
            graph,
            params.start_id,
            &params.edge_types,
            params.direction,
            params.max_depth,
            params.max_results,
            params.min_confidence,
        )?;

        Ok(TraversalResult {
            visited,
            edges_traversed,
            depths,
        })
    }

    /// Find nodes matching conditions.
    pub fn pattern<'a>(
        &self,
        graph: &'a MemoryGraph,
        params: PatternParams,
    ) -> AmemResult<Vec<&'a CognitiveEvent>> {
        // Start with candidate set
        let mut candidates: Vec<&CognitiveEvent> = if !params.event_types.is_empty() {
            let ids = graph.type_index().get_any(&params.event_types);
            ids.iter().filter_map(|&id| graph.get_node(id)).collect()
        } else if !params.session_ids.is_empty() {
            let ids = graph.session_index().get_sessions(&params.session_ids);
            ids.iter().filter_map(|&id| graph.get_node(id)).collect()
        } else {
            graph.nodes().iter().collect()
        };

        // Apply filters
        if !params.event_types.is_empty() {
            let type_set: HashSet<EventType> = params.event_types.iter().copied().collect();
            candidates.retain(|n| type_set.contains(&n.event_type));
        }

        if !params.session_ids.is_empty() {
            let session_set: HashSet<u32> = params.session_ids.iter().copied().collect();
            candidates.retain(|n| session_set.contains(&n.session_id));
        }

        if let Some(min_conf) = params.min_confidence {
            candidates.retain(|n| n.confidence >= min_conf);
        }
        if let Some(max_conf) = params.max_confidence {
            candidates.retain(|n| n.confidence <= max_conf);
        }
        if let Some(after) = params.created_after {
            candidates.retain(|n| n.created_at >= after);
        }
        if let Some(before) = params.created_before {
            candidates.retain(|n| n.created_at <= before);
        }
        if let Some(min_decay) = params.min_decay_score {
            candidates.retain(|n| n.decay_score >= min_decay);
        }

        // Sort
        match params.sort_by {
            PatternSort::MostRecent => {
                candidates.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            }
            PatternSort::HighestConfidence => {
                candidates.sort_by(|a, b| {
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            PatternSort::MostAccessed => {
                candidates.sort_by(|a, b| b.access_count.cmp(&a.access_count));
            }
            PatternSort::MostImportant => {
                candidates.sort_by(|a, b| {
                    b.decay_score
                        .partial_cmp(&a.decay_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        candidates.truncate(params.max_results);
        Ok(candidates)
    }

    /// Compare graph state across time ranges or sessions.
    pub fn temporal(
        &self,
        graph: &MemoryGraph,
        params: TemporalParams,
    ) -> AmemResult<TemporalResult> {
        let nodes_a = self.collect_range_nodes(graph, &params.range_a);
        let nodes_b = self.collect_range_nodes(graph, &params.range_b);

        let set_a: HashSet<u64> = nodes_a.iter().copied().collect();
        let _set_b: HashSet<u64> = nodes_b.iter().copied().collect();

        // Find corrections: SUPERSEDES edges from range_b nodes to range_a nodes
        let mut corrected = Vec::new();
        for &id_b in &nodes_b {
            for edge in graph.edges_from(id_b) {
                if edge.edge_type == EdgeType::Supersedes && set_a.contains(&edge.target_id) {
                    corrected.push((edge.target_id, id_b));
                }
            }
        }

        let corrected_a: HashSet<u64> = corrected.iter().map(|(old, _)| *old).collect();

        // Added: in B but not connected to A via supersedes
        let added: Vec<u64> = nodes_b
            .iter()
            .filter(|id| !set_a.contains(id))
            .copied()
            .collect();

        // Unchanged: in A, not corrected, decay_score > 0.3
        let unchanged: Vec<u64> = nodes_a
            .iter()
            .filter(|&&id| {
                !corrected_a.contains(&id)
                    && graph
                        .get_node(id)
                        .map(|n| n.decay_score > 0.3)
                        .unwrap_or(false)
            })
            .copied()
            .collect();

        // Potentially stale: in A, decay_score < 0.3, no access in B
        let potentially_stale: Vec<u64> = nodes_a
            .iter()
            .filter(|&&id| {
                !corrected_a.contains(&id)
                    && graph
                        .get_node(id)
                        .map(|n| n.decay_score < 0.3)
                        .unwrap_or(false)
            })
            .copied()
            .collect();

        Ok(TemporalResult {
            added,
            corrected,
            unchanged,
            potentially_stale,
        })
    }

    fn collect_range_nodes(&self, graph: &MemoryGraph, range: &TimeRange) -> Vec<u64> {
        match range {
            TimeRange::TimeWindow { start, end } => graph.temporal_index().range(*start, *end),
            TimeRange::Session(sid) => graph.session_index().get_session(*sid).to_vec(),
            TimeRange::Sessions(sids) => graph.session_index().get_sessions(sids),
        }
    }

    /// Impact analysis: what depends on a given node?
    pub fn causal(&self, graph: &MemoryGraph, params: CausalParams) -> AmemResult<CausalResult> {
        if graph.get_node(params.node_id).is_none() {
            return Err(AmemError::NodeNotFound(params.node_id));
        }

        let dep_set: HashSet<EdgeType> = params.dependency_types.iter().copied().collect();
        let mut dependents: Vec<u64> = Vec::new();
        let mut dependency_tree: HashMap<u64, Vec<(u64, EdgeType)>> = HashMap::new();
        let mut visited: HashSet<u64> = HashSet::new();
        let mut queue: VecDeque<(u64, u32)> = VecDeque::new();

        visited.insert(params.node_id);
        queue.push_back((params.node_id, 0));

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= params.max_depth {
                continue;
            }

            // Find all nodes that have dependency edges pointing TO current_id
            // These are nodes that depend on current_id
            for edge in graph.edges_to(current_id) {
                if dep_set.contains(&edge.edge_type) && !visited.contains(&edge.source_id) {
                    visited.insert(edge.source_id);
                    dependents.push(edge.source_id);
                    dependency_tree
                        .entry(current_id)
                        .or_default()
                        .push((edge.source_id, edge.edge_type));
                    queue.push_back((edge.source_id, depth + 1));
                }
            }
        }

        let mut affected_decisions = 0;
        let mut affected_inferences = 0;
        for &dep_id in &dependents {
            if let Some(node) = graph.get_node(dep_id) {
                match node.event_type {
                    EventType::Decision => affected_decisions += 1,
                    EventType::Inference => affected_inferences += 1,
                    _ => {}
                }
            }
        }

        Ok(CausalResult {
            root_id: params.node_id,
            dependents,
            dependency_tree,
            affected_decisions,
            affected_inferences,
        })
    }

    /// Find similar nodes using feature vector cosine similarity.
    pub fn similarity(
        &self,
        graph: &MemoryGraph,
        params: SimilarityParams,
    ) -> AmemResult<Vec<SimilarityMatchResult>> {
        let type_filter: HashSet<EventType> = params.event_types.iter().copied().collect();

        let mut matches: Vec<SimilarityMatchResult> = Vec::new();

        for node in graph.nodes() {
            // Type filter
            if !type_filter.is_empty() && !type_filter.contains(&node.event_type) {
                continue;
            }

            // Skip zero vectors
            if params.skip_zero_vectors && node.feature_vec.iter().all(|&x| x == 0.0) {
                continue;
            }

            let sim = cosine_similarity(&params.query_vec, &node.feature_vec);
            if sim >= params.min_similarity {
                matches.push(SimilarityMatchResult {
                    node_id: node.id,
                    similarity: sim,
                });
            }
        }

        matches.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(params.top_k);

        Ok(matches)
    }

    /// Get the full context for a node: the node itself, all edges, and connected nodes.
    pub fn context(&self, graph: &MemoryGraph, node_id: u64, depth: u32) -> AmemResult<SubGraph> {
        if graph.get_node(node_id).is_none() {
            return Err(AmemError::NodeNotFound(node_id));
        }

        // BFS in all directions, following all edge types
        let all_edge_types: Vec<EdgeType> = vec![
            EdgeType::CausedBy,
            EdgeType::Supports,
            EdgeType::Contradicts,
            EdgeType::Supersedes,
            EdgeType::RelatedTo,
            EdgeType::PartOf,
            EdgeType::TemporalNext,
        ];

        let (visited, _, _) = bfs_traverse(
            graph,
            node_id,
            &all_edge_types,
            TraversalDirection::Both,
            depth,
            usize::MAX,
            0.0,
        )?;

        let visited_set: HashSet<u64> = visited.iter().copied().collect();

        // Collect nodes
        let nodes: Vec<CognitiveEvent> = visited
            .iter()
            .filter_map(|&id| graph.get_node(id).cloned())
            .collect();

        // Collect edges where both endpoints are in the visited set
        let edges: Vec<Edge> = graph
            .edges()
            .iter()
            .filter(|e| visited_set.contains(&e.source_id) && visited_set.contains(&e.target_id))
            .copied()
            .collect();

        Ok(SubGraph {
            nodes,
            edges,
            center_id: node_id,
        })
    }

    /// Get the latest version of a node, following SUPERSEDES chains.
    pub fn resolve<'a>(
        &self,
        graph: &'a MemoryGraph,
        node_id: u64,
    ) -> AmemResult<&'a CognitiveEvent> {
        let mut current_id = node_id;

        if graph.get_node(current_id).is_none() {
            return Err(AmemError::NodeNotFound(node_id));
        }

        for _ in 0..100 {
            // Find if any node supersedes the current one
            let mut superseded_by = None;
            for edge in graph.edges_to(current_id) {
                if edge.edge_type == EdgeType::Supersedes {
                    superseded_by = Some(edge.source_id);
                    break;
                }
            }

            match superseded_by {
                Some(new_id) => current_id = new_id,
                None => break,
            }
        }

        graph
            .get_node(current_id)
            .ok_or(AmemError::NodeNotFound(current_id))
    }
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}
