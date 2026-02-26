//! V3 Immortal Architecture MCP Tools — 13 tools for capture, retrieval, search, and stats.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};

use agentic_memory::v3::{
    BlockHash, BoundaryType, FileOperation, MemoryEngineV3,
    RetrievalRequest, RetrievalStrategy,
};

use crate::types::{McpError, McpResult, ToolCallResult, ToolDefinition};

// ═══════════════════════════════════════════════════════════════════
// Helper: shared engine type alias
// ═══════════════════════════════════════════════════════════════════

pub type SharedEngine = Arc<Mutex<Option<MemoryEngineV3>>>;

fn require_engine(engine: &Option<MemoryEngineV3>) -> McpResult<&MemoryEngineV3> {
    engine
        .as_ref()
        .ok_or_else(|| McpError::InternalError("V3 engine not initialized".to_string()))
}

// ═══════════════════════════════════════════════════════════════════
// 1. memory_capture_message
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct CaptureMessageParams {
    role: String,
    content: String,
    tokens: Option<u32>,
}

pub fn capture_message_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_capture_message".to_string(),
        description: Some("Capture a message to the immortal log".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "role": {
                    "type": "string",
                    "enum": ["user", "assistant", "system"],
                    "description": "Message role"
                },
                "content": {
                    "type": "string",
                    "description": "Message content"
                },
                "tokens": {
                    "type": "integer",
                    "description": "Optional token count"
                }
            },
            "required": ["role", "content"]
        }),
    }
}

pub async fn capture_message_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: CaptureMessageParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let hash = match params.role.as_str() {
        "user" => eng.capture_user_message(&params.content, params.tokens),
        "assistant" => eng.capture_assistant_message(&params.content, params.tokens),
        "system" => eng.capture_user_message(&params.content, params.tokens),
        _ => return Err(McpError::InvalidParams(format!("Unknown role: {}", params.role))),
    }
    .map_err(|e| McpError::InternalError(e.to_string()))?;

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "block_hash": hash.to_hex()
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 2. memory_capture_tool
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct CaptureToolParams {
    tool_name: String,
    input: Value,
    output: Option<Value>,
    duration_ms: Option<u64>,
    success: bool,
}

pub fn capture_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_capture_tool".to_string(),
        description: Some("Capture a tool call to the immortal log".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "tool_name": { "type": "string", "description": "Name of the tool" },
                "input": { "type": "object", "description": "Tool input parameters" },
                "output": { "type": "object", "description": "Tool output (optional)" },
                "duration_ms": { "type": "integer", "description": "Execution duration in ms" },
                "success": { "type": "boolean", "description": "Whether the tool succeeded" }
            },
            "required": ["tool_name", "input", "success"]
        }),
    }
}

pub async fn capture_tool_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: CaptureToolParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let hash = eng
        .capture_tool_call(
            &params.tool_name,
            params.input,
            params.output,
            params.duration_ms,
            params.success,
        )
        .map_err(|e| McpError::InternalError(e.to_string()))?;

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "block_hash": hash.to_hex()
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 3. memory_capture_file
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct CaptureFileParams {
    path: String,
    operation: String,
    lines: Option<u32>,
    diff: Option<String>,
}

pub fn capture_file_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_capture_file".to_string(),
        description: Some("Capture a file operation to the immortal log".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path" },
                "operation": {
                    "type": "string",
                    "enum": ["create", "read", "update", "delete", "rename"],
                    "description": "File operation type"
                },
                "lines": { "type": "integer", "description": "Number of lines" },
                "diff": { "type": "string", "description": "Diff content (optional)" }
            },
            "required": ["path", "operation"]
        }),
    }
}

pub async fn capture_file_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: CaptureFileParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let op = match params.operation.as_str() {
        "create" => FileOperation::Create,
        "read" => FileOperation::Read,
        "update" => FileOperation::Update,
        "delete" => FileOperation::Delete,
        "rename" => FileOperation::Rename,
        _ => return Err(McpError::InvalidParams(format!("Unknown operation: {}", params.operation))),
    };

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let hash = eng
        .capture_file_operation(
            &params.path,
            op,
            None,         // content_hash
            params.lines,
            params.diff,  // diff: Option<String>
        )
        .map_err(|e| McpError::InternalError(e.to_string()))?;

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "block_hash": hash.to_hex()
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 4. memory_capture_decision
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct CaptureDecisionParams {
    decision: String,
    reasoning: Option<String>,
    evidence_hashes: Option<Vec<String>>,
    confidence: Option<f32>,
}

