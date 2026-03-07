//! SQLite longevity store — the cold-path persistence layer.
//!
//! All compressed memories, consolidation logs, schema versions, embedding models,
//! and integrity proofs live here. This is the 20-year truth store.

use super::hierarchy::{HierarchyStats, MemoryLayer, MemoryRecord};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

/// The SQLite-backed longevity store.
pub struct LongevityStore {
    conn: Connection,
    path: PathBuf,
}

impl LongevityStore {
    /// Open or create a longevity database at the given path.
    pub fn open(path: &Path) -> Result<Self, LongevityError> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for concurrent reads + writes
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")?;

        let store = Self {
            conn,
            path: path.to_path_buf(),
        };
        store.initialize_schema()?;
        Ok(store)
    }

    /// Open an in-memory store (for testing).
    pub fn open_memory() -> Result<Self, LongevityError> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn,
            path: PathBuf::from(":memory:"),
        };
        store.initialize_schema()?;
        Ok(store)
    }

    /// Get the database file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Initialize the database schema (creates tables if not exist).
    fn initialize_schema(&self) -> Result<(), LongevityError> {
        self.conn.execute_batch(SCHEMA_V1)?;
        // Record schema version 1 if not already present
        let count: u32 = self
            .conn
            .query_row("SELECT COUNT(*) FROM schema_versions", [], |row| {
                row.get(0)
            })?;
        if count == 0 {
            self.conn.execute(
                "INSERT INTO schema_versions (version, applied_at, description) VALUES (1, datetime('now'), 'Initial longevity schema')",
                [],
            )?;
        }
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    // MEMORY CRUD
    // ═══════════════════════════════════════════════════════════════

    /// Insert a memory record.
    pub fn insert_memory(&self, record: &MemoryRecord) -> Result<(), LongevityError> {
        let embedding_blob = record.embedding.as_ref().map(|v| {
            v.iter()
                .flat_map(|f| f.to_le_bytes())
                .collect::<Vec<u8>>()
        });
        let original_ids_json = record
            .original_ids
            .as_ref()
            .map(|ids| serde_json::to_string(ids).unwrap_or_default());
        let metadata_json = record
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

        self.conn.execute(
            "INSERT OR REPLACE INTO memories (
                id, layer, content, content_type, embedding, embedding_model,
                significance, access_count, last_accessed, created_at,
                original_ids, session_id, project_id, metadata,
                encryption_key_id, schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                record.id,
                record.layer as u8,
                serde_json::to_string(&record.content).unwrap_or_default(),
                record.content_type,
                embedding_blob,
                record.embedding_model,
                record.significance,
                record.access_count,
                record.last_accessed,
                record.created_at,
                original_ids_json,
                record.session_id,
                record.project_id,
                metadata_json,
                record.encryption_key_id,
                record.schema_version,
            ],
        )?;

        // Update FTS index
        let text = record.extract_text();
        if !text.is_empty() {
            // Delete any existing FTS entry first
            self.conn.execute(
                "DELETE FROM memories_fts WHERE rowid = (SELECT rowid FROM memories WHERE id = ?1)",
                params![record.id],
            ).ok(); // Ignore errors for missing rows
            self.conn.execute(
                "INSERT INTO memories_fts (rowid, content) SELECT rowid, ?2 FROM memories WHERE id = ?1",
                params![record.id, text],
            )?;
        }

        Ok(())
    }

    /// Get a memory by ID.
    pub fn get_memory(&self, id: &str) -> Result<Option<MemoryRecord>, LongevityError> {
        let result = self
            .conn
            .query_row(
                "SELECT id, layer, content, content_type, embedding, embedding_model,
                 significance, access_count, last_accessed, created_at,
                 original_ids, session_id, project_id, metadata,
                 encryption_key_id, schema_version
                 FROM memories WHERE id = ?1",
                params![id],
                |row| Self::row_to_record(row),
            )
            .optional()?;
        Ok(result)
    }

    /// Query memories by layer and project.
    pub fn query_by_layer(
        &self,
        project_id: &str,
        layer: MemoryLayer,
        limit: u32,
    ) -> Result<Vec<MemoryRecord>, LongevityError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, layer, content, content_type, embedding, embedding_model,
             significance, access_count, last_accessed, created_at,
             original_ids, session_id, project_id, metadata,
             encryption_key_id, schema_version
             FROM memories WHERE project_id = ?1 AND layer = ?2
             ORDER BY created_at DESC LIMIT ?3",
        )?;

        let records = stmt
            .query_map(params![project_id, layer as u8, limit], |row| {
                Self::row_to_record(row)
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    /// Query memories by significance range.
    pub fn query_by_significance(
        &self,
        project_id: &str,
        min_significance: f64,
        max_significance: f64,
        limit: u32,
    ) -> Result<Vec<MemoryRecord>, LongevityError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, layer, content, content_type, embedding, embedding_model,
             significance, access_count, last_accessed, created_at,
             original_ids, session_id, project_id, metadata,
             encryption_key_id, schema_version
             FROM memories WHERE project_id = ?1
             AND significance >= ?2 AND significance <= ?3
             ORDER BY significance DESC LIMIT ?4",
        )?;

        let records = stmt
            .query_map(
                params![project_id, min_significance, max_significance, limit],
                |row| Self::row_to_record(row),
            )?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    /// Full-text search across all memories for a project.
    pub fn search_fulltext(
        &self,
        project_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<MemoryRecord>, LongevityError> {
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.layer, m.content, m.content_type, m.embedding, m.embedding_model,
             m.significance, m.access_count, m.last_accessed, m.created_at,
             m.original_ids, m.session_id, m.project_id, m.metadata,
             m.encryption_key_id, m.schema_version
             FROM memories m
             INNER JOIN memories_fts fts ON fts.rowid = m.rowid
             WHERE fts.content MATCH ?1 AND m.project_id = ?2
             ORDER BY rank LIMIT ?3",
        )?;

        let records = stmt
            .query_map(params![query, project_id, limit], |row| {
                Self::row_to_record(row)
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    /// Update significance score for a memory.
    pub fn update_significance(
        &self,
        id: &str,
        significance: f64,
    ) -> Result<(), LongevityError> {
        self.conn.execute(
            "UPDATE memories SET significance = ?1 WHERE id = ?2",
            params![significance, id],
        )?;
        Ok(())
    }

    /// Increment access count and update last_accessed.
    pub fn record_access(&self, id: &str) -> Result<(), LongevityError> {
        self.conn.execute(
            "UPDATE memories SET access_count = access_count + 1, last_accessed = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// Delete memories by ID.
    pub fn delete_memories(&self, ids: &[String]) -> Result<u64, LongevityError> {
        let mut count = 0u64;
        for id in ids {
            count += self
                .conn
                .execute("DELETE FROM memories WHERE id = ?1", params![id])? as u64;
        }
        Ok(count)
    }

    /// Get memories older than a given date at a specific layer.
    pub fn get_old_memories(
        &self,
        project_id: &str,
        layer: MemoryLayer,
        older_than: &str,
        limit: u32,
    ) -> Result<Vec<MemoryRecord>, LongevityError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, layer, content, content_type, embedding, embedding_model,
             significance, access_count, last_accessed, created_at,
             original_ids, session_id, project_id, metadata,
             encryption_key_id, schema_version
             FROM memories WHERE project_id = ?1 AND layer = ?2 AND created_at < ?3
             ORDER BY created_at ASC LIMIT ?4",
        )?;

        let records = stmt
            .query_map(params![project_id, layer as u8, older_than, limit], |row| {
                Self::row_to_record(row)
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    // ═══════════════════════════════════════════════════════════════
    // CONSOLIDATION LOG
    // ═══════════════════════════════════════════════════════════════

    /// Log a consolidation event.
    pub fn log_consolidation(
        &self,
        id: &str,
        from_layer: MemoryLayer,
        to_layer: MemoryLayer,
        memories_processed: u32,
        memories_created: u32,
        compression_ratio: f64,
        algorithm: &str,
        duration_ms: u64,
    ) -> Result<(), LongevityError> {
        self.conn.execute(
            "INSERT INTO consolidation_log (
                id, from_layer, to_layer, memories_processed, memories_created,
                compression_ratio, algorithm, executed_at, duration_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'), ?8)",
            params![
                id,
                from_layer as u8,
                to_layer as u8,
                memories_processed,
                memories_created,
                compression_ratio,
                algorithm,
                duration_ms,
            ],
        )?;
        Ok(())
    }

    /// Get consolidation history.
    pub fn get_consolidation_log(
        &self,
        limit: u32,
    ) -> Result<Vec<ConsolidationLogEntry>, LongevityError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_layer, to_layer, memories_processed, memories_created,
             compression_ratio, algorithm, executed_at, duration_ms
             FROM consolidation_log ORDER BY executed_at DESC LIMIT ?1",
        )?;

        let entries = stmt
            .query_map(params![limit], |row| {
                Ok(ConsolidationLogEntry {
                    id: row.get(0)?,
                    from_layer: row.get(1)?,
                    to_layer: row.get(2)?,
                    memories_processed: row.get(3)?,
                    memories_created: row.get(4)?,
                    compression_ratio: row.get(5)?,
                    algorithm: row.get(6)?,
                    executed_at: row.get(7)?,
                    duration_ms: row.get(8)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    // ═══════════════════════════════════════════════════════════════
    // EMBEDDING MODELS
    // ═══════════════════════════════════════════════════════════════

    /// Register a new embedding model.
    pub fn register_embedding_model(
        &self,
        model_id: &str,
        model_name: &str,
        dimension: u32,
        provider: &str,
    ) -> Result<(), LongevityError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO embedding_models (model_id, model_name, dimension, provider, registered_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![model_id, model_name, dimension, provider],
        )?;
        Ok(())
    }

    /// Retire an embedding model and optionally set its successor.
    pub fn retire_embedding_model(
        &self,
        model_id: &str,
        successor: Option<&str>,
    ) -> Result<(), LongevityError> {
        self.conn.execute(
            "UPDATE embedding_models SET retired_at = datetime('now'), mapping_to = ?2 WHERE model_id = ?1",
            params![model_id, successor],
        )?;
        Ok(())
    }

    /// Get the currently active embedding model.
    pub fn get_active_embedding_model(
        &self,
    ) -> Result<Option<EmbeddingModelEntry>, LongevityError> {
        let result = self
            .conn
            .query_row(
                "SELECT model_id, model_name, dimension, provider, registered_at, retired_at, mapping_to
                 FROM embedding_models WHERE retired_at IS NULL
                 ORDER BY registered_at DESC LIMIT 1",
                [],
                |row| {
                    Ok(EmbeddingModelEntry {
                        model_id: row.get(0)?,
                        model_name: row.get(1)?,
                        dimension: row.get(2)?,
                        provider: row.get(3)?,
                        registered_at: row.get(4)?,
                        retired_at: row.get(5)?,
                        mapping_to: row.get(6)?,
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    /// Count memories using a specific embedding model.
    pub fn count_memories_with_model(
        &self,
        model_id: &str,
    ) -> Result<u64, LongevityError> {
        let count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE embedding_model = ?1",
            params![model_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // ═══════════════════════════════════════════════════════════════
    // ENCRYPTION KEYS
    // ═══════════════════════════════════════════════════════════════

    /// Store an encryption key.
    pub fn store_encryption_key(
        &self,
        key_id: &str,
        algorithm: &str,
        status: &str,
        key_blob: &[u8],
    ) -> Result<(), LongevityError> {
        self.conn.execute(
            "INSERT INTO encryption_keys (key_id, algorithm, created_at, status, key_blob)
             VALUES (?1, ?2, datetime('now'), ?3, ?4)",
            params![key_id, algorithm, status, key_blob],
        )?;
        Ok(())
    }

    /// Get the active encryption key.
    pub fn get_active_encryption_key(&self) -> Result<Option<EncryptionKeyEntry>, LongevityError> {
        let result = self
            .conn
            .query_row(
                "SELECT key_id, algorithm, created_at, retired_at, status, key_blob
                 FROM encryption_keys WHERE status = 'active'
                 ORDER BY created_at DESC LIMIT 1",
                [],
                |row| {
                    Ok(EncryptionKeyEntry {
                        key_id: row.get(0)?,
                        algorithm: row.get(1)?,
                        created_at: row.get(2)?,
                        retired_at: row.get(3)?,
                        status: row.get(4)?,
                        key_blob: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    /// Retire an encryption key.
    pub fn retire_encryption_key(&self, key_id: &str) -> Result<(), LongevityError> {
        self.conn.execute(
            "UPDATE encryption_keys SET status = 'retired', retired_at = datetime('now') WHERE key_id = ?1",
            params![key_id],
        )?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════
    // SCHEMA VERSIONING
    // ═══════════════════════════════════════════════════════════════

    /// Get current schema version.
    pub fn current_schema_version(&self) -> Result<u32, LongevityError> {
        let version: u32 = self.conn.query_row(
            "SELECT MAX(version) FROM schema_versions",
            [],
            |row| row.get(0),
        )?;
        Ok(version)
    }

    /// Record a schema migration.
    pub fn record_migration(
        &self,
        version: u32,
        description: &str,
        migration_sql: &str,
    ) -> Result<(), LongevityError> {
        self.conn.execute(
            "INSERT INTO schema_versions (version, applied_at, description, migration_sql)
             VALUES (?1, datetime('now'), ?2, ?3)",
            params![version, description, migration_sql],
        )?;
        Ok(())
    }

    /// Get schema version history.
    pub fn schema_history(&self) -> Result<Vec<SchemaVersionEntry>, LongevityError> {
        let mut stmt = self.conn.prepare(
            "SELECT version, applied_at, description, migration_sql
             FROM schema_versions ORDER BY version ASC",
        )?;

        let entries = stmt
            .query_map([], |row| {
                Ok(SchemaVersionEntry {
                    version: row.get(0)?,
                    applied_at: row.get(1)?,
                    description: row.get(2)?,
                    migration_sql: row.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    // ═══════════════════════════════════════════════════════════════
    // INTEGRITY PROOFS
    // ═══════════════════════════════════════════════════════════════

    /// Store a Merkle root proof.
    pub fn store_integrity_proof(
        &self,
        proof_id: &str,
        proof_type: &str,
        root_hash: &str,
        block_count: u64,
    ) -> Result<(), LongevityError> {
        self.conn.execute(
            "INSERT INTO integrity_proofs (proof_id, proof_type, root_hash, block_count, created_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![proof_id, proof_type, root_hash, block_count],
        )?;
        Ok(())
    }

    /// Get the latest integrity proof.
    pub fn latest_integrity_proof(&self) -> Result<Option<IntegrityProofEntry>, LongevityError> {
        let result = self
            .conn
            .query_row(
                "SELECT proof_id, proof_type, root_hash, block_count, created_at
                 FROM integrity_proofs ORDER BY created_at DESC LIMIT 1",
                [],
                |row| {
                    Ok(IntegrityProofEntry {
                        proof_id: row.get(0)?,
                        proof_type: row.get(1)?,
                        root_hash: row.get(2)?,
                        block_count: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    // ═══════════════════════════════════════════════════════════════
    // STATISTICS
    // ═══════════════════════════════════════════════════════════════

    /// Get hierarchy statistics for a project.
    pub fn hierarchy_stats(&self, project_id: &str) -> Result<HierarchyStats, LongevityError> {
        let mut stats = HierarchyStats::default();

        let mut stmt = self.conn.prepare(
            "SELECT layer, COUNT(*), COALESCE(SUM(LENGTH(content)), 0)
             FROM memories WHERE project_id = ?1
             GROUP BY layer",
        )?;

        let rows = stmt.query_map(params![project_id], |row| {
            Ok((row.get::<_, u8>(0)?, row.get::<_, u64>(1)?, row.get::<_, u64>(2)?))
        })?;

        for row in rows.flatten() {
            let (layer, count, bytes) = row;
            match MemoryLayer::from_u8(layer) {
                Some(MemoryLayer::Raw) => {
                    stats.raw_count = count;
                    stats.raw_bytes = bytes;
                }
                Some(MemoryLayer::Episode) => {
                    stats.episode_count = count;
                    stats.episode_bytes = bytes;
                }
                Some(MemoryLayer::Summary) => {
                    stats.summary_count = count;
                    stats.summary_bytes = bytes;
                }
                Some(MemoryLayer::Pattern) => {
                    stats.pattern_count = count;
                    stats.pattern_bytes = bytes;
                }
                Some(MemoryLayer::Trait) => {
                    stats.trait_count = count;
                    stats.trait_bytes = bytes;
                }
                Some(MemoryLayer::Identity) => {
                    stats.identity_count = count;
                    stats.identity_bytes = bytes;
                }
                None => {}
            }
        }

        stats.total_count = stats.raw_count
            + stats.episode_count
            + stats.summary_count
            + stats.pattern_count
            + stats.trait_count
            + stats.identity_count;
        stats.total_bytes = stats.raw_bytes
            + stats.episode_bytes
            + stats.summary_bytes
            + stats.pattern_bytes
            + stats.trait_bytes
            + stats.identity_bytes;

        Ok(stats)
    }

    /// Get total memory count for a project.
    pub fn total_count(&self, project_id: &str) -> Result<u64, LongevityError> {
        let count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE project_id = ?1",
            params![project_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get total database file size in bytes.
    pub fn database_size_bytes(&self) -> Result<u64, LongevityError> {
        let page_count: u64 = self
            .conn
            .query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let page_size: u64 = self
            .conn
            .query_row("PRAGMA page_size", [], |row| row.get(0))?;
        Ok(page_count * page_size)
    }

    /// Get the maximum access count across all memories.
    pub fn max_access_count(&self, project_id: &str) -> Result<u64, LongevityError> {
        let count: u64 = self.conn.query_row(
            "SELECT COALESCE(MAX(access_count), 0) FROM memories WHERE project_id = ?1",
            params![project_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // ═══════════════════════════════════════════════════════════════
    // INTERNAL HELPERS
    // ═══════════════════════════════════════════════════════════════

    fn row_to_record(row: &rusqlite::Row) -> Result<MemoryRecord, rusqlite::Error> {
        let layer_u8: u8 = row.get(1)?;
        let content_str: String = row.get(2)?;
        let embedding_blob: Option<Vec<u8>> = row.get(4)?;
        let original_ids_str: Option<String> = row.get(10)?;
        let metadata_str: Option<String> = row.get(13)?;

        let content = serde_json::from_str(&content_str).unwrap_or(serde_json::Value::Null);
        let embedding = embedding_blob.map(|blob| {
            blob.chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect()
        });
        let original_ids = original_ids_str
            .and_then(|s| serde_json::from_str(&s).ok());
        let metadata = metadata_str
            .and_then(|s| serde_json::from_str(&s).ok());

        Ok(MemoryRecord {
            id: row.get(0)?,
            layer: MemoryLayer::from_u8(layer_u8).unwrap_or(MemoryLayer::Raw),
            content,
            content_type: row.get(3)?,
            embedding,
            embedding_model: row.get(5)?,
            significance: row.get(6)?,
            access_count: row.get(7)?,
            last_accessed: row.get(8)?,
            created_at: row.get(9)?,
            original_ids,
            session_id: row.get(11)?,
            project_id: row.get(12)?,
            metadata,
            encryption_key_id: row.get(14)?,
            schema_version: row.get(15)?,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════
// SCHEMA SQL
// ═══════════════════════════════════════════════════════════════════

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS memories (
    id              TEXT PRIMARY KEY,
    layer           INTEGER NOT NULL,
    content         TEXT NOT NULL,
    content_type    TEXT NOT NULL,
    embedding       BLOB,
    embedding_model TEXT,
    significance    REAL NOT NULL DEFAULT 0.5,
    access_count    INTEGER DEFAULT 0,
    last_accessed   TEXT,
    created_at      TEXT NOT NULL,
    original_ids    TEXT,
    session_id      TEXT,
    project_id      TEXT NOT NULL,
    metadata        TEXT,
    encryption_key_id TEXT,
    schema_version  INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_memories_layer ON memories(layer);
CREATE INDEX IF NOT EXISTS idx_memories_project ON memories(project_id);
CREATE INDEX IF NOT EXISTS idx_memories_created ON memories(created_at);
CREATE INDEX IF NOT EXISTS idx_memories_significance ON memories(significance);
CREATE INDEX IF NOT EXISTS idx_memories_session ON memories(session_id);
CREATE INDEX IF NOT EXISTS idx_memories_layer_created ON memories(layer, created_at);
CREATE INDEX IF NOT EXISTS idx_memories_embedding_model ON memories(embedding_model);

CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    content,
    content='memories',
    content_rowid='rowid'
);

CREATE TABLE IF NOT EXISTS consolidation_log (
    id              TEXT PRIMARY KEY,
    from_layer      INTEGER NOT NULL,
    to_layer        INTEGER NOT NULL,
    memories_processed INTEGER NOT NULL,
    memories_created   INTEGER NOT NULL,
    compression_ratio  REAL,
    algorithm       TEXT NOT NULL,
    executed_at     TEXT NOT NULL,
    duration_ms     INTEGER
);

CREATE TABLE IF NOT EXISTS embedding_models (
    model_id        TEXT PRIMARY KEY,
    model_name      TEXT NOT NULL,
    dimension       INTEGER NOT NULL,
    provider        TEXT,
    registered_at   TEXT NOT NULL,
    retired_at      TEXT,
    mapping_to      TEXT
);

CREATE TABLE IF NOT EXISTS schema_versions (
    version         INTEGER PRIMARY KEY,
    applied_at      TEXT NOT NULL,
    description     TEXT,
    migration_sql   TEXT
);

CREATE TABLE IF NOT EXISTS encryption_keys (
    key_id      TEXT PRIMARY KEY,
    algorithm   TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    retired_at  TEXT,
    status      TEXT NOT NULL,
    key_blob    BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS integrity_proofs (
    proof_id    TEXT PRIMARY KEY,
    proof_type  TEXT NOT NULL,
    root_hash   TEXT NOT NULL,
    block_count INTEGER NOT NULL,
    created_at  TEXT NOT NULL
);
"#;

// ═══════════════════════════════════════════════════════════════════
// ERROR TYPE
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub enum LongevityError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
    Schema(String),
    Integrity(String),
    NotFound(String),
}

impl std::fmt::Display for LongevityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "SQLite error: {}", e),
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Json(e) => write!(f, "JSON error: {}", e),
            Self::Schema(msg) => write!(f, "Schema error: {}", msg),
            Self::Integrity(msg) => write!(f, "Integrity error: {}", msg),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}

impl std::error::Error for LongevityError {}

impl From<rusqlite::Error> for LongevityError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<std::io::Error> for LongevityError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for LongevityError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

// ═══════════════════════════════════════════════════════════════════
// SUPPORTING TYPES
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsolidationLogEntry {
    pub id: String,
    pub from_layer: u8,
    pub to_layer: u8,
    pub memories_processed: u32,
    pub memories_created: u32,
    pub compression_ratio: f64,
    pub algorithm: String,
    pub executed_at: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingModelEntry {
    pub model_id: String,
    pub model_name: String,
    pub dimension: u32,
    pub provider: Option<String>,
    pub registered_at: String,
    pub retired_at: Option<String>,
    pub mapping_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EncryptionKeyEntry {
    pub key_id: String,
    pub algorithm: String,
    pub created_at: String,
    pub retired_at: Option<String>,
    pub status: String,
    pub key_blob: Vec<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaVersionEntry {
    pub version: u32,
    pub applied_at: String,
    pub description: Option<String>,
    pub migration_sql: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntegrityProofEntry {
    pub proof_id: String,
    pub proof_type: String,
    pub root_hash: String,
    pub block_count: u64,
    pub created_at: String,
}
