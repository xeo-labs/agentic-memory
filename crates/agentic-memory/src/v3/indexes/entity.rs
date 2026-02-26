//! Inverted index for entity mentions. O(1) lookup by entity.

use super::{Index, IndexResult};
use crate::v3::block::{Block, BlockContent, BlockHash};
use std::collections::{HashMap, HashSet};

/// Entity type categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    File,
    Directory,
    Person,
    Project,
    Tool,
    Concept,
    Other,
}

/// Inverted index for entity mentions.
pub struct EntityIndex {
    /// Entity -> blocks that mention it
    by_entity: HashMap<String, HashSet<u64>>,

    /// Block hashes
    hashes: HashMap<u64, BlockHash>,

    /// Entity types for categorization
    entity_types: HashMap<String, EntityType>,
}

impl EntityIndex {
    pub fn new() -> Self {
        Self {
            by_entity: HashMap::new(),
            hashes: HashMap::new(),
            entity_types: HashMap::new(),
        }
    }

    /// Add entity mention
    pub fn add_mention(&mut self, entity: &str, sequence: u64, entity_type: EntityType) {
        self.by_entity
            .entry(entity.to_string())
            .or_default()
            .insert(sequence);
        self.entity_types.insert(entity.to_string(), entity_type);
    }

    /// Query blocks mentioning an entity
    pub fn query_entity(&self, entity: &str) -> Vec<IndexResult> {
        self.by_entity
            .get(entity)
            .map(|sequences| {
                sequences
                    .iter()
                    .filter_map(|&seq| {
                        self.hashes.get(&seq).map(|&hash| IndexResult {
                            block_sequence: seq,
                            block_hash: hash,
                            score: 1.0,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Query blocks mentioning entities matching a prefix
    pub fn query_prefix(&self, prefix: &str) -> Vec<IndexResult> {
        let mut sequences = HashSet::new();

        for (entity, seqs) in &self.by_entity {
            if entity.starts_with(prefix) {
                sequences.extend(seqs);
            }
        }

        sequences
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

    /// Get all entities of a type
    pub fn get_entities_by_type(&self, entity_type: EntityType) -> Vec<String> {
        self.entity_types
            .iter()
            .filter(|(_, &t)| t == entity_type)
            .map(|(e, _)| e.clone())
            .collect()
    }

    /// Get all files mentioned
    pub fn get_all_files(&self) -> Vec<String> {
        self.get_entities_by_type(EntityType::File)
    }

    /// Get indexed entity count
    pub fn len(&self) -> usize {
        self.by_entity.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.by_entity.is_empty()
    }
}

impl Default for EntityIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl Index for EntityIndex {
    fn index(&mut self, block: &Block) {
        self.hashes.insert(block.sequence, block.hash);

        match &block.content {
            BlockContent::File { path, .. } => {
                self.add_mention(path, block.sequence, EntityType::File);

                // Also index parent directories
                let parts: Vec<&str> = path.split('/').collect();
                for i in 1..parts.len() {
                    let dir = parts[..i].join("/");
                    self.add_mention(&dir, block.sequence, EntityType::Directory);
                }
            }
            BlockContent::Tool { tool_name, .. } => {
                self.add_mention(tool_name, block.sequence, EntityType::Tool);
            }
            BlockContent::Text { text, .. } => {
                // Extract file paths mentioned in text
                for word in text.split_whitespace() {
                    if word.contains('/') && !word.starts_with("http") {
                        self.add_mention(word, block.sequence, EntityType::File);
                    }
                }
            }
            _ => {}
        }
    }

    fn remove(&mut self, sequence: u64) {
        self.hashes.remove(&sequence);
        for sequences in self.by_entity.values_mut() {
            sequences.remove(&sequence);
        }
    }

    fn rebuild(&mut self, blocks: impl Iterator<Item = Block>) {
        self.by_entity.clear();
        self.hashes.clear();
        self.entity_types.clear();
        for block in blocks {
            self.index(&block);
        }
    }
}
