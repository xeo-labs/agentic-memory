//! Phase 5: End-to-end MCP integration tests.
//!
//! Simulates a full MCP conversation:
//! Initialize → tools/list → tools/call → resources/read → prompts/get → shutdown

mod common;

use serde_json::json;

use agentic_memory_mcp::protocol::ProtocolHandler;

use common::fixtures::create_test_session;
use common::mock_client::MockClient;

fn create_client() -> MockClient {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);
    MockClient::new(handler)
}

#[tokio::test]
async fn test_full_mcp_conversation() {
    let mut client = create_client();

    // 1. Initialize
    let init = client.initialize().await;
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init["result"]["serverInfo"]["name"], "agentic-memory-mcp");

    // 2. List tools
    let tools = client.request("tools/list", None).await;
    let tool_list = tools["result"]["tools"].as_array().unwrap();
    assert!(tool_list.len() >= 12);

    // 3. Add a memory via tool call
    let add_result = client
        .call_tool(
            "memory_add",
            Some(json!({
                "event_type": "fact",
                "content": "Integration test fact",
                "confidence": 0.95
            })),
        )
        .await;
    let add_content = &add_result["result"]["content"][0]["text"];
    let add_parsed: serde_json::Value =
        serde_json::from_str(add_content.as_str().unwrap()).unwrap();
    let node_id = add_parsed["node_id"].as_u64().unwrap();

    // 4. Read the node via resource
    let resource = client
        .read_resource(&format!("amem://node/{node_id}"))
        .await;
    let resource_text = resource["result"]["contents"][0]["text"].as_str().unwrap();
    let resource_parsed: serde_json::Value = serde_json::from_str(resource_text).unwrap();
    assert_eq!(resource_parsed["id"], node_id);
    assert_eq!(resource_parsed["content"], "Integration test fact");

    // 5. Query the memory
    let query_result = client
        .call_tool("memory_query", Some(json!({"event_types": ["fact"]})))
        .await;
    let query_text = &query_result["result"]["content"][0]["text"];
    let query_parsed: serde_json::Value =
        serde_json::from_str(query_text.as_str().unwrap()).unwrap();
    assert_eq!(query_parsed["count"], 1);

    // 6. Read graph stats
    let stats = client.read_resource("amem://graph/stats").await;
    let stats_text = stats["result"]["contents"][0]["text"].as_str().unwrap();
    let stats_parsed: serde_json::Value = serde_json::from_str(stats_text).unwrap();
    assert_eq!(stats_parsed["node_count"], 1);

    // 7. Get a prompt
    let prompt = client
        .get_prompt(
            "remember",
            Some(json!({"information": "Integration test info"})),
        )
        .await;
    assert!(prompt["result"]["messages"].is_array());
    assert!(!prompt["result"]["messages"].as_array().unwrap().is_empty());

    // 8. Shutdown
    let shutdown = client.shutdown().await;
    assert!(shutdown["result"].is_object());
}

#[tokio::test]
async fn test_add_correct_resolve_flow() {
    let mut client = create_client();
    client.initialize().await;

    // Add initial fact
    let add = client
        .call_tool(
            "memory_add",
            Some(json!({"event_type": "fact", "content": "Sky is green"})),
        )
        .await;
    let add_text = add["result"]["content"][0]["text"].as_str().unwrap();
    let add_parsed: serde_json::Value = serde_json::from_str(add_text).unwrap();
    let old_id = add_parsed["node_id"].as_u64().unwrap();

    // Correct the fact
    let correct = client
        .call_tool(
            "memory_correct",
            Some(json!({
                "old_node_id": old_id,
                "new_content": "Sky is blue",
                "reason": "Observation"
            })),
        )
        .await;
    let correct_text = correct["result"]["content"][0]["text"].as_str().unwrap();
    let correct_parsed: serde_json::Value = serde_json::from_str(correct_text).unwrap();
    let new_id = correct_parsed["new_node_id"].as_u64().unwrap();
    assert_ne!(old_id, new_id);

    // Resolve from old ID should point to new
    let resolve = client
        .call_tool("memory_resolve", Some(json!({"node_id": old_id})))
        .await;
    let resolve_text = resolve["result"]["content"][0]["text"].as_str().unwrap();
    let resolve_parsed: serde_json::Value = serde_json::from_str(resolve_text).unwrap();
    assert_ne!(resolve_parsed["resolved_id"], old_id);

    client.shutdown().await;
}

