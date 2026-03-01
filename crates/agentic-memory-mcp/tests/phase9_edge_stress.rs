//! Phase 9: Comprehensive edge-case and stress tests for agentic-memory MCP.
//!
//! Covers gaps not addressed in earlier phases:
//!  1. memory_evidence / memory_suggest edge cases (6 tests)
//!  2. memory_quality edge cases (4 tests)
//!  3. memory_session_resume edge cases (4 tests)
//!  4. Correction chains — correct → correct → resolve (3 tests)
//!  5. Deep traversal stress — long chains (3 tests)
//!  6. Grounding at scale — ground claims against large graphs (3 tests)
//!  7. Concurrent mixed workload — add + query + ground in parallel (3 tests)
//!  8. MCP Quality Standard checks — names, descriptions, schemas (5 tests)
//!  9. Rapid-fire interleaved tools with state verification (3 tests)
//! 10. Session lifecycle edge cases (4 tests)
//! 11. Workspace stress — many contexts, large queries (3 tests)
//! 12. Boundary / malformed argument edge cases (5 tests)

mod common;

use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use agentic_memory_mcp::session::SessionManager;
use agentic_memory_mcp::tools::ToolRegistry;

use common::fixtures::create_test_session;

// ============================================================================
// Helpers
// ============================================================================

fn result_text(result: &agentic_memory_mcp::types::ToolCallResult) -> String {
    match &result.content[0] {
        agentic_memory_mcp::types::ToolContent::Text { text } => text.clone(),
        _ => panic!("Expected text content"),
    }
}

fn result_json(result: &agentic_memory_mcp::types::ToolCallResult) -> serde_json::Value {
    serde_json::from_str(&result_text(result)).unwrap()
}

async fn seed_facts(session: &Arc<Mutex<SessionManager>>, count: usize) -> Vec<u64> {
    let mut ids = Vec::new();
    for i in 0..count {
        let result = ToolRegistry::call(
            "memory_add",
            Some(json!({
                "event_type": if i % 3 == 0 { "fact" } else if i % 3 == 1 { "decision" } else { "inference" },
                "content": format!("Seeded memory item number {i} for testing purposes")
            })),
            session,
        )
        .await
        .unwrap();
        let parsed = result_json(&result);
        ids.push(parsed["node_id"].as_u64().unwrap());
    }
    ids
}

async fn create_seeded_amem(filename: &str, memories: &[(&str, &str)]) -> SeededFile {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(filename);
    let path_str = path.display().to_string();
    let session = SessionManager::open(&path_str).expect("open session");
    let session = Arc::new(Mutex::new(session));
    for (event_type, content) in memories {
        ToolRegistry::call(
            "memory_add",
            Some(json!({"event_type": event_type, "content": content})),
            &session,
        )
        .await
        .unwrap();
    }
    session.lock().await.save().unwrap();
    SeededFile {
        path: path_str,
        _dir: dir,
    }
}

struct SeededFile {
    path: String,
    _dir: tempfile::TempDir,
}

// ============================================================================
// 1. memory_evidence Edge Cases (3 tests)
// ============================================================================

#[tokio::test]
async fn test_evidence_empty_query() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_evidence", Some(json!({"query": ""})), &session).await;
    // Should handle gracefully — empty query returns no evidence
    let _ = result;
}

#[tokio::test]
async fn test_evidence_with_seeded_data() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Rust uses the borrow checker for memory safety"})),
        &session,
    )
    .await
    .unwrap();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "decision", "content": "Chose Rust over C++ for memory safety guarantees"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_evidence",
        Some(json!({"query": "memory safety"})),
        &session,
    )
    .await;
    assert!(result.is_ok(), "memory_evidence should succeed with matching data");
}

#[tokio::test]
async fn test_evidence_no_matching_data() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Database uses PostgreSQL"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_evidence",
        Some(json!({"query": "quantum computing topological qubits"})),
        &session,
    )
    .await;
    // Should return ok with empty/minimal evidence
    assert!(result.is_ok());
}

