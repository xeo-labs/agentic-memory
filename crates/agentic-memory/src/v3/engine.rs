//! The V3 Memory Engine: Immortal Architecture.
//! Integrates log + storage + indexes + retrieval into a single API.

use super::block::*;
use super::edge_cases::{self, NormalizedContent, RecoveryMarker};
use super::immortal_log::*;
use super::indexes::*;
use super::recovery::RecoveryManager;
use super::retrieval::*;
use super::tiered::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// The V3 Memory Engine
pub struct MemoryEngineV3 {
    /// The append-only immortal log (source of truth)
    log: Arc<RwLock<ImmortalLog>>,
    /// Tiered storage for fast access
    storage: Arc<RwLock<TieredStorage>>,
    /// The five indexes
    temporal_index: Arc<RwLock<temporal::TemporalIndex>>,
    semantic_index: Arc<RwLock<semantic::SemanticIndex>>,
    causal_index: Arc<RwLock<causal::CausalIndex>>,
    entity_index: Arc<RwLock<entity::EntityIndex>>,
    procedural_index: Arc<RwLock<procedural::ProceduralIndex>>,
    /// Smart retrieval engine
    retrieval: Arc<SmartRetrievalEngine>,
    /// Current session ID
    session_id: String,
    /// Configuration
    #[allow(dead_code)]
    config: EngineConfig,
}

/// Engine configuration
#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// Data directory
    pub data_dir: PathBuf,
    /// Embedding dimension (for semantic index)
    pub embedding_dim: usize,
    /// Tier configuration
    pub tier_config: TierConfig,
    /// Auto-checkpoint interval (blocks)
    pub checkpoint_interval: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from(".agentic/memory"),
            embedding_dim: 384,
            tier_config: TierConfig::default(),
            checkpoint_interval: 100,
        }
    }
}

impl MemoryEngineV3 {
    /// Create or open memory engine
    pub fn open(config: EngineConfig) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(&config.data_dir)?;

        let log_path = config.data_dir.join("immortal.log");
        let log = ImmortalLog::open(log_path)?;

        // Build indexes from log
        let mut temporal = temporal::TemporalIndex::new();
        let mut semantic = semantic::SemanticIndex::new(config.embedding_dim);
        let mut causal = causal::CausalIndex::new();
        let mut entity = entity::EntityIndex::new();
        let mut procedural = procedural::ProceduralIndex::new();
        let mut storage = TieredStorage::new(config.tier_config.clone());

        for block in log.iter() {
            temporal.index(&block);
            semantic.index(&block);
            causal.index(&block);
            entity.index(&block);
            procedural.index(&block);
            storage.store(block);
        }

