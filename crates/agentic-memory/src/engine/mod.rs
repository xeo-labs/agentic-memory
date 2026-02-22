//! High-level operations — write engine and query engine.

pub mod cognitive;
pub mod decay;
pub mod graph_algo;
pub mod maintenance;
pub mod query;
pub mod text_search;
pub mod tokenizer;
pub mod write;

pub use query::{
    CausalParams, CausalResult, MemoryQualityParams, MemoryQualityReport, PatternParams,
    PatternSort, QueryEngine, SimilarityMatchResult, SimilarityParams, SubGraph, TemporalParams,
    TemporalResult, TimeRange, TraversalParams, TraversalResult,
};
pub use write::{DecayReport, IngestResult, WriteEngine};

// New query expansion types
pub use cognitive::{
    AnalogicalAnchor, AnalogicalParams, Analogy, BeliefRevisionParams, BeliefSnapshot,
    BeliefTimeline, CascadeEffect, CascadeStep, ChangeType, ContradictedNode, DriftParams,
    DriftReport, Gap, GapDetectionParams, GapReport, GapSeverity, GapSummary, GapType,
    PatternMatch, RevisionReport, WeakenedNode,
};
pub use graph_algo::{
    CentralityAlgorithm, CentralityParams, CentralityResult, PathResult, ShortestPathParams,
};
pub use maintenance::{
    ConsolidationAction, ConsolidationOp, ConsolidationParams, ConsolidationReport,
};
pub use text_search::{HybridMatch, HybridSearchParams, TextMatch, TextSearchParams};
pub use tokenizer::Tokenizer;
