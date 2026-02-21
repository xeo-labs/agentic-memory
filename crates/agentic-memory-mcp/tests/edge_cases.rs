//! Comprehensive edge-case tests for every tool, resource, and prompt.
//!
//! Tests cover: missing params, invalid types, empty inputs, boundary values,
//! malformed JSON-RPC, concurrent access, large payloads, and error paths.

mod common;

use serde_json::json;

use agentic_memory_mcp::protocol::ProtocolHandler;
use agentic_memory_mcp::resources::ResourceRegistry;
use agentic_memory_mcp::tools::ToolRegistry;
use agentic_memory_mcp::types::*;

use common::fixtures::create_test_session;
use common::mock_client::MockClient;

fn create_client() -> MockClient {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);
    MockClient::new(handler)
}

// ============================================================
// JSON-RPC Protocol Edge Cases
// ============================================================

#[tokio::test]
async fn test_malformed_jsonrpc_version() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = JsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: "1.0".to_string(), // wrong version
        id: RequestId::Number(1),
        method: "ping".to_string(),
        params: None,
    });

    let response = handler.handle_message(msg).await;
    assert!(response.is_some());
    let val = response.unwrap();
    assert!(val["error"].is_object());
}

#[tokio::test]
async fn test_empty_method_name() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = JsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::Number(1),
        method: "".to_string(),
        params: None,
    });

    let response = handler.handle_message(msg).await.unwrap();
    assert!(response["error"].is_object());
}

#[tokio::test]
async fn test_request_with_string_id() {
    let mut client = create_client();
    client.initialize().await;

    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = JsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::String("string-id-42".to_string()),
        method: "ping".to_string(),
        params: None,
    });

    let response = handler.handle_message(msg).await.unwrap();
    assert_eq!(response["id"], "string-id-42");
    assert!(response["result"].is_object());
}

#[tokio::test]
async fn test_request_with_null_id() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = JsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::Null,
        method: "ping".to_string(),
        params: None,
    });

    let response = handler.handle_message(msg).await.unwrap();
    assert!(response["result"].is_object());
}

#[tokio::test]
async fn test_unknown_notification_method() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = JsonRpcMessage::Notification(JsonRpcNotification::new(
        "unknown/notification".to_string(),
        None,
    ));

    // Should return None (notifications don't produce responses)
    let response = handler.handle_message(msg).await;
    assert!(response.is_none());
}

#[tokio::test]
async fn test_cancel_notification() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = JsonRpcMessage::Notification(JsonRpcNotification::new(
        "$/cancelRequest".to_string(),
        Some(json!({"id": 42})),
    ));

    let response = handler.handle_message(msg).await;
    assert!(response.is_none());
}

// ============================================================
// memory_add Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_add_missing_event_type() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"content": "No event type"})),
        &session,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_add_missing_content() {
    let session = create_test_session();
    let result =
        ToolRegistry::call("memory_add", Some(json!({"event_type": "fact"})), &session).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_add_invalid_event_type() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "invalid_type", "content": "test"})),
        &session,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_add_empty_content() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": ""})),
        &session,
    )
    .await;
    // Empty content should still succeed (server doesn't reject empty strings)
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_memory_add_very_long_content() {
    let session = create_test_session();
    // Use 10k chars — within typical graph limits
    let long_content = "x".repeat(10_000);
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": long_content})),
        &session,
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_memory_add_all_event_types() {
    let session = create_test_session();

    for event_type in &[
        "fact",
        "decision",
        "inference",
        "correction",
        "skill",
        "episode",
    ] {
        let result = ToolRegistry::call(
            "memory_add",
            Some(json!({"event_type": event_type, "content": format!("Test {event_type}")})),
            &session,
        )
        .await;
        assert!(result.is_ok(), "Failed for event_type: {event_type}");
    }

    // Verify counts
    let stats = ToolRegistry::call("memory_stats", Some(json!({})), &session)
        .await
        .unwrap();
    let text = match &stats.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["node_count"], 6);
}

#[tokio::test]
async fn test_memory_add_with_confidence_boundaries() {
    let session = create_test_session();

    // Confidence = 0.0
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "zero conf", "confidence": 0.0})),
        &session,
    )
    .await;
    assert!(result.is_ok());

    // Confidence = 1.0
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "max conf", "confidence": 1.0})),
        &session,
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_memory_add_with_unicode_content() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Unicode: \u{1F600} \u{4E16}\u{754C} \u{0410}\u{0411}"})),
        &session,
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_memory_add_with_edge_to_nonexistent_node() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_add",
        Some(json!({
            "event_type": "fact",
            "content": "Test",
            "edges": [{"target_id": 999999, "edge_type": "related_to", "weight": 0.5}]
        })),
        &session,
    )
    .await;
    // Should fail because target node doesn't exist
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_add_with_null_params() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_add", None, &session).await;
    assert!(result.is_err());
}

