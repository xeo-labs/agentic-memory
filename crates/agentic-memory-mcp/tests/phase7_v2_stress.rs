//! Phase 7: V2 stress tests — grounding (anti-hallucination) and multi-context workspaces.
//!
//! Tests: memory_ground (12), memory_workspace_* (13), integration (5) — 30 total.

mod common;

use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use agentic_memory_mcp::session::SessionManager;
use agentic_memory_mcp::tools::ToolRegistry;

use common::fixtures::create_test_session;

fn result_text(result: &agentic_memory_mcp::types::ToolCallResult) -> String {
    match &result.content[0] {
        agentic_memory_mcp::types::ToolContent::Text { text } => text.clone(),
        _ => panic!("Expected text content"),
    }
}

fn result_json(result: &agentic_memory_mcp::types::ToolCallResult) -> serde_json::Value {
    serde_json::from_str(&result_text(result)).unwrap()
}

struct SeededFile {
    path: String,
    _dir: tempfile::TempDir,
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

// ============================================================================
// 1. Grounding — memory_ground (12 tests)
// ============================================================================

#[tokio::test]
async fn test_grounding_verified_fact() {
    let session = create_test_session();

    // Seed a fact
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "User prefers dark mode"})),
        &session,
    )
    .await
    .unwrap();

    // Ground the same claim
    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "User prefers dark mode"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["status"], "verified");
    assert!(
        !parsed["evidence"].as_array().unwrap().is_empty(),
        "Evidence should be non-empty for a verified claim"
    );
    assert!(parsed["confidence"].as_f64().unwrap() > 0.0);
    assert!(parsed["evidence_count"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_grounding_verified_decision() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "decision", "content": "Decided to use Axum framework"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "decision was Axum framework"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["status"], "verified");
    assert!(!parsed["evidence"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_grounding_verified_preference() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "User prefers tabs over spaces for indentation"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "tabs over spaces for indentation"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["status"], "verified");
    assert!(parsed["evidence_count"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn test_grounding_ungrounded_no_data() {
    let session = create_test_session();

    // Empty session — nothing stored
    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "User prefers light mode"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["status"], "ungrounded");
}

#[tokio::test]
async fn test_grounding_ungrounded_wrong_topic() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Rust is a systems programming language with memory safety"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "User enjoys mountain biking on weekends"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["status"], "ungrounded");
}

#[tokio::test]
async fn test_grounding_partial_overlap() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Deployment uses Docker containers on AWS ECS"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "Deployment uses Docker containers with Kubernetes on GCP"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    // Partial overlap may be verified (BM25 matches on shared terms) or ungrounded
    let status = parsed["status"].as_str().unwrap();
    assert!(
        status == "verified" || status == "ungrounded",
        "Expected verified or ungrounded, got: {}",
        status
    );
}

#[tokio::test]
async fn test_grounding_empty_claim() {
    let session = create_test_session();

    let result = ToolRegistry::call("memory_ground", Some(json!({"claim": ""})), &session)
        .await
        .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["status"], "ungrounded");
    assert_eq!(parsed["reason"], "Empty claim");
}

#[tokio::test]
async fn test_grounding_long_claim() {
    let session = create_test_session();

    // Seed some data so the graph is not empty
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "The project uses Rust"})),
        &session,
    )
    .await
    .unwrap();

    // Build a 1000+ character claim
    let long_claim = "The user mentioned that ".to_string() + &"important detail ".repeat(70);
    assert!(long_claim.len() > 1000);

    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": long_claim})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    let status = parsed["status"].as_str().unwrap();
    assert!(
        status == "verified" || status == "ungrounded",
        "Should return a valid status, got: {}",
        status
    );
}

#[tokio::test]
async fn test_grounding_unicode_content() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Chinese: \u{7528}\u{6237}\u{504F}\u{597D}\u{6697}\u{8272}\u{6A21}\u{5F0F}"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "\u{7528}\u{6237}\u{504F}\u{597D}\u{6697}\u{8272}\u{6A21}\u{5F0F}"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    let status = parsed["status"].as_str().unwrap();
    assert!(
        status == "verified" || status == "ungrounded",
        "Unicode claim should not panic, got status: {}",
        status
    );
}

