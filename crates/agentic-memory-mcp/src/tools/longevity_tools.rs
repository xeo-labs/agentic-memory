//! V4 Longevity Engine MCP tools.
//!
//! Provides 8 longevity-specific tools for querying hierarchy, stats,
//! consolidation, integrity, significance, backup, and embedding status.

use std::sync::Arc;
use tokio::sync::Mutex;

use serde_json::Value;

use crate::session::SessionManager;
use crate::types::{McpResult, ToolCallResult, ToolDefinition};

use agentic_memory::v3::longevity::budget::StorageBudget;
use agentic_memory::v3::longevity::consolidation::ConsolidationEngine;
use agentic_memory::v3::longevity::embedding_migration::EmbeddingMigrator;
use agentic_memory::v3::longevity::hierarchy::MemoryLayer;
use agentic_memory::v3::longevity::integrity::IntegrityVerifier;
use agentic_memory::v3::longevity::store::LongevityStore;

/// Return all longevity tool definitions.
pub fn all_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "memory_longevity_stats".to_string(),
            description: Some("Return storage budget, layer distribution, and 20-year projections"
                .to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project identifier" }
                },
                "required": ["project_id"]
            }),
        },
        ToolDefinition {
            name: "memory_hierarchy_query".to_string(),
            description: Some("Query memories at a specific cognitive layer (raw, episode, summary, pattern, trait, identity)"
                .to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project identifier" },
                    "layer": { "type": "string", "enum": ["raw", "episode", "summary", "pattern", "trait", "identity"], "description": "Memory layer to query" },
                    "limit": { "type": "integer", "default": 20, "description": "Maximum results" }
                },
                "required": ["project_id", "layer"]
            }),
        },
        ToolDefinition {
            name: "memory_hierarchy_navigate".to_string(),
            description: Some("Drill down from a compressed memory to its source memories"
                .to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_id": { "type": "string", "description": "Memory ID to navigate from" }
                },
                "required": ["memory_id"]
            }),
        },
        ToolDefinition {
            name: "memory_longevity_consolidate".to_string(),
            description: Some("Trigger manual consolidation for a project across all eligible layers"
                .to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project identifier" }
                },
                "required": ["project_id"]
            }),
        },
        ToolDefinition {
            name: "memory_longevity_health".to_string(),
            description: Some("Return overall health score with integrity verification and recommendations"
                .to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project identifier" }
                },
                "required": ["project_id"]
            }),
        },
        ToolDefinition {
            name: "memory_hierarchy_significance".to_string(),
            description: Some("Get or set the significance score for a specific memory"
                .to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_id": { "type": "string", "description": "Memory ID" },
                    "set_significance": { "type": "number", "description": "If provided, set significance to this value (0.0-1.0)" }
                },
                "required": ["memory_id"]
            }),
        },
        ToolDefinition {
            name: "memory_embedding_status".to_string(),
            description: Some("Return which embedding models are in use and migration progress"
                .to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "memory_longevity_search".to_string(),
            description: Some("Full-text search across all memory layers in the longevity store"
                .to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project identifier" },
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "default": 20, "description": "Maximum results" }
                },
                "required": ["project_id", "query"]
            }),
        },
    ]
}

/// Try to execute a longevity tool. Returns None if the tool name doesn't match.
pub async fn try_execute(
    name: &str,
    args: Value,
    _session: &Arc<Mutex<SessionManager>>,
) -> Option<McpResult<ToolCallResult>> {
    match name {
        "memory_longevity_stats" => Some(execute_longevity_stats(args).await),
        "memory_hierarchy_query" => Some(execute_hierarchy_query(args).await),
        "memory_hierarchy_navigate" => Some(execute_hierarchy_navigate(args).await),
        "memory_longevity_consolidate" => Some(execute_consolidate(args).await),
        "memory_longevity_health" => Some(execute_health(args).await),
        "memory_hierarchy_significance" => Some(execute_significance(args).await),
        "memory_embedding_status" => Some(execute_embedding_status(args).await),
        "memory_longevity_search" => Some(execute_search(args).await),
        _ => None,
    }
}

fn get_longevity_db_path(project_id: &str) -> std::path::PathBuf {
    let base = dirs::home_dir()
        .unwrap_or_default()
        .join(".agentic")
        .join("memory");
    base.join(format!("{}.longevity.db", project_id))
}

