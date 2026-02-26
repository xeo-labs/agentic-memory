//! DAG-based causal index. Tracks decision chains: what led to what.

use super::{Index, IndexResult};
use crate::v3::block::{Block, BlockContent, BlockHash, BlockType};
use std::collections::{HashMap, HashSet, VecDeque};

/// DAG-based causal index.
pub struct CausalIndex {
    /// Forward edges: block -> blocks it caused
    forward: HashMap<u64, Vec<u64>>,

    /// Backward edges: block -> blocks that caused it
    backward: HashMap<u64, Vec<u64>>,

    /// Decision blocks (entry points for causal queries)
    decisions: HashSet<u64>,

    /// Block hashes
    hashes: HashMap<u64, BlockHash>,
}

impl CausalIndex {
    pub fn new() -> Self {
        Self {
            forward: HashMap::new(),
            backward: HashMap::new(),
            decisions: HashSet::new(),
            hashes: HashMap::new(),
        }
    }

    /// Add causal link: cause -> effect
    pub fn add_link(&mut self, cause: u64, effect: u64) {
        self.forward.entry(cause).or_default().push(effect);
        self.backward.entry(effect).or_default().push(cause);
    }

    /// Get all blocks that led to this block (ancestors)
    pub fn get_ancestors(&self, sequence: u64, max_depth: usize) -> Vec<IndexResult> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((sequence, 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth > max_depth || visited.contains(&current) {
                continue;
            }
            visited.insert(current);

            if current != sequence {
                if let Some(&hash) = self.hashes.get(&current) {
                    result.push(IndexResult {
                        block_sequence: current,
                        block_hash: hash,
                        score: 1.0 - (depth as f32 / max_depth as f32),
                    });
                }
            }

            if let Some(causes) = self.backward.get(&current) {
                for &cause in causes {
                    queue.push_back((cause, depth + 1));
                }
            }
        }

        result
    }

    /// Get all blocks that resulted from this block (descendants)
    pub fn get_descendants(&self, sequence: u64, max_depth: usize) -> Vec<IndexResult> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((sequence, 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth > max_depth || visited.contains(&current) {
                continue;
            }
            visited.insert(current);

            if current != sequence {
                if let Some(&hash) = self.hashes.get(&current) {
                    result.push(IndexResult {
                        block_sequence: current,
                        block_hash: hash,
                        score: 1.0 - (depth as f32 / max_depth as f32),
                    });
                }
            }

            if let Some(effects) = self.forward.get(&current) {
                for &effect in effects {
                    queue.push_back((effect, depth + 1));
                }
            }
        }

        result
    }

    /// Get all decision blocks
    pub fn get_decisions(&self) -> Vec<IndexResult> {
        self.decisions
            .iter()
            .filter_map(|&seq| {
                self.hashes.get(&seq).map(|&hash| IndexResult {
                    block_sequence: seq,
                    block_hash: hash,
                    score: 1.0,
                })
            })
            .collect()
    }

    /// Get decision chain leading to a block
    pub fn get_decision_chain(&self, sequence: u64) -> Vec<IndexResult> {
        self.get_ancestors(sequence, 100)
            .into_iter()
            .filter(|r| self.decisions.contains(&r.block_sequence))
            .collect()
    }
}

impl Default for CausalIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl Index for CausalIndex {
    fn index(&mut self, block: &Block) {
        self.hashes.insert(block.sequence, block.hash);

        if matches!(block.block_type, BlockType::Decision) {
            self.decisions.insert(block.sequence);
        }

        // Extract causal links from content
        match &block.content {
            BlockContent::Decision {
                evidence_blocks, ..
            } => {
                for evidence_hash in evidence_blocks {
                    for (&seq, &hash) in &self.hashes {
                        if &hash == evidence_hash {
                            self.add_link(seq, block.sequence);
                            break;
                        }
                    }
                }
            }
            // Tool results are caused by tool calls
            BlockContent::Tool { .. } if block.sequence > 0 => {
                self.add_link(block.sequence - 1, block.sequence);
            }
            _ => {}
        }

        // Default: previous block causes current block
        if block.sequence > 0 {
            self.add_link(block.sequence - 1, block.sequence);
        }
    }

    fn remove(&mut self, sequence: u64) {
        self.forward.remove(&sequence);
        self.backward.remove(&sequence);
        self.decisions.remove(&sequence);
        self.hashes.remove(&sequence);

        for edges in self.forward.values_mut() {
            edges.retain(|&s| s != sequence);
        }
        for edges in self.backward.values_mut() {
            edges.retain(|&s| s != sequence);
        }
    }

    fn rebuild(&mut self, blocks: impl Iterator<Item = Block>) {
        self.forward.clear();
        self.backward.clear();
        self.decisions.clear();
        self.hashes.clear();
        for block in blocks {
            self.index(&block);
        }
    }
}
