//! Embedding model registry and migration strategies.
//!
//! Tracks which embedding model generated each memory's vector,
//! and provides strategies for migrating between models.

use super::store::{LongevityError, LongevityStore};
use serde::{Deserialize, Serialize};

/// Migration strategy for transitioning between embedding models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MigrationStrategy {
    /// Re-embed on access (spread cost over time)
    #[default]
    LazyReEmbedding,
    /// Train linear mapping from old space to new space
    ProjectionMapping,
    /// Use anchor memories as bridge between embedding spaces
    SemanticAnchors,
}

/// Registered embedding model with lifecycle tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModel {
    pub model_id: String,
    pub model_name: String,
    pub dimension: u32,
    pub provider: String,
    pub is_active: bool,
    pub memories_count: u64,
}

/// Status of an embedding migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingMigrationStatus {
    pub from_model: String,
    pub to_model: String,
    pub strategy: MigrationStrategy,
    pub total_memories: u64,
    pub migrated_memories: u64,
    pub remaining_memories: u64,
    pub progress_percent: f64,
}

/// The embedding migrator manages model transitions.
pub struct EmbeddingMigrator {
    strategy: MigrationStrategy,
}

impl EmbeddingMigrator {
    pub fn new(strategy: MigrationStrategy) -> Self {
        Self { strategy }
    }

    /// Register a new embedding model in the store.
    pub fn register_model(
        store: &LongevityStore,
        model_id: &str,
        model_name: &str,
        dimension: u32,
        provider: &str,
    ) -> Result<(), LongevityError> {
        store.register_embedding_model(model_id, model_name, dimension, provider)
    }

    /// Switch to a new embedding model, retiring the old one.
    pub fn switch_model(
        store: &LongevityStore,
        old_model_id: &str,
        new_model_id: &str,
    ) -> Result<(), LongevityError> {
        store.retire_embedding_model(old_model_id, Some(new_model_id))
    }

    /// Get migration status between models.
    pub fn migration_status(
        store: &LongevityStore,
        from_model: &str,
        to_model: &str,
    ) -> Result<EmbeddingMigrationStatus, LongevityError> {
        let total = store.count_memories_with_model(from_model)?;
        let migrated = store.count_memories_with_model(to_model)?;
        let remaining = total;

        let progress = if total + migrated > 0 {
            (migrated as f64 / (total + migrated) as f64) * 100.0
        } else {
            100.0
        };

        Ok(EmbeddingMigrationStatus {
            from_model: from_model.to_string(),
            to_model: to_model.to_string(),
            strategy: MigrationStrategy::LazyReEmbedding,
            total_memories: total + migrated,
            migrated_memories: migrated,
            remaining_memories: remaining,
            progress_percent: progress,
        })
    }

    /// Get all registered models with their status.
    pub fn list_models(store: &LongevityStore) -> Result<Vec<EmbeddingModel>, LongevityError> {
        // For now, just return the active model
        let mut models = Vec::new();
        if let Some(active) = store.get_active_embedding_model()? {
            let count = store.count_memories_with_model(&active.model_id)?;
            models.push(EmbeddingModel {
                model_id: active.model_id,
                model_name: active.model_name,
                dimension: active.dimension,
                provider: active.provider.unwrap_or_default(),
                is_active: true,
                memories_count: count,
            });
        }
        Ok(models)
    }

    /// Get the current migration strategy.
    pub fn strategy(&self) -> MigrationStrategy {
        self.strategy
    }
}

impl Default for EmbeddingMigrator {
    fn default() -> Self {
        Self::new(MigrationStrategy::LazyReEmbedding)
    }
}
