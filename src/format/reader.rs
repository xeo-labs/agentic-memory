//! Reads .amem files into in-memory graph.

use std::io::Read;
use std::path::Path;

use crate::graph::MemoryGraph;
use crate::types::error::{AmemError, AmemResult};
use crate::types::header::FileHeader;
use crate::types::{CognitiveEvent, Edge, EdgeType, EventType};

use super::compression::decompress_content;

/// Reader for .amem binary files.
pub struct AmemReader;

impl AmemReader {
    /// Read an .amem file into a MemoryGraph.
    pub fn read_from_file(path: &Path) -> AmemResult<MemoryGraph> {
        let data = std::fs::read(path)?;
        let mut cursor = std::io::Cursor::new(data);
        Self::read_from(&mut cursor)
    }

    /// Read from any reader into a MemoryGraph.
    pub fn read_from(reader: &mut impl Read) -> AmemResult<MemoryGraph> {
        // Read all data into a buffer
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        if data.len() < 64 {
            return Err(AmemError::Truncated);
        }

        // Parse header
        let header = FileHeader::read_from(&mut std::io::Cursor::new(&data[..64]))?;

        let dimension = header.dimension as usize;
        let node_count = header.node_count as usize;
        let edge_count = header.edge_count as usize;

        // Read node table
        let node_table_start = header.node_table_offset as usize;
        let mut nodes: Vec<CognitiveEvent> = Vec::with_capacity(node_count);
        let mut node_content_info: Vec<(u64, u32)> = Vec::with_capacity(node_count);

        for i in 0..node_count {
            let offset = node_table_start + i * 72;
            if offset + 72 > data.len() {
                return Err(AmemError::Truncated);
            }
            let record = &data[offset..offset + 72];
            let (event, content_offset, content_length) = parse_node_record(record)?;
            node_content_info.push((content_offset, content_length));
            nodes.push(event);
        }

        // Read edge table
        let edge_table_start = header.edge_table_offset as usize;
        let mut edges: Vec<Edge> = Vec::with_capacity(edge_count);

        for i in 0..edge_count {
            let offset = edge_table_start + i * 32;
            if offset + 32 > data.len() {
                return Err(AmemError::Truncated);
            }
            let record = &data[offset..offset + 32];
            edges.push(parse_edge_record(record)?);
        }

        // Read content block
        let content_block_start = header.content_block_offset as usize;
        for (i, node) in nodes.iter_mut().enumerate() {
            let (content_offset, content_length) = node_content_info[i];
            if content_length > 0 {
                let start = content_block_start + content_offset as usize;
                let end = start + content_length as usize;
                if end > data.len() {
                    return Err(AmemError::Truncated);
                }
                node.content = decompress_content(&data[start..end])?;
            }
        }

        // Read feature vectors
        let fv_start = header.feature_vec_offset as usize;
        for (i, node) in nodes.iter_mut().enumerate() {
            let offset = fv_start + i * dimension * 4;
            if offset + dimension * 4 > data.len() {
                return Err(AmemError::Truncated);
            }
            let mut vec = Vec::with_capacity(dimension);
            for j in 0..dimension {
                let byte_offset = offset + j * 4;
                let bytes: [u8; 4] = data[byte_offset..byte_offset + 4].try_into().unwrap();
                vec.push(f32::from_le_bytes(bytes));
            }
            node.feature_vec = vec;
        }

        // Build graph from parts (this rebuilds indexes)
        MemoryGraph::from_parts(nodes, edges, dimension)
    }
}

/// Parse a 72-byte node record.
fn parse_node_record(data: &[u8]) -> AmemResult<(CognitiveEvent, u64, u32)> {
    let id = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let event_type_byte = data[8];
    let event_type = EventType::from_u8(event_type_byte).ok_or(AmemError::Corrupt(0))?;
    // bytes 9..12: padding
    let created_at = u64::from_le_bytes(data[12..20].try_into().unwrap());
    let session_id = u32::from_le_bytes(data[20..24].try_into().unwrap());
    let confidence = f32::from_le_bytes(data[24..28].try_into().unwrap());
    let access_count = u32::from_le_bytes(data[28..32].try_into().unwrap());
    let last_accessed = u64::from_le_bytes(data[32..40].try_into().unwrap());
    let decay_score = f32::from_le_bytes(data[40..44].try_into().unwrap());
    let content_offset = u64::from_le_bytes(data[44..52].try_into().unwrap());
    let content_length = u32::from_le_bytes(data[52..56].try_into().unwrap());
    // edge_offset at 56..64 (not needed for in-memory construction)
    // edge_count at 64..66 (not needed)
    // padding at 66..72 (not needed)

    let event = CognitiveEvent {
        id,
        event_type,
        created_at,
        session_id,
        confidence,
        access_count,
        last_accessed,
        decay_score,
        content: String::new(),  // Will be filled from content block
        feature_vec: Vec::new(), // Will be filled from feature vec block
    };

    Ok((event, content_offset, content_length))
}

/// Parse a 32-byte edge record.
fn parse_edge_record(data: &[u8]) -> AmemResult<Edge> {
    let source_id = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let target_id = u64::from_le_bytes(data[8..16].try_into().unwrap());
    let edge_type_byte = data[16];
    let edge_type = EdgeType::from_u8(edge_type_byte).ok_or(AmemError::Corrupt(0))?;
    // bytes 17..20: padding
    let weight = f32::from_le_bytes(data[20..24].try_into().unwrap());
    let created_at = u64::from_le_bytes(data[24..32].try_into().unwrap());

    Ok(Edge {
        source_id,
        target_id,
        edge_type,
        weight,
        created_at,
    })
}