// ============================================================
// memory_query Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_query_empty_graph() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_query", Some(json!({})), &session)
        .await
        .unwrap();
    let text = match &result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["count"], 0);
}

#[tokio::test]
async fn test_memory_query_with_nonexistent_type() {
    let session = create_test_session();
    // Add a fact
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "test"})),
        &session,
    )
    .await
    .unwrap();

    // Query for decisions only (should find 0)
    let result = ToolRegistry::call(
        "memory_query",
        Some(json!({"event_types": ["decision"]})),
        &session,
    )
    .await
    .unwrap();
    let text = match &result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["count"], 0);
}

#[tokio::test]
async fn test_memory_query_null_params() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_query", None, &session).await;
    // Should work with default params (empty query = return all)
    assert!(result.is_ok());
}

// ============================================================
// memory_correct Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_correct_nonexistent_node() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_correct",
        Some(json!({"old_node_id": 99999, "new_content": "corrected"})),
        &session,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_correct_missing_old_node_id() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_correct",
        Some(json!({"new_content": "corrected"})),
        &session,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_correct_missing_new_content() {
    let session = create_test_session();
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "old"})),
        &session,
    )
    .await
    .unwrap();

    let result =
        ToolRegistry::call("memory_correct", Some(json!({"old_node_id": 0})), &session).await;
    assert!(result.is_err());
}

// ============================================================
// memory_resolve Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_resolve_nonexistent_node() {
    let session = create_test_session();
    let result =
        ToolRegistry::call("memory_resolve", Some(json!({"node_id": 99999})), &session).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_resolve_node_without_corrections() {
    let session = create_test_session();

    // Add a fact (no corrections)
    let add_result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "original fact"})),
        &session,
    )
    .await
    .unwrap();
    let text = match &add_result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    let node_id = parsed["node_id"].as_u64().unwrap();

    // Resolve should return the same node
    let resolve_result = ToolRegistry::call(
        "memory_resolve",
        Some(json!({"node_id": node_id})),
        &session,
    )
    .await
    .unwrap();
    let text = match &resolve_result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let resolved: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(resolved["is_latest"], true);
}

// ============================================================
// memory_traverse Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_traverse_nonexistent_start() {
    let session = create_test_session();
    let result = ToolRegistry::call(
        "memory_traverse",
        Some(json!({"start_id": 99999})),
        &session,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_traverse_single_node_no_edges() {
    let session = create_test_session();

    let add_result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "isolated node"})),
        &session,
    )
    .await
    .unwrap();
    let text = match &add_result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    let node_id = parsed["node_id"].as_u64().unwrap();

    let traverse_result = ToolRegistry::call(
        "memory_traverse",
        Some(json!({"start_id": node_id, "max_depth": 3})),
        &session,
    )
    .await
    .unwrap();
    let text = match &traverse_result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let traversed: serde_json::Value = serde_json::from_str(text).unwrap();
    // Should return at least the start node in the visited array
    assert!(traversed["visited_count"].as_u64().unwrap() >= 1);
    assert!(!traversed["visited"].as_array().unwrap().is_empty());
}

// ============================================================
// memory_context Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_context_nonexistent_node() {
    let session = create_test_session();
    let result =
        ToolRegistry::call("memory_context", Some(json!({"node_id": 99999})), &session).await;
    assert!(result.is_err());
}

// ============================================================
// memory_similar Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_similar_missing_query() {
    let session = create_test_session();
    // Neither query_vec nor query_text provided — should error
    let result = ToolRegistry::call("memory_similar", Some(json!({})), &session).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_similar_with_query_text() {
    let session = create_test_session();
    // query_text without embedding model returns a helpful error (not a crash)
    let result = ToolRegistry::call(
        "memory_similar",
        Some(json!({"query_text": "test"})),
        &session,
    )
    .await;
    // Should succeed but return an error message in the content
    assert!(result.is_ok());
    let text = match &result.unwrap().content[0] {
        ToolContent::Text { text } => text.clone(),
        _ => panic!("Expected text"),
    };
    assert!(text.contains("embedding model"));
}

