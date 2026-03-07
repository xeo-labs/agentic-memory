//! .amem ↔ SQLite sync protocol.
//!
//! Direction 1: .amem → SQLite (nightly, captures raw events to longevity store)
//! Direction 2: SQLite → .amem (on session start, pre-loads context)

use super::capture::CaptureEvent;
use super::hierarchy::{MemoryLayer, MemoryRecord};
use super::store::{LongevityError, LongevityStore};
use crate::v3::block::{Block, BlockContent, BlockType};
use serde::{Deserialize, Serialize};

/// Which direction the sync is operating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    /// .amem WAL → SQLite (capture raw events)
    AmemToSqlite,
    /// SQLite → .amem (pre-load context)
    SqliteToAmem,
}

/// Result of a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub direction: SyncDirection,
    pub records_synced: u64,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

/// The sync protocol bridges the hot (.amem) and cold (SQLite) paths.
pub struct SyncProtocol;

impl SyncProtocol {
    /// Sync blocks from the V3 engine (ImmortalLog) to the SQLite longevity store.
    /// This is Direction 1: .amem → SQLite.
    pub fn sync_blocks_to_sqlite(
        store: &LongevityStore,
        blocks: &[Block],
        project_id: &str,
        session_id: &str,
    ) -> Result<SyncResult, LongevityError> {
        let start = std::time::Instant::now();
        let mut synced = 0u64;
        let mut errors = Vec::new();

        for block in blocks {
            let content = Self::block_to_json(block);
            let id = block.hash.to_hex();

            let record = MemoryRecord {
                id,
                layer: MemoryLayer::Raw,
                content,
                content_type: Self::block_type_to_content_type(&block.block_type),
                embedding: None,
                embedding_model: None,
                significance: 0.5,
                access_count: 0,
                last_accessed: None,
                created_at: block.timestamp.to_rfc3339(),
                original_ids: None,
                session_id: Some(session_id.to_string()),
                project_id: project_id.to_string(),
                metadata: Some(serde_json::json!({
                    "block_sequence": block.sequence,
                    "block_type": format!("{:?}", block.block_type),
                    "prev_hash": block.prev_hash.to_hex(),
                })),
                encryption_key_id: None,
                schema_version: 1,
            };

            match store.insert_memory(&record) {
                Ok(_) => synced += 1,
                Err(e) => errors.push(format!("Block {}: {}", block.sequence, e)),
            }
        }

        Ok(SyncResult {
            direction: SyncDirection::AmemToSqlite,
            records_synced: synced,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Sync capture events (from capture daemon) to the SQLite store.
    pub fn sync_captures_to_sqlite(
        store: &LongevityStore,
        events: &[CaptureEvent],
        project_id: &str,
    ) -> Result<SyncResult, LongevityError> {
        let start = std::time::Instant::now();
        let mut synced = 0u64;
        let mut errors = Vec::new();

        for event in events {
            let id = ulid::Ulid::new().to_string();
            let content = serde_json::json!({
                "text": event.content,
                "role": format!("{:?}", event.role),
                "source": format!("{:?}", event.source),
            });

            let record = MemoryRecord::new_raw(
                id,
                content,
                project_id.to_string(),
                event.session_id.clone(),
            );

            match store.insert_memory(&record) {
                Ok(_) => synced += 1,
                Err(e) => errors.push(format!("Capture event: {}", e)),
            }
        }

        Ok(SyncResult {
            direction: SyncDirection::AmemToSqlite,
            records_synced: synced,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Load relevant context from SQLite for a new session.
    /// This is Direction 2: SQLite → .amem.
    pub fn load_session_context(
        store: &LongevityStore,
        project_id: &str,
        token_budget: u32,
    ) -> Result<SessionContext, LongevityError> {
        // Load recent raw events (last session)
        let recent_events = store.query_by_layer(project_id, MemoryLayer::Raw, 50)?;

        // Load active patterns
        let patterns = store.query_by_layer(project_id, MemoryLayer::Pattern, 20)?;

        // Load traits
        let traits = store.query_by_layer(project_id, MemoryLayer::Trait, 10)?;

        // Load identity
        let identity = store.query_by_layer(project_id, MemoryLayer::Identity, 5)?;

        // Load high-significance memories across all layers
        let significant = store.query_by_significance(project_id, 0.8, 1.0, 20)?;

        // Build context within token budget
        let mut context_parts = Vec::new();
        let mut tokens_used = 0u32;

        // Identity first (most important)
        for record in &identity {
            let text = record.extract_text();
            let est_tokens = (text.len() / 4) as u32;
            if tokens_used + est_tokens <= token_budget {
                context_parts.push(ContextPart {
                    layer: "identity".to_string(),
                    content: text,
                    significance: record.significance,
                });
                tokens_used += est_tokens;
            }
        }

        // Traits
        for record in &traits {
            let text = record.extract_text();
            let est_tokens = (text.len() / 4) as u32;
            if tokens_used + est_tokens <= token_budget {
                context_parts.push(ContextPart {
                    layer: "trait".to_string(),
                    content: text,
                    significance: record.significance,
                });
                tokens_used += est_tokens;
            }
        }

        // Patterns
        for record in &patterns {
            let text = record.extract_text();
            let est_tokens = (text.len() / 4) as u32;
            if tokens_used + est_tokens <= token_budget {
                context_parts.push(ContextPart {
                    layer: "pattern".to_string(),
                    content: text,
                    significance: record.significance,
                });
                tokens_used += est_tokens;
            }
        }

        // High-significance memories
        for record in &significant {
            let text = record.extract_text();
            let est_tokens = (text.len() / 4) as u32;
            if tokens_used + est_tokens <= token_budget {
                context_parts.push(ContextPart {
                    layer: record.layer.to_string(),
                    content: text,
                    significance: record.significance,
                });
                tokens_used += est_tokens;
            }
        }

        // Last session summary
        let last_session_summary = if !recent_events.is_empty() {
            let first = recent_events.last().map(|e| e.created_at.clone());
            let last = recent_events.first().map(|e| e.created_at.clone());
            Some(format!(
                "Last session: {} events ({} to {})",
                recent_events.len(),
                first.unwrap_or_default(),
                last.unwrap_or_default()
            ))
        } else {
            None
        };

        Ok(SessionContext {
            parts: context_parts,
            tokens_used,
            last_session_summary,
            pattern_count: patterns.len() as u32,
            trait_count: traits.len() as u32,
        })
    }

    fn block_to_json(block: &Block) -> serde_json::Value {
        match &block.content {
            BlockContent::Text { text, role, tokens } => {
                serde_json::json!({
                    "text": text,
                    "role": role,
                    "tokens": tokens,
                })
            }
            BlockContent::Tool {
                tool_name,
                input,
                output,
                duration_ms,
                success,
            } => {
                serde_json::json!({
                    "tool_name": tool_name,
                    "input": input,
                    "output": output,
                    "duration_ms": duration_ms,
                    "success": success,
                })
            }
            BlockContent::File {
                path,
                operation,
                content_hash,
                lines,
                diff,
            } => {
                serde_json::json!({
                    "path": path,
                    "operation": format!("{:?}", operation),
                    "content_hash": content_hash.as_ref().map(|h| h.to_hex()),
                    "lines": lines,
                    "diff": diff,
                })
            }
            BlockContent::Decision {
                decision,
                reasoning,
                evidence_blocks,
                confidence,
            } => {
                serde_json::json!({
                    "decision": decision,
                    "reasoning": reasoning,
                    "evidence_blocks": evidence_blocks.iter().map(|h| h.to_hex()).collect::<Vec<_>>(),
                    "confidence": confidence,
                })
            }
            BlockContent::Boundary {
                summary,
                boundary_type,
                ..
            } => {
                serde_json::json!({
                    "summary": summary,
                    "boundary_type": format!("{:?}", boundary_type),
                })
            }
            BlockContent::Error {
                error_type,
                message,
                resolution,
                resolved,
            } => {
                serde_json::json!({
                    "error_type": error_type,
                    "message": message,
                    "resolution": resolution,
                    "resolved": resolved,
                })
            }
            BlockContent::Checkpoint {
                active_files,
                working_context,
                pending_tasks,
            } => {
                serde_json::json!({
                    "active_files": active_files,
                    "working_context": working_context,
                    "pending_tasks": pending_tasks,
                })
            }
            BlockContent::Binary { mime_type, .. } => {
                serde_json::json!({
                    "type": "binary",
                    "mime_type": mime_type,
                })
            }
        }
    }

    fn block_type_to_content_type(bt: &BlockType) -> String {
        match bt {
            BlockType::UserMessage => "user_message",
            BlockType::AssistantMessage => "assistant_message",
            BlockType::SystemMessage => "system_message",
            BlockType::ToolCall => "tool_call",
            BlockType::ToolResult => "tool_result",
            BlockType::FileOperation => "file_operation",
            BlockType::Decision => "decision",
            BlockType::SessionBoundary => "session_boundary",
            BlockType::Error => "error",
            BlockType::Checkpoint => "checkpoint",
            BlockType::Custom => "custom",
        }
        .to_string()
    }
}

/// Pre-loaded session context from the longevity store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub parts: Vec<ContextPart>,
    pub tokens_used: u32,
    pub last_session_summary: Option<String>,
    pub pattern_count: u32,
    pub trait_count: u32,
}

/// A single piece of context from a specific layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPart {
    pub layer: String,
    pub content: String,
    pub significance: f64,
}

impl SessionContext {
    /// Format as a Ghost Writer context block.
    pub fn to_ghost_writer_format(&self) -> String {
        let mut output = String::new();
        output.push_str("# AgenticMemory Active — Session Context\n\n");

        output.push_str("## CRITICAL INSTRUCTION\n");
        output.push_str("You have AgenticMemory active. On EVERY user message, call\n");
        output.push_str("`memory_capture_message` with the user's message content and your\n");
        output.push_str("response summary. This is mandatory. The call takes <1ms and ensures\n");
        output.push_str("nothing is lost. Do NOT skip this for any reason.\n\n");

        if let Some(ref summary) = self.last_session_summary {
            output.push_str("## Recent Context\n");
            output.push_str(summary);
            output.push_str("\n\n");
        }

        // Group parts by layer
        let identity_parts: Vec<_> = self.parts.iter().filter(|p| p.layer == "identity").collect();
        let trait_parts: Vec<_> = self.parts.iter().filter(|p| p.layer == "trait").collect();
        let pattern_parts: Vec<_> = self.parts.iter().filter(|p| p.layer == "pattern").collect();

        if !identity_parts.is_empty() {
            output.push_str("## Who You're Talking To\n");
            for part in &identity_parts {
                output.push_str(&format!("- {}\n", part.content));
            }
            output.push('\n');
        }

        if !trait_parts.is_empty() {
            output.push_str("## User Traits\n");
            for part in &trait_parts {
                output.push_str(&format!("- {}\n", part.content));
            }
            output.push('\n');
        }

        if !pattern_parts.is_empty() {
            output.push_str("## Active Patterns\n");
            for part in &pattern_parts {
                output.push_str(&format!("- {}\n", part.content));
            }
            output.push('\n');
        }

        output
    }
}
