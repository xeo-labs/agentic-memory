//! Index by event type — maps each EventType to sorted node IDs.

use std::collections::HashMap;

use crate::types::{CognitiveEvent, EventType};

/// Maps each EventType to a sorted list of node IDs.
pub struct TypeIndex {
    index: HashMap<EventType, Vec<u64>>,
}

impl TypeIndex {
    /// Create a new, empty type index.
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
        }
    }

    /// Get all node IDs of a given type.
    pub fn get(&self, event_type: EventType) -> &[u64] {
        self.index
            .get(&event_type)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all node IDs matching any of the given types, merged and sorted.
    pub fn get_any(&self, event_types: &[EventType]) -> Vec<u64> {
        let mut result: Vec<u64> = Vec::new();
        for et in event_types {
            if let Some(ids) = self.index.get(et) {
                result.extend_from_slice(ids);
            }
        }
        result.sort_unstable();
        result.dedup();
        result
    }

    /// Count nodes of a given type.
    pub fn count(&self, event_type: EventType) -> usize {
        self.index.get(&event_type).map(|v| v.len()).unwrap_or(0)
    }

    /// Rebuild the entire index from a slice of nodes.
    pub fn rebuild(&mut self, nodes: &[CognitiveEvent]) {
        self.index.clear();
        for node in nodes {
            self.index.entry(node.event_type).or_default().push(node.id);
        }
        // Ensure each list is sorted
        for list in self.index.values_mut() {
            list.sort_unstable();
        }
    }

    /// Incrementally add a new node.
    pub fn add_node(&mut self, event: &CognitiveEvent) {
        let list = self.index.entry(event.event_type).or_default();
        let pos = list.binary_search(&event.id).unwrap_or_else(|p| p);
        list.insert(pos, event.id);
    }

    /// Remove a node from the index.
    pub fn remove_node(&mut self, id: u64, event_type: EventType) {
        if let Some(list) = self.index.get_mut(&event_type) {
            if let Ok(pos) = list.binary_search(&id) {
                list.remove(pos);
            }
        }
    }

    /// Clear the index.
    pub fn clear(&mut self) {
        self.index.clear();
    }

    /// Number of total entries across all types.
    pub fn len(&self) -> usize {
        self.index.values().map(|v| v.len()).sum()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a reference to the underlying map (for serialization).
    pub fn inner(&self) -> &HashMap<EventType, Vec<u64>> {
        &self.index
    }
}

impl Default for TypeIndex {
    fn default() -> Self {
        Self::new()
    }
}
