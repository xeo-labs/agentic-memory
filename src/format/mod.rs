//! Binary file I/O for .amem files.

pub mod compression;
pub mod mmap;
pub mod reader;
pub mod writer;

pub use mmap::{MmapReader, SimilarityMatch};
pub use reader::AmemReader;
pub use writer::AmemWriter;