#[tokio::test]
async fn test_memory_similar_with_zero_vector() {
    let session = create_test_session();
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "cats are fluffy"})),
        &session,
    )
    .await
    .unwrap();

    // Use a zero vector — should return no matches with skip_zero_vectors
    let dimension = {
        let sess = session.lock().await;
        sess.graph().dimension()
    };
    let zero_vec: Vec<f32> = vec![0.0; dimension];
    let result = ToolRegistry::call(
        "memory_similar",
        Some(json!({"query_vec": zero_vec})),
        &session,
    )
    .await
    .unwrap();
    let text = match &result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(parsed["matches"].is_array());
}

// ============================================================
// memory_causal Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_causal_nonexistent_node() {
    let session = create_test_session();
    let result =
        ToolRegistry::call("memory_causal", Some(json!({"node_id": 99999})), &session).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_causal_node_without_causal_edges() {
    let session = create_test_session();

    let add_result = ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "isolated"})),
        &session,
    )
    .await
    .unwrap();
    let text = match &add_result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    let node_id = parsed["node_id"].as_u64().unwrap();

    let result = ToolRegistry::call("memory_causal", Some(json!({"node_id": node_id})), &session)
        .await
        .unwrap();
    let text = match &result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let causal: serde_json::Value = serde_json::from_str(text).unwrap();
    // Should have empty dependents (no causal edges)
    assert_eq!(causal["dependent_count"], 0);
    assert!(causal["dependents"].as_array().unwrap().is_empty());
}

// ============================================================
// memory_temporal Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_temporal_missing_params() {
    let session = create_test_session();
    // Missing required range_a and range_b
    let result = ToolRegistry::call("memory_temporal", Some(json!({})), &session).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_memory_temporal_with_session_ranges() {
    let session = create_test_session();

    // Add facts in session 1
    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "Temporal fact"})),
        &session,
    )
    .await
    .unwrap();

    let result = ToolRegistry::call(
        "memory_temporal",
        Some(json!({
            "range_a": {"type": "session", "session_id": 1},
            "range_b": {"type": "session", "session_id": 2}
        })),
        &session,
    )
    .await;
    assert!(result.is_ok());
}

// ============================================================
// memory_stats Edge Cases
// ============================================================

#[tokio::test]
async fn test_memory_stats_empty_graph() {
    let session = create_test_session();
    let result = ToolRegistry::call("memory_stats", Some(json!({})), &session)
        .await
        .unwrap();
    let text = match &result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["node_count"], 0);
    assert_eq!(parsed["edge_count"], 0);
}

#[tokio::test]
async fn test_memory_stats_after_many_operations() {
    let session = create_test_session();

    // Add several nodes
    for i in 0..10 {
        ToolRegistry::call(
            "memory_add",
            Some(json!({"event_type": "fact", "content": format!("Fact {i}")})),
            &session,
        )
        .await
        .unwrap();
    }

    let result = ToolRegistry::call("memory_stats", Some(json!({})), &session)
        .await
        .unwrap();
    let text = match &result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["node_count"], 10);
}

// ============================================================
// session_start / session_end Edge Cases
// ============================================================

#[tokio::test]
async fn test_session_end_without_episode() {
    let session = create_test_session();

    ToolRegistry::call("session_start", Some(json!({})), &session)
        .await
        .unwrap();

    ToolRegistry::call(
        "memory_add",
        Some(json!({"event_type": "fact", "content": "session fact"})),
        &session,
    )
    .await
    .unwrap();

    // End without creating an episode
    let result = ToolRegistry::call(
        "session_end",
        Some(json!({"create_episode": false})),
        &session,
    )
    .await
    .unwrap();

    let text = match &result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(parsed.get("episode_node_id").is_none() || parsed["episode_node_id"].is_null());
}

#[tokio::test]
async fn test_session_start_with_explicit_id() {
    let session = create_test_session();

    let result = ToolRegistry::call("session_start", Some(json!({"session_id": 42})), &session)
        .await
        .unwrap();

    let text = match &result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["session_id"], 42);
}

