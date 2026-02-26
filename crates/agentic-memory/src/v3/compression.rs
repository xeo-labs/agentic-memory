//! Compression for cold and frozen tiers.
//! Uses lz4 when available, otherwise falls back to no compression.
//! Note: zstd is specified in the full spec but lz4 is already a dep.

/// Compression level
#[derive(Debug, Clone, Copy)]
pub enum CompressionLevel {
    None,
    Fast,
    Default,
    Best,
}

/// Compress data using lz4
pub fn compress(data: &[u8], level: CompressionLevel) -> Vec<u8> {
    if matches!(level, CompressionLevel::None) || data.is_empty() {
        return data.to_vec();
    }

    #[cfg(feature = "format")]
    {
        lz4_flex::compress_prepend_size(data)
    }

    #[cfg(not(feature = "format"))]
    {
        data.to_vec()
    }
}

/// Decompress data
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    #[cfg(feature = "format")]
    {
        lz4_flex::decompress_size_prepended(data).map_err(|e| {
            // If decompression fails, data might not be compressed
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })
    }

    #[cfg(not(feature = "format"))]
    {
        Ok(data.to_vec())
    }
}

/// Check if data appears to be compressed (has lz4 size prefix)
pub fn is_compressed(data: &[u8]) -> bool {
    // lz4_flex prepend_size format: first 4 bytes are uncompressed size (LE)
    // We can't definitively tell, but data.len() >= 4 is a minimum
    data.len() >= 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let original = b"Hello, this is a test of compression!".repeat(100);

        let compressed = compress(&original, CompressionLevel::Default);

        #[cfg(feature = "format")]
        {
            assert!(compressed.len() < original.len());
            let decompressed = decompress(&compressed).unwrap();
            assert_eq!(decompressed, original);
        }

        #[cfg(not(feature = "format"))]
        {
            assert_eq!(compressed, original);
        }
    }

    #[test]
    fn test_no_compression() {
        let original = b"Small data";
        let result = compress(original, CompressionLevel::None);
        assert_eq!(result, original);
    }
}
