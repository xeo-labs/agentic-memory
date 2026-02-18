//! Error types for the AgenticMemory library.

use thiserror::Error;

/// All errors that can occur in the AgenticMemory library.
#[derive(Error, Debug)]
pub enum AmemError {
    /// Invalid magic bytes in file header.
    #[error("Invalid magic bytes in file header")]
    InvalidMagic,

    /// Unsupported format version.
    #[error("Unsupported format version: {0}")]
    UnsupportedVersion(u32),

    /// Node not found by ID.
    #[error("Node ID {0} not found")]
    NodeNotFound(u64),

    /// Edge references an invalid node ID.
    #[error("Edge references invalid node ID: {0}")]
    InvalidEdgeTarget(u64),

    /// Self-edge not allowed.
    #[error("Self-edge not allowed on node {0}")]
    SelfEdge(u64),

    /// Content exceeds maximum size.
    #[error("Content exceeds maximum size: {size} > {max}")]
    ContentTooLarge { size: usize, max: usize },

    /// Feature vector dimension mismatch.
    #[error("Feature vector dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    /// Too many edges on a single node.
    #[error("Maximum edges per node exceeded: {0}")]
    TooManyEdges(u16),

    /// Confidence value out of valid range.
    #[error("Confidence value out of range [0.0, 1.0]: {0}")]
    InvalidConfidence(f32),

    /// Weight value out of valid range.
    #[error("Weight value out of range [0.0, 1.0]: {0}")]
    InvalidWeight(f32),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Compression error.
    #[error("Compression error: {0}")]
    Compression(String),

    /// File is empty or truncated.
    #[error("File is empty or truncated")]
    Truncated,

    /// Corrupt data at a given offset.
    #[error("Corrupt data at offset {0}")]
    Corrupt(u64),
}

/// Convenience result type for AgenticMemory operations.
pub type AmemResult<T> = Result<T, AmemError>;
