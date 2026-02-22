//! Multi-agent scenarios: Agent A and Agent B share memory via .amem file.
//!
//! Tests verify that multiple sequential agents can share a single .amem file,
//! with writes from one agent visible to the next, corrections propagating
//! across agents, and causal chains spanning agent boundaries.

use std::sync::Arc;
use tokio::sync::Mutex;

use agentic_memory_mcp::types::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, RequestId};
use agentic_memory_mcp::{ProtocolHandler, SessionManager};
use serde_json::json;
use tempfile::tempdir;

// ─── Helpers ───────────────────────────────────────────────────────────────

fn create_handler(path_str: &str) -> ProtocolHandler {
    let session = SessionManager::open(path_str).expect("Failed to open session");
    let session_arc = Arc::new(Mutex::new(session));
    ProtocolHandler::new(session_arc)
}

async fn init_handler(handler: &ProtocolHandler) {
    let init_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::Number(0),
        method: "initialize".to_string(),
        params: Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        })),
    };
    handler
        .handle_message(JsonRpcMessage::Request(init_req))
        .await;

    let init_notif = JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "initialized".to_string(),
        params: None,
    };
    handler
        .handle_message(JsonRpcMessage::Notification(init_notif))
        .await;
}

async fn call_tool(
    handler: &ProtocolHandler,
    name: &str,
    args: serde_json::Value,
) -> serde_json::Value {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::Number(1),
        method: "tools/call".to_string(),
        params: Some(json!({"name": name, "arguments": args})),
    };
    handler
        .handle_message(JsonRpcMessage::Request(req))
        .await
        .unwrap()
}

async fn read_resource(handler: &ProtocolHandler, uri: &str) -> serde_json::Value {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::Number(1),
        method: "resources/read".to_string(),
        params: Some(json!({"uri": uri})),
    };
    let resp = handler
        .handle_message(JsonRpcMessage::Request(req))
        .await
        .unwrap();
    let text = resp["result"]["contents"][0]["text"]
        .as_str()
        .expect("Expected text in resource response");
    serde_json::from_str(text).expect("Expected JSON in resource text")
}

fn extract_node_id(response: &serde_json::Value) -> u64 {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("Expected text in tool response");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Expected JSON in text");
    parsed["node_id"]
        .as_u64()
        .or_else(|| parsed["new_node_id"].as_u64())
        .expect("Expected node_id or new_node_id in response")
}

fn parse_query_results(response: &serde_json::Value) -> Vec<serde_json::Value> {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("Expected text in tool response");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Expected JSON in text");
    parsed["nodes"]
        .as_array()
        .expect("Expected nodes array")
        .clone()
}

// ─── Tests ─────────────────────────────────────────────────────────────────

/// Agent A writes, Agent B reads (sequential)
#[tokio::test]
async fn test_agent_a_writes_agent_b_reads() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("shared.amem");
    let path_str = path.to_str().unwrap();

    // Agent A: Write facts
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        call_tool(&handler, "session_start", json!({"session_id": 100})).await;

        call_tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "User's name is Alice"
            }),
        )
        .await;

        call_tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "User prefers dark mode"
            }),
        )
        .await;

        call_tool(
            &handler,
            "session_end",
            json!({
                "create_episode": true,
                "summary": "Agent A learned user preferences"
            }),
        )
        .await;
    }

    // Agent B: Read what Agent A wrote
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        let response = call_tool(&handler, "memory_query", json!({"event_types": ["fact"]})).await;

        let facts = parse_query_results(&response);
        assert_eq!(facts.len(), 2);

        let contents: Vec<&str> = facts
            .iter()
            .map(|f| f["content"].as_str().unwrap())
            .collect();
        assert!(contents.contains(&"User's name is Alice"));
        assert!(contents.contains(&"User prefers dark mode"));

        // Check episode exists
        let episodes = call_tool(
            &handler,
            "memory_query",
            json!({"event_types": ["episode"]}),
        )
        .await;
        let episode_list = parse_query_results(&episodes);
        assert_eq!(episode_list.len(), 1);
    }
}