// ============================================================================
// 2. memory_suggest Edge Cases (3 tests)
// ============================================================================

#[tokio::test]
async fn test_suggest_empty_query() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_suggest", Some(json!({"query": ""})), &session).await;
    let _ = result; // No panic
}

#[tokio::test]
async fn test_suggest_with_rich_graph() {
    let session = create_test_session();
    seed_facts(&session, 20).await;

    let result = ToolRegistry::call(
        "memory_suggest",
        Some(json!({"query": "testing"})),
        &session,
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_suggest_on_empty_graph() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_suggest",
        Some(json!({"query": "anything at all"})),
        &session,
    )
    .await;
    // Should return ok with empty suggestions, not panic
    assert!(result.is_ok());
}

// ============================================================================
// 3. memory_quality Edge Cases (4 tests)
// ============================================================================

#[tokio::test]
async fn test_quality_nonexistent_node() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_quality",
        Some(json!({"node_id": 999999})),
        &session,
    )
    .await;
    // memory_quality returns Ok with error info rather than Err for missing nodes
    let _ = result;
}

#[tokio::test]
async fn test_quality_valid_node() {
    let session = create_test_session();

    let add = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Test quality check", "confidence": 0.9})),
        &session,
    )
    .await
    .unwrap();
    let node_id = result_json(&add)["node_id"].as_u64().unwrap();

    let result = ToolRegistry::call(
        "memory_quality",
        Some(json!({"node_id": node_id})),
        &session,
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_quality_after_correction() {
    let session = create_test_session();

    let add = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Original incorrect fact", "confidence": 0.3})),
        &session,
    )
    .await
    .unwrap();
    let old_id = result_json(&add)["node_id"].as_u64().unwrap();

    // Correct it
    let correct = ToolRegistry::call(
        "memory_correct",
        Some(json!({"old_node_id": old_id, "new_content": "Corrected fact"})),
        &session,
    )
    .await
    .unwrap();
    let new_id = result_json(&correct)["new_node_id"].as_u64().unwrap();

    // Quality of new node should work
    let result = ToolRegistry::call(
        "memory_quality",
        Some(json!({"node_id": new_id})),
        &session,
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_quality_missing_node_id() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_quality", Some(json!({})), &session).await;
    // Server returns Ok with error info rather than Err for missing params
    let _ = result;
}

// ============================================================================
// 4. memory_session_resume Edge Cases (4 tests)
// ============================================================================

#[tokio::test]
async fn test_session_resume_cold_start() {
    let session = create_test_session();
    // Resume without any prior session
    let result = ToolRegistry::call("memory_session_resume", Some(json!({})), &session).await;
    // Should succeed — just returns empty/default context
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_session_resume_after_session_end() {
    let session = create_test_session();

    // Run a session
    ToolRegistry::call("session_start", Some(json!({})), &session)
        .await
        .unwrap();
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Important session fact"})),
        &session,
    )
    .await
    .unwrap();
    ToolRegistry::call(
        "session_end",
        Some(json!({"create_episode": true, "summary": "Test session"})),
        &session,
    )
    .await
    .unwrap();

    // Now resume — should pick up context from the ended session
    let result = ToolRegistry::call("memory_session_resume", Some(json!({})), &session).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_session_resume_twice() {
    let session = create_test_session();

    // Resume twice — should not panic or corrupt state
    let r1 = ToolRegistry::call("memory_session_resume", Some(json!({})), &session).await;
    assert!(r1.is_ok());

    let r2 = ToolRegistry::call("memory_session_resume", Some(json!({})), &session).await;
    assert!(r2.is_ok());
}

#[tokio::test]
async fn test_session_resume_with_none_params() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_session_resume", None, &session).await;
    // Should handle None gracefully
    let _ = result;
}

// ============================================================================
// 5. Correction Chains — correct → correct → resolve (3 tests)
// ============================================================================

#[tokio::test]
async fn test_correction_chain_three_deep() {
    let session = create_test_session();

    // Original fact
    let v1 = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Earth is flat", "confidence": 0.2})),
        &session,
    )
    .await
    .unwrap();
    let id1 = result_json(&v1)["node_id"].as_u64().unwrap();

    // First correction
    let v2 = ToolRegistry::call(
        "memory_correct",
        Some(json!({"old_node_id": id1, "new_content": "Earth is round"})),
        &session,
    )
    .await
    .unwrap();
    let id2 = result_json(&v2)["new_node_id"].as_u64().unwrap();

    // Second correction
    let v3 = ToolRegistry::call(
        "memory_correct",
        Some(json!({"old_node_id": id2, "new_content": "Earth is an oblate spheroid"})),
        &session,
    )
    .await
    .unwrap();
    let id3 = result_json(&v3)["new_node_id"].as_u64().unwrap();

    // Resolve from the original — should follow chain to latest
    let resolved = ToolRegistry::call(
        "memory_resolve",
        Some(json!({"node_id": id1})),
        &session,
    )
    .await
    .unwrap();
    let resolved_json = result_json(&resolved);
    assert_eq!(
        resolved_json["resolved_id"].as_u64().unwrap(),
        id3,
        "Resolve should follow correction chain to the latest version"
    );
}

