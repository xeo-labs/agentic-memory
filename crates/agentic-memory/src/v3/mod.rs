//! V3 Immortal Architecture — Memory That Never Dies.
//!
//! The V3 engine implements an append-only, content-addressed memory system
//! built on immutable blocks with BLAKE3 integrity chains.
//!
//! # Architecture
//! - **Block**: Content-addressed, immutable unit of storage
//! - **ImmortalLog**: Append-only log (source of truth)
//! - **Five Indexes**: Temporal, Semantic, Causal, Entity, Procedural
//! - **TieredStorage**: Hot → Warm → Cold → Frozen
//! - **SmartRetrieval**: Multi-index fusion with token budgeting
//! - **GhostWriter**: Background sync to ALL AI coding assistants (Claude, Cursor, Windsurf, Cody)

pub mod block;
pub mod claude_hooks;
pub mod compression;
pub mod config;
pub mod edge_cases;
pub mod embeddings;
pub mod engine;
pub mod ghost_writer;
pub mod immortal_log;
pub mod indexes;
pub mod migration;
pub mod recovery;
pub mod retrieval;
pub mod tiered;

#[cfg(feature = "encryption")]
pub mod encryption;

#[cfg(test)]
pub mod tests;

// ═══════════════════════════════════════════════════════════════════
// RE-EXPORTS
// ═══════════════════════════════════════════════════════════════════

pub use block::{Block, BlockContent, BlockHash, BlockType, BoundaryType, FileOperation};
pub use claude_hooks::ClaudeHooks;
pub use compression::{compress, decompress, CompressionLevel};
pub use config::MemoryV3Config;
pub use edge_cases::{
    atomic_write, check_disk_space, detect_content_type, find_writable_location,
    merge_preserving_user_sections, normalize_content, normalize_path, paths_equal, safe_path,
    safe_write_to_claude, validate_content_size, validated_timestamp, ContentType, FileLock,
    IndexConsistencyReport, LockError, NormalizedContent, ProjectIsolation, RecoveryMarker,
    StorageError, ValidationError,
};
pub use embeddings::EmbeddingManager;
pub use engine::{
    EngineConfig, EngineStats, MemoryEngineV3, ResurrectionResult, SessionResumeResult,
};
pub use ghost_writer::{ClientType, DetectedClient, GhostWriter};
pub use immortal_log::{ImmortalLog, IntegrityReport};
pub use indexes::{Index, IndexResult};
pub use migration::V2ToV3Migration;
pub use recovery::{RecoveryManager, WriteAheadLog};
pub use retrieval::{
    RetrievalCoverage, RetrievalRequest, RetrievalResult, RetrievalStrategy, SmartRetrievalEngine,
};
pub use tiered::{TierConfig, TierStats, TieredStorage};

#[cfg(feature = "encryption")]
pub use encryption::{decrypt, derive_key, encrypt, generate_key, EncryptionKey};

/// V3 format version string
pub const V3_VERSION: &str = "3.0.0";

/// Magic bytes for V3 .imem files
pub const V3_MAGIC: &[u8; 4] = b"IMRT";
