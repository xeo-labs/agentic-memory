//! MCP tool implementations — the primary way LLMs interact with memory.

pub mod conversation_log;
pub mod memory_add;
pub mod memory_causal;
pub mod memory_context;
pub mod memory_correct;
pub mod memory_evidence;
pub mod memory_ground;
pub mod memory_quality;
pub mod memory_query;
pub mod memory_resolve;
pub mod memory_session_resume;
pub mod memory_similar;
pub mod memory_stats;
pub mod memory_suggest;
pub mod memory_temporal;
pub mod memory_traverse;
pub mod memory_workspace_add;
pub mod memory_workspace_compare;
pub mod memory_workspace_create;
pub mod memory_workspace_list;
pub mod memory_workspace_query;
pub mod memory_workspace_xref;
pub mod registry;
pub mod session_end;
pub mod session_start;

// 24 Inventions — INFINITUS
pub mod invention_collective; // Inventions 9-12:  Ancestor Memory, Collective Memory, Memory Fusion, Memory Telepathy
pub mod invention_infinite; // Inventions 1-4:   Immortal Memory, Semantic Compression, Context Optimization, Memory Metabolism
pub mod invention_metamemory; // Inventions 17-20: Self-Awareness, Memory Dreams, Belief Revision, Cognitive Load Balancing
pub mod invention_prophetic; // Inventions 5-8:   Predictive Memory, Memory Prophecy, Counterfactual Memory, Déjà Vu Detection
pub mod invention_resurrection; // Inventions 13-16: Memory Archaeology, Holographic Memory, Memory Immune System, Phoenix Protocol
pub mod invention_transcendent; // Inventions 21-24: Memory Singularity, Temporal Omniscience, Consciousness Crystal, Memory Transcendence

#[cfg(feature = "v3")]
pub mod v3_tools;

pub use registry::ToolRegistry;
