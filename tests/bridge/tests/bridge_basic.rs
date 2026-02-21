//! Basic integration: MCP tools call real AgenticMemory operations.
//!
//! Tests verify that the MCP server correctly creates, queries, traverses,
//! and corrects nodes in the underlying AgenticMemory graph, and that data
//! persists correctly to .amem files.

use std::sync::Arc;
use tokio::sync::Mutex;

use agentic_memory::{
    AmemReader, AmemWriter, CognitiveEventBuilder, EdgeType, EventType, MemoryGraph,
};
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

fn parse_traverse_results(response: &serde_json::Value) -> Vec<u64> {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("Expected text in tool response");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("Expected JSON in text");
    parsed["visited"]
        .as_array()
        .expect("Expected visited array")
        .iter()
        .map(|n| n["id"].as_u64().expect("Expected id in visited node"))
        .collect()
}

// ─── Tests ─────────────────────────────────────────────────────────────────

/// MCP memory_add creates real nodes in .amem file
#[tokio::test]
async fn test_mcp_add_creates_real_node() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.amem");
    let path_str = path.to_str().unwrap();

    let handler = create_handler(path_str);
    init_handler(&handler).await;

    let response = call_tool(
        &handler,
        "memory_add",
        json!({
            "event_type": "fact",
            "content": "User prefers Rust",
            "confidence": 0.95
        }),
    )
    .await;

    let node_id = extract_node_id(&response);

    // Save and close MCP
    call_tool(&handler, "session_end", json!({"create_episode": false})).await;
    drop(handler);

    // Read directly with core library
    let graph = AmemReader::read_from_file(&path).unwrap();

    let node = graph.get_node(node_id).unwrap();
    assert_eq!(node.content, "User prefers Rust");
    assert_eq!(node.event_type, EventType::Fact);
    assert!((node.confidence - 0.95).abs() < 0.001);
}

/// Core library writes, MCP reads
#[tokio::test]
async fn test_core_write_mcp_read() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.amem");
    let path_str = path.to_str().unwrap();

    // Write with core library
    let dimension = agentic_memory::DEFAULT_DIMENSION;
    let mut graph = MemoryGraph::new(dimension);
    let event = CognitiveEventBuilder::new(EventType::Decision, "Chose microservices architecture")
        .confidence(0.9)
        .session_id(1)
        .build();
    let node_id = graph.add_node(event).unwrap();

    let writer = AmemWriter::new(dimension);
    writer.write_to_file(&graph, &path).unwrap();
    drop(graph);

    // Read via MCP
    let handler = create_handler(path_str);
    init_handler(&handler).await;

    let response = call_tool(
        &handler,
        "memory_query",
        json!({"event_types": ["decision"]}),
    )
    .await;

    let results = parse_query_results(&response);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"].as_u64().unwrap(), node_id);
    assert_eq!(results[0]["content"], "Chose microservices architecture");
}

/// MCP traverse follows real edges
#[tokio::test]
async fn test_mcp_traverse_real_edges() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.amem");
    let path_str = path.to_str().unwrap();

    let handler = create_handler(path_str);
    init_handler(&handler).await;

    // Create chain: fact -> inference -> decision
    let r1 = call_tool(
        &handler,
        "memory_add",
        json!({
            "event_type": "fact",
            "content": "Team has Python experience"
        }),
    )
    .await;
    let fact_id = extract_node_id(&r1);

    let r2 = call_tool(
        &handler,
        "memory_add",
        json!({
            "event_type": "inference",
            "content": "Python would reduce learning curve",
            "edges": [{"target_id": fact_id, "edge_type": "caused_by"}]
        }),
    )
    .await;
    let inference_id = extract_node_id(&r2);

    let r3 = call_tool(
        &handler,
        "memory_add",
        json!({
            "event_type": "decision",
            "content": "Use Python for the project",
            "edges": [{"target_id": inference_id, "edge_type": "caused_by"}]
        }),
    )
    .await;
    let decision_id = extract_node_id(&r3);

    // Traverse forward from decision (follows caused_by edges outward)
    let traverse_response = call_tool(
        &handler,
        "memory_traverse",
        json!({
            "start_id": decision_id,
            "edge_types": ["caused_by"],
            "direction": "forward",
            "max_depth": 5
        }),
    )
    .await;

    let visited = parse_traverse_results(&traverse_response);
    assert!(visited.contains(&inference_id));
    assert!(visited.contains(&fact_id));

    // Verify with core library
    call_tool(&handler, "session_end", json!({"create_episode": false})).await;
    drop(handler);

    let graph = AmemReader::read_from_file(&path).unwrap();
    let edges = graph.edges_from(decision_id);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].target_id, inference_id);
}

/// MCP correct creates SUPERSEDES edge
#[tokio::test]
async fn test_mcp_correct_creates_supersedes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.amem");
    let path_str = path.to_str().unwrap();

    let handler = create_handler(path_str);
    init_handler(&handler).await;

    let r1 = call_tool(
        &handler,
        "memory_add",
        json!({
            "event_type": "fact",
            "content": "API rate limit is 100/min"
        }),
    )
    .await;
    let old_id = extract_node_id(&r1);

    let r2 = call_tool(
        &handler,
        "memory_correct",
        json!({
            "old_node_id": old_id,
            "new_content": "API rate limit is 200/min",
            "reason": "Documentation updated"
        }),
    )
    .await;
    let new_id = extract_node_id(&r2);

    // Verify SUPERSEDES edge via core library
    call_tool(&handler, "session_end", json!({"create_episode": false})).await;
    drop(handler);

    let graph = AmemReader::read_from_file(&path).unwrap();
    let edges = graph.edges_from(new_id);
    let supersedes_edge = edges
        .iter()
        .find(|e| e.edge_type == EdgeType::Supersedes)
        .expect("SUPERSEDES edge should exist");
    assert_eq!(supersedes_edge.target_id, old_id);
}
