//! Semantic similarity index with text fallback search.

use super::{Index, IndexResult};
use crate::v3::block::{Block, BlockHash};
use std::collections::HashMap;

/// Semantic similarity index.
/// Supports both embedding-based and text-based search.
pub struct SemanticIndex {
    /// Embeddings storage: sequence -> embedding vector
    embeddings: HashMap<u64, Vec<f32>>,

    /// Text content for fallback search
    text_content: HashMap<u64, String>,

    /// Block hashes
    hashes: HashMap<u64, BlockHash>,

    /// Embedding dimension
    dimension: usize,
}

impl SemanticIndex {
    pub fn new(dimension: usize) -> Self {
        Self {
            embeddings: HashMap::new(),
            text_content: HashMap::new(),
            hashes: HashMap::new(),
            dimension,
        }
    }

    /// Add embedding for a block
    pub fn add_embedding(&mut self, sequence: u64, embedding: Vec<f32>) {
        if embedding.len() == self.dimension {
            self.embeddings.insert(sequence, embedding);
        }
    }

    /// Search by embedding vector
    pub fn search_by_embedding(&self, query: &[f32], limit: usize) -> Vec<IndexResult> {
        if query.len() != self.dimension {
            return vec![];
        }

        let mut scores: Vec<(u64, f32)> = self
            .embeddings
            .iter()
            .map(|(seq, emb)| {
                let score = cosine_similarity(query, emb);
                (*seq, score)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scores
            .into_iter()
            .take(limit)
            .filter_map(|(seq, score)| {
                self.hashes.get(&seq).map(|hash| IndexResult {
                    block_sequence: seq,
                    block_hash: *hash,
                    score,
                })
            })
            .collect()
    }

    /// Search by text (fallback when no embeddings available)
    pub fn search_by_text(&self, query: &str, limit: usize) -> Vec<IndexResult> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        if query_words.is_empty() {
            return vec![];
        }

        let mut scores: Vec<(u64, f32)> = self
            .text_content
            .iter()
            .map(|(seq, text)| {
                let text_lower = text.to_lowercase();
                let matches = query_words.iter().filter(|w| text_lower.contains(*w)).count();
                let score = matches as f32 / query_words.len() as f32;
                (*seq, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scores
            .into_iter()
            .take(limit)
            .filter_map(|(seq, score)| {
                self.hashes.get(&seq).map(|hash| IndexResult {
                    block_sequence: seq,
                    block_hash: *hash,
                    score,
                })
            })
            .collect()
    }

    /// Get indexed block count
    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }
}

impl Index for SemanticIndex {
    fn index(&mut self, block: &Block) {
        self.hashes.insert(block.sequence, block.hash);

        if let Some(text) = block.extract_text() {
            self.text_content.insert(block.sequence, text);
        }
    }

    fn remove(&mut self, sequence: u64) {
        self.embeddings.remove(&sequence);
        self.text_content.remove(&sequence);
        self.hashes.remove(&sequence);
    }

    fn rebuild(&mut self, blocks: impl Iterator<Item = Block>) {
        self.embeddings.clear();
        self.text_content.clear();
        self.hashes.clear();
        for block in blocks {
            self.index(&block);
        }
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}
