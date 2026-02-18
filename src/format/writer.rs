//! Writes .amem files from in-memory graph.

use std::io::Write;
use std::path::Path;

use crate::graph::MemoryGraph;
use crate::types::error::AmemResult;
use crate::types::header::{FileHeader, HEADER_SIZE};
use crate::types::{Edge, EventType, AMEM_MAGIC, FORMAT_VERSION};

use super::compression::compress_content;

/// Size of a single node record on disk: 72 bytes.
const NODE_RECORD_SIZE: u64 = 72;

/// Size of a single edge record on disk: 32 bytes.
const EDGE_RECORD_SIZE: u64 = 32;

/// Writer for .amem binary files.
pub struct AmemWriter {
    dimension: usize,
}

impl AmemWriter {
    /// Create a new writer with the given feature vector dimension.
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// Write a complete MemoryGraph to an .amem file.
    pub fn write_to_file(&self, graph: &MemoryGraph, path: &Path) -> AmemResult<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.write_to(graph, &mut writer)
    }

    /// Write a complete MemoryGraph to any writer.
    pub fn write_to(&self, graph: &MemoryGraph, writer: &mut impl Write) -> AmemResult<()> {
        let nodes = graph.nodes();
        // Sort edges by source_id for correct edge offset computation
        let mut edges_sorted: Vec<Edge> = graph.edges().to_vec();
        edges_sorted.sort_by(|a, b| {
            a.source_id
                .cmp(&b.source_id)
                .then(a.target_id.cmp(&b.target_id))
        });
        let edges = &edges_sorted[..];
        let node_count = nodes.len() as u64;
        let edge_count = edges.len() as u64;

        // Step 1: Compress all node contents and record offsets
        let mut compressed_contents: Vec<Vec<u8>> = Vec::with_capacity(nodes.len());
        let mut content_offsets: Vec<u64> = Vec::with_capacity(nodes.len());
        let mut content_total_size: u64 = 0;

        for node in nodes {
            let compressed = compress_content(&node.content)?;
            content_offsets.push(content_total_size);
            content_total_size += compressed.len() as u64;
            compressed_contents.push(compressed);
        }

        // Step 2: Calculate edge offsets per node
        // Edges are sorted by source_id. We need to compute the offset for each node's edges.
        let mut edge_offsets: Vec<(u64, u16)> = vec![(0, 0); nodes.len()];
        {
            let mut edge_idx = 0usize;
            for node in nodes {
                let start = edge_idx;
                while edge_idx < edges.len() && edges[edge_idx].source_id == node.id {
                    edge_idx += 1;
                }
                let count = edge_idx - start;
                // Find the node's position in the sorted nodes list
                if let Some(pos) = nodes.iter().position(|n| n.id == node.id) {
                    edge_offsets[pos] = ((start as u64) * EDGE_RECORD_SIZE, count as u16);
                }
            }
        }

        // Step 3: Calculate section offsets
        let node_table_offset = HEADER_SIZE;
        let edge_table_offset = node_table_offset + node_count * NODE_RECORD_SIZE;
        let content_block_offset = edge_table_offset + edge_count * EDGE_RECORD_SIZE;
        let feature_vec_offset = content_block_offset + content_total_size;

        // Step 4: Write header
        let header = FileHeader {
            magic: AMEM_MAGIC,
            version: FORMAT_VERSION,
            dimension: self.dimension as u32,
            node_count,
            edge_count,
            node_table_offset,
            edge_table_offset,
            content_block_offset,
            feature_vec_offset,
        };
        header.write_to(writer)?;

        // Step 5: Write node table
        for (i, node) in nodes.iter().enumerate() {
            write_node_record(
                writer,
                node,
                content_offsets[i],
                compressed_contents[i].len() as u32,
                edge_offsets[i].0,
                edge_offsets[i].1,
            )?;
        }

        // Step 6: Write edge table
        for edge in edges {
            write_edge_record(writer, edge)?;
        }

        // Step 7: Write content block
        for compressed in &compressed_contents {
            writer.write_all(compressed)?;
        }

        // Step 8: Write feature vector block
        for node in nodes {
            for &val in &node.feature_vec {
                writer.write_all(&val.to_le_bytes())?;
            }
            // Pad if feature vec is shorter than dimension
            let remaining = self.dimension.saturating_sub(node.feature_vec.len());
            for _ in 0..remaining {
                writer.write_all(&0.0f32.to_le_bytes())?;
            }
        }

        // Step 9: Write index block
        self.write_indexes(writer, graph)?;

        writer.flush()?;
        Ok(())
    }

    fn write_indexes(&self, writer: &mut impl Write, graph: &MemoryGraph) -> AmemResult<()> {
        // Type Index (tag 0x01)
        {
            let mut buf: Vec<u8> = Vec::new();
            let type_idx = graph.type_index();
            for event_type_val in 0u8..=5 {
                if let Some(et) = EventType::from_u8(event_type_val) {
                    let ids = type_idx.get(et);
                    if !ids.is_empty() {
                        buf.push(event_type_val);
                        buf.extend_from_slice(&(ids.len() as u64).to_le_bytes());
                        for &id in ids {
                            buf.extend_from_slice(&id.to_le_bytes());
                        }
                    }
                }
            }
            writer.write_all(&[0x01u8])?;
            writer.write_all(&(buf.len() as u64).to_le_bytes())?;
            writer.write_all(&buf)?;
        }

        // Temporal Index (tag 0x02)
        {
            let temporal_idx = graph.temporal_index();
            let entries = temporal_idx.entries();
            let mut buf: Vec<u8> = Vec::new();
            buf.extend_from_slice(&(entries.len() as u64).to_le_bytes());
            for &(created_at, node_id) in entries {
                buf.extend_from_slice(&created_at.to_le_bytes());
                buf.extend_from_slice(&node_id.to_le_bytes());
            }
            writer.write_all(&[0x02u8])?;
            writer.write_all(&(buf.len() as u64).to_le_bytes())?;
            writer.write_all(&buf)?;
        }

        // Session Index (tag 0x03)
        {
            let session_idx = graph.session_index();
            let inner = session_idx.inner();
            let mut buf: Vec<u8> = Vec::new();
            buf.extend_from_slice(&(inner.len() as u32).to_le_bytes());
            let mut session_ids: Vec<u32> = inner.keys().copied().collect();
            session_ids.sort_unstable();
            for sid in session_ids {
                let ids = session_idx.get_session(sid);
                buf.extend_from_slice(&sid.to_le_bytes());
                buf.extend_from_slice(&(ids.len() as u64).to_le_bytes());
                for &id in ids {
                    buf.extend_from_slice(&id.to_le_bytes());
                }
            }
            writer.write_all(&[0x03u8])?;
            writer.write_all(&(buf.len() as u64).to_le_bytes())?;
            writer.write_all(&buf)?;
        }

        // Cluster Map (tag 0x04)
        {
            let cluster = graph.cluster_map();
            let mut buf: Vec<u8> = Vec::new();
            buf.extend_from_slice(&(cluster.cluster_count() as u32).to_le_bytes());
            buf.extend_from_slice(&(cluster.dimension() as u32).to_le_bytes());
            for i in 0..cluster.cluster_count() {
                if let Some(centroid) = cluster.centroid(i) {
                    for &val in centroid {
                        buf.extend_from_slice(&val.to_le_bytes());
                    }
                }
                let members = cluster.get_cluster(i);
                buf.extend_from_slice(&(members.len() as u64).to_le_bytes());
                for &id in members {
                    buf.extend_from_slice(&id.to_le_bytes());
                }
            }
            writer.write_all(&[0x04u8])?;
            writer.write_all(&(buf.len() as u64).to_le_bytes())?;
            writer.write_all(&buf)?;
        }

        Ok(())
    }
}