/// Agent A corrects, Agent B sees via resolve
#[tokio::test]
async fn test_correction_propagates_to_agent_b() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("shared.amem");
    let path_str = path.to_str().unwrap();

    let original_id: u64;
    let corrected_id: u64;

    // Agent A: Create fact then correct it
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        let r1 = call_tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "User works at Company X"
            }),
        )
        .await;
        original_id = extract_node_id(&r1);

        let r2 = call_tool(
            &handler,
            "memory_correct",
            json!({
                "old_node_id": original_id,
                "new_content": "User left Company X, now at Company Y"
            }),
        )
        .await;
        corrected_id = extract_node_id(&r2);

        call_tool(&handler, "session_end", json!({"create_episode": false})).await;
    }

    // Agent B: Resolve the original ID
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        let resolve_response =
            call_tool(&handler, "memory_resolve", json!({"node_id": original_id})).await;

        let text = resolve_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

        assert_eq!(parsed["resolved_id"].as_u64().unwrap(), corrected_id);
        assert!(!parsed["is_latest"].as_bool().unwrap()); // was superseded
        assert!(parsed["latest"]["content"]
            .as_str()
            .unwrap()
            .contains("Company Y"));
    }
}

/// Session handoff: Agent A ends, Agent B continues
#[tokio::test]
async fn test_session_handoff() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("shared.amem");
    let path_str = path.to_str().unwrap();

    // Agent A: Start session, add context
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        call_tool(&handler, "session_start", json!({"session_id": 200})).await;

        call_tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "Debugging payment timeout issue"
            }),
        )
        .await;

        call_tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "inference",
                "content": "Likely connection pool exhaustion"
            }),
        )
        .await;

        call_tool(&handler, "session_end", json!({"create_episode": false})).await;
    }

    // Agent B: Pick up session 200, continue work
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        // Read session 200's context
        let session_resource = read_resource(&handler, "amem://session/200").await;
        let nodes = session_resource["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 2);

        // Continue in same session
        call_tool(&handler, "session_start", json!({"session_id": 200})).await;

        call_tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "decision",
                "content": "Increase connection pool size to 50"
            }),
        )
        .await;

        call_tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "Issue resolved after pool increase"
            }),
        )
        .await;

        call_tool(
            &handler,
            "session_end",
            json!({
                "create_episode": true,
                "summary": "Debugged payment timeout. Root cause: connection pool exhaustion. Fix: increased pool to 50."
            }),
        )
        .await;
    }

    // Verify final state
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        let session_nodes = read_resource(&handler, "amem://session/200").await;
        let nodes = session_nodes["nodes"].as_array().unwrap();
        // 2 from A + 2 from B + 1 episode (+ optional auto-captured feedback context)
        assert!(nodes.len() >= 5);

        let episodes = call_tool(
            &handler,
            "memory_query",
            json!({"event_types": ["episode"]}),
        )
        .await;
        let episode_list = parse_query_results(&episodes);
        assert_eq!(episode_list.len(), 1);
    }
}

/// Causal chain spans multiple agents
#[tokio::test]
async fn test_cross_agent_causal_chain() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("shared.amem");
    let path_str = path.to_str().unwrap();

    let fact_id: u64;

    // Agent A: Establish foundational fact
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        let r = call_tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "Budget limit is $50,000"
            }),
        )
        .await;
        fact_id = extract_node_id(&r);

        call_tool(&handler, "session_end", json!({"create_episode": false})).await;
    }

    // Agent B: Make decision based on Agent A's fact
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        call_tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "decision",
                "content": "Selected mid-tier vendor (within budget)",
                "edges": [{"target_id": fact_id, "edge_type": "caused_by"}]
            }),
        )
        .await;

        call_tool(&handler, "session_end", json!({"create_episode": false})).await;
    }

    // Agent C: Analyze impact of the budget fact
    {
        let handler = create_handler(path_str);
        init_handler(&handler).await;

        let causal_response = call_tool(
            &handler,
            "memory_causal",
            json!({
                "node_id": fact_id,
                "max_depth": 5
            }),
        )
        .await;

        let text = causal_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

        assert!(parsed["dependent_count"].as_u64().unwrap() >= 1);
        assert!(parsed["affected_decisions"].as_u64().unwrap() >= 1);
    }
}
