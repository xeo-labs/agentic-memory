//! Memory-mapped file access for .amem files.

use std::path::Path;

use memmap2::Mmap;

use crate::graph::MemoryGraph;
use crate::index::cosine_similarity;
use crate::types::error::{AmemError, AmemResult};
use crate::types::header::FileHeader;
use crate::types::{CognitiveEvent, Edge, EdgeType, EventType};

use super::compression::decompress_content;

/// A match result from a similarity search.
#[derive(Debug, Clone)]
pub struct SimilarityMatch {
    /// The node ID that matched.
    pub node_id: u64,
    /// The cosine similarity score.
    pub similarity: f32,
}

/// Read-only memory-mapped access to an .amem file.
pub struct MmapReader {
    mmap: Mmap,
    header: FileHeader,
}

impl MmapReader {
    /// Open an .amem file for memory-mapped read access.
    pub fn open(path: &Path) -> AmemResult<Self> {
        let file = std::fs::File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        if mmap.len() < 64 {
            return Err(AmemError::Truncated);
        }

        let header = FileHeader::read_from(&mut std::io::Cursor::new(&mmap[..64]))?;

        Ok(Self { mmap, header })
    }

    /// Get the file header.
    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    /// Read a single node record by ID (O(1) access).
    pub fn read_node(&self, id: u64) -> AmemResult<CognitiveEvent> {
        if id >= self.header.node_count {
            return Err(AmemError::NodeNotFound(id));
        }

        let offset = self.header.node_table_offset as usize + (id as usize) * 72;
        if offset + 72 > self.mmap.len() {
            return Err(AmemError::Truncated);
        }

        let record = &self.mmap[offset..offset + 72];
        let mut event = parse_node_record_mmap(record)?;

        // Read content
        event.content = self.read_content_internal(record)?;

        // Read feature vec
        event.feature_vec = self.read_feature_vec_internal(id)?;

        Ok(event)
    }

    /// Read a node's content (decompress from content block).
    pub fn read_content(&self, id: u64) -> AmemResult<String> {
        if id >= self.header.node_count {
            return Err(AmemError::NodeNotFound(id));
        }

        let offset = self.header.node_table_offset as usize + (id as usize) * 72;
        if offset + 72 > self.mmap.len() {
            return Err(AmemError::Truncated);
        }

        let record = &self.mmap[offset..offset + 72];
        self.read_content_internal(record)
    }

    fn read_content_internal(&self, node_record: &[u8]) -> AmemResult<String> {
        let content_offset = u64::from_le_bytes(node_record[44..52].try_into().unwrap());
        let content_length = u32::from_le_bytes(node_record[52..56].try_into().unwrap());

        if content_length == 0 {
            return Ok(String::new());
        }

        let start = self.header.content_block_offset as usize + content_offset as usize;
        let end = start + content_length as usize;
        if end > self.mmap.len() {
            return Err(AmemError::Truncated);
        }

        decompress_content(&self.mmap[start..end])
    }

    /// Read a node's feature vector.
    pub fn read_feature_vec(&self, id: u64) -> AmemResult<Vec<f32>> {
        self.read_feature_vec_internal(id)
    }

    fn read_feature_vec_internal(&self, id: u64) -> AmemResult<Vec<f32>> {
        if id >= self.header.node_count {
            return Err(AmemError::NodeNotFound(id));
        }

        let dim = self.header.dimension as usize;
        let offset = self.header.feature_vec_offset as usize + (id as usize) * dim * 4;
        if offset + dim * 4 > self.mmap.len() {
            return Err(AmemError::Truncated);
        }

        let mut vec = Vec::with_capacity(dim);
        for i in 0..dim {
            let byte_offset = offset + i * 4;
            let bytes: [u8; 4] = self.mmap[byte_offset..byte_offset + 4].try_into().unwrap();
            vec.push(f32::from_le_bytes(bytes));
        }
        Ok(vec)
    }

    /// Read all edges from a node.
    pub fn read_edges(&self, id: u64) -> AmemResult<Vec<Edge>> {
        if id >= self.header.node_count {
            return Err(AmemError::NodeNotFound(id));
        }

        let node_offset = self.header.node_table_offset as usize + (id as usize) * 72;
        if node_offset + 72 > self.mmap.len() {
            return Err(AmemError::Truncated);
        }

        let record = &self.mmap[node_offset..node_offset + 72];
        let edge_offset = u64::from_le_bytes(record[56..64].try_into().unwrap());
        let edge_count = u16::from_le_bytes(record[64..66].try_into().unwrap());

        let mut edges = Vec::with_capacity(edge_count as usize);
        let edge_base = self.header.edge_table_offset as usize + edge_offset as usize;

        for i in 0..edge_count as usize {
            let offset = edge_base + i * 32;
            if offset + 32 > self.mmap.len() {
                return Err(AmemError::Truncated);
            }
            let data = &self.mmap[offset..offset + 32];
            let source_id = u64::from_le_bytes(data[0..8].try_into().unwrap());
            let target_id = u64::from_le_bytes(data[8..16].try_into().unwrap());
            let edge_type = EdgeType::from_u8(data[16]).ok_or(AmemError::Corrupt(offset as u64))?;
            let weight = f32::from_le_bytes(data[20..24].try_into().unwrap());
            let created_at = u64::from_le_bytes(data[24..32].try_into().unwrap());
            edges.push(Edge {
                source_id,
                target_id,
                edge_type,
                weight,
                created_at,
            });
        }

        Ok(edges)
    }