/// Write a single 72-byte node record.
fn write_node_record(
    writer: &mut impl Write,
    node: &crate::types::CognitiveEvent,
    content_offset: u64,
    content_length: u32,
    edge_offset: u64,
    edge_count: u16,
) -> AmemResult<()> {
    writer.write_all(&node.id.to_le_bytes())?; // 8 bytes
    writer.write_all(&[node.event_type as u8])?; // 1 byte
    writer.write_all(&[0u8; 3])?; // 3 bytes padding
    writer.write_all(&node.created_at.to_le_bytes())?; // 8 bytes
    writer.write_all(&node.session_id.to_le_bytes())?; // 4 bytes
    writer.write_all(&node.confidence.to_le_bytes())?; // 4 bytes
    writer.write_all(&node.access_count.to_le_bytes())?; // 4 bytes
    writer.write_all(&node.last_accessed.to_le_bytes())?; // 8 bytes
    writer.write_all(&node.decay_score.to_le_bytes())?; // 4 bytes
    writer.write_all(&content_offset.to_le_bytes())?; // 8 bytes
    writer.write_all(&content_length.to_le_bytes())?; // 4 bytes
    writer.write_all(&edge_offset.to_le_bytes())?; // 8 bytes
    writer.write_all(&edge_count.to_le_bytes())?; // 2 bytes
    writer.write_all(&[0u8; 6])?; // 6 bytes padding
                                  // Total: 8+1+3+8+4+4+4+8+4+8+4+8+2+6 = 72
    Ok(())
}

/// Write a single 32-byte edge record.
fn write_edge_record(writer: &mut impl Write, edge: &crate::types::Edge) -> AmemResult<()> {
    writer.write_all(&edge.source_id.to_le_bytes())?; // 8 bytes
    writer.write_all(&edge.target_id.to_le_bytes())?; // 8 bytes
    writer.write_all(&[edge.edge_type as u8])?; // 1 byte
    writer.write_all(&[0u8; 3])?; // 3 bytes padding
    writer.write_all(&edge.weight.to_le_bytes())?; // 4 bytes
    writer.write_all(&edge.created_at.to_le_bytes())?; // 8 bytes
                                                       // Total: 8+8+1+3+4+8 = 32
    Ok(())
}
