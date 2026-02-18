//! Core graph structure — nodes + edges with adjacency indexes.

use std::collections::HashMap;

use crate::index::{ClusterMap, SessionIndex, TemporalIndex, TypeIndex};
use crate::types::{AmemError, AmemResult, CognitiveEvent, Edge, EdgeType, MAX_EDGES_PER_NODE};

/// The core in-memory graph structure holding cognitive events and their relationships.
pub struct MemoryGraph {
    /// All nodes, indexed by ID.
    nodes: Vec<CognitiveEvent>,
    /// All edges, grouped by source_id.
    edges: Vec<Edge>,
    /// Adjacency index: source_id -> (start_index, count) in edges vec.
    adjacency: HashMap<u64, (usize, usize)>,
    /// Reverse adjacency: target_id -> list of source_ids.
    reverse_adjacency: HashMap<u64, Vec<u64>>,
    /// Next available node ID.
    next_id: u64,
    /// Feature vector dimension.
    dimension: usize,
    /// Type index.
    pub(crate) type_index: TypeIndex,
    /// Temporal index.
    pub(crate) temporal_index: TemporalIndex,
    /// Session index.
    pub(crate) session_index: SessionIndex,
    /// Cluster map.
    pub(crate) cluster_map: ClusterMap,
}

impl MemoryGraph {
    /// Create a new empty graph.
    pub fn new(dimension: usize) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            adjacency: HashMap::new(),
            reverse_adjacency: HashMap::new(),
            next_id: 0,
            dimension,
            type_index: TypeIndex::new(),
            temporal_index: TemporalIndex::new(),
            session_index: SessionIndex::new(),
            cluster_map: ClusterMap::new(dimension),
        }
    }

    /// Create from pre-existing data (used by reader).
    pub fn from_parts(
        nodes: Vec<CognitiveEvent>,
        edges: Vec<Edge>,
        dimension: usize,
    ) -> AmemResult<Self> {
        let next_id = nodes.iter().map(|n| n.id + 1).max().unwrap_or(0);

        let mut graph = Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            adjacency: HashMap::new(),
            reverse_adjacency: HashMap::new(),
            next_id,
            dimension,
            type_index: TypeIndex::new(),
            temporal_index: TemporalIndex::new(),
            session_index: SessionIndex::new(),
            cluster_map: ClusterMap::new(dimension),
        };

        // Insert nodes directly (they already have IDs assigned)
        graph.nodes = nodes;

        // Rebuild indexes from nodes
        graph.type_index.rebuild(&graph.nodes);
        graph.temporal_index.rebuild(&graph.nodes);
        graph.session_index.rebuild(&graph.nodes);

        // Sort edges by source_id, then target_id
        let mut sorted_edges = edges;
        sorted_edges.sort_by(|a, b| {
            a.source_id
                .cmp(&b.source_id)
                .then(a.target_id.cmp(&b.target_id))
        });
        graph.edges = sorted_edges;

        // Build adjacency indexes
        graph.rebuild_adjacency();

        Ok(graph)
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get a node by ID (immutable).
    pub fn get_node(&self, id: u64) -> Option<&CognitiveEvent> {
        // Fast path: if IDs are sequential, nodes[id] has id == id
        let idx = id as usize;
        if idx < self.nodes.len() && self.nodes[idx].id == id {
            return Some(&self.nodes[idx]);
        }
        // Fallback: linear scan (needed after remove_node)
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get a node by ID (mutable).
    pub fn get_node_mut(&mut self, id: u64) -> Option<&mut CognitiveEvent> {
        // Fast path: if IDs are sequential, nodes[id] has id == id
        let idx = id as usize;
        if idx < self.nodes.len() && self.nodes[idx].id == id {
            return Some(&mut self.nodes[idx]);
        }
        // Fallback: linear scan (needed after remove_node)
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Ensure adjacency indexes are up to date.
    /// No-op in the current implementation (adjacency is always up to date).
    pub fn ensure_adjacency(&mut self) {
        // Currently a no-op — adjacency is rebuilt on every mutation.
    }

    /// Get all edges from a source node.
    pub fn edges_from(&self, source_id: u64) -> &[Edge] {
        if let Some(&(start, count)) = self.adjacency.get(&source_id) {
            &self.edges[start..start + count]
        } else {
            &[]
        }
    }

    /// Get all edges that point TO this node.
    pub fn edges_to(&self, target_id: u64) -> Vec<&Edge> {
        if let Some(sources) = self.reverse_adjacency.get(&target_id) {
            let mut result = Vec::new();
            for &src_id in sources {
                for edge in self.edges_from(src_id) {
                    if edge.target_id == target_id {
                        result.push(edge);
                    }
                }
            }
            result
        } else {
            Vec::new()
        }
    }

    /// Get all nodes (immutable slice).
    pub fn nodes(&self) -> &[CognitiveEvent] {
        &self.nodes
    }

    /// Get all edges (immutable slice).
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The feature vector dimension for this graph.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Add a node, returns the assigned ID.
    pub fn add_node(&mut self, mut event: CognitiveEvent) -> AmemResult<u64> {
        // Validate content size
        event.validate(self.dimension)?;

        // Pad feature vec if empty
        if event.feature_vec.is_empty() {
            event.feature_vec = vec![0.0; self.dimension];
        } else if event.feature_vec.len() != self.dimension {
            return Err(AmemError::DimensionMismatch {
                expected: self.dimension,
                got: event.feature_vec.len(),
            });
        }

        // Assign ID
        let id = self.next_id;
        event.id = id;
        self.next_id += 1;

        // Update indexes
        self.type_index.add_node(&event);
        self.temporal_index.add_node(&event);
        self.session_index.add_node(&event);

        self.nodes.push(event);

        Ok(id)
    }

    /// Add an edge between two existing nodes.
    pub fn add_edge(&mut self, edge: Edge) -> AmemResult<()> {
        // Validate: no self-edges
        if edge.source_id == edge.target_id {
            return Err(AmemError::SelfEdge(edge.source_id));
        }

        // Validate: source exists
        if self.get_node(edge.source_id).is_none() {
            return Err(AmemError::NodeNotFound(edge.source_id));
        }

        // Validate: target exists
        if self.get_node(edge.target_id).is_none() {
            return Err(AmemError::InvalidEdgeTarget(edge.target_id));
        }

        // Check max edges per node
        let current_count = self
            .adjacency
            .get(&edge.source_id)
            .map(|(_, c)| *c)
            .unwrap_or(0);
        if current_count >= MAX_EDGES_PER_NODE as usize {
            return Err(AmemError::TooManyEdges(MAX_EDGES_PER_NODE));
        }

        self.edges.push(edge);
        self.rebuild_adjacency();

        Ok(())
    }

    /// Remove a node and all its edges.
    pub fn remove_node(&mut self, id: u64) -> AmemResult<CognitiveEvent> {
        let pos = self
            .nodes
            .iter()
            .position(|n| n.id == id)
            .ok_or(AmemError::NodeNotFound(id))?;

        let removed = self.nodes.remove(pos);

        // Remove from indexes
        self.type_index.remove_node(id, removed.event_type);
        self.temporal_index.remove_node(id, removed.created_at);
        self.session_index.remove_node(id, removed.session_id);

        // Remove all edges involving this node
        self.edges
            .retain(|e| e.source_id != id && e.target_id != id);

        // Rebuild adjacency
        self.rebuild_adjacency();

        Ok(removed)
    }

    /// Remove a specific edge.
    pub fn remove_edge(
        &mut self,
        source_id: u64,
        target_id: u64,
        edge_type: EdgeType,
    ) -> AmemResult<()> {
        let initial_len = self.edges.len();
        self.edges.retain(|e| {
            !(e.source_id == source_id && e.target_id == target_id && e.edge_type == edge_type)
        });
        if self.edges.len() == initial_len {
            return Err(AmemError::NodeNotFound(source_id));
        }
        self.rebuild_adjacency();
        Ok(())
    }

    /// Rebuild adjacency indexes from the current edge list.
    fn rebuild_adjacency(&mut self) {
        self.adjacency.clear();
        self.reverse_adjacency.clear();

        // Sort edges by source_id, then target_id
        self.edges.sort_by(|a, b| {
            a.source_id
                .cmp(&b.source_id)
                .then(a.target_id.cmp(&b.target_id))
        });

        let mut i = 0;
        while i < self.edges.len() {
            let source = self.edges[i].source_id;
            let start = i;
            while i < self.edges.len() && self.edges[i].source_id == source {
                // Build reverse adjacency
                self.reverse_adjacency
                    .entry(self.edges[i].target_id)
                    .or_default()
                    .push(source);
                i += 1;
            }
            self.adjacency.insert(source, (start, i - start));
        }

        // Dedup reverse adjacency
        for list in self.reverse_adjacency.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
    }

    /// Get the next available node ID (for builder use).
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Get the type index.
    pub fn type_index(&self) -> &TypeIndex {
        &self.type_index
    }

    /// Get the temporal index.
    pub fn temporal_index(&self) -> &TemporalIndex {
        &self.temporal_index
    }

    /// Get the session index.
    pub fn session_index(&self) -> &SessionIndex {
        &self.session_index
    }

    /// Get the cluster map.
    pub fn cluster_map(&self) -> &ClusterMap {
        &self.cluster_map
    }

    /// Get a mutable reference to the cluster map.
    pub fn cluster_map_mut(&mut self) -> &mut ClusterMap {
        &mut self.cluster_map
    }
}