pub fn capture_decision_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_capture_decision".to_string(),
        description: Some("Capture a decision to the immortal log".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "decision": { "type": "string", "description": "The decision made" },
                "reasoning": { "type": "string", "description": "Reasoning behind the decision" },
                "evidence_hashes": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Block hashes of supporting evidence"
                },
                "confidence": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Confidence in the decision"
                }
            },
            "required": ["decision"]
        }),
    }
}

pub async fn capture_decision_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: CaptureDecisionParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let evidence: Vec<BlockHash> = params
        .evidence_hashes
        .unwrap_or_default()
        .iter()
        .filter_map(|h| BlockHash::from_hex(h))
        .collect();

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let hash = eng
        .capture_decision(
            &params.decision,
            params.reasoning.as_deref(),
            evidence,
            params.confidence,
        )
        .map_err(|e| McpError::InternalError(e.to_string()))?;

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "block_hash": hash.to_hex()
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 5. memory_capture_boundary
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct CaptureBoundaryParams {
    boundary_type: String,
    context_tokens_before: Option<u32>,
    context_tokens_after: Option<u32>,
    summary: Option<String>,
}

pub fn capture_boundary_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_capture_boundary".to_string(),
        description: Some("Capture a session boundary event".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "boundary_type": {
                    "type": "string",
                    "enum": ["session_start", "session_end", "compaction", "context_pressure", "checkpoint"],
                    "description": "Type of boundary"
                },
                "context_tokens_before": { "type": "integer" },
                "context_tokens_after": { "type": "integer" },
                "summary": { "type": "string", "description": "Summary of the boundary event" }
            },
            "required": ["boundary_type"]
        }),
    }
}

pub async fn capture_boundary_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: CaptureBoundaryParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let bt = match params.boundary_type.as_str() {
        "session_start" => BoundaryType::SessionStart,
        "session_end" => BoundaryType::SessionEnd,
        "compaction" => BoundaryType::Compaction,
        "context_pressure" => BoundaryType::ContextPressure,
        "checkpoint" => BoundaryType::Checkpoint,
        _ => return Err(McpError::InvalidParams(format!("Unknown boundary type: {}", params.boundary_type))),
    };

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let hash = eng
        .capture_boundary(
            bt,
            params.context_tokens_before.unwrap_or(0),
            params.context_tokens_after.unwrap_or(0),
            params.summary.as_deref().unwrap_or(""),
            None,
        )
        .map_err(|e| McpError::InternalError(e.to_string()))?;

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "block_hash": hash.to_hex()
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 6. memory_retrieve (Smart context assembly)
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct RetrieveParams {
    query: String,
    #[serde(default = "default_token_budget")]
    token_budget: u32,
    #[serde(default = "default_strategy")]
    strategy: String,
}

fn default_token_budget() -> u32 {
    50000
}
fn default_strategy() -> String {
    "balanced".to_string()
}

pub fn retrieve_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_retrieve".to_string(),
        description: Some("Smart context retrieval - assemble perfect context for a query".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "What context to retrieve" },
                "token_budget": { "type": "integer", "default": 50000, "description": "Maximum tokens" },
                "strategy": {
                    "type": "string",
                    "enum": ["recency", "relevance", "causal", "balanced"],
                    "default": "balanced",
                    "description": "Retrieval strategy"
                }
            },
            "required": ["query"]
        }),
    }
}

pub async fn retrieve_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: RetrieveParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let strategy = match params.strategy.as_str() {
        "recency" => RetrievalStrategy::Recency,
        "relevance" => RetrievalStrategy::Relevance,
        "causal" => RetrievalStrategy::Causal,
        _ => RetrievalStrategy::Balanced,
    };

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let result = eng.retrieve(RetrievalRequest {
        query: params.query,
        token_budget: params.token_budget,
        strategy,
        min_relevance: 0.0,
    });

    let context: Vec<String> = result
        .blocks
        .iter()
        .map(|b| format!("[{:?}] {}", b.block_type, b.content_summary()))
        .collect();

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "context": context.join("\n\n"),
        "blocks_used": result.blocks.len(),
        "tokens_used": result.tokens_used,
        "retrieval_ms": result.retrieval_ms
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 7. memory_resurrect
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct ResurrectParams {
    timestamp: String,
}

