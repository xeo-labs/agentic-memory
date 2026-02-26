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
pub mod immortal_log;
pub mod indexes;
pub mod tiered;
pub mod retrieval;
pub mod engine;
pub mod config;
pub mod embeddings;
pub mod compression;
pub mod recovery;
pub mod migration;
pub mod claude_hooks;
pub mod ghost_writer;
pub mod edge_cases;

#[cfg(feature = "encryption")]
pub mod encryption;

#[cfg(test)]
pub mod tests;

// ═══════════════════════════════════════════════════════════════════
// RE-EXPORTS
// ═══════════════════════════════════════════════════════════════════

pub use block::{Block, BlockContent, BlockHash, BlockType, BoundaryType, FileOperation};
pub use engine::{EngineConfig, EngineStats, MemoryEngineV3, ResurrectionResult, SessionResumeResult};
pub use immortal_log::{ImmortalLog, IntegrityReport};
pub use indexes::{Index, IndexResult};
pub use retrieval::{
    RetrievalCoverage, RetrievalRequest, RetrievalResult, RetrievalStrategy,
    SmartRetrievalEngine,
};
pub use tiered::{TierConfig, TierStats, TieredStorage};
pub use config::MemoryV3Config;
pub use embeddings::EmbeddingManager;
pub use compression::{compress, decompress, CompressionLevel};
pub use recovery::{RecoveryManager, WriteAheadLog};
pub use migration::V2ToV3Migration;
pub use claude_hooks::ClaudeHooks;
pub use ghost_writer::{ClientType, DetectedClient, GhostWriter};
pub use edge_cases::{
    StorageError, LockError, ValidationError, FileLock, ProjectIsolation,
    NormalizedContent, ContentType, RecoveryMarker, IndexConsistencyReport,
    normalize_content, detect_content_type, validate_content_size,
    validated_timestamp, normalize_path, paths_equal, find_writable_location,
    safe_path, atomic_write, check_disk_space, safe_write_to_claude,
    merge_preserving_user_sections,
};

#[cfg(feature = "encryption")]
pub use encryption::{decrypt, derive_key, encrypt, generate_key, EncryptionKey};

/// V3 format version string
pub const V3_VERSION: &str = "3.0.0";

/// Magic bytes for V3 .imem files
pub const V3_MAGIC: &[u8; 4] = b"IMRT";
