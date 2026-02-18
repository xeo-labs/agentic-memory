//! AgenticMemory — binary graph-based memory system for AI agents.
//!
//! Stores cognitive events (facts, decisions, inferences, corrections, skills, episodes)
//! as nodes in a graph, with typed edges representing relationships between them.

pub mod cli;
pub mod engine;
pub mod ffi;
pub mod format;
pub mod graph;
pub mod index;
pub mod types;

// Re-export commonly used types at the crate root
pub use engine::{
    CausalParams, CausalResult, DecayReport, IngestResult, PatternParams, PatternSort, QueryEngine,
    SimilarityMatchResult, SimilarityParams, SubGraph, TemporalParams, TemporalResult, TimeRange,
    TraversalParams, TraversalResult, WriteEngine,
};
pub use format::{AmemReader, AmemWriter, MmapReader, SimilarityMatch};
pub use graph::{GraphBuilder, MemoryGraph, TraversalDirection};
pub use index::{cosine_similarity, ClusterMap, SessionIndex, TemporalIndex, TypeIndex};
pub use types::{
    now_micros, AmemError, AmemResult, CognitiveEvent, CognitiveEventBuilder, Edge, EdgeType,
    EventType, FileHeader, DEFAULT_DIMENSION, MAX_CONTENT_SIZE, MAX_EDGES_PER_NODE,
};