#[tokio::test]
async fn test_grounding_special_chars() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Config lives at /etc/config.json with key=value pairs"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "/etc/config.json with key=value"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    let status = parsed["status"].as_str().unwrap();
    assert!(
        status == "verified" || status == "ungrounded",
        "Special chars should not panic, got status: {}",
        status
    );
}

#[tokio::test]
async fn test_grounding_case_insensitive() {
    let session = create_test_session();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "USER PREFERS DARK MODE ALWAYS"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "user prefers dark mode always"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(
        parsed["status"], "verified",
        "BM25 should match case-insensitively"
    );
}

#[tokio::test]
async fn test_grounding_multiple_evidence() {
    let session = create_test_session();

    // Add 3 related memories about Rust
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "User prefers dark mode for the editor interface and terminal"})),
        &session,
    )
    .await
    .unwrap();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "decision", "content": "User set dark mode as default for all new projects"})),
        &session,
    )
    .await
    .unwrap();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Dark mode preference saved in user settings profile"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "User prefers dark mode for the editor interface"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["status"], "verified");
    assert!(
        parsed["evidence_count"].as_u64().unwrap() >= 2,
        "Should find multiple evidence nodes for a well-supported claim, got: {}",
        parsed["evidence_count"]
    );
}

// ============================================================================
// 2. Workspace — memory_workspace_* (13 tests)
// ============================================================================

#[tokio::test]
async fn test_workspace_create() {
    let session = create_test_session();

    let result = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "test-workspace"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["status"], "created");
    assert_eq!(parsed["name"], "test-workspace");

    let ws_id = parsed["workspace_id"].as_str().unwrap();
    assert!(
        ws_id.starts_with("ws_"),
        "Workspace ID should start with ws_, got: {}",
        ws_id
    );
}

#[tokio::test]
async fn test_workspace_create_multiple() {
    let session = create_test_session();

    let mut ids = Vec::new();
    for i in 0..3 {
        let result = ToolRegistry::call(
            "memory_workspace_create",
            Some(json!({"name": format!("workspace-{i}")})),
            &session,
        )
        .await
        .unwrap();

        let parsed = result_json(&result);
        ids.push(parsed["workspace_id"].as_str().unwrap().to_string());
    }

    // All IDs must be unique
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 3, "All workspace IDs should be unique");
}

#[tokio::test]
async fn test_workspace_add_context() {
    let session = create_test_session();

    // Create a seeded .amem file
    let seeded = create_seeded_amem(
        "ctx.amem",
        &[
            ("fact", "OAuth requires client_id and client_secret"),
            ("decision", "Use PKCE flow for public clients"),
        ],
    )
    .await;

    // Create workspace
    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "auth-workspace"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Add context
    let result = ToolRegistry::call(
        "memory_workspace_add",
        Some(json!({
            "workspace_id": ws_id,
            "path": seeded.path,
            "role": "primary",
            "label": "auth-context"
        })),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["status"], "added");
    assert!(parsed["context_id"].as_str().is_some());
    assert_eq!(parsed["role"], "primary");
}

#[tokio::test]
async fn test_workspace_add_multiple_contexts() {
    let session = create_test_session();

    let s1 = create_seeded_amem(
        "primary.amem",
        &[("fact", "Frontend uses React with TypeScript")],
    )
    .await;
    let s2 = create_seeded_amem("secondary.amem", &[("fact", "Backend uses Axum with Rust")]).await;
    let s3 = create_seeded_amem(
        "reference.amem",
        &[("fact", "API spec follows OpenAPI 3.1")],
    )
    .await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "multi-context"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    let roles = ["primary", "secondary", "reference"];
    let paths = [&s1.path, &s2.path, &s3.path];

    for (i, (path, role)) in paths.iter().zip(roles.iter()).enumerate() {
        let result = ToolRegistry::call(
            "memory_workspace_add",
            Some(json!({
                "workspace_id": ws_id,
                "path": path,
                "role": role,
                "label": format!("context-{i}")
            })),
            &session,
        )
        .await
        .unwrap();

        let parsed = result_json(&result);
        assert_eq!(parsed["status"], "added");
        assert_eq!(parsed["role"], *role);
    }
}

