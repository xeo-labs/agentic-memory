//! B-tree index for temporal queries. O(log n) lookup by timestamp.

use super::{Index, IndexResult};
use crate::v3::block::{Block, BlockHash};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

/// B-tree index for temporal queries.
pub struct TemporalIndex {
    /// Timestamp (millis) -> (sequence, hash)
    by_time: BTreeMap<i64, Vec<(u64, BlockHash)>>,

    /// Sequence -> timestamp (for reverse lookup)
    by_sequence: Vec<i64>,
}

impl TemporalIndex {
    pub fn new() -> Self {
        Self {
            by_time: BTreeMap::new(),
            by_sequence: Vec::new(),
        }
    }

    /// Query blocks in time range
    pub fn query_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<IndexResult> {
        let start_ts = start.timestamp_millis();
        let end_ts = end.timestamp_millis();

        self.by_time
            .range(start_ts..=end_ts)
            .flat_map(|(_, blocks)| blocks.iter())
            .map(|(seq, hash)| IndexResult {
                block_sequence: *seq,
                block_hash: *hash,
                score: 1.0,
            })
            .collect()
    }

    /// Get blocks from last N seconds
    pub fn query_recent(&self, seconds: i64) -> Vec<IndexResult> {
        let now = Utc::now().timestamp_millis();
        let start = now - (seconds * 1000);

        self.by_time
            .range(start..=now)
            .flat_map(|(_, blocks)| blocks.iter())
            .map(|(seq, hash)| IndexResult {
                block_sequence: *seq,
                block_hash: *hash,
                score: 1.0,
            })
            .collect()
    }

    /// Get total indexed block count
    pub fn len(&self) -> usize {
        self.by_sequence.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.by_sequence.is_empty()
    }
}

impl Default for TemporalIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl Index for TemporalIndex {
    fn index(&mut self, block: &Block) {
        let ts = block.timestamp.timestamp_millis();

        self.by_time
            .entry(ts)
            .or_default()
            .push((block.sequence, block.hash));

        // Ensure by_sequence is large enough
        while self.by_sequence.len() <= block.sequence as usize {
            self.by_sequence.push(0);
        }
        self.by_sequence[block.sequence as usize] = ts;
    }

    fn remove(&mut self, sequence: u64) {
        if let Some(&ts) = self.by_sequence.get(sequence as usize) {
            if let Some(blocks) = self.by_time.get_mut(&ts) {
                blocks.retain(|(seq, _)| *seq != sequence);
            }
        }
    }

    fn rebuild(&mut self, blocks: impl Iterator<Item = Block>) {
        self.by_time.clear();
        self.by_sequence.clear();
        for block in blocks {
            self.index(&block);
        }
    }
}