#[tokio::test]
async fn test_session_lifecycle_integration() {
    let mut client = create_client();
    client.initialize().await;

    // Start session
    let start = client.call_tool("session_start", None).await;
    let start_text = start["result"]["content"][0]["text"].as_str().unwrap();
    let start_parsed: serde_json::Value = serde_json::from_str(start_text).unwrap();
    assert!(start_parsed["session_id"].as_u64().is_some());

    // Add memories
    client
        .call_tool(
            "memory_add",
            Some(json!({"event_type": "fact", "content": "Session lifecycle fact"})),
        )
        .await;

    // End session with episode
    let end = client
        .call_tool(
            "session_end",
            Some(json!({
                "create_episode": true,
                "summary": "Integration test session"
            })),
        )
        .await;
    let end_text = end["result"]["content"][0]["text"].as_str().unwrap();
    let end_parsed: serde_json::Value = serde_json::from_str(end_text).unwrap();
    assert!(end_parsed["episode_node_id"].as_u64().is_some());

    // Check stats - should have 2 nodes (fact + episode)
    let stats = client.read_resource("amem://graph/stats").await;
    let stats_text = stats["result"]["contents"][0]["text"].as_str().unwrap();
    let stats_parsed: serde_json::Value = serde_json::from_str(stats_text).unwrap();
    assert!(stats_parsed["node_count"].as_u64().unwrap() >= 2);

    client.shutdown().await;
}

#[tokio::test]
async fn test_resource_templates_and_reads() {
    let mut client = create_client();
    client.initialize().await;

    // List resource templates
    let templates = client.request("resources/templates/list", None).await;
    let template_list = templates["result"]["resourceTemplates"].as_array().unwrap();
    assert!(template_list.len() >= 3);

    // List concrete resources
    let resources = client.request("resources/list", None).await;
    let resource_list = resources["result"]["resources"].as_array().unwrap();
    assert!(resource_list.len() >= 3);

    // Read graph stats (always available)
    let stats = client.read_resource("amem://graph/stats").await;
    assert!(stats["result"]["contents"].is_array());

    // Read recent (may be empty)
    let recent = client.read_resource("amem://graph/recent").await;
    assert!(recent["result"]["contents"].is_array());

    client.shutdown().await;
}

#[tokio::test]
async fn test_all_prompts() {
    let mut client = create_client();
    client.initialize().await;

    // List prompts
    let prompts = client.request("prompts/list", None).await;
    let prompt_list = prompts["result"]["prompts"].as_array().unwrap();
    assert!(prompt_list.len() >= 4);

    // Get each prompt
    let remember = client
        .get_prompt(
            "remember",
            Some(json!({"information": "Test info", "context": "Testing"})),
        )
        .await;
    assert!(remember["result"]["messages"].is_array());

    let reflect = client
        .get_prompt("reflect", Some(json!({"topic": "Test decision"})))
        .await;
    assert!(reflect["result"]["messages"].is_array());

    let correct = client
        .get_prompt(
            "correct",
            Some(json!({
                "old_belief": "Old info",
                "new_information": "New info",
                "reason": "Testing"
            })),
        )
        .await;
    assert!(correct["result"]["messages"].is_array());

    let summarize = client.get_prompt("summarize", Some(json!({}))).await;
    assert!(summarize["result"]["messages"].is_array());

    client.shutdown().await;
}

#[tokio::test]
async fn test_prompt_auto_capture_persists_context() {
    let mut client = create_client();
    client.initialize().await;

    let remember = client
        .get_prompt(
            "remember",
            Some(json!({"information": "Capture this prompt", "context": "integration"})),
        )
        .await;
    assert!(remember["result"]["messages"].is_array());

    let recent = client.read_resource("amem://graph/recent").await;
    let recent_text = recent["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(recent_text.contains("[auto-capture][prompt]"));

    client.shutdown().await;
}

#[tokio::test]
async fn test_error_handling_integration() {
    let mut client = create_client();
    client.initialize().await;

    // Non-existent tool
    let result = client.call_tool("nonexistent_tool", None).await;
    assert!(result.get("error").is_some());

    // Non-existent resource
    let result = client.read_resource("amem://node/999999").await;
    assert!(result.get("error").is_some());

    // Non-existent prompt
    let result = client.get_prompt("nonexistent_prompt", None).await;
    assert!(result.get("error").is_some());

    // Invalid method
    let result = client.request("invalid/method", None).await;
    assert!(result.get("error").is_some());

    client.shutdown().await;
}

#[tokio::test]
async fn test_ping() {
    let mut client = create_client();

    let result = client.request("ping", None).await;
    assert!(result["result"].is_object());
}
