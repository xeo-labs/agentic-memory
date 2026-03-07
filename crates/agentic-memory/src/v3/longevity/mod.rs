//! V4 Longevity Engine — Memory That Survives 20 Years.
//!
//! The longevity engine bridges V3's immortal log with a SQLite backing store
//! that provides cognitive compression, schema versioning, embedding migration,
//! encryption rotation, and storage budget management.
//!
//! # Architecture
//! - **LongevityStore**: SQLite backing store for compressed, long-term memories
//! - **SignificanceScorer**: 7-factor weighted scoring for memory importance
//! - **MemoryHierarchy**: 6-layer compression (Raw → Episode → Summary → Pattern → Trait → Identity)
//! - **ConsolidationEngine**: Scheduled compression daemon
//! - **SchemaVersioning**: Automatic forward-compatible migrations
//! - **EmbeddingMigration**: Track and migrate between embedding models
//! - **EncryptionRotation**: Key lifecycle management
//! - **StorageBudget**: Budget allocation and projection
//! - **BackupDaemon**: Email, local, cloud backup with rotation
//! - **CaptureDaemon**: Client log monitoring + deduplication
//! - **SyncProtocol**: .amem ↔ SQLite bidirectional sync

pub mod backup;
pub mod budget;
pub mod capture;
pub mod consolidation;
pub mod embedding_migration;
pub mod encryption_rotation;
pub mod forgetting;
pub mod hierarchy;
pub mod integrity;
pub mod schema;
pub mod significance;
pub mod store;
pub mod sync;

#[cfg(test)]
pub mod tests;

// Re-exports
pub use backup::{BackupConfig, BackupDaemon, BackupMode, BackupSchedule};
pub use budget::{BudgetAlert, LayerBudget, StorageBudget, StorageProjection};
pub use capture::{CaptureDaemon, CaptureEvent, ClientLogMonitor, ContentDedup};
pub use consolidation::{ConsolidationEngine, ConsolidationSchedule, ConsolidationTask};
pub use embedding_migration::{EmbeddingMigrator, EmbeddingModel, MigrationStrategy};
pub use encryption_rotation::{EncryptionRotator, KeyLifecycle, KeyStatus};
pub use forgetting::{ForgettingProtocol, ForgettingVerdict};
pub use hierarchy::{MemoryHierarchy, MemoryLayer, MemoryRecord};
pub use integrity::{IntegrityVerifier, MerkleProof};
pub use schema::{MigrationEngine, SchemaVersion};
pub use significance::{SignificanceFactor, SignificanceScorer, SignificanceThreshold};
pub use store::LongevityStore;
pub use sync::{SyncDirection, SyncProtocol, SyncResult};