#[tokio::test]
async fn test_multiple_session_start() {
    let session = create_test_session();

    let r1 = ToolRegistry::call("session_start", Some(json!({})), &session)
        .await
        .unwrap();
    let text1 = match &r1.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let p1: serde_json::Value = serde_json::from_str(text1).unwrap();
    let id1 = p1["session_id"].as_u64().unwrap();

    let r2 = ToolRegistry::call("session_start", Some(json!({})), &session)
        .await
        .unwrap();
    let text2 = match &r2.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let p2: serde_json::Value = serde_json::from_str(text2).unwrap();
    let id2 = p2["session_id"].as_u64().unwrap();

    assert!(id2 >= id1);
}

// ============================================================
// Resource Edge Cases
// ============================================================

#[tokio::test]
async fn test_resource_invalid_uri_scheme() {
    let session = create_test_session();
    let result = ResourceRegistry::read("http://invalid/uri", &session).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_resource_node_invalid_id_format() {
    let session = create_test_session();
    let result = ResourceRegistry::read("amem://node/not-a-number", &session).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_resource_session_invalid_id_format() {
    let session = create_test_session();
    let result = ResourceRegistry::read("amem://session/abc", &session).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_resource_types_invalid_type() {
    let session = create_test_session();
    let result = ResourceRegistry::read("amem://types/nonexistent_type", &session).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_resource_types_all_valid_types() {
    let session = create_test_session();
    for type_name in &[
        "fact",
        "decision",
        "inference",
        "correction",
        "skill",
        "episode",
    ] {
        let result = ResourceRegistry::read(&format!("amem://types/{type_name}"), &session).await;
        assert!(result.is_ok(), "Failed for type: {type_name}");
    }
}

#[tokio::test]
async fn test_resource_graph_stats_empty() {
    let session = create_test_session();
    let result = ResourceRegistry::read("amem://graph/stats", &session)
        .await
        .unwrap();
    let text = result.contents[0].text.as_ref().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["node_count"], 0);
}

#[tokio::test]
async fn test_resource_graph_recent_empty() {
    let session = create_test_session();
    let result = ResourceRegistry::read("amem://graph/recent", &session)
        .await
        .unwrap();
    let text = result.contents[0].text.as_ref().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["count"], 0);
}

#[tokio::test]
async fn test_resource_graph_important_empty() {
    let session = create_test_session();
    let result = ResourceRegistry::read("amem://graph/important", &session)
        .await
        .unwrap();
    let text = result.contents[0].text.as_ref().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["count"], 0);
}

// ============================================================
// Prompt Edge Cases
// ============================================================

#[tokio::test]
async fn test_prompt_remember_with_context() {
    let session = create_test_session();
    let result = agentic_memory_mcp::prompts::PromptRegistry::get(
        "remember",
        Some(json!({"information": "test", "context": "important context"})),
        &session,
    )
    .await
    .unwrap();
    let msg_text = match &result.messages[0].content {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    assert!(msg_text.contains("important context"));
}

#[tokio::test]
async fn test_prompt_reflect_with_node_id() {
    let session = create_test_session();
    let result = agentic_memory_mcp::prompts::PromptRegistry::get(
        "reflect",
        Some(json!({"topic": "test topic", "node_id": 42})),
        &session,
    )
    .await
    .unwrap();
    let msg_text = match &result.messages[0].content {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    assert!(msg_text.contains("#42"));
}

#[tokio::test]
async fn test_prompt_correct_with_reason() {
    let session = create_test_session();
    let result = agentic_memory_mcp::prompts::PromptRegistry::get(
        "correct",
        Some(json!({
            "old_belief": "old",
            "new_information": "new",
            "reason": "because reasons"
        })),
        &session,
    )
    .await
    .unwrap();
    let msg_text = match &result.messages[0].content {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    assert!(msg_text.contains("because reasons"));
}

#[tokio::test]
async fn test_prompt_correct_missing_required_arg() {
    let session = create_test_session();
    // Missing new_information
    let result = agentic_memory_mcp::prompts::PromptRegistry::get(
        "correct",
        Some(json!({"old_belief": "old"})),
        &session,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_prompt_summarize_explicit_session_id() {
    let session = create_test_session();
    let result = agentic_memory_mcp::prompts::PromptRegistry::get(
        "summarize",
        Some(json!({"session_id": 999})),
        &session,
    )
    .await
    .unwrap();
    let msg_text = match &result.messages[0].content {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    assert!(msg_text.contains("session 999"));
}

// ============================================================
// Integration: Multi-step workflows
// ============================================================

#[tokio::test]
async fn test_workflow_add_query_traverse_correct_resolve() {
    let mut client = create_client();
    client.initialize().await;

    // Add fact
    let add = client
        .call_tool(
            "memory_add",
            Some(json!({"event_type": "fact", "content": "Earth is flat", "confidence": 0.5})),
        )
        .await;
    let add_text = add["result"]["content"][0]["text"].as_str().unwrap();
    let add_parsed: serde_json::Value = serde_json::from_str(add_text).unwrap();
    let old_id = add_parsed["node_id"].as_u64().unwrap();

    // Query to find it
    let query = client
        .call_tool("memory_query", Some(json!({"event_types": ["fact"]})))
        .await;
    let query_text = query["result"]["content"][0]["text"].as_str().unwrap();
    let query_parsed: serde_json::Value = serde_json::from_str(query_text).unwrap();
    assert_eq!(query_parsed["count"], 1);

    // Traverse from it
    let traverse = client
        .call_tool("memory_traverse", Some(json!({"start_id": old_id})))
        .await;
    assert!(traverse["result"]["content"][0]["text"].is_string());

    // Correct it
    let correct = client
        .call_tool(
            "memory_correct",
            Some(json!({"old_node_id": old_id, "new_content": "Earth is round"})),
        )
        .await;
    let correct_text = correct["result"]["content"][0]["text"].as_str().unwrap();
    let correct_parsed: serde_json::Value = serde_json::from_str(correct_text).unwrap();
    let _new_id = correct_parsed["new_node_id"].as_u64().unwrap();

    // Resolve from old ID
    let resolve = client
        .call_tool("memory_resolve", Some(json!({"node_id": old_id})))
        .await;
    let resolve_text = resolve["result"]["content"][0]["text"].as_str().unwrap();
    let resolve_parsed: serde_json::Value = serde_json::from_str(resolve_text).unwrap();
    assert_ne!(resolve_parsed["resolved_id"].as_u64().unwrap(), old_id);

    // Stats should show 2 nodes now (original + correction)
    let stats = client.read_resource("amem://graph/stats").await;
    let stats_text = stats["result"]["contents"][0]["text"].as_str().unwrap();
    let stats_parsed: serde_json::Value = serde_json::from_str(stats_text).unwrap();
    assert_eq!(stats_parsed["node_count"], 2);

    client.shutdown().await;
}

#[tokio::test]
async fn test_workflow_many_adds_then_query() {
    let mut client = create_client();
    client.initialize().await;

    // Add 50 facts
    for i in 0..50 {
        client
            .call_tool(
                "memory_add",
                Some(json!({"event_type": "fact", "content": format!("Fact number {i}")})),
            )
            .await;
    }

    // Add 20 decisions
    for i in 0..20 {
        client
            .call_tool(
                "memory_add",
                Some(json!({"event_type": "decision", "content": format!("Decision {i}")})),
            )
            .await;
    }

    // Query all with high max_results
    let query_all = client
        .call_tool("memory_query", Some(json!({"max_results": 100})))
        .await;
    let text = query_all["result"]["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["count"], 70);

    // Query facts only with high max_results
    let query_facts = client
        .call_tool(
            "memory_query",
            Some(json!({"event_types": ["fact"], "max_results": 100})),
        )
        .await;
    let text = query_facts["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["count"], 50);

    // Stats
    let stats = client.read_resource("amem://graph/stats").await;
    let stats_text = stats["result"]["contents"][0]["text"].as_str().unwrap();
    let stats_parsed: serde_json::Value = serde_json::from_str(stats_text).unwrap();
    assert_eq!(stats_parsed["node_count"], 70);
    assert_eq!(stats_parsed["type_counts"]["fact"], 50);
    assert_eq!(stats_parsed["type_counts"]["decision"], 20);

    client.shutdown().await;
}

// ============================================================
// Concurrent access simulation
// ============================================================

#[tokio::test]
async fn test_concurrent_tool_calls() {
    let session = create_test_session();

    // Spawn multiple concurrent add operations
    let mut handles = vec![];
    for i in 0..10 {
        let session = session.clone();
        handles.push(tokio::spawn(async move {
            ToolRegistry::call(
                "memory_add",
                Some(json!({"event_type": "fact", "content": format!("Concurrent fact {i}")})),
                &session,
            )
            .await
        }));
    }

    // Wait for all to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Verify all 10 were added
    let result = ToolRegistry::call("memory_stats", Some(json!({})), &session)
        .await
        .unwrap();
    let text = match &result.content[0] {
        ToolContent::Text { text } => text,
        _ => panic!("Expected text"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["node_count"], 10);
}
