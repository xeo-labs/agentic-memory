//! LZ4 content compression/decompression.

use crate::types::error::{AmemError, AmemResult};

/// Compress UTF-8 content bytes with LZ4 (prepend size for decompression).
pub fn compress_content(content: &str) -> AmemResult<Vec<u8>> {
    Ok(lz4_flex::compress_prepend_size(content.as_bytes()))
}

/// Decompress LZ4-compressed content bytes back to a UTF-8 string.
pub fn decompress_content(data: &[u8]) -> AmemResult<String> {
    let bytes = lz4_flex::decompress_size_prepended(data)
        .map_err(|e| AmemError::Compression(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| AmemError::Compression(e.to_string()))
}