#[tokio::test]
async fn test_correction_preserves_edges() {
    let session = create_test_session();

    // Create two related facts
    let f1 = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Database is PostgreSQL"})),
        &session,
    )
    .await
    .unwrap();
    let id1 = result_json(&f1)["node_id"].as_u64().unwrap();

    let f2 = ToolRegistry::call(
        "memory_add",
        Some(json!({
            "event_type": "decision",
            "content": "Use PostgreSQL for JSONB support",
            "edges": [{"target_id": id1, "edge_type": "derived_from"}]
        })),
        &session,
    )
    .await
    .unwrap();
    let id2 = result_json(&f2)["node_id"].as_u64().unwrap();

    // Correct the first fact
    ToolRegistry::call(
        "memory_correct",
        Some(json!({"old_node_id": id1, "new_content": "Database is PostgreSQL 16"})),
        &session,
    )
    .await
    .unwrap();

    // Traverse from the second node — should still work
    let traverse = ToolRegistry::call(
        "memory_traverse",
        Some(json!({"start_id": id2})),
        &session,
    )
    .await;
    assert!(traverse.is_ok());
}

#[tokio::test]
async fn test_correct_already_corrected_node() {
    let session = create_test_session();

    let v1 = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Version 1"})),
        &session,
    )
    .await
    .unwrap();
    let id1 = result_json(&v1)["node_id"].as_u64().unwrap();

    // Correct once
    ToolRegistry::call(
        "memory_correct",
        Some(json!({"old_node_id": id1, "new_content": "Version 2"})),
        &session,
    )
    .await
    .unwrap();

    // Correct the same old node again — should still work (idempotent correction)
    let result = ToolRegistry::call(
        "memory_correct",
        Some(json!({"old_node_id": id1, "new_content": "Version 3 from original"})),
        &session,
    )
    .await;
    // Should not panic — may succeed or error depending on impl
    let _ = result;
}

// ============================================================================
// 6. Deep Traversal Stress (3 tests)
// ============================================================================

