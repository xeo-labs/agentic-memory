//! Temporal index — sorted (timestamp, node_id) pairs for time range queries.

use crate::types::CognitiveEvent;

/// Sorted list of (created_at, node_id) pairs for efficient time range queries.
pub struct TemporalIndex {
    /// Sorted by timestamp ascending.
    entries: Vec<(u64, u64)>,
}

impl TemporalIndex {
    /// Create a new, empty temporal index.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Get all node IDs created within a time range (inclusive).
    pub fn range(&self, start: u64, end: u64) -> Vec<u64> {
        let lo = self.entries.partition_point(|(ts, _)| *ts < start);
        let hi = self.entries.partition_point(|(ts, _)| *ts <= end);
        self.entries[lo..hi].iter().map(|(_, id)| *id).collect()
    }

    /// Get all node IDs created after a timestamp (exclusive).
    pub fn after(&self, timestamp: u64) -> Vec<u64> {
        let lo = self.entries.partition_point(|(ts, _)| *ts <= timestamp);
        self.entries[lo..].iter().map(|(_, id)| *id).collect()
    }

    /// Get all node IDs created before a timestamp (exclusive).
    pub fn before(&self, timestamp: u64) -> Vec<u64> {
        let hi = self.entries.partition_point(|(ts, _)| *ts < timestamp);
        self.entries[..hi].iter().map(|(_, id)| *id).collect()
    }

    /// Get the most recent N node IDs.
    pub fn most_recent(&self, n: usize) -> Vec<u64> {
        let start = self.entries.len().saturating_sub(n);
        self.entries[start..]
            .iter()
            .rev()
            .map(|(_, id)| *id)
            .collect()
    }

    /// Get the oldest N node IDs.
    pub fn oldest(&self, n: usize) -> Vec<u64> {
        let end = n.min(self.entries.len());
        self.entries[..end].iter().map(|(_, id)| *id).collect()
    }

    /// Rebuild the entire index from a slice of nodes.
    pub fn rebuild(&mut self, nodes: &[CognitiveEvent]) {
        self.entries.clear();
        self.entries.reserve(nodes.len());
        for node in nodes {
            self.entries.push((node.created_at, node.id));
        }
        self.entries.sort_unstable();
    }

    /// Incrementally add a new node.
    pub fn add_node(&mut self, event: &CognitiveEvent) {
        let entry = (event.created_at, event.id);
        let pos = self.entries.partition_point(|e| *e < entry);
        self.entries.insert(pos, entry);
    }

    /// Remove a node from the index.
    pub fn remove_node(&mut self, id: u64, created_at: u64) {
        let entry = (created_at, id);
        if let Ok(pos) = self.entries.binary_search(&entry) {
            self.entries.remove(pos);
        }
    }

    /// Clear the index.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get a reference to the underlying entries (for serialization).
    pub fn entries(&self) -> &[(u64, u64)] {
        &self.entries
    }
}

impl Default for TemporalIndex {
    fn default() -> Self {
        Self::new()
    }
}