pub fn resurrect_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_resurrect".to_string(),
        description: Some("Fully restore state at any timestamp".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "timestamp": {
                    "type": "string",
                    "description": "ISO 8601 timestamp to resurrect to"
                }
            },
            "required": ["timestamp"]
        }),
    }
}

pub async fn resurrect_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: ResurrectParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let ts = chrono::DateTime::parse_from_rfc3339(&params.timestamp)
        .map_err(|e| McpError::InvalidParams(format!("Invalid timestamp: {}", e)))?
        .with_timezone(&chrono::Utc);

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let result = eng.resurrect(ts);

    // files_state is HashMap<String, bool>
    let files: Vec<String> = result.files_state.keys().cloned().collect();

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "block_count": result.block_count,
        "files": files,
        "decisions": result.decisions,
        "messages": result.messages.len()
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 8. memory_v3_session_resume
// ═══════════════════════════════════════════════════════════════════

pub fn v3_session_resume_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_v3_session_resume".to_string(),
        description: Some("Resume session with full V3 immortal context".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn v3_session_resume_exec(
    _args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let result = eng.session_resume();

    Ok(ToolCallResult::json(&json!({
        "session_id": result.session_id,
        "block_count": result.block_count,
        "recent_messages": result.recent_messages,
        "files_touched": result.files_touched,
        "decisions": result.decisions,
        "errors_resolved": result.errors_resolved,
        "all_known_files": result.all_known_files
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 9. memory_search_temporal
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct SearchTemporalParams {
    start: String,
    end: String,
}

pub fn search_temporal_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_search_temporal".to_string(),
        description: Some("Search blocks by time range".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start": { "type": "string", "description": "ISO 8601 start timestamp" },
                "end": { "type": "string", "description": "ISO 8601 end timestamp" }
            },
            "required": ["start", "end"]
        }),
    }
}

pub async fn search_temporal_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: SearchTemporalParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let start = chrono::DateTime::parse_from_rfc3339(&params.start)
        .map_err(|e| McpError::InvalidParams(format!("Invalid start: {}", e)))?
        .with_timezone(&chrono::Utc);
    let end = chrono::DateTime::parse_from_rfc3339(&params.end)
        .map_err(|e| McpError::InvalidParams(format!("Invalid end: {}", e)))?
        .with_timezone(&chrono::Utc);

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let blocks = eng.search_temporal(start, end);

    let results: Vec<Value> = blocks
        .iter()
        .map(|b| {
            json!({
                "hash": b.hash.to_hex(),
                "type": format!("{:?}", b.block_type),
                "timestamp": b.timestamp.to_rfc3339(),
                "summary": b.content_summary()
            })
        })
        .collect();

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "blocks": results,
        "count": results.len()
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 10. memory_search_semantic
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct SearchSemanticParams {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

pub fn search_semantic_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_search_semantic".to_string(),
        description: Some("Search blocks by meaning/text".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "limit": { "type": "integer", "default": 20, "description": "Max results" }
            },
            "required": ["query"]
        }),
    }
}

pub async fn search_semantic_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: SearchSemanticParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let results = eng.search_semantic(&params.query, params.limit);

    let blocks: Vec<Value> = results
        .iter()
        .map(|b| {
            json!({
                "hash": b.hash.to_hex(),
                "sequence": b.sequence,
                "type": format!("{:?}", b.block_type),
                "timestamp": b.timestamp.to_rfc3339(),
                "summary": b.content_summary()
            })
        })
        .collect();

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "blocks": blocks,
        "count": blocks.len()
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 11. memory_search_entity
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct SearchEntityParams {
    entity: String,
}

pub fn search_entity_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_search_entity".to_string(),
        description: Some("Search blocks mentioning a file, person, or entity".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "entity": { "type": "string", "description": "Entity to search for (file path, name, etc.)" }
            },
            "required": ["entity"]
        }),
    }
}