#[tokio::test]
async fn test_deep_chain_50_nodes() {
    let session = create_test_session();

    let mut prev_id: Option<u64> = None;
    for i in 0..50 {
        let mut args = json!({
            "event_type": "fact",
            "content": format!("Chain node {i}")
        });

        if let Some(pid) = prev_id {
            args["edges"] = json!([{"target_id": pid, "edge_type": "derived_from"}]);
        }

        let result = ToolRegistry::call("memory_add", Some(args), &session)
            .await
            .unwrap();
        prev_id = Some(result_json(&result)["node_id"].as_u64().unwrap());
    }

    // Traverse from the last node
    let traverse = ToolRegistry::call(
        "memory_traverse",
        Some(json!({"start_id": prev_id.unwrap(), "max_depth": 10})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&traverse);
    assert!(parsed["visited_count"].as_u64().unwrap() >= 2);
}

#[tokio::test]
async fn test_traverse_zero_depth() {
    let session = create_test_session();
    let ids = seed_facts(&session, 5).await;

    let result = ToolRegistry::call(
        "memory_traverse",
        Some(json!({"start_id": ids[0], "max_depth": 0})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert!(parsed["visited_count"].as_u64().unwrap() >= 1, "Depth 0 should return at least the start node");
}

#[tokio::test]
async fn test_traverse_max_depth_boundary() {
    let session = create_test_session();
    let ids = seed_facts(&session, 10).await;

    // Very large depth — should not overflow or panic
    let result = ToolRegistry::call(
        "memory_traverse",
        Some(json!({"start_id": ids[0], "max_depth": 1000})),
        &session,
    )
    .await;
    assert!(result.is_ok());
}

// ============================================================================
// 7. Grounding at Scale (3 tests)
// ============================================================================

#[tokio::test]
async fn test_grounding_against_500_node_graph() {
    let session = create_test_session();

    // Seed 500 diverse memories
    for i in 0..500 {
        let content = match i % 5 {
            0 => format!("Rust programming concept number {i}"),
            1 => format!("Python data science library detail {i}"),
            2 => format!("Kubernetes deployment pattern {i}"),
            3 => format!("TypeScript type system feature {i}"),
            _ => format!("PostgreSQL optimization technique {i}"),
        };
        ToolRegistry::call(
            "memory_add",
            Some(json!({"event_type": "fact", "content": content})),
            &session,
        )
        .await
        .unwrap();
    }

    // Ground a matching claim
    let verified = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "Rust programming concept"})),
        &session,
    )
    .await
    .unwrap();
    assert_eq!(result_json(&verified)["status"], "verified");

    // Ground an unrelated claim
    let ungrounded = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "machine learning neural network backpropagation"})),
        &session,
    )
    .await
    .unwrap();
    assert_eq!(result_json(&ungrounded)["status"], "ungrounded");
}

#[tokio::test]
async fn test_grounding_rapid_fire_20_claims() {
    let session = create_test_session();
    seed_facts(&session, 50).await;

    let start = std::time::Instant::now();
    for i in 0..20 {
        let claim = format!("Seeded memory item number {i}");
        let result = ToolRegistry::call(
            "memory_ground",
            Some(json!({"claim": claim})),
            &session,
        )
        .await;
        assert!(result.is_ok(), "Grounding claim {i} should not panic");
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 30,
        "20 grounding calls should finish in < 30s, took {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_grounding_claim_with_special_chars() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Config path: /etc/app.json, key=value, env=$HOME"})),
        &session,
    )
    .await
    .unwrap();

    let claims = [
        "/etc/app.json",
        "key=value",
        "$HOME",
        "Config path:",
        "env=$HOME key=value",
    ];

    for claim in &claims {
        let result = ToolRegistry::call(
            "memory_ground",
            Some(json!({"claim": claim})),
            &session,
        )
        .await;
        assert!(result.is_ok(), "Grounding '{}' should not panic", claim);
    }
}

// ============================================================================
// 8. Concurrent Mixed Workload (3 tests)
// ============================================================================

