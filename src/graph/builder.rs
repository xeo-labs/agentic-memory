//! Fluent API for building MemoryGraph instances.

use crate::types::{
    AmemResult, CognitiveEvent, CognitiveEventBuilder, Edge, EdgeType, EventType, DEFAULT_DIMENSION,
};

use super::MemoryGraph;

/// Fluent builder for constructing a MemoryGraph.
pub struct GraphBuilder {
    dimension: usize,
    nodes: Vec<CognitiveEvent>,
    edges: Vec<Edge>,
    next_id: u64,
}

impl GraphBuilder {
    /// Create a new builder with the default dimension.
    pub fn new() -> Self {
        Self {
            dimension: DEFAULT_DIMENSION,
            nodes: Vec::new(),
            edges: Vec::new(),
            next_id: 0,
        }
    }

    /// Create a new builder with a specific dimension.
    pub fn with_dimension(dim: usize) -> Self {
        Self {
            dimension: dim,
            nodes: Vec::new(),
            edges: Vec::new(),
            next_id: 0,
        }
    }

    fn add_event(
        &mut self,
        event_type: EventType,
        content: &str,
        session_id: u32,
        confidence: f32,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let event = CognitiveEventBuilder::new(event_type, content)
            .session_id(session_id)
            .confidence(confidence)
            .feature_vec(vec![0.0; self.dimension])
            .build();
        let mut event = event;
        event.id = id;
        self.nodes.push(event);
        id
    }

    /// Add a fact.
    pub fn add_fact(&mut self, content: &str, session_id: u32, confidence: f32) -> u64 {
        self.add_event(EventType::Fact, content, session_id, confidence)
    }

    /// Add a decision.
    pub fn add_decision(&mut self, content: &str, session_id: u32, confidence: f32) -> u64 {
        self.add_event(EventType::Decision, content, session_id, confidence)
    }

    /// Add an inference.
    pub fn add_inference(&mut self, content: &str, session_id: u32, confidence: f32) -> u64 {
        self.add_event(EventType::Inference, content, session_id, confidence)
    }

    /// Add a correction (automatically creates SUPERSEDES edge to old_node_id).
    pub fn add_correction(&mut self, content: &str, session_id: u32, old_node_id: u64) -> u64 {
        let id = self.add_event(EventType::Correction, content, session_id, 1.0);
        self.edges
            .push(Edge::new(id, old_node_id, EdgeType::Supersedes, 1.0));
        // Reduce old node's confidence
        if let Some(old_node) = self.nodes.iter_mut().find(|n| n.id == old_node_id) {
            old_node.confidence = 0.0;
        }
        id
    }

    /// Add a skill.
    pub fn add_skill(&mut self, content: &str, session_id: u32, confidence: f32) -> u64 {
        self.add_event(EventType::Skill, content, session_id, confidence)
    }

    /// Add an episode summary (automatically creates PART_OF edges).
    pub fn add_episode(&mut self, content: &str, session_id: u32, member_node_ids: &[u64]) -> u64 {
        let id = self.add_event(EventType::Episode, content, session_id, 1.0);
        for &member_id in member_node_ids {
            self.edges
                .push(Edge::new(member_id, id, EdgeType::PartOf, 1.0));
        }
        id
    }

    /// Add an edge between two nodes.
    pub fn link(
        &mut self,
        source_id: u64,
        target_id: u64,
        edge_type: EdgeType,
        weight: f32,
    ) -> &mut Self {
        self.edges
            .push(Edge::new(source_id, target_id, edge_type, weight));
        self
    }

    /// Set feature vector for a node.
    pub fn set_feature_vec(&mut self, node_id: u64, vec: Vec<f32>) -> &mut Self {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            node.feature_vec = vec;
        }
        self
    }

    /// Build the final MemoryGraph.
    pub fn build(self) -> AmemResult<MemoryGraph> {
        MemoryGraph::from_parts(self.nodes, self.edges, self.dimension)
    }
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}
