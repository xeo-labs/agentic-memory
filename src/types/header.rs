//! File header for .amem binary files.

use std::io::{Read, Write};

use crate::types::error::{AmemError, AmemResult};
use crate::types::{AMEM_MAGIC, FORMAT_VERSION};

/// Header of an .amem file. Fixed size: 64 bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileHeader {
    /// Magic bytes: [0x41, 0x4D, 0x45, 0x4D] ("AMEM").
    pub magic: [u8; 4],
    /// Format version (currently 1).
    pub version: u32,
    /// Feature vector dimensionality.
    pub dimension: u32,
    /// Total number of nodes in the file.
    pub node_count: u64,
    /// Total number of edges in the file.
    pub edge_count: u64,
    /// Byte offset where the node table starts.
    pub node_table_offset: u64,
    /// Byte offset where the edge table starts.
    pub edge_table_offset: u64,
    /// Byte offset where the content block starts.
    pub content_block_offset: u64,
    /// Byte offset where the feature vector block starts.
    pub feature_vec_offset: u64,
}

/// The fixed size of a FileHeader on disk: 64 bytes.
pub const HEADER_SIZE: u64 = 64;

impl FileHeader {
    /// Create a new header with default magic and version.
    pub fn new(dimension: u32) -> Self {
        Self {
            magic: AMEM_MAGIC,
            version: FORMAT_VERSION,
            dimension,
            node_count: 0,
            edge_count: 0,
            node_table_offset: HEADER_SIZE,
            edge_table_offset: HEADER_SIZE,
            content_block_offset: HEADER_SIZE,
            feature_vec_offset: HEADER_SIZE,
        }
    }

    /// Write this header to the given writer. Writes exactly 64 bytes.
    ///
    /// Layout (all little-endian):
    /// - 0x00..0x04: magic (4 bytes)
    /// - 0x04..0x08: version (u32, 4 bytes)
    /// - 0x08..0x0C: dimension (u32, 4 bytes)
    /// - 0x0C..0x10: _reserved (u32, 4 bytes, written as 0)
    /// - 0x10..0x18: node_count (u64, 8 bytes)
    /// - 0x18..0x20: edge_count (u64, 8 bytes)
    /// - 0x20..0x28: node_table_offset (u64, 8 bytes)
    /// - 0x28..0x30: edge_table_offset (u64, 8 bytes)
    /// - 0x30..0x38: content_block_offset (u64, 8 bytes)
    /// - 0x38..0x40: feature_vec_offset (u64, 8 bytes)
    ///   Total: 64 bytes
    pub fn write_to(&self, writer: &mut impl Write) -> AmemResult<()> {
        writer.write_all(&self.magic)?;
        writer.write_all(&self.version.to_le_bytes())?;
        writer.write_all(&self.dimension.to_le_bytes())?;
        writer.write_all(&0u32.to_le_bytes())?; // _reserved
        writer.write_all(&self.node_count.to_le_bytes())?;
        writer.write_all(&self.edge_count.to_le_bytes())?;
        writer.write_all(&self.node_table_offset.to_le_bytes())?;
        writer.write_all(&self.edge_table_offset.to_le_bytes())?;
        writer.write_all(&self.content_block_offset.to_le_bytes())?;
        writer.write_all(&self.feature_vec_offset.to_le_bytes())?;
        Ok(())
    }

    /// Read a header from the given reader. Reads exactly 64 bytes.
    pub fn read_from(reader: &mut impl Read) -> AmemResult<Self> {
        let mut buf = [0u8; 64];
        reader.read_exact(&mut buf).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                AmemError::Truncated
            } else {
                AmemError::Io(e)
            }
        })?;

        let magic = [buf[0], buf[1], buf[2], buf[3]];
        if magic != AMEM_MAGIC {
            return Err(AmemError::InvalidMagic);
        }

        let version = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        if version != FORMAT_VERSION {
            return Err(AmemError::UnsupportedVersion(version));
        }

        let dimension = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        // bytes 12..16 are reserved
        let node_count = u64::from_le_bytes(buf[16..24].try_into().unwrap());
        let edge_count = u64::from_le_bytes(buf[24..32].try_into().unwrap());
        let node_table_offset = u64::from_le_bytes(buf[32..40].try_into().unwrap());
        let edge_table_offset = u64::from_le_bytes(buf[40..48].try_into().unwrap());
        let content_block_offset = u64::from_le_bytes(buf[48..56].try_into().unwrap());
        let feature_vec_offset = u64::from_le_bytes(buf[56..64].try_into().unwrap());

        Ok(Self {
            magic,
            version,
            dimension,
            node_count,
            edge_count,
            node_table_offset,
            edge_table_offset,
            content_block_offset,
            feature_vec_offset,
        })
    }
}