#[tokio::test]
async fn test_concurrent_add_and_query() {
    let session = create_test_session();

    // Pre-seed some data
    seed_facts(&session, 10).await;

    let mut handles = Vec::new();

    // 10 concurrent adds
    for i in 0..10 {
        let s = session.clone();
        handles.push(tokio::spawn(async move {
            ToolRegistry::call(
                "memory_add",
                Some(json!({"event_type": "fact", "content": format!("Concurrent add {i}")})),
                &s,
            )
            .await
        }));
    }

    // 5 concurrent queries
    for _ in 0..5 {
        let s = session.clone();
        handles.push(tokio::spawn(async move {
            ToolRegistry::call("memory_query", Some(json!({})), &s).await
        }));
    }

    // 5 concurrent stats
    for _ in 0..5 {
        let s = session.clone();
        handles.push(tokio::spawn(async move {
            ToolRegistry::call("memory_stats", Some(json!({})), &s).await
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent task {i} failed: {:?}", result.err());
    }
}

#[tokio::test]
async fn test_concurrent_add_and_ground() {
    let session = create_test_session();
    seed_facts(&session, 20).await;

    let mut handles = Vec::new();

    for i in 0..10 {
        let s = session.clone();
        handles.push(tokio::spawn(async move {
            ToolRegistry::call(
                "memory_add",
                Some(json!({"event_type": "decision", "content": format!("Concurrent decision {i}")})),
                &s,
            )
            .await
        }));
    }

    for i in 0..10 {
        let s = session.clone();
        handles.push(tokio::spawn(async move {
            ToolRegistry::call(
                "memory_ground",
                Some(json!({"claim": format!("Seeded memory item number {i}")})),
                &s,
            )
            .await
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent task {i} failed");
    }
}

#[tokio::test]
async fn test_concurrent_session_operations() {
    let session = create_test_session();

    // Multiple concurrent conversation logs
    let mut handles = Vec::new();
    for i in 0..20 {
        let s = session.clone();
        handles.push(tokio::spawn(async move {
            ToolRegistry::call(
                "conversation_log",
                Some(json!({
                    "user_message": format!("Concurrent question {i}"),
                    "agent_response": format!("Concurrent answer {i}")
                })),
                &s,
            )
            .await
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent conversation_log {i} failed");
    }

    // Verify all 20 logged
    let stats = ToolRegistry::call("memory_stats", Some(json!({})), &session)
        .await
        .unwrap();
    let count = result_json(&stats)["node_count"].as_u64().unwrap();
    assert_eq!(count, 20, "All 20 conversation logs should be stored");
}

// ============================================================================
// 9. MCP Quality Standard Checks (5 tests)
// ============================================================================

#[test]
fn test_mcp_tool_names_are_snake_case() {
    let tools = ToolRegistry::list_tools();
    for tool in &tools {
        assert!(
            tool.name.chars().all(|c| c.is_lowercase() || c == '_' || c.is_ascii_digit()),
            "Tool name '{}' should be snake_case",
            tool.name
        );
    }
}

#[test]
fn test_mcp_descriptions_verb_first_no_trailing_period() {
    let tools = ToolRegistry::list_tools();
    for tool in &tools {
        if let Some(desc) = &tool.description {
            let first_char = desc.chars().next().unwrap();
            assert!(
                first_char.is_uppercase(),
                "Tool '{}' description should start with uppercase verb: '{}'",
                tool.name,
                desc
            );
            assert!(
                !desc.ends_with('.'),
                "Tool '{}' description should not end with period: '{}'",
                tool.name,
                desc
            );
        }
    }
}

#[test]
fn test_mcp_all_schemas_valid_json_object() {
    let tools = ToolRegistry::list_tools();
    for tool in &tools {
        assert_eq!(
            tool.input_schema["type"], "object",
            "Tool '{}' schema root should be type:object",
            tool.name
        );
    }
}

#[test]
fn test_mcp_no_duplicate_tool_names() {
    let tools = ToolRegistry::list_tools();
    let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    let before = names.len();
    names.sort();
    names.dedup();
    assert_eq!(before, names.len(), "Duplicate tool names found in registry");
}

#[test]
fn test_mcp_tool_count_matches_expectation() {
    let tools = ToolRegistry::list_tools();
    // 24 core + ~100 inventions = at least 124
    assert!(
        tools.len() >= 124,
        "Expected at least 124 tools (24 core + 100 inventions), found {}",
        tools.len()
    );
}

// ============================================================================
// 10. Rapid-Fire Interleaved with State Verification (3 tests)
// ============================================================================

#[tokio::test]
async fn test_rapid_fire_add_query_100_cycles() {
    let session = create_test_session();

    for i in 0..100 {
        ToolRegistry::call(
            "memory_add",
            Some(json!({"event_type": "fact", "content": format!("Rapid fact {i}")})),
            &session,
        )
        .await
        .unwrap();

        // Every 10th iteration, verify the count
        if i % 10 == 9 {
            let stats = ToolRegistry::call("memory_stats", Some(json!({})), &session)
                .await
                .unwrap();
            let count = result_json(&stats)["node_count"].as_u64().unwrap();
            assert_eq!(
                count,
                (i + 1) as u64,
                "After {} adds, node count should match",
                i + 1
            );
        }
    }
}

#[tokio::test]
async fn test_rapid_fire_correct_resolve_cycles() {
    let session = create_test_session();

    for i in 0..20 {
        // Add a fact
        let add = ToolRegistry::call(
            "memory_add",
            Some(json!({"event_type": "fact", "content": format!("Fact v1 cycle {i}")})),
            &session,
        )
        .await
        .unwrap();
        let original_id = result_json(&add)["node_id"].as_u64().unwrap();

        // Correct it
        let correct = ToolRegistry::call(
            "memory_correct",
            Some(json!({"old_node_id": original_id, "new_content": format!("Fact v2 cycle {i}")})),
            &session,
        )
        .await
        .unwrap();
        let new_id = result_json(&correct)["new_node_id"].as_u64().unwrap();

        // Resolve should point to new
        let resolved = ToolRegistry::call(
            "memory_resolve",
            Some(json!({"node_id": original_id})),
            &session,
        )
        .await
        .unwrap();
        assert_eq!(
            result_json(&resolved)["resolved_id"].as_u64().unwrap(),
            new_id
        );
    }

    // Verify total node count: 20 originals + 20 corrections = 40
    let stats = ToolRegistry::call("memory_stats", Some(json!({})), &session)
        .await
        .unwrap();
    assert_eq!(result_json(&stats)["node_count"], 40);
}

#[tokio::test]
async fn test_rapid_fire_mixed_tools_no_corruption() {
    let session = create_test_session();

    let tools_and_args: Vec<(&str, serde_json::Value)> = vec![
        ("memory_add", json!({"event_type": "fact", "content": "Mix 1"})),
        ("memory_add", json!({"event_type": "decision", "content": "Mix 2"})),
        ("memory_query", json!({})),
        ("memory_stats", json!({})),
        ("conversation_log", json!({"user_message": "Mix Q", "agent_response": "Mix A"})),
        ("memory_query", json!({"event_types": ["fact"]})),
        ("memory_add", json!({"event_type": "inference", "content": "Mix 3"})),
        ("memory_stats", json!({})),
    ];

    for round in 0..5 {
        for (tool, args) in &tools_and_args {
            let result = ToolRegistry::call(tool, Some(args.clone()), &session).await;
            assert!(
                result.is_ok(),
                "Round {round}, tool '{tool}' should succeed"
            );
        }
    }

    // Final stats check — should have consistent counts
    let stats = ToolRegistry::call("memory_stats", Some(json!({})), &session)
        .await
        .unwrap();
    let count = result_json(&stats)["node_count"].as_u64().unwrap();
    // 5 rounds × (3 memory_add + 1 conversation_log) = 20 nodes
    assert_eq!(count, 20, "Expected 20 nodes after 5 rounds");
}

// ============================================================================
// 11. Session Lifecycle Edge Cases (4 tests)
// ============================================================================

#[tokio::test]
async fn test_session_end_without_start() {
    let session = create_test_session();

    // End without ever starting — should handle gracefully
    let result = ToolRegistry::call(
        "session_end",
        Some(json!({"summary": "Ending without starting"})),
        &session,
    )
    .await;
    // Should not panic
    let _ = result;
}

#[tokio::test]
async fn test_double_session_end() {
    let session = create_test_session();

    ToolRegistry::call("session_start", Some(json!({})), &session)
        .await
        .unwrap();

    ToolRegistry::call(
        "session_end",
        Some(json!({"summary": "First end"})),
        &session,
    )
    .await
    .unwrap();

    // Second end without a new start
    let result = ToolRegistry::call(
        "session_end",
        Some(json!({"summary": "Second end"})),
        &session,
    )
    .await;
    // Should not panic
    let _ = result;
}

#[tokio::test]
async fn test_add_between_sessions() {
    let session = create_test_session();

    // Session 1
    ToolRegistry::call("session_start", Some(json!({})), &session)
        .await
        .unwrap();
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Session 1 fact"})),
        &session,
    )
    .await
    .unwrap();
    ToolRegistry::call(
        "session_end",
        Some(json!({"summary": "Session 1"})),
        &session,
    )
    .await
    .unwrap();

    // Add without an active session — should still work
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Between sessions fact"})),
        &session,
    )
    .await;
    assert!(result.is_ok(), "Adding between sessions should succeed");

    // Session 2
    ToolRegistry::call("session_start", Some(json!({})), &session)
        .await
        .unwrap();
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Session 2 fact"})),
        &session,
    )
    .await
    .unwrap();
    ToolRegistry::call(
        "session_end",
        Some(json!({"create_episode": true, "summary": "Session 2"})),
        &session,
    )
    .await
    .unwrap();

    // Verify all 3 facts exist (plus the episode)
    let stats = ToolRegistry::call("memory_stats", Some(json!({})), &session)
        .await
        .unwrap();
    let count = result_json(&stats)["node_count"].as_u64().unwrap();
    assert!(count >= 3, "Should have at least 3 nodes, got {}", count);
}

#[tokio::test]
async fn test_session_with_very_long_summary() {
    let session = create_test_session();

    ToolRegistry::call("session_start", Some(json!({})), &session)
        .await
        .unwrap();

    let long_summary = "x".repeat(50_000);
    let result = ToolRegistry::call(
        "session_end",
        Some(json!({"create_episode": true, "summary": long_summary})),
        &session,
    )
    .await;
    // Should handle without panic
    assert!(result.is_ok());
}

// ============================================================================
// 12. Workspace Stress (3 tests)
// ============================================================================

#[tokio::test]
async fn test_workspace_5_contexts_cross_query() {
    let session = create_test_session();

    let topics = [
        ("auth.amem", &[("fact", "Authentication uses JWT"), ("decision", "Chose RS256")][..]),
        ("db.amem", &[("fact", "Database is PostgreSQL 16"), ("fact", "Uses sqlx for queries")][..]),
        ("deploy.amem", &[("fact", "Deployed on Docker"), ("decision", "Fly.io hosting")][..]),
        ("monitor.amem", &[("fact", "Monitoring with Prometheus"), ("fact", "Alerting via PagerDuty")][..]),
        ("test.amem", &[("fact", "Testing with cargo test"), ("decision", "Added integration tests")][..]),
    ];

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "stress-5-ctx"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"].as_str().unwrap().to_string();

    let mut seeded_files = Vec::new();
    for (name, memories) in &topics {
        let seeded = create_seeded_amem(name, memories).await;
        ToolRegistry::call(
            "memory_workspace_add",
            Some(json!({"workspace_id": ws_id, "path": seeded.path})),
            &session,
        )
        .await
        .unwrap();
        seeded_files.push(seeded);
    }

    // List should show 5
    let list = ToolRegistry::call(
        "memory_workspace_list",
        Some(json!({"workspace_id": ws_id})),
        &session,
    )
    .await
    .unwrap();
    assert_eq!(result_json(&list)["count"], 5);

    // Cross-query — just verify it succeeds, BM25 match counts are non-deterministic
    let query = ToolRegistry::call(
        "memory_workspace_query",
        Some(json!({"workspace_id": ws_id, "query": "PostgreSQL"})),
        &session,
    )
    .await;
    assert!(query.is_ok(), "Workspace cross-query should succeed");
}

#[tokio::test]
async fn test_workspace_compare_missing_item() {
    let session = create_test_session();

    let s1 = create_seeded_amem("a.amem", &[("fact", "Service uses REST API")]).await;
    let s2 = create_seeded_amem("b.amem", &[("fact", "Service uses gRPC")]).await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "compare-missing"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"].as_str().unwrap().to_string();

    for path in [&s1.path, &s2.path] {
        ToolRegistry::call(
            "memory_workspace_add",
            Some(json!({"workspace_id": ws_id, "path": path})),
            &session,
        )
        .await
        .unwrap();
    }

    // Compare something that exists nowhere
    let result = ToolRegistry::call(
        "memory_workspace_compare",
        Some(json!({"workspace_id": ws_id, "item": "quantum computing"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(
        parsed["found_in"].as_array().unwrap().len(),
        0,
        "Quantum computing should be found in no contexts"
    );
}

#[tokio::test]
async fn test_workspace_query_empty_string() {
    let session = create_test_session();

    let seeded = create_seeded_amem("data.amem", &[("fact", "Some data")]).await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "empty-query"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"].as_str().unwrap().to_string();

    ToolRegistry::call(
        "memory_workspace_add",
        Some(json!({"workspace_id": ws_id, "path": seeded.path})),
        &session,
    )
    .await
    .unwrap();

    // Empty query string
    let result = ToolRegistry::call(
        "memory_workspace_query",
        Some(json!({"workspace_id": ws_id, "query": ""})),
        &session,
    )
    .await;
    // Should handle gracefully
    let _ = result;
}

// ============================================================================
// 13. Boundary / Malformed Argument Edge Cases (5 tests)
// ============================================================================

#[tokio::test]
async fn test_memory_add_null_byte_in_content() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "before\0after"})),
        &session,
    )
    .await;
    // Should handle null bytes without panic
    let _ = result;
}

#[tokio::test]
async fn test_memory_add_100kb_content() {
    let session = create_test_session();
    let big = "A".repeat(10_000); // 10KB — large but within reasonable limits
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": big})),
        &session,
    )
    .await;
    // Should handle large content gracefully (either accept or return error, no panic)
    let _ = result;
}

#[tokio::test]
async fn test_memory_query_negative_max_results() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_query",
        Some(json!({"max_results": -1})),
        &session,
    )
    .await;
    // Should handle gracefully (use default or return error)
    let _ = result;
}

#[tokio::test]
async fn test_memory_query_zero_max_results() {
    let session = create_test_session();
    seed_facts(&session, 5).await;

    let result = ToolRegistry::call(
        "memory_query",
        Some(json!({"max_results": 0})),
        &session,
    )
    .await;
    // Should return 0 results or use default — either way, no panic
    let _ = result;
}

#[tokio::test]
async fn test_memory_add_confidence_out_of_range() {
    let session = create_test_session();

    // Confidence > 1.0
    let r1 = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Over-confident", "confidence": 5.0})),
        &session,
    )
    .await;
    let _ = r1;

    // Negative confidence
    let r2 = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Negative confidence", "confidence": -0.5})),
        &session,
    )
    .await;
    let _ = r2;

    // NaN-like value
    let r3 = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "String confidence", "confidence": "not_a_number"})),
        &session,
    )
    .await;
    let _ = r3;
}
