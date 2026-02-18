//! Session index — maps each session_id to sorted node IDs in that session.

use std::collections::HashMap;

use crate::types::CognitiveEvent;

/// Maps each session_id to a sorted list of node IDs in that session.
pub struct SessionIndex {
    index: HashMap<u32, Vec<u64>>,
}

impl SessionIndex {
    /// Create a new, empty session index.
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
        }
    }

    /// Get all node IDs from a specific session.
    pub fn get_session(&self, session_id: u32) -> &[u64] {
        self.index
            .get(&session_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all node IDs from multiple sessions, merged and sorted.
    pub fn get_sessions(&self, session_ids: &[u32]) -> Vec<u64> {
        let mut result: Vec<u64> = Vec::new();
        for sid in session_ids {
            if let Some(ids) = self.index.get(sid) {
                result.extend_from_slice(ids);
            }
        }
        result.sort_unstable();
        result.dedup();
        result
    }

    /// Get all known session IDs.
    pub fn session_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.index.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Count sessions.
    pub fn session_count(&self) -> usize {
        self.index.len()
    }

    /// Count nodes in a session.
    pub fn node_count(&self, session_id: u32) -> usize {
        self.index.get(&session_id).map(|v| v.len()).unwrap_or(0)
    }

    /// Rebuild the entire index from a slice of nodes.
    pub fn rebuild(&mut self, nodes: &[CognitiveEvent]) {
        self.index.clear();
        for node in nodes {
            self.index.entry(node.session_id).or_default().push(node.id);
        }
        for list in self.index.values_mut() {
            list.sort_unstable();
        }
    }

    /// Incrementally add a new node.
    pub fn add_node(&mut self, event: &CognitiveEvent) {
        let list = self.index.entry(event.session_id).or_default();
        let pos = list.binary_search(&event.id).unwrap_or_else(|p| p);
        list.insert(pos, event.id);
    }

    /// Remove a node from the index.
    pub fn remove_node(&mut self, id: u64, session_id: u32) {
        if let Some(list) = self.index.get_mut(&session_id) {
            if let Ok(pos) = list.binary_search(&id) {
                list.remove(pos);
            }
            if list.is_empty() {
                self.index.remove(&session_id);
            }
        }
    }

    /// Clear the index.
    pub fn clear(&mut self) {
        self.index.clear();
    }

    /// Number of total entries across all sessions.
    pub fn len(&self) -> usize {
        self.index.values().map(|v| v.len()).sum()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a reference to the underlying map (for serialization).
    pub fn inner(&self) -> &HashMap<u32, Vec<u64>> {
        &self.index
    }
}

impl Default for SessionIndex {
    fn default() -> Self {
        Self::new()
    }
}