    /// Read the full graph into memory.
    pub fn read_full_graph(&self) -> AmemResult<MemoryGraph> {
        let dimension = self.header.dimension as usize;
        let node_count = self.header.node_count as usize;
        let edge_count = self.header.edge_count as usize;

        let mut nodes = Vec::with_capacity(node_count);
        for id in 0..node_count as u64 {
            nodes.push(self.read_node(id)?);
        }

        let mut edges = Vec::with_capacity(edge_count);
        let edge_base = self.header.edge_table_offset as usize;
        for i in 0..edge_count {
            let offset = edge_base + i * 32;
            if offset + 32 > self.mmap.len() {
                return Err(AmemError::Truncated);
            }
            let data = &self.mmap[offset..offset + 32];
            let source_id = u64::from_le_bytes(data[0..8].try_into().unwrap());
            let target_id = u64::from_le_bytes(data[8..16].try_into().unwrap());
            let edge_type = EdgeType::from_u8(data[16]).ok_or(AmemError::Corrupt(offset as u64))?;
            let weight = f32::from_le_bytes(data[20..24].try_into().unwrap());
            let created_at = u64::from_le_bytes(data[24..32].try_into().unwrap());
            edges.push(Edge {
                source_id,
                target_id,
                edge_type,
                weight,
                created_at,
            });
        }

        MemoryGraph::from_parts(nodes, edges, dimension)
    }

    /// Compute cosine similarity between a query and a node's feature vector.
    pub fn similarity_to(&self, id: u64, query: &[f32]) -> AmemResult<f32> {
        let vec = self.read_feature_vec_internal(id)?;
        Ok(cosine_similarity(query, &vec))
    }

    /// Batch similarity: scan all feature vectors and return top-k matches.
    pub fn batch_similarity(
        &self,
        query: &[f32],
        top_k: usize,
        min_similarity: f32,
    ) -> AmemResult<Vec<SimilarityMatch>> {
        let dim = self.header.dimension as usize;
        let node_count = self.header.node_count as usize;

        let mut matches: Vec<SimilarityMatch> = Vec::new();

        for id in 0..node_count {
            let offset = self.header.feature_vec_offset as usize + id * dim * 4;
            if offset + dim * 4 > self.mmap.len() {
                break;
            }

            // Read feature vector directly from mmap
            let mut vec = Vec::with_capacity(dim);
            let mut is_zero = true;
            for j in 0..dim {
                let byte_offset = offset + j * 4;
                let bytes: [u8; 4] = self.mmap[byte_offset..byte_offset + 4].try_into().unwrap();
                let val = f32::from_le_bytes(bytes);
                if val != 0.0 {
                    is_zero = false;
                }
                vec.push(val);
            }

            if is_zero {
                continue;
            }

            let sim = cosine_similarity(query, &vec);
            if sim >= min_similarity {
                matches.push(SimilarityMatch {
                    node_id: id as u64,
                    similarity: sim,
                });
            }
        }

        matches.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(top_k);
        Ok(matches)
    }
}

/// Parse a node record from mmap bytes (without content/feature vec).
fn parse_node_record_mmap(data: &[u8]) -> AmemResult<CognitiveEvent> {
    let id = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let event_type = EventType::from_u8(data[8]).ok_or(AmemError::Corrupt(0))?;
    let created_at = u64::from_le_bytes(data[12..20].try_into().unwrap());
    let session_id = u32::from_le_bytes(data[20..24].try_into().unwrap());
    let confidence = f32::from_le_bytes(data[24..28].try_into().unwrap());
    let access_count = u32::from_le_bytes(data[28..32].try_into().unwrap());
    let last_accessed = u64::from_le_bytes(data[32..40].try_into().unwrap());
    let decay_score = f32::from_le_bytes(data[40..44].try_into().unwrap());

    Ok(CognitiveEvent {
        id,
        event_type,
        created_at,
        session_id,
        confidence,
        access_count,
        last_accessed,
        decay_score,
        content: String::new(),
        feature_vec: Vec::new(),
    })
}