fn open_store(project_id: &str) -> Result<LongevityStore, String> {
    let path = get_longevity_db_path(project_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    LongevityStore::open(&path).map_err(|e| format!("Failed to open longevity store: {}", e))
}

fn ok_result(content: Value) -> McpResult<ToolCallResult> {
    Ok(ToolCallResult::text(
        serde_json::to_string_pretty(&content).unwrap_or_default(),
    ))
}

fn err_result(msg: &str) -> McpResult<ToolCallResult> {
    Ok(ToolCallResult {
        content: vec![crate::types::ToolContent::Text {
            text: msg.to_string(),
        }],
        is_error: Some(true),
    })
}

async fn execute_longevity_stats(args: Value) -> McpResult<ToolCallResult> {
    let project_id = args["project_id"].as_str().unwrap_or("default");
    let store = match open_store(project_id) {
        Ok(s) => s,
        Err(e) => return err_result(&e),
    };

    let stats = store
        .hierarchy_stats(project_id)
        .map_err(|e| format!("{}", e));
    let stats = match stats {
        Ok(s) => s,
        Err(e) => return err_result(&e),
    };

    let budget = StorageBudget::new();
    let status = budget.overall_status(&stats);
    let layers = budget.layer_budgets(&stats);

    let projection = budget.project_growth(&store, project_id).ok();

    ok_result(serde_json::json!({
        "hierarchy": {
            "raw": stats.raw_count,
            "episode": stats.episode_count,
            "summary": stats.summary_count,
            "pattern": stats.pattern_count,
            "trait": stats.trait_count,
            "identity": stats.identity_count,
            "total": stats.total_count,
        },
        "storage": {
            "total_bytes": stats.total_bytes,
            "budget_status": status.message,
        },
        "layers": layers.iter().map(|l| serde_json::json!({
            "layer": l.layer,
            "used_bytes": l.used_bytes,
            "allocated_bytes": l.allocated_bytes,
            "used_percent": l.used_percent,
            "status": l.status.message,
        })).collect::<Vec<_>>(),
        "projection": projection.map(|p| serde_json::json!({
            "daily_growth_bytes": p.daily_growth_bytes,
            "projected_1_year": p.projected_1_year,
            "projected_5_year": p.projected_5_year,
            "projected_20_year": p.projected_20_year,
        })),
    }))
}

async fn execute_hierarchy_query(args: Value) -> McpResult<ToolCallResult> {
    let project_id = args["project_id"].as_str().unwrap_or("default");
    let layer_str = args["layer"].as_str().unwrap_or("raw");
    let limit = args["limit"].as_u64().unwrap_or(20) as u32;

    let layer = match layer_str {
        "raw" => MemoryLayer::Raw,
        "episode" => MemoryLayer::Episode,
        "summary" => MemoryLayer::Summary,
        "pattern" => MemoryLayer::Pattern,
        "trait" => MemoryLayer::Trait,
        "identity" => MemoryLayer::Identity,
        _ => return err_result(&format!("Unknown layer: {}", layer_str)),
    };

    let store = match open_store(project_id) {
        Ok(s) => s,
        Err(e) => return err_result(&e),
    };

    let memories = store
        .query_by_layer(project_id, layer, limit)
        .map_err(|e| format!("{}", e));
    let memories = match memories {
        Ok(m) => m,
        Err(e) => return err_result(&e),
    };

    let results: Vec<Value> = memories
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "layer": m.layer.content_type(),
                "content": m.content,
                "significance": m.significance,
                "created_at": m.created_at,
                "access_count": m.access_count,
            })
        })
        .collect();

    ok_result(serde_json::json!({
        "layer": layer_str,
        "count": results.len(),
        "memories": results,
    }))
}

async fn execute_hierarchy_navigate(args: Value) -> McpResult<ToolCallResult> {
    let memory_id = args["memory_id"].as_str().unwrap_or("");
    if memory_id.is_empty() {
        return err_result("memory_id is required");
    }

    // Try to find the memory in any project store
    // For now, use "default" — in production, iterate project stores
    let store = match open_store("default") {
        Ok(s) => s,
        Err(e) => return err_result(&e),
    };

    let memory = store.get_memory(memory_id).map_err(|e| format!("{}", e));
    let memory = match memory {
        Ok(Some(m)) => m,
        Ok(None) => return err_result(&format!("Memory {} not found", memory_id)),
        Err(e) => return err_result(&e),
    };

    let source_ids = memory.original_ids.clone().unwrap_or_default();
    let mut sources = Vec::new();
    for sid in &source_ids {
        if let Ok(Some(src)) = store.get_memory(sid) {
            sources.push(serde_json::json!({
                "id": src.id,
                "layer": src.layer.content_type(),
                "content": src.content,
                "significance": src.significance,
            }));
        }
    }

    ok_result(serde_json::json!({
        "memory": {
            "id": memory.id,
            "layer": memory.layer.content_type(),
            "content": memory.content,
        },
        "source_count": source_ids.len(),
        "sources": sources,
    }))
}