        Ok(Self {
            log: Arc::new(RwLock::new(log)),
            storage: Arc::new(RwLock::new(storage)),
            temporal_index: Arc::new(RwLock::new(temporal)),
            semantic_index: Arc::new(RwLock::new(semantic)),
            causal_index: Arc::new(RwLock::new(causal)),
            entity_index: Arc::new(RwLock::new(entity)),
            procedural_index: Arc::new(RwLock::new(procedural)),
            retrieval: Arc::new(SmartRetrievalEngine::new()),
            session_id: uuid::Uuid::new_v4().to_string(),
            config,
        })
    }

    /// Open with crash recovery: check WAL, verify integrity, rebuild indexes if needed
    pub fn open_with_recovery(config: EngineConfig) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(&config.data_dir)?;

        // Check recovery markers
        let marker = RecoveryMarker::new(&config.data_dir);
        if marker.needs_recovery() && !marker.recovery_completed() {
            log::warn!("Previous recovery was interrupted — restarting recovery");
        }

        // Try WAL recovery first
        if let Ok(recovery) = RecoveryManager::new(&config.data_dir) {
            match recovery.recover() {
                Ok(blocks) if !blocks.is_empty() => {
                    marker.mark_in_progress();
                    log::info!("Recovering {} blocks from WAL", blocks.len());

                    let log_path = config.data_dir.join("immortal.log");
                    let mut log = ImmortalLog::open(log_path)?;

                    for block in &blocks {
                        // Check if block already in log (idempotent)
                        if log.get_by_hash(&block.hash).is_none() {
                            // Re-append the block content
                            let _ = log.append(block.block_type, block.content.clone());
                        }
                    }

                    marker.mark_complete();
                }
                _ => {}
            }
        }

        // Open normally and verify integrity
        let engine = Self::open(config)?;
        let report = engine.verify_integrity();

        if !report.verified {
            log::warn!(
                "Integrity issues detected: {} corrupted, {} missing blocks",
                report.corrupted_blocks.len(),
                report.missing_blocks.len()
            );
            // Indexes were already rebuilt from log during open()
            // The log is the source of truth
        }

        Ok(engine)
    }

    /// Rebuild all indexes from the immortal log (source of truth)
    pub fn rebuild_all_indexes(&self) {
        let log = self.log.read().unwrap();
        let blocks: Vec<Block> = log.iter().collect();

        // Rebuild each index
        self.temporal_index
            .write()
            .unwrap()
            .rebuild(blocks.iter().cloned());
        self.semantic_index
            .write()
            .unwrap()
            .rebuild(blocks.iter().cloned());
        self.causal_index
            .write()
            .unwrap()
            .rebuild(blocks.iter().cloned());
        self.entity_index
            .write()
            .unwrap()
            .rebuild(blocks.iter().cloned());
        self.procedural_index
            .write()
            .unwrap()
            .rebuild(blocks.iter().cloned());

        // Rebuild tiered storage
        let mut storage = self.storage.write().unwrap();
        *storage = TieredStorage::new(self.config.tier_config.clone());
        for block in blocks {
            storage.store(block);
        }

        log::info!("All indexes rebuilt from log");
    }

    /// Verify index consistency against the log
    pub fn verify_index_consistency(&self) -> edge_cases::IndexConsistencyReport {
        let log = self.log.read().unwrap();
        let temporal = self.temporal_index.read().unwrap();
        let semantic = self.semantic_index.read().unwrap();

        let mut report = edge_cases::IndexConsistencyReport::default();
        report.total_blocks = log.len();

        for seq in 0..log.len() {
            if let Some(block) = log.get(seq) {
                // Check temporal index
                let in_temporal = temporal
                    .query_range(
                        block.timestamp - chrono::Duration::seconds(1),
                        block.timestamp + chrono::Duration::seconds(1),
                    )
                    .iter()
                    .any(|r| r.block_sequence == seq);

                if !in_temporal {
                    report.missing_in_temporal.push(seq);
                }

                // Check semantic index (only for text content)
                if block.extract_text().is_some() && semantic.len() < seq as usize + 1 {
                    report.missing_in_semantic.push(seq);
                }
            }
        }

        report.consistent = report.missing_in_temporal.is_empty()
            && report.missing_in_semantic.is_empty()
            && report.missing_in_entity.is_empty();

        report
    }

    /// Rebuild indexes if inconsistencies are detected
    pub fn rebuild_indexes_if_needed(&self) -> bool {
        let report = self.verify_index_consistency();
        if !report.consistent {
            log::warn!("Index inconsistency detected, rebuilding...");
            self.rebuild_all_indexes();
            true
        } else {
            false
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // CAPTURE API
    // ═══════════════════════════════════════════════════════════════════

    /// Capture a user message (with content validation)
    pub fn capture_user_message(
        &self,
        text: &str,
        tokens: Option<u32>,
    ) -> Result<BlockHash, std::io::Error> {
        let validated = match edge_cases::normalize_content(text) {
            NormalizedContent::Empty => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Cannot capture empty message",
                ));
            }
            NormalizedContent::WhitespaceOnly => {
                log::warn!("Captured whitespace-only user message");
                text.to_string()
            }
            NormalizedContent::Valid(v) => v,
        };

        self.append_block(
            BlockType::UserMessage,
            BlockContent::Text {
                text: validated,
                role: Some("user".to_string()),
                tokens,
            },
        )
    }

    /// Capture an assistant message (with content validation)
    pub fn capture_assistant_message(
        &self,
        text: &str,
        tokens: Option<u32>,
    ) -> Result<BlockHash, std::io::Error> {
        let validated = match edge_cases::normalize_content(text) {
            NormalizedContent::Empty => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Cannot capture empty message",
                ));
            }
            NormalizedContent::WhitespaceOnly => {
                log::warn!("Captured whitespace-only assistant message");
                text.to_string()
            }
            NormalizedContent::Valid(v) => v,
        };

        self.append_block(
            BlockType::AssistantMessage,
            BlockContent::Text {
                text: validated,
                role: Some("assistant".to_string()),
                tokens,
            },
        )
    }

    /// Capture a tool call
    pub fn capture_tool_call(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        output: Option<serde_json::Value>,
        duration_ms: Option<u64>,
        success: bool,
    ) -> Result<BlockHash, std::io::Error> {
        self.append_block(
            BlockType::ToolCall,
            BlockContent::Tool {
                tool_name: tool_name.to_string(),
                input,
                output,
                duration_ms,
                success,
            },
        )
    }

    /// Capture a file operation
    pub fn capture_file_operation(
        &self,
        path: &str,
        operation: FileOperation,
        content_hash: Option<BlockHash>,
        lines: Option<u32>,
        diff: Option<String>,
    ) -> Result<BlockHash, std::io::Error> {
        self.append_block(
            BlockType::FileOperation,
            BlockContent::File {
                path: path.to_string(),
                operation,
                content_hash,
                lines,
                diff,
            },
        )
    }

    /// Capture a decision
    pub fn capture_decision(
        &self,
        decision: &str,
        reasoning: Option<&str>,
        evidence_blocks: Vec<BlockHash>,
        confidence: Option<f32>,
    ) -> Result<BlockHash, std::io::Error> {
        self.append_block(
            BlockType::Decision,
            BlockContent::Decision {
                decision: decision.to_string(),
                reasoning: reasoning.map(String::from),
                evidence_blocks,
                confidence,
            },
        )
    }

    /// Capture an error
    pub fn capture_error(
        &self,
        error_type: &str,
        message: &str,
        resolution: Option<&str>,
        resolved: bool,
    ) -> Result<BlockHash, std::io::Error> {
        self.append_block(
            BlockType::Error,
            BlockContent::Error {
                error_type: error_type.to_string(),
                message: message.to_string(),
                resolution: resolution.map(String::from),
                resolved,
            },
        )
    }

    /// Capture a session boundary
    pub fn capture_boundary(
        &self,
        boundary_type: BoundaryType,
        context_tokens_before: u32,
        context_tokens_after: u32,
        summary: &str,
        continuation_hint: Option<&str>,
    ) -> Result<BlockHash, std::io::Error> {
        self.append_block(
            BlockType::SessionBoundary,
            BlockContent::Boundary {
                boundary_type,
                context_tokens_before,
                context_tokens_after,
                summary: summary.to_string(),
                continuation_hint: continuation_hint.map(String::from),
            },
        )
    }

    /// Capture a checkpoint
    pub fn capture_checkpoint(
        &self,
        active_files: Vec<String>,
        working_context: &str,
        pending_tasks: Vec<String>,
    ) -> Result<BlockHash, std::io::Error> {
        self.append_block(
            BlockType::Checkpoint,
            BlockContent::Checkpoint {
                active_files,
                working_context: working_context.to_string(),
                pending_tasks,
            },
        )
    }

    // ═══════════════════════════════════════════════════════════════════
    // INTERNAL
    // ═══════════════════════════════════════════════════════════════════

    fn append_block(
        &self,
        block_type: BlockType,
        content: BlockContent,
    ) -> Result<BlockHash, std::io::Error> {
        // Acquire write lock with poisoning recovery
        let mut log = self.log.write().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Log lock poisoned: {}", e),
            )
        })?;

        let block = log.append(block_type, content)?;
        let hash = block.hash;

        // Update indexes (continue even if individual index fails)
        if let Ok(mut idx) = self.temporal_index.write() {
            idx.index(&block);
        }
        if let Ok(mut idx) = self.semantic_index.write() {
            idx.index(&block);
        }
        if let Ok(mut idx) = self.causal_index.write() {
            idx.index(&block);
        }
        if let Ok(mut idx) = self.entity_index.write() {
            idx.index(&block);
        }
        if let Ok(mut idx) = self.procedural_index.write() {
            idx.index(&block);
        }

        // Store in tiered storage
        if let Ok(mut s) = self.storage.write() {
            s.store(block);
        }

        Ok(hash)
    }

    // ═══════════════════════════════════════════════════════════════════
    // RETRIEVAL API
    // ═══════════════════════════════════════════════════════════════════

    /// Smart retrieval: assemble perfect context for a query
    pub fn retrieve(&self, request: RetrievalRequest) -> RetrievalResult {
        self.retrieval.retrieve(
            request,
            &self.log.read().unwrap(),
            &self.storage.read().unwrap(),
            &self.temporal_index.read().unwrap(),
            &self.semantic_index.read().unwrap(),
            &self.causal_index.read().unwrap(),
            &self.entity_index.read().unwrap(),
            &self.procedural_index.read().unwrap(),
        )
    }

    /// Resurrect: fully restore state at any timestamp
    pub fn resurrect(&self, timestamp: DateTime<Utc>) -> ResurrectionResult {
        let log = self.log.read().unwrap();
        let storage = self.storage.read().unwrap();

        let mut blocks = Vec::new();
        for seq in 0..log.len() {
            if let Some(block) = storage.get(seq) {
                if block.timestamp <= timestamp {
                    blocks.push(block);
                }
            }
        }

        let mut messages = Vec::new();
        let mut files_state = std::collections::HashMap::new();
        let mut decisions = Vec::new();
        let mut last_checkpoint = None;

        for block in &blocks {
            match &block.content {
                BlockContent::Text { text, role, .. } => {
                    messages.push((role.clone().unwrap_or_default(), text.clone()));
                }
                BlockContent::File {
                    path, operation, ..
                } => match operation {
                    FileOperation::Create | FileOperation::Update => {
                        files_state.insert(path.clone(), true);
                    }
                    FileOperation::Delete => {
                        files_state.insert(path.clone(), false);
                    }
                    _ => {}
                },
                BlockContent::Decision { decision, .. } => {
                    decisions.push(decision.clone());
                }
                BlockContent::Checkpoint { .. } => {
                    last_checkpoint = Some(block.clone());
                }
                _ => {}
            }
        }

        ResurrectionResult {
            timestamp,
            block_count: blocks.len(),
            messages,
            files_state,
            decisions,
            last_checkpoint,
        }
    }

    /// Search by time range
    pub fn search_temporal(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<Block> {
        let temporal = self.temporal_index.read().unwrap();
        let storage = self.storage.read().unwrap();

        temporal
            .query_range(start, end)
            .into_iter()
            .filter_map(|r| storage.get(r.block_sequence))
            .collect()
    }

    /// Search by text/meaning
    pub fn search_semantic(&self, query: &str, limit: usize) -> Vec<Block> {
        let semantic = self.semantic_index.read().unwrap();
        let storage = self.storage.read().unwrap();

        semantic
            .search_by_text(query, limit)
            .into_iter()
            .filter_map(|r| storage.get(r.block_sequence))
            .collect()
    }

    /// Search by entity (file, person, etc.)
    pub fn search_entity(&self, entity: &str) -> Vec<Block> {
        let entity_idx = self.entity_index.read().unwrap();
        let storage = self.storage.read().unwrap();

        entity_idx
            .query_entity(entity)
            .into_iter()
            .filter_map(|r| storage.get(r.block_sequence))
            .collect()
    }

    /// Get decision chain
    pub fn get_decision_chain(&self, block_sequence: u64) -> Vec<Block> {
        let causal = self.causal_index.read().unwrap();
        let storage = self.storage.read().unwrap();

        causal
            .get_decision_chain(block_sequence)
            .into_iter()
            .filter_map(|r| storage.get(r.block_sequence))
            .collect()
    }

    /// Get current session blocks
    pub fn get_current_session(&self) -> Vec<Block> {
        let procedural = self.procedural_index.read().unwrap();
        let storage = self.storage.read().unwrap();

        procedural
            .get_current_session()
            .into_iter()
            .filter_map(|r| storage.get(r.block_sequence))
            .collect()
    }

    /// Verify integrity
    pub fn verify_integrity(&self) -> IntegrityReport {
        self.log.read().unwrap().verify_integrity()
    }

    /// Get statistics
    pub fn stats(&self) -> EngineStats {
        let log = self.log.read().unwrap();
        let tier_stats = self.storage.read().unwrap().stats();

        EngineStats {
            total_blocks: log.len(),
            tier_stats,
            session_id: self.session_id.clone(),
        }
    }

    /// Session resume: get everything needed to continue
    pub fn session_resume(&self) -> SessionResumeResult {
        let procedural = self.procedural_index.read().unwrap();
        let storage = self.storage.read().unwrap();
        let entity_idx = self.entity_index.read().unwrap();

        let recent = procedural.get_recent_steps(50);
        let recent_blocks: Vec<Block> = recent
            .into_iter()
            .filter_map(|r| storage.get(r.block_sequence))
            .collect();

        let mut messages = Vec::new();
        let mut files_touched = Vec::new();
        let mut decisions = Vec::new();
        let mut errors_resolved = Vec::new();

        for block in &recent_blocks {
            match &block.content {
                BlockContent::Text { text, role, .. } => {
                    let preview = if text.len() > 200 {
                        format!("{}...", &text[..200])
                    } else {
                        text.clone()
                    };
                    messages.push((role.clone().unwrap_or_default(), preview));
                }
                BlockContent::File {
                    path, operation, ..
                } => {
                    files_touched.push((path.clone(), format!("{:?}", operation)));
                }
                BlockContent::Decision { decision, .. } => {
                    decisions.push(decision.clone());
                }
                BlockContent::Error {
                    error_type,
                    message,
                    resolution,
                    resolved,
                } => {
                    if *resolved {
                        errors_resolved.push((
                            format!("{}: {}", error_type, message),
                            resolution.clone().unwrap_or_default(),
                        ));
                    }
                }
                _ => {}
            }
        }

        let all_files = entity_idx.get_all_files();

        SessionResumeResult {
            session_id: self.session_id.clone(),
            block_count: recent_blocks.len(),
            recent_messages: messages,
            files_touched,
            decisions,
            errors_resolved,
            all_known_files: all_files,
        }
    }
}

/// Result of resurrect operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResurrectionResult {
    pub timestamp: DateTime<Utc>,
    pub block_count: usize,
    pub messages: Vec<(String, String)>,
    pub files_state: std::collections::HashMap<String, bool>,
    pub decisions: Vec<String>,
    pub last_checkpoint: Option<Block>,
}

/// Result of session resume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResumeResult {
    pub session_id: String,
    pub block_count: usize,
    pub recent_messages: Vec<(String, String)>,
    pub files_touched: Vec<(String, String)>,
    pub decisions: Vec<String>,
    pub errors_resolved: Vec<(String, String)>,
    pub all_known_files: Vec<String>,
}

/// Engine statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStats {
    pub total_blocks: u64,
    pub tier_stats: TierStats,
    pub session_id: String,
}
