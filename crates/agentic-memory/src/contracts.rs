//! Contracts bridge — implements agentic-sdk v0.2.0 traits for Memory.
//!
//! This module provides `MemorySister`, a contracts-compliant wrapper
//! around the core `MemoryGraph` + engines. It implements:
//!
//! - `Sister` — lifecycle management
//! - `SessionManagement` — append-only sequential sessions
//! - `Grounding` — BM25-based claim verification
//! - `Queryable` — unified query interface
//! - `FileFormatReader/FileFormatWriter` — .amem file I/O
//!
//! The MCP server can use `MemorySister` instead of raw graph + engines
//! to get compile-time contracts compliance.

use agentic_sdk::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::engine::text_search::TextSearchParams;
use crate::engine::{QueryEngine, WriteEngine};
use crate::graph::MemoryGraph;
use crate::types::{AmemError, CognitiveEvent, DEFAULT_DIMENSION};

// ═══════════════════════════════════════════════════════════════════
// ERROR BRIDGE: AmemError → SisterError
// ═══════════════════════════════════════════════════════════════════

impl From<AmemError> for SisterError {
    fn from(e: AmemError) -> Self {
        match &e {
            AmemError::NodeNotFound(id) => SisterError::not_found(format!("node {}", id)),
            AmemError::InvalidMagic => {
                SisterError::new(ErrorCode::VersionMismatch, "Invalid .amem magic bytes")
            }
            AmemError::UnsupportedVersion(v) => SisterError::new(
                ErrorCode::VersionMismatch,
                format!("Unsupported .amem version: {}", v),
            ),
            AmemError::ContentTooLarge { size, max } => SisterError::new(
                ErrorCode::InvalidInput,
                format!("Content too large: {} > {} bytes", size, max),
            ),
            AmemError::DimensionMismatch { expected, got } => SisterError::new(
                ErrorCode::InvalidInput,
                format!("Dimension mismatch: expected {}, got {}", expected, got),
            ),
            AmemError::InvalidConfidence(v) => SisterError::new(
                ErrorCode::InvalidInput,
                format!("Confidence must be [0.0, 1.0], got {}", v),
            ),
            AmemError::Io(io_err) => {
                SisterError::new(ErrorCode::StorageError, format!("I/O error: {}", io_err))
            }
            AmemError::Truncated => {
                SisterError::new(ErrorCode::StorageError, "File is empty or truncated")
            }
            AmemError::Corrupt(offset) => SisterError::new(
                ErrorCode::ChecksumMismatch,
                format!("Corrupt data at offset {}", offset),
            ),
            _ => SisterError::new(ErrorCode::MemoryError, e.to_string()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// SESSION STATE
// ═══════════════════════════════════════════════════════════════════

/// Session record for tracking sessions in MemorySister.
#[derive(Debug, Clone)]
struct SessionRecord {
    id: ContextId,
    session_id: u32,
    name: String,
    created_at: chrono::DateTime<chrono::Utc>,
    node_count_at_start: usize,
}

// ═══════════════════════════════════════════════════════════════════
// MEMORY SISTER — The contracts-compliant facade
// ═══════════════════════════════════════════════════════════════════

/// Contracts-compliant Memory sister.
///
/// Wraps `MemoryGraph` + engines and implements all v0.2.0 traits.
/// This is the canonical "Memory as a sister" interface.
pub struct MemorySister {
    graph: MemoryGraph,
    query_engine: QueryEngine,
    write_engine: WriteEngine,
    file_path: Option<PathBuf>,
    start_time: Instant,

    // Session state
    current_session: Option<SessionRecord>,
    sessions: Vec<SessionRecord>,
    next_session_id: u32,
}

impl MemorySister {
    /// Create from an existing graph (for migration from SessionManager).
    pub fn from_graph(graph: MemoryGraph, file_path: Option<PathBuf>) -> Self {
        let dimension = graph.dimension();
        Self {
            graph,
            query_engine: QueryEngine::new(),
            write_engine: WriteEngine::new(dimension),
            file_path,
            start_time: Instant::now(),
            current_session: None,
            sessions: vec![],
            next_session_id: 1,
        }
    }

    /// Get a reference to the underlying graph.
    pub fn graph(&self) -> &MemoryGraph {
        &self.graph
    }

    /// Get a mutable reference to the underlying graph.
    pub fn graph_mut(&mut self) -> &mut MemoryGraph {
        &mut self.graph
    }

    /// Get the query engine.
    pub fn query_engine(&self) -> &QueryEngine {
        &self.query_engine
    }

    /// Get the write engine.
    pub fn write_engine(&self) -> &WriteEngine {
        &self.write_engine
    }

    /// Get the current u32 session ID (for interop with existing code).
    pub fn current_session_id(&self) -> Option<u32> {
        self.current_session.as_ref().map(|s| s.session_id)
    }
}

// ═══════════════════════════════════════════════════════════════════
// SISTER TRAIT
// ═══════════════════════════════════════════════════════════════════

impl Sister for MemorySister {
    const SISTER_TYPE: SisterType = SisterType::Memory;
    const FILE_EXTENSION: &'static str = "amem";

    fn init(config: SisterConfig) -> SisterResult<Self>
    where
        Self: Sized,
    {
        let dimension = config
            .get_option::<usize>("dimension")
            .unwrap_or(DEFAULT_DIMENSION);

        let file_path = config.data_path.clone();

        let graph = if let Some(ref path) = file_path {
            if path.exists() {
                #[cfg(feature = "format")]
                {
                    crate::format::AmemReader::read_from_file(path).map_err(SisterError::from)?
                }
                #[cfg(not(feature = "format"))]
                {
                    MemoryGraph::new(dimension)
                }
            } else if config.create_if_missing {
                MemoryGraph::new(dimension)
            } else {
                return Err(SisterError::new(
                    ErrorCode::NotFound,
                    format!("Memory file not found: {}", path.display()),
                ));
            }
        } else {
            MemoryGraph::new(dimension)
        };

        Ok(Self::from_graph(graph, file_path))
    }

    fn health(&self) -> HealthStatus {
        HealthStatus {
            healthy: true,
            status: Status::Ready,
            uptime: self.start_time.elapsed(),
            resources: ResourceUsage {
                memory_bytes: self.graph.node_count() * 256, // rough estimate
                disk_bytes: 0,
                open_handles: if self.file_path.is_some() { 1 } else { 0 },
            },
            warnings: vec![],
            last_error: None,
        }
    }

    fn version(&self) -> Version {
        Version::new(0, 4, 1) // matches agentic-memory crate version
    }

    fn shutdown(&mut self) -> SisterResult<()> {
        // End current session if active
        if self.current_session.is_some() {
            let _ = SessionManagement::end_session(self);
        }

        // Save to file if path is set
        #[cfg(feature = "format")]
        if let Some(ref path) = self.file_path {
            let writer = crate::format::AmemWriter::new(self.graph.dimension());
            writer
                .write_to_file(&self.graph, path)
                .map_err(SisterError::from)?;
        }

        Ok(())
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::new("memory_add", "Add cognitive events to graph"),
            Capability::new("memory_query", "Query memory by filters"),
            Capability::new("memory_ground", "Verify claims against stored memories"),
            Capability::new("memory_evidence", "Get detailed evidence for a query"),
            Capability::new(
                "memory_suggest",
                "Find similar memories when exact match fails",
            ),
            Capability::new("memory_similar", "Find semantically similar memories"),
            Capability::new("memory_traverse", "Walk the graph following edges"),
            Capability::new("memory_temporal", "Compare knowledge across time periods"),
            Capability::new("memory_correct", "Record corrections to previous beliefs"),
            Capability::new("conversation_log", "Log conversation context"),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════════
// SESSION MANAGEMENT
// ═══════════════════════════════════════════════════════════════════

impl SessionManagement for MemorySister {
    fn start_session(&mut self, name: &str) -> SisterResult<ContextId> {
        // End current session if active
        if self.current_session.is_some() {
            self.end_session()?;
        }

        let session_id = self.next_session_id;
        self.next_session_id += 1;
        let context_id = ContextId::new();

        let record = SessionRecord {
            id: context_id,
            session_id,
            name: name.to_string(),
            created_at: chrono::Utc::now(),
            node_count_at_start: self.graph.node_count(),
        };

        self.current_session = Some(record.clone());
        self.sessions.push(record);

        Ok(context_id)
    }

    fn end_session(&mut self) -> SisterResult<()> {
        if self.current_session.is_none() {
            return Err(SisterError::new(
                ErrorCode::InvalidState,
                "No active session to end",
            ));
        }
        self.current_session = None;
        Ok(())
    }

    fn current_session(&self) -> Option<ContextId> {
        self.current_session.as_ref().map(|s| s.id)
    }

    fn current_session_info(&self) -> SisterResult<ContextInfo> {
        let session = self
            .current_session
            .as_ref()
            .ok_or_else(|| SisterError::new(ErrorCode::InvalidState, "No active session"))?;

        let nodes_in_session = self.graph.node_count() - session.node_count_at_start;

        Ok(ContextInfo {
            id: session.id,
            name: session.name.clone(),
            created_at: session.created_at,
            updated_at: chrono::Utc::now(),
            item_count: nodes_in_session,
            size_bytes: nodes_in_session * 256,
            metadata: Metadata::new(),
        })
    }

    fn list_sessions(&self) -> SisterResult<Vec<ContextSummary>> {
        Ok(self
            .sessions
            .iter()
            .rev() // most recent first
            .map(|s| ContextSummary {
                id: s.id,
                name: s.name.clone(),
                created_at: s.created_at,
                updated_at: s.created_at, // approximate
                item_count: 0,            // would need per-session tracking
                size_bytes: 0,
            })
            .collect())
    }

    fn export_session(&self, id: ContextId) -> SisterResult<ContextSnapshot> {
        let session = self
            .sessions
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| SisterError::context_not_found(id.to_string()))?;

        // Export all nodes from this session
        let session_nodes: Vec<&CognitiveEvent> = self
            .graph
            .nodes()
            .iter()
            .filter(|n| n.session_id == session.session_id)
            .collect();

        let data = serde_json::to_vec(&session_nodes)
            .map_err(|e| SisterError::new(ErrorCode::Internal, e.to_string()))?;
        let checksum = *blake3::hash(&data).as_bytes();

        Ok(ContextSnapshot {
            sister_type: SisterType::Memory,
            version: Version::new(0, 4, 1),
            context_info: ContextInfo {
                id,
                name: session.name.clone(),
                created_at: session.created_at,
                updated_at: chrono::Utc::now(),
                item_count: session_nodes.len(),
                size_bytes: data.len(),
                metadata: Metadata::new(),
            },
            data,
            checksum,
            snapshot_at: chrono::Utc::now(),
        })
    }

    fn import_session(&mut self, snapshot: ContextSnapshot) -> SisterResult<ContextId> {
        if !snapshot.verify() {
            return Err(SisterError::new(
                ErrorCode::ChecksumMismatch,
                "Session snapshot checksum verification failed",
            ));
        }

        // Start a new session for the imported data
        let context_id = self.start_session(&snapshot.context_info.name)?;

        // Deserialize and ingest the nodes
        let nodes: Vec<CognitiveEvent> = serde_json::from_slice(&snapshot.data)
            .map_err(|e| SisterError::new(ErrorCode::InvalidInput, e.to_string()))?;

        let session_id = self.current_session_id().unwrap_or(0);
        // Re-tag nodes with the new session ID
        let retagged: Vec<CognitiveEvent> = nodes
            .into_iter()
            .map(|mut n| {
                n.session_id = session_id;
                n
            })
            .collect();

        self.write_engine
            .ingest(&mut self.graph, retagged, vec![])
            .map_err(SisterError::from)?;

        Ok(context_id)
    }
}

// ═══════════════════════════════════════════════════════════════════
// GROUNDING
// ═══════════════════════════════════════════════════════════════════

impl Grounding for MemorySister {
    fn ground(&self, claim: &str) -> SisterResult<GroundingResult> {
        let params = TextSearchParams {
            query: claim.to_string(),
            max_results: 10,
            event_types: vec![],
            session_ids: vec![],
            min_score: 0.3,
        };

        let matches = self
            .query_engine
            .text_search(
                &self.graph,
                self.graph.term_index.as_ref(),
                self.graph.doc_lengths.as_ref(),
                params,
            )
            .map_err(SisterError::from)?;

        if matches.is_empty() {
            return Ok(
                GroundingResult::ungrounded(claim, "No matching memories found").with_suggestions(
                    self.graph
                        .nodes()
                        .iter()
                        .rev()
                        .take(3)
                        .map(|n| n.content.clone())
                        .collect(),
                ),
            );
        }

        let best_score = matches.iter().map(|m| m.score).fold(0.0f32, f32::max);

        let evidence: Vec<GroundingEvidence> = matches
            .iter()
            .filter_map(|m| {
                self.graph.get_node(m.node_id).map(|node| {
                    GroundingEvidence::new(
                        "memory_node",
                        format!("node_{}", node.id),
                        m.score as f64,
                        &node.content,
                    )
                    .with_data("event_type", format!("{:?}", node.event_type))
                    .with_data("session_id", node.session_id)
                    .with_data("confidence", node.confidence)
                    .with_data("matched_terms", m.matched_terms.clone())
                })
            })
            .collect();

        let confidence = best_score as f64;

        if confidence > 0.5 {
            Ok(GroundingResult::verified(claim, confidence)
                .with_evidence(evidence)
                .with_reason("Found matching memories via BM25 search"))
        } else {
            Ok(GroundingResult::partial(claim, confidence)
                .with_evidence(evidence)
                .with_reason("Some evidence found but low relevance"))
        }
    }

    fn evidence(&self, query: &str, max_results: usize) -> SisterResult<Vec<EvidenceDetail>> {
        let params = TextSearchParams {
            query: query.to_string(),
            max_results,
            event_types: vec![],
            session_ids: vec![],
            min_score: 0.0,
        };

        let matches = self
            .query_engine
            .text_search(
                &self.graph,
                self.graph.term_index.as_ref(),
                self.graph.doc_lengths.as_ref(),
                params,
            )
            .map_err(SisterError::from)?;

        Ok(matches
            .iter()
            .filter_map(|m| {
                self.graph.get_node(m.node_id).map(|node| {
                    let created_at =
                        chrono::DateTime::from_timestamp_micros(node.created_at as i64)
                            .unwrap_or_default();

                    EvidenceDetail {
                        evidence_type: "memory_node".to_string(),
                        id: format!("node_{}", node.id),
                        score: m.score as f64,
                        created_at,
                        source_sister: SisterType::Memory,
                        content: node.content.clone(),
                        data: {
                            let mut meta = Metadata::new();
                            if let Ok(v) = serde_json::to_value(format!("{:?}", node.event_type)) {
                                meta.insert("event_type".to_string(), v);
                            }
                            if let Ok(v) = serde_json::to_value(node.session_id) {
                                meta.insert("session_id".to_string(), v);
                            }
                            if let Ok(v) = serde_json::to_value(node.confidence) {
                                meta.insert("confidence".to_string(), v);
                            }
                            meta
                        },
                    }
                })
            })
            .collect())
    }

    fn suggest(&self, query: &str, limit: usize) -> SisterResult<Vec<GroundingSuggestion>> {
        // Word-overlap fallback (similar to existing memory_suggest tool)
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(f64, &CognitiveEvent)> = self
            .graph
            .nodes()
            .iter()
            .map(|node| {
                let content_lower = node.content.to_lowercase();
                let matched = query_words
                    .iter()
                    .filter(|w| content_lower.contains(**w))
                    .count();
                let score = if query_words.is_empty() {
                    0.0
                } else {
                    matched as f64 / query_words.len() as f64
                };
                (score, node)
            })
            .filter(|(score, _)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored
            .into_iter()
            .take(limit)
            .map(|(score, node)| GroundingSuggestion {
                item_type: "memory_node".to_string(),
                id: format!("node_{}", node.id),
                relevance_score: score,
                description: node.content.clone(),
                data: Metadata::new(),
            })
            .collect())
    }
}

// ═══════════════════════════════════════════════════════════════════
// QUERYABLE
// ═══════════════════════════════════════════════════════════════════

impl Queryable for MemorySister {
    fn query(&self, query: Query) -> SisterResult<QueryResult> {
        let start = Instant::now();

        let results: Vec<serde_json::Value> = match query.query_type.as_str() {
            "list" => {
                let limit = query.limit.unwrap_or(50);
                let offset = query.offset.unwrap_or(0);
                self.graph
                    .nodes()
                    .iter()
                    .skip(offset)
                    .take(limit)
                    .map(|n| {
                        serde_json::json!({
                            "id": n.id,
                            "event_type": format!("{:?}", n.event_type),
                            "content": n.content,
                            "confidence": n.confidence,
                            "session_id": n.session_id,
                            "created_at": n.created_at,
                        })
                    })
                    .collect()
            }
            "search" => {
                let text = query.get_string("text").unwrap_or_default();
                let max = query.limit.unwrap_or(20);

                let params = TextSearchParams {
                    query: text,
                    max_results: max,
                    event_types: vec![],
                    session_ids: vec![],
                    min_score: 0.0,
                };

                let matches = self
                    .query_engine
                    .text_search(
                        &self.graph,
                        self.graph.term_index.as_ref(),
                        self.graph.doc_lengths.as_ref(),
                        params,
                    )
                    .map_err(SisterError::from)?;

                matches
                    .iter()
                    .filter_map(|m| {
                        self.graph.get_node(m.node_id).map(|n| {
                            serde_json::json!({
                                "id": n.id,
                                "content": n.content,
                                "score": m.score,
                                "matched_terms": m.matched_terms,
                            })
                        })
                    })
                    .collect()
            }
            "recent" => {
                let count = query.limit.unwrap_or(10);
                self.graph
                    .nodes()
                    .iter()
                    .rev()
                    .take(count)
                    .map(|n| {
                        serde_json::json!({
                            "id": n.id,
                            "event_type": format!("{:?}", n.event_type),
                            "content": n.content,
                            "confidence": n.confidence,
                            "session_id": n.session_id,
                            "created_at": n.created_at,
                        })
                    })
                    .collect()
            }
            "get" => {
                let id_str = query.get_string("id").unwrap_or_default();
                let id: u64 = id_str.parse().unwrap_or(0);
                if let Some(n) = self.graph.get_node(id) {
                    vec![serde_json::json!({
                        "id": n.id,
                        "event_type": format!("{:?}", n.event_type),
                        "content": n.content,
                        "confidence": n.confidence,
                        "session_id": n.session_id,
                        "created_at": n.created_at,
                    })]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        };

        let total = self.graph.node_count();
        let has_more = results.len() < total;

        Ok(QueryResult::new(query, results, start.elapsed()).with_pagination(total, has_more))
    }

    fn supports_query(&self, query_type: &str) -> bool {
        matches!(
            query_type,
            "list" | "search" | "recent" | "get" | "related" | "temporal"
        )
    }

    fn query_types(&self) -> Vec<QueryTypeInfo> {
        vec![
            QueryTypeInfo::new("list", "List all memory nodes with pagination")
                .optional(vec!["limit", "offset"]),
            QueryTypeInfo::new("search", "Search memories by text (BM25)")
                .required(vec!["text"])
                .optional(vec!["limit"]),
            QueryTypeInfo::new("recent", "Get most recent memories").optional(vec!["limit"]),
            QueryTypeInfo::new("get", "Get a specific memory node by ID").required(vec!["id"]),
        ]
    }
}

// ═══════════════════════════════════════════════════════════════════
// FILE FORMAT
// ═══════════════════════════════════════════════════════════════════

#[cfg(feature = "format")]
impl FileFormatReader for MemorySister {
    fn read_file(path: &Path) -> SisterResult<Self> {
        let graph = crate::format::AmemReader::read_from_file(path).map_err(SisterError::from)?;
        Ok(Self::from_graph(graph, Some(path.to_path_buf())))
    }

    fn can_read(path: &Path) -> SisterResult<FileInfo> {
        // Read just the 64-byte header to check format validity
        let data = std::fs::read(path)
            .map_err(|e| SisterError::new(ErrorCode::StorageError, e.to_string()))?;
        if data.len() < 64 {
            return Err(SisterError::new(
                ErrorCode::StorageError,
                "File too small for .amem format",
            ));
        }
        let header = crate::types::FileHeader::read_from(&mut std::io::Cursor::new(&data[..64]))
            .map_err(SisterError::from)?;

        let metadata = std::fs::metadata(path)
            .map_err(|e| SisterError::new(ErrorCode::StorageError, e.to_string()))?;

        Ok(FileInfo {
            sister_type: SisterType::Memory,
            version: Version::new(header.version as u8, 0, 0),
            created_at: chrono::Utc::now(), // .amem doesn't store creation time in header
            updated_at: chrono::DateTime::from(
                metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            ),
            content_length: metadata.len(),
            needs_migration: header.version < crate::types::FORMAT_VERSION,
            format_id: "AMEM".to_string(),
        })
    }

    fn file_version(path: &Path) -> SisterResult<Version> {
        let data = std::fs::read(path)
            .map_err(|e| SisterError::new(ErrorCode::StorageError, e.to_string()))?;
        if data.len() < 64 {
            return Err(SisterError::new(
                ErrorCode::StorageError,
                "File too small for .amem format",
            ));
        }
        let header = crate::types::FileHeader::read_from(&mut std::io::Cursor::new(&data[..64]))
            .map_err(SisterError::from)?;
        Ok(Version::new(header.version as u8, 0, 0))
    }

    fn migrate(_data: &[u8], _from_version: Version) -> SisterResult<Vec<u8>> {
        // Memory format v1 is the only version — no migration needed yet
        Err(SisterError::new(
            ErrorCode::NotImplemented,
            "No migration path available (only v1 exists)",
        ))
    }
}

#[cfg(feature = "format")]
impl FileFormatWriter for MemorySister {
    fn write_file(&self, path: &Path) -> SisterResult<()> {
        let writer = crate::format::AmemWriter::new(self.graph.dimension());
        writer
            .write_to_file(&self.graph, path)
            .map_err(SisterError::from)
    }

    fn to_bytes(&self) -> SisterResult<Vec<u8>> {
        let writer = crate::format::AmemWriter::new(self.graph.dimension());
        let mut buffer = Vec::new();
        writer
            .write_to(&self.graph, &mut buffer)
            .map_err(SisterError::from)?;
        Ok(buffer)
    }
}

// ═══════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CognitiveEventBuilder, EventType};

    fn make_test_sister() -> MemorySister {
        let config = SisterConfig::stateless().option("dimension", DEFAULT_DIMENSION);
        MemorySister::init(config).unwrap()
    }

    fn add_test_nodes(sister: &mut MemorySister) {
        let session_id = sister.current_session_id().unwrap_or(0);
        let events = vec![
            CognitiveEventBuilder::new(EventType::Fact, "The sky is blue")
                .confidence(0.95)
                .session_id(session_id)
                .build(),
            CognitiveEventBuilder::new(EventType::Fact, "Rust is fast and memory safe")
                .confidence(0.9)
                .session_id(session_id)
                .build(),
            CognitiveEventBuilder::new(EventType::Decision, "Use BM25 for text search")
                .confidence(0.85)
                .session_id(session_id)
                .build(),
        ];
        sister
            .write_engine
            .ingest(&mut sister.graph, events, vec![])
            .unwrap();
    }

    /// Helper to build BM25 term index and doc lengths for text search tests.
    fn build_indexes(sister: &mut MemorySister) {
        use crate::engine::Tokenizer;
        use crate::index::{DocLengths, TermIndex};
        let tokenizer = Tokenizer::new();
        let term_index = TermIndex::build(&sister.graph, &tokenizer);
        sister.graph.set_term_index(term_index);
        let doc_lengths = DocLengths::build(&sister.graph, &tokenizer);
        sister.graph.set_doc_lengths(doc_lengths);
    }

    #[test]
    fn test_sister_trait() {
        let sister = make_test_sister();
        assert_eq!(sister.sister_type(), SisterType::Memory);
        assert_eq!(sister.file_extension(), "amem");
        assert_eq!(sister.mcp_prefix(), "memory");
        assert!(sister.is_healthy());
        assert_eq!(sister.version(), Version::new(0, 4, 1));
        assert!(!sister.capabilities().is_empty());
    }

    #[test]
    fn test_sister_info() {
        let sister = make_test_sister();
        let info = SisterInfo::from_sister(&sister);
        assert_eq!(info.sister_type, SisterType::Memory);
        assert_eq!(info.file_extension, "amem");
        assert_eq!(info.mcp_prefix, "memory");
    }

    #[test]
    fn test_session_management() {
        let mut sister = make_test_sister();

        // No session initially
        assert!(sister.current_session().is_none());
        assert!(sister.current_session_info().is_err());

        // Start session
        let sid = sister.start_session("test_session").unwrap();
        assert!(sister.current_session().is_some());
        assert_eq!(sister.current_session().unwrap(), sid);

        // Session info
        let info = sister.current_session_info().unwrap();
        assert_eq!(info.name, "test_session");

        // List sessions
        let sessions = sister.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "test_session");

        // End session
        sister.end_session().unwrap();
        assert!(sister.current_session().is_none());

        // Can't end twice
        assert!(sister.end_session().is_err());
    }

    #[test]
    fn test_grounding_with_data() {
        let mut sister = make_test_sister();
        sister.start_session("grounding_test").unwrap();
        add_test_nodes(&mut sister);

        // Ensure term index is built for BM25
        build_indexes(&mut sister);

        // Ground a claim that should match
        let result = sister.ground("sky is blue").unwrap();
        assert!(
            result.status == GroundingStatus::Verified || result.status == GroundingStatus::Partial,
            "Expected verified or partial, got {:?}",
            result.status
        );
        assert!(!result.evidence.is_empty());

        // Ground a claim that should NOT match
        let result = sister.ground("cats can teleport").unwrap();
        assert_eq!(result.status, GroundingStatus::Ungrounded);
    }

    #[test]
    fn test_evidence_query() {
        let mut sister = make_test_sister();
        sister.start_session("evidence_test").unwrap();
        add_test_nodes(&mut sister);
        build_indexes(&mut sister);

        let evidence = sister.evidence("rust", 10).unwrap();
        // BM25 should find the "Rust is fast" node
        assert!(!evidence.is_empty(), "Expected evidence for 'rust' query");
        assert_eq!(evidence[0].source_sister, SisterType::Memory);
    }

    #[test]
    fn test_suggest_fallback() {
        let mut sister = make_test_sister();
        sister.start_session("suggest_test").unwrap();
        add_test_nodes(&mut sister);

        let suggestions = sister.suggest("blue sky", 5).unwrap();
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].relevance_score > 0.0);
    }

    #[test]
    fn test_queryable_list() {
        let mut sister = make_test_sister();
        sister.start_session("query_test").unwrap();
        add_test_nodes(&mut sister);

        let result = sister.query(Query::list().limit(2)).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.has_more);
    }

    #[test]
    fn test_queryable_recent() {
        let mut sister = make_test_sister();
        sister.start_session("recent_test").unwrap();
        add_test_nodes(&mut sister);

        let result = sister.recent(2).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_queryable_search() {
        let mut sister = make_test_sister();
        sister.start_session("search_test").unwrap();
        add_test_nodes(&mut sister);
        build_indexes(&mut sister);

        let result = sister.search("rust").unwrap();
        assert!(!result.is_empty(), "Expected search results for 'rust'");
    }

    #[test]
    fn test_queryable_types() {
        let sister = make_test_sister();
        assert!(sister.supports_query("list"));
        assert!(sister.supports_query("search"));
        assert!(sister.supports_query("recent"));
        assert!(sister.supports_query("get"));
        assert!(!sister.supports_query("nonexistent"));

        let types = sister.query_types();
        assert_eq!(types.len(), 4);
    }

    #[test]
    fn test_error_bridge() {
        let amem_err = AmemError::NodeNotFound(42);
        let sister_err: SisterError = amem_err.into();
        assert_eq!(sister_err.code, ErrorCode::NotFound);
        assert!(sister_err.message.contains("42"));

        let amem_err2 = AmemError::InvalidMagic;
        let sister_err2: SisterError = amem_err2.into();
        assert_eq!(sister_err2.code, ErrorCode::VersionMismatch);
    }

    #[test]
    fn test_session_export_import() {
        let mut sister = make_test_sister();
        let sid = sister.start_session("export_test").unwrap();
        add_test_nodes(&mut sister);

        // Export
        let snapshot = sister.export_session(sid).unwrap();
        assert!(snapshot.verify());
        assert_eq!(snapshot.sister_type, SisterType::Memory);

        // Import into fresh sister
        let mut sister2 = make_test_sister();
        let _imported_sid = sister2.import_session(snapshot).unwrap();
        assert!(sister2.current_session().is_some());
        // Imported session should have nodes
        assert!(sister2.graph().node_count() > 0);
    }

    #[test]
    fn test_config_patterns() {
        // Single path config
        let config = SisterConfig::new("/tmp/test.amem");
        let sister = MemorySister::init(config).unwrap();
        assert!(sister.is_healthy());

        // Stateless config
        let config2 = SisterConfig::stateless();
        let sister2 = MemorySister::init(config2).unwrap();
        assert!(sister2.is_healthy());
    }

    #[test]
    fn test_shutdown() {
        let mut sister = make_test_sister();
        sister.start_session("shutdown_test").unwrap();
        sister.shutdown().unwrap();
        // Session should be ended after shutdown
        assert!(sister.current_session().is_none());
    }
}