async fn execute_consolidate(args: Value) -> McpResult<ToolCallResult> {
    let project_id = args["project_id"].as_str().unwrap_or("default");
    let store = match open_store(project_id) {
        Ok(s) => s,
        Err(e) => return err_result(&e),
    };

    let engine = ConsolidationEngine::new();
    let results = engine
        .run_all(&store, project_id)
        .map_err(|e| format!("{}", e));
    let results = match results {
        Ok(r) => r,
        Err(e) => return err_result(&e),
    };

    let summaries: Vec<Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "schedule": format!("{:?}", r.task.schedule),
                "from_layer": r.task.from_layer.content_type(),
                "to_layer": r.task.to_layer.content_type(),
                "processed": r.memories_processed,
                "created": r.memories_created,
                "preserved": r.memories_preserved,
                "compression_ratio": r.compression_ratio,
                "duration_ms": r.duration_ms,
            })
        })
        .collect();

    ok_result(serde_json::json!({
        "consolidation_results": summaries,
        "total_processed": results.iter().map(|r| r.memories_processed).sum::<u32>(),
        "total_created": results.iter().map(|r| r.memories_created).sum::<u32>(),
    }))
}

async fn execute_health(args: Value) -> McpResult<ToolCallResult> {
    let project_id = args["project_id"].as_str().unwrap_or("default");
    let store = match open_store(project_id) {
        Ok(s) => s,
        Err(e) => return err_result(&e),
    };

    let report = IntegrityVerifier::verify(&store, project_id).map_err(|e| format!("{}", e));
    let report = match report {
        Ok(r) => r,
        Err(e) => return err_result(&e),
    };

    ok_result(serde_json::json!({
        "database_ok": report.database_ok,
        "schema_version": report.schema_version,
        "total_memories": report.total_memories,
        "fts_synced": report.fts_synced,
        "issues": report.issues,
        "recommendations": report.recommendations,
        "latest_proof": report.latest_proof,
    }))
}

async fn execute_significance(args: Value) -> McpResult<ToolCallResult> {
    let memory_id = args["memory_id"].as_str().unwrap_or("");
    if memory_id.is_empty() {
        return err_result("memory_id is required");
    }

    let store = match open_store("default") {
        Ok(s) => s,
        Err(e) => return err_result(&e),
    };

    // Set significance if requested
    if let Some(new_sig) = args["set_significance"].as_f64() {
        if !(0.0..=1.0).contains(&new_sig) {
            return err_result("significance must be between 0.0 and 1.0");
        }
        store
            .update_significance(memory_id, new_sig)
            .map_err(|e| format!("{}", e))
            .ok();
    }

    let memory = store.get_memory(memory_id).map_err(|e| format!("{}", e));
    match memory {
        Ok(Some(m)) => ok_result(serde_json::json!({
            "memory_id": m.id,
            "significance": m.significance,
            "layer": m.layer.content_type(),
            "access_count": m.access_count,
        })),
        Ok(None) => err_result(&format!("Memory {} not found", memory_id)),
        Err(e) => err_result(&e),
    }
}

async fn execute_embedding_status(_args: Value) -> McpResult<ToolCallResult> {
    let store = match open_store("default") {
        Ok(s) => s,
        Err(e) => return err_result(&e),
    };

    let models = EmbeddingMigrator::list_models(&store).map_err(|e| format!("{}", e));
    let models = match models {
        Ok(m) => m,
        Err(e) => return err_result(&e),
    };

    let model_data: Vec<Value> = models
        .iter()
        .map(|m| {
            serde_json::json!({
                "model_id": m.model_id,
                "model_name": m.model_name,
                "dimension": m.dimension,
                "provider": m.provider,
                "is_active": m.is_active,
                "memories_count": m.memories_count,
            })
        })
        .collect();

    ok_result(serde_json::json!({
        "models": model_data,
        "active_count": models.iter().filter(|m| m.is_active).count(),
    }))
}

async fn execute_search(args: Value) -> McpResult<ToolCallResult> {
    let project_id = args["project_id"].as_str().unwrap_or("default");
    let query = args["query"].as_str().unwrap_or("");
    let limit = args["limit"].as_u64().unwrap_or(20) as u32;

    if query.is_empty() {
        return err_result("query is required");
    }

    let store = match open_store(project_id) {
        Ok(s) => s,
        Err(e) => return err_result(&e),
    };

    let results = store
        .search_fulltext(project_id, query, limit)
        .map_err(|e| format!("{}", e));
    let results = match results {
        Ok(r) => r,
        Err(e) => return err_result(&e),
    };

    let memories: Vec<Value> = results
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "layer": m.layer.content_type(),
                "content": m.content,
                "significance": m.significance,
                "created_at": m.created_at,
            })
        })
        .collect();

    ok_result(serde_json::json!({
        "query": query,
        "count": memories.len(),
        "results": memories,
    }))
}