#[tokio::test]
async fn test_workspace_list() {
    let session = create_test_session();

    let s1 = create_seeded_amem("a.amem", &[("fact", "Fact A")]).await;
    let s2 = create_seeded_amem("b.amem", &[("fact", "Fact B")]).await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "list-test"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    ToolRegistry::call(
        "memory_workspace_add",
        Some(json!({"workspace_id": ws_id, "path": s1.path})),
        &session,
    )
    .await
    .unwrap();

    ToolRegistry::call(
        "memory_workspace_add",
        Some(json!({"workspace_id": ws_id, "path": s2.path})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_workspace_list",
        Some(json!({"workspace_id": ws_id})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["count"], 2);
    assert_eq!(parsed["contexts"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_workspace_query_single_context() {
    let session = create_test_session();

    let seeded = create_seeded_amem(
        "oauth.amem",
        &[
            ("fact", "OAuth 2.0 requires client_id and redirect_uri"),
            ("decision", "Use authorization code flow with PKCE"),
            ("fact", "Access tokens expire after 3600 seconds"),
        ],
    )
    .await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "oauth-ws"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    ToolRegistry::call(
        "memory_workspace_add",
        Some(json!({"workspace_id": ws_id, "path": seeded.path, "role": "primary"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_workspace_query",
        Some(json!({"workspace_id": ws_id, "query": "OAuth"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert!(
        parsed["total_matches"].as_u64().unwrap() >= 1,
        "Should find at least 1 match for OAuth query"
    );
}

#[tokio::test]
async fn test_workspace_query_across_contexts() {
    let session = create_test_session();

    let s1 = create_seeded_amem(
        "frontend.amem",
        &[
            ("fact", "Frontend uses TypeScript with React"),
            ("decision", "Chose TypeScript over plain JavaScript"),
        ],
    )
    .await;
    let s2 = create_seeded_amem(
        "backend.amem",
        &[("fact", "Backend types generated from TypeScript interfaces")],
    )
    .await;
    let s3 = create_seeded_amem(
        "tooling.amem",
        &[(
            "fact",
            "Build pipeline uses esbuild for TypeScript compilation",
        )],
    )
    .await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "cross-query"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    for path in [&s1.path, &s2.path, &s3.path] {
        ToolRegistry::call(
            "memory_workspace_add",
            Some(json!({"workspace_id": ws_id, "path": path})),
            &session,
        )
        .await
        .unwrap();
    }

    let result = ToolRegistry::call(
        "memory_workspace_query",
        Some(json!({"workspace_id": ws_id, "query": "TypeScript"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert!(
        parsed["total_matches"].as_u64().unwrap() >= 2,
        "TypeScript should appear in multiple contexts"
    );
    assert!(
        parsed["contexts_searched"].as_u64().unwrap() >= 2,
        "Should search across all loaded contexts"
    );
}

#[tokio::test]
async fn test_workspace_compare_found_both() {
    let session = create_test_session();

    let s1 = create_seeded_amem(
        "dev.amem",
        &[("fact", "Development uses Docker Compose for local setup")],
    )
    .await;
    let s2 = create_seeded_amem(
        "prod.amem",
        &[("fact", "Production uses Docker containers on ECS")],
    )
    .await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "compare-both"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    for path in [&s1.path, &s2.path] {
        ToolRegistry::call(
            "memory_workspace_add",
            Some(json!({"workspace_id": ws_id, "path": path})),
            &session,
        )
        .await
        .unwrap();
    }

    let result = ToolRegistry::call(
        "memory_workspace_compare",
        Some(json!({"workspace_id": ws_id, "item": "Docker"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(
        parsed["found_in"].as_array().unwrap().len(),
        2,
        "Docker should be found in both contexts"
    );
}

#[tokio::test]
async fn test_workspace_compare_found_one() {
    let session = create_test_session();

    let s1 = create_seeded_amem(
        "api.amem",
        &[("fact", "API uses GraphQL with Apollo Server")],
    )
    .await;
    let s2 = create_seeded_amem("cli.amem", &[("fact", "CLI tool built with clap in Rust")]).await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "compare-one"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    for path in [&s1.path, &s2.path] {
        ToolRegistry::call(
            "memory_workspace_add",
            Some(json!({"workspace_id": ws_id, "path": path})),
            &session,
        )
        .await
        .unwrap();
    }

    let result = ToolRegistry::call(
        "memory_workspace_compare",
        Some(json!({"workspace_id": ws_id, "item": "GraphQL"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(
        parsed["found_in"].as_array().unwrap().len(),
        1,
        "GraphQL should only be in one context"
    );
    assert_eq!(
        parsed["missing_from"].as_array().unwrap().len(),
        1,
        "GraphQL should be missing from one context"
    );
}

#[tokio::test]
async fn test_workspace_xref() {
    let session = create_test_session();

    let s1 = create_seeded_amem(
        "svc-a.amem",
        &[(
            "fact",
            "Service A uses structured logging with tracing crate",
        )],
    )
    .await;
    let s2 = create_seeded_amem(
        "svc-b.amem",
        &[("fact", "Service B uses logging via log4j")],
    )
    .await;
    let s3 = create_seeded_amem(
        "svc-c.amem",
        &[("fact", "Service C has no logging configured yet")],
    )
    .await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "xref-test"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    for path in [&s1.path, &s2.path, &s3.path] {
        ToolRegistry::call(
            "memory_workspace_add",
            Some(json!({"workspace_id": ws_id, "path": path})),
            &session,
        )
        .await
        .unwrap();
    }

    let result = ToolRegistry::call(
        "memory_workspace_xref",
        Some(json!({"workspace_id": ws_id, "item": "logging"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert!(parsed["present_in"].as_array().is_some());
    assert!(parsed["absent_from"].as_array().is_some());
    assert!(
        parsed["coverage"].as_str().is_some(),
        "Should include coverage ratio"
    );
}

#[tokio::test]
async fn test_workspace_empty() {
    let session = create_test_session();

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "empty-ws"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Query an empty workspace (no contexts added)
    let result = ToolRegistry::call(
        "memory_workspace_query",
        Some(json!({"workspace_id": ws_id, "query": "anything"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["total_matches"], 0);
}

#[tokio::test]
async fn test_workspace_missing_id() {
    let session = create_test_session();

    let result = ToolRegistry::call(
        "memory_workspace_list",
        Some(json!({"workspace_id": "ws_nonexistent_99999"})),
        &session,
    )
    .await;

    assert!(
        result.is_err(),
        "Non-existent workspace ID should return error"
    );
}

#[tokio::test]
async fn test_workspace_add_invalid_path() {
    let session = create_test_session();

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "invalid-path-ws"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    let result = ToolRegistry::call(
        "memory_workspace_add",
        Some(json!({
            "workspace_id": ws_id,
            "path": "/nonexistent/path/to/nowhere.amem"
        })),
        &session,
    )
    .await;

    assert!(result.is_err(), "Invalid path should return error");
}

// ============================================================================
// 3. Integration — cross-feature (5 tests)
// ============================================================================

#[tokio::test]
async fn test_ground_then_workspace() {
    let session = create_test_session();

    // Seed memories in the main session
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Project uses PostgreSQL database"})),
        &session,
    )
    .await
    .unwrap();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "decision", "content": "Chose PostgreSQL over MySQL for JSONB support"})),
        &session,
    )
    .await
    .unwrap();

    // Ground a claim first
    let ground_result = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "Project uses PostgreSQL"})),
        &session,
    )
    .await
    .unwrap();

    let ground_parsed = result_json(&ground_result);
    assert_eq!(ground_parsed["status"], "verified");

    // Save the session to disk
    {
        let mut sess = session.lock().await;
        sess.save().unwrap();
    }

    // Now create a workspace and add the saved file
    let file_path = {
        let sess = session.lock().await;
        sess.file_path().display().to_string()
    };

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "ground-then-ws"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    ToolRegistry::call(
        "memory_workspace_add",
        Some(json!({"workspace_id": ws_id, "path": file_path})),
        &session,
    )
    .await
    .unwrap();

    // Query the workspace
    let query_result = ToolRegistry::call(
        "memory_workspace_query",
        Some(json!({"workspace_id": ws_id, "query": "PostgreSQL"})),
        &session,
    )
    .await
    .unwrap();

    let query_parsed = result_json(&query_result);
    assert!(
        query_parsed["total_matches"].as_u64().unwrap() >= 1,
        "Workspace query should find PostgreSQL memories"
    );
}

#[tokio::test]
async fn test_workspace_cross_project_query() {
    let session = create_test_session();

    let frontend = create_seeded_amem(
        "frontend-project.amem",
        &[
            ("fact", "Frontend uses Jest for testing React components"),
            ("fact", "Frontend uses Cypress for end-to-end testing"),
            (
                "decision",
                "Chose Vitest over Jest for faster test execution",
            ),
        ],
    )
    .await;

    let backend = create_seeded_amem(
        "backend-project.amem",
        &[
            ("fact", "Backend uses cargo test for unit testing"),
            (
                "fact",
                "Backend uses integration testing with testcontainers",
            ),
            (
                "decision",
                "Added property-based testing with proptest crate",
            ),
        ],
    )
    .await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "cross-project"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    ToolRegistry::call(
        "memory_workspace_add",
        Some(json!({"workspace_id": ws_id, "path": frontend.path, "label": "frontend"})),
        &session,
    )
    .await
    .unwrap();

    ToolRegistry::call(
        "memory_workspace_add",
        Some(json!({"workspace_id": ws_id, "path": backend.path, "label": "backend"})),
        &session,
    )
    .await
    .unwrap();

    // Query a shared topic
    let result = ToolRegistry::call(
        "memory_workspace_query",
        Some(json!({"workspace_id": ws_id, "query": "testing"})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert!(
        parsed["total_matches"].as_u64().unwrap() >= 2,
        "Testing should appear in both projects"
    );
}

#[tokio::test]
async fn test_grounding_with_many_memories() {
    let session = create_test_session();

    // Seed 50+ memories
    for i in 0..55 {
        let event_type = match i % 3 {
            0 => "fact",
            1 => "decision",
            _ => "inference",
        };
        let content = if i % 10 == 0 {
            format!("Rust memory safety feature number {i}")
        } else {
            format!("Generic project detail number {i} about the codebase architecture")
        };

        ToolRegistry::call(
            "memory_add",
            Some(json!({"event_type": event_type, "content": content})),
            &session,
        )
        .await
        .unwrap();
    }

    // Ground a claim that matches seeded Rust memories
    let verified = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "Rust memory safety feature"})),
        &session,
    )
    .await
    .unwrap();

    let v = result_json(&verified);
    assert_eq!(v["status"], "verified");

    // Ground a completely unrelated claim
    let ungrounded = ToolRegistry::call(
        "memory_ground",
        Some(json!({"claim": "quantum computing with topological qubits"})),
        &session,
    )
    .await
    .unwrap();

    let u = result_json(&ungrounded);
    assert_eq!(u["status"], "ungrounded");
}

#[tokio::test]
async fn test_workspace_role_filtering() {
    let session = create_test_session();

    let s_primary =
        create_seeded_amem("primary-role.amem", &[("fact", "Primary context data")]).await;
    let s_secondary =
        create_seeded_amem("secondary-role.amem", &[("fact", "Secondary context data")]).await;
    let s_reference =
        create_seeded_amem("reference-role.amem", &[("fact", "Reference context data")]).await;
    let s_archive =
        create_seeded_amem("archive-role.amem", &[("fact", "Archive context data")]).await;

    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "role-filter"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();

    let pairs = [
        (&s_primary.path, "primary"),
        (&s_secondary.path, "secondary"),
        (&s_reference.path, "reference"),
        (&s_archive.path, "archive"),
    ];

    for (path, role) in &pairs {
        ToolRegistry::call(
            "memory_workspace_add",
            Some(json!({
                "workspace_id": ws_id,
                "path": path,
                "role": role,
                "label": format!("{}-label", role)
            })),
            &session,
        )
        .await
        .unwrap();
    }

    // List and verify all 4 contexts present with correct roles
    let result = ToolRegistry::call(
        "memory_workspace_list",
        Some(json!({"workspace_id": ws_id})),
        &session,
    )
    .await
    .unwrap();

    let parsed = result_json(&result);
    assert_eq!(parsed["count"], 4);

    let contexts = parsed["contexts"].as_array().unwrap();
    let roles_found: Vec<&str> = contexts
        .iter()
        .map(|c| c["role"].as_str().unwrap())
        .collect();

    assert!(roles_found.contains(&"primary"));
    assert!(roles_found.contains(&"secondary"));
    assert!(roles_found.contains(&"reference"));
    assert!(roles_found.contains(&"archive"));
}

#[tokio::test]
async fn test_full_workflow() {
    let session = create_test_session();

    // Step 1: Create workspace
    let ws = ToolRegistry::call(
        "memory_workspace_create",
        Some(json!({"name": "full-workflow"})),
        &session,
    )
    .await
    .unwrap();
    let ws_id = result_json(&ws)["workspace_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(ws_id.starts_with("ws_"));

    // Step 2: Create and add 3 contexts
    let ctx_auth = create_seeded_amem(
        "auth.amem",
        &[
            ("fact", "Authentication uses JWT tokens"),
            ("decision", "Chose RS256 algorithm for JWT signing"),
            ("fact", "Refresh tokens stored in HttpOnly cookies"),
        ],
    )
    .await;

    let ctx_db = create_seeded_amem(
        "database.amem",
        &[
            ("fact", "Database uses PostgreSQL 16"),
            ("decision", "Chose sqlx over diesel for async support"),
            ("fact", "Migrations managed with sqlx-cli"),
        ],
    )
    .await;

    let ctx_deploy = create_seeded_amem(
        "deploy.amem",
        &[
            ("fact", "Deployment uses Docker with multi-stage builds"),
            ("decision", "Chose Fly.io over AWS for simplicity"),
            ("fact", "CI/CD pipeline runs on GitHub Actions"),
        ],
    )
    .await;

    for (path, role, label) in [
        (&ctx_auth.path, "primary", "auth"),
        (&ctx_db.path, "secondary", "database"),
        (&ctx_deploy.path, "reference", "deployment"),
    ] {
        let add_result = ToolRegistry::call(
            "memory_workspace_add",
            Some(json!({
                "workspace_id": ws_id,
                "path": path,
                "role": role,
                "label": label
            })),
            &session,
        )
        .await
        .unwrap();
        assert_eq!(result_json(&add_result)["status"], "added");
    }

    // Step 3: List contexts
    let list_result = ToolRegistry::call(
        "memory_workspace_list",
        Some(json!({"workspace_id": ws_id})),
        &session,
    )
    .await
    .unwrap();
    assert_eq!(result_json(&list_result)["count"], 3);

    // Step 4: Query across all contexts
    let query_result = ToolRegistry::call(
        "memory_workspace_query",
        Some(json!({"workspace_id": ws_id, "query": "chose"})),
        &session,
    )
    .await
    .unwrap();

    let query_parsed = result_json(&query_result);
    assert!(
        query_parsed["total_matches"].as_u64().unwrap() >= 2,
        "Should find decisions across contexts, got: {}",
        query_parsed["total_matches"]
    );

    // Step 5: Compare a topic present in some contexts
    let compare_result = ToolRegistry::call(
        "memory_workspace_compare",
        Some(json!({"workspace_id": ws_id, "item": "Docker"})),
        &session,
    )
    .await
    .unwrap();

    let compare_parsed = result_json(&compare_result);
    assert!(
        !compare_parsed["found_in"].as_array().unwrap().is_empty(),
        "Docker should be found in at least one context"
    );

    // Step 6: Cross-reference a topic
    let xref_result = ToolRegistry::call(
        "memory_workspace_xref",
        Some(json!({"workspace_id": ws_id, "item": "PostgreSQL"})),
        &session,
    )
    .await
    .unwrap();

    let xref_parsed = result_json(&xref_result);
    assert!(xref_parsed["present_in"].as_array().is_some());
    assert!(xref_parsed["absent_from"].as_array().is_some());
    assert!(xref_parsed["coverage"].as_str().is_some());
}
