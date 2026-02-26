//! Five indexes for the V3 immortal architecture.

pub mod temporal;
pub mod semantic;
pub mod causal;
pub mod entity;
pub mod procedural;

use super::block::{Block, BlockHash};

/// Result from any index query
#[derive(Debug, Clone)]
pub struct IndexResult {
    pub block_sequence: u64,
    pub block_hash: BlockHash,
    pub score: f32, // Relevance score (0.0 - 1.0)
}

/// Common trait for all indexes
pub trait Index {
    /// Add a block to the index
    fn index(&mut self, block: &Block);

    /// Remove a block from the index (for reindexing only)
    fn remove(&mut self, sequence: u64);

    /// Rebuild entire index from blocks
    fn rebuild(&mut self, blocks: impl Iterator<Item = Block>);
}
