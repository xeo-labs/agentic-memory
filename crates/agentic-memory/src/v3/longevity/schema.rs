//! Schema versioning and migration engine.
//!
//! Guarantees: Any `.longevity.db` created by any version of AgenticMemory
//! will be readable by any future version.

use super::store::{LongevityError, LongevityStore};
use serde::{Deserialize, Serialize};

/// Current schema version.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// A schema version descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub version: u32,
    pub description: String,
    pub migration_sql: Option<String>,
}

/// The migration engine manages schema evolution.
pub struct MigrationEngine;

impl MigrationEngine {
    /// Check if the database needs migration and run it if so.
    pub fn migrate_if_needed(store: &LongevityStore) -> Result<Vec<SchemaVersion>, LongevityError> {
        let current = store.current_schema_version()?;
        let mut applied = Vec::new();

        if current >= CURRENT_SCHEMA_VERSION {
            return Ok(applied);
        }

        // Run migrations sequentially
        for version in (current + 1)..=CURRENT_SCHEMA_VERSION {
            if let Some(migration) = Self::get_migration(version) {
                // Migrations are currently additive-only in V1
                // Future versions will add ALTER TABLE statements here
                store.record_migration(version, &migration.description, migration.migration_sql.as_deref().unwrap_or(""))?;
                applied.push(migration);
            }
        }

        Ok(applied)
    }

    /// Get a specific version's migration.
    fn get_migration(version: u32) -> Option<SchemaVersion> {
        match version {
            1 => Some(SchemaVersion {
                version: 1,
                description: "Initial longevity schema".to_string(),
                migration_sql: None, // Schema V1 is created by initialize_schema
            }),
            // Future migrations go here:
            // 2 => Some(SchemaVersion { ... }),
            _ => None,
        }
    }

    /// Get the list of all known migrations.
    pub fn all_migrations() -> Vec<SchemaVersion> {
        let mut migrations = Vec::new();
        for v in 1..=CURRENT_SCHEMA_VERSION {
            if let Some(m) = Self::get_migration(v) {
                migrations.push(m);
            }
        }
        migrations
    }
}