pub async fn search_entity_exec(
    args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let params: SearchEntityParams =
        serde_json::from_value(args).map_err(|e| McpError::InvalidParams(e.to_string()))?;

    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let results = eng.search_entity(&params.entity);

    let blocks: Vec<Value> = results
        .iter()
        .map(|b| {
            json!({
                "hash": b.hash.to_hex(),
                "sequence": b.sequence,
                "type": format!("{:?}", b.block_type),
                "timestamp": b.timestamp.to_rfc3339(),
                "summary": b.content_summary()
            })
        })
        .collect();

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "blocks": blocks,
        "count": blocks.len()
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 12. memory_verify_integrity
// ═══════════════════════════════════════════════════════════════════

pub fn verify_integrity_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_verify_integrity".to_string(),
        description: Some("Verify cryptographic integrity of the memory log".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn verify_integrity_exec(
    _args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let report = eng.verify_integrity();

    Ok(ToolCallResult::json(&json!({
        "success": true,
        "verified": report.verified,
        "blocks_checked": report.blocks_checked,
        "chain_intact": report.chain_intact,
        "missing_blocks": report.missing_blocks,
        "corrupted_blocks": report.corrupted_blocks
    })))
}

// ═══════════════════════════════════════════════════════════════════
// 13. memory_v3_stats
// ═══════════════════════════════════════════════════════════════════

pub fn v3_stats_def() -> ToolDefinition {
    ToolDefinition {
        name: "memory_v3_stats".to_string(),
        description: Some("Get V3 memory engine statistics".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn v3_stats_exec(
    _args: Value,
    engine: &SharedEngine,
) -> McpResult<ToolCallResult> {
    let eng = engine.lock().await;
    let eng = require_engine(&eng)?;

    let stats = eng.stats();

    Ok(ToolCallResult::json(&json!({
        "total_blocks": stats.total_blocks,
        "session_id": stats.session_id,
        "tier_stats": {
            "hot": { "blocks": stats.tier_stats.hot_blocks, "bytes": stats.tier_stats.hot_bytes },
            "warm": { "blocks": stats.tier_stats.warm_blocks, "bytes": stats.tier_stats.warm_bytes },
            "cold": { "blocks": stats.tier_stats.cold_blocks, "bytes": stats.tier_stats.cold_bytes },
            "frozen": { "blocks": stats.tier_stats.frozen_blocks }
        }
    })))
}

// ═══════════════════════════════════════════════════════════════════
// REGISTRATION: List + Dispatch
// ═══════════════════════════════════════════════════════════════════

/// Return all V3 tool definitions.
pub fn list_v3_tools() -> Vec<ToolDefinition> {
    vec![
        capture_message_def(),
        capture_tool_def(),
        capture_file_def(),
        capture_decision_def(),
        capture_boundary_def(),
        retrieve_def(),
        resurrect_def(),
        v3_session_resume_def(),
        search_temporal_def(),
        search_semantic_def(),
        search_entity_def(),
        verify_integrity_def(),
        v3_stats_def(),
    ]
}

/// Dispatch a V3 tool call. Returns None if the tool is not a V3 tool.
pub async fn dispatch_v3_tool(
    name: &str,
    args: Value,
    engine: &SharedEngine,
) -> Option<McpResult<ToolCallResult>> {
    match name {
        "memory_capture_message" => Some(capture_message_exec(args, engine).await),
        "memory_capture_tool" => Some(capture_tool_exec(args, engine).await),
        "memory_capture_file" => Some(capture_file_exec(args, engine).await),
        "memory_capture_decision" => Some(capture_decision_exec(args, engine).await),
        "memory_capture_boundary" => Some(capture_boundary_exec(args, engine).await),
        "memory_retrieve" => Some(retrieve_exec(args, engine).await),
        "memory_resurrect" => Some(resurrect_exec(args, engine).await),
        "memory_v3_session_resume" => Some(v3_session_resume_exec(args, engine).await),
        "memory_search_temporal" => Some(search_temporal_exec(args, engine).await),
        "memory_search_semantic" => Some(search_semantic_exec(args, engine).await),
        "memory_search_entity" => Some(search_entity_exec(args, engine).await),
        "memory_verify_integrity" => Some(verify_integrity_exec(args, engine).await),
        "memory_v3_stats" => Some(v3_stats_exec(args, engine).await),
        _ => None,
    }
}
