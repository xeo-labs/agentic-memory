//! Memory formation pipeline — the write engine.

use crate::graph::MemoryGraph;
use crate::types::{
    now_micros, AmemError, AmemResult, CognitiveEvent, CognitiveEventBuilder, Edge, EdgeType,
    EventType,
};

use super::decay::calculate_decay;

/// Result of an ingest operation.
#[derive(Debug)]
pub struct IngestResult {
    /// IDs of newly created nodes.
    pub new_node_ids: Vec<u64>,
    /// Number of new edges created.
    pub new_edge_count: usize,
    /// IDs of nodes that were updated (touch count, last_accessed).
    pub touched_node_ids: Vec<u64>,
}

/// Report from running decay calculations.
#[derive(Debug)]
pub struct DecayReport {
    /// Number of nodes whose decay_score was updated.
    pub nodes_decayed: usize,
    /// Nodes whose decay_score dropped below 0.1 (candidates for archival).
    pub low_importance_nodes: Vec<u64>,
}

/// The write engine orchestrates memory formation.
pub struct WriteEngine {
    dimension: usize,
}

impl WriteEngine {
    /// Create a new write engine.
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// Process a batch of new cognitive events and integrate them into the graph.
    pub fn ingest(
        &self,
        graph: &mut MemoryGraph,
        events: Vec<CognitiveEvent>,
        edges: Vec<Edge>,
    ) -> AmemResult<IngestResult> {
        let mut new_node_ids = Vec::with_capacity(events.len());
        let mut touched_node_ids = Vec::new();

        // Step 1-3: Validate and add all events
        for event in events {
            let id = graph.add_node(event)?;
            new_node_ids.push(id);
        }

        // Step 4-5: Validate and add all edges
        let mut new_edge_count = 0;
        for edge in edges {
            graph.add_edge(edge)?;
            new_edge_count += 1;
        }

        // Ensure adjacency is rebuilt after bulk edge insertion
        graph.ensure_adjacency();

        // Step 8: Touch referenced nodes (nodes that existing edges point to)
        let new_id_set: std::collections::HashSet<u64> = new_node_ids.iter().copied().collect();
        for edge in graph.edges() {
            // If a new node has an edge to an existing node, touch that existing node
            if new_id_set.contains(&edge.source_id)
                && !new_id_set.contains(&edge.target_id)
                && !touched_node_ids.contains(&edge.target_id)
            {
                touched_node_ids.push(edge.target_id);
            }
        }

        for &id in &touched_node_ids {
            if let Some(node) = graph.get_node_mut(id) {
                node.access_count += 1;
                node.last_accessed = now_micros();
            }
        }

        Ok(IngestResult {
            new_node_ids,
            new_edge_count,
            touched_node_ids,
        })
    }

    /// Record a correction: marks old node as superseded, adds new node.
    pub fn correct(
        &self,
        graph: &mut MemoryGraph,
        old_node_id: u64,
        new_content: &str,
        session_id: u32,
    ) -> AmemResult<u64> {
        // Verify old node exists
        if graph.get_node(old_node_id).is_none() {
            return Err(AmemError::NodeNotFound(old_node_id));
        }

        // Create new correction node
        let event = CognitiveEventBuilder::new(EventType::Correction, new_content)
            .session_id(session_id)
            .confidence(1.0)
            .feature_vec(vec![0.0; self.dimension])
            .build();

        let new_id = graph.add_node(event)?;

        // Create SUPERSEDES edge from new to old
        let edge = Edge::new(new_id, old_node_id, EdgeType::Supersedes, 1.0);
        graph.add_edge(edge)?;

        // Ensure adjacency is rebuilt
        graph.ensure_adjacency();

        // Reduce old node's confidence to 0.0
        if let Some(old_node) = graph.get_node_mut(old_node_id) {
            old_node.confidence = 0.0;
        }

        Ok(new_id)
    }

    /// Compress a session into an episode node.
    pub fn compress_session(
        &self,
        graph: &mut MemoryGraph,
        session_id: u32,
        summary: &str,
    ) -> AmemResult<u64> {
        // Find all nodes in this session
        let session_node_ids: Vec<u64> = graph.session_index().get_session(session_id).to_vec();

        // Create episode node
        let event = CognitiveEventBuilder::new(EventType::Episode, summary)
            .session_id(session_id)
            .confidence(1.0)
            .feature_vec(vec![0.0; self.dimension])
            .build();

        let episode_id = graph.add_node(event)?;

        // Create PART_OF edges from each session node to the episode
        for &node_id in &session_node_ids {
            let edge = Edge::new(node_id, episode_id, EdgeType::PartOf, 1.0);
            graph.add_edge(edge)?;
        }

        // Ensure adjacency is rebuilt
        graph.ensure_adjacency();

        Ok(episode_id)
    }

    /// Touch a node (update access_count and last_accessed).
    pub fn touch(&self, graph: &mut MemoryGraph, node_id: u64) -> AmemResult<()> {
        let node = graph
            .get_node_mut(node_id)
            .ok_or(AmemError::NodeNotFound(node_id))?;
        node.access_count += 1;
        node.last_accessed = now_micros();
        Ok(())
    }

    /// Run decay calculations across all nodes.
    pub fn run_decay(&self, graph: &mut MemoryGraph, current_time: u64) -> AmemResult<DecayReport> {
        let mut nodes_decayed = 0;
        let mut low_importance_nodes = Vec::new();

        // Collect node IDs first to avoid borrow issues
        let node_ids: Vec<u64> = graph.nodes().iter().map(|n| n.id).collect();

        for id in node_ids {
            if let Some(node) = graph.get_node_mut(id) {
                let new_score = calculate_decay(node, current_time);
                if (new_score - node.decay_score).abs() > f32::EPSILON {
                    node.decay_score = new_score;
                    nodes_decayed += 1;
                }
                if new_score < 0.1 {
                    low_importance_nodes.push(id);
                }
            }
        }

        Ok(DecayReport {
            nodes_decayed,
            low_importance_nodes,
        })
    }
}
