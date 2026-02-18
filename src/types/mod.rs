//! All data types for the AgenticMemory library.

pub mod edge;
pub mod error;
pub mod event;
pub mod header;

pub use edge::{Edge, EdgeType};
pub use error::{AmemError, AmemResult};
pub use event::{CognitiveEvent, CognitiveEventBuilder, EventType};
pub use header::{FileHeader, HEADER_SIZE};

/// Magic bytes at the start of every .amem file.
pub const AMEM_MAGIC: [u8; 4] = [0x41, 0x4D, 0x45, 0x4D]; // "AMEM"

/// Current format version.
pub const FORMAT_VERSION: u32 = 1;

/// Default feature vector dimensionality.
pub const DEFAULT_DIMENSION: usize = 128;

/// Maximum content size per node (before compression): 64KB.
pub const MAX_CONTENT_SIZE: usize = 65_536;

/// Maximum edges per node.
pub const MAX_EDGES_PER_NODE: u16 = 4096;

/// Returns the current time as Unix epoch microseconds.
pub fn now_micros() -> u64 {
    chrono::Utc::now().timestamp_micros() as u64
}
