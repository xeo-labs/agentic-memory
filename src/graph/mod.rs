//! In-memory graph operations — the core data structure.

pub mod builder;
pub mod memory_graph;
pub mod traversal;

pub use builder::GraphBuilder;
pub use memory_graph::MemoryGraph;
pub use traversal::{bfs_traverse, TraversalDirection};
