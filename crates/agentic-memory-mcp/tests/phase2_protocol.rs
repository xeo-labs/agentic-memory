//! Phase 2: Protocol handling tests.

mod common;

use serde_json::json;

use agentic_memory_mcp::protocol::ProtocolHandler;
use agentic_memory_mcp::types::*;

use common::fixtures::create_test_session;

fn make_request(id: i64, method: &str, params: Option<serde_json::Value>) -> JsonRpcMessage {
    JsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::Number(id),
        method: method.to_string(),
        params,
    })
}

#[tokio::test]
async fn test_initialize_handshake() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = make_request(
        1,
        "initialize",
        Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "1.0" }
        })),
    );

    let response = handler.handle_message(msg).await.unwrap();
    let result = &response["result"];

    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert!(result["capabilities"]["tools"].is_object());
    assert_eq!(result["serverInfo"]["name"], "agentic-memory-mcp");
}

#[tokio::test]
async fn test_tools_list() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = make_request(1, "tools/list", None);
    let response = handler.handle_message(msg).await.unwrap();
    let tools = &response["result"]["tools"];

    assert!(tools.is_array());
    let tools_arr = tools.as_array().unwrap();
    assert!(tools_arr.len() >= 12);

    let names: Vec<&str> = tools_arr
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(names.contains(&"memory_add"));
    assert!(names.contains(&"memory_query"));
    assert!(names.contains(&"memory_traverse"));
    assert!(names.contains(&"memory_correct"));
    assert!(names.contains(&"memory_resolve"));
    assert!(names.contains(&"memory_context"));
    assert!(names.contains(&"memory_similar"));
    assert!(names.contains(&"memory_causal"));
    assert!(names.contains(&"memory_temporal"));
    assert!(names.contains(&"memory_stats"));
    assert!(names.contains(&"session_start"));
    assert!(names.contains(&"session_end"));
}

#[tokio::test]
async fn test_resources_list() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = make_request(1, "resources/list", None);
    let response = handler.handle_message(msg).await.unwrap();
    let resources = &response["result"]["resources"];

    assert!(resources.is_array());
    assert!(resources.as_array().unwrap().len() >= 3);
}

#[tokio::test]
async fn test_prompts_list() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = make_request(1, "prompts/list", None);
    let response = handler.handle_message(msg).await.unwrap();
    let prompts = &response["result"]["prompts"];

    assert!(prompts.is_array());
    let prompts_arr = prompts.as_array().unwrap();
    assert!(prompts_arr.len() >= 4);
}

#[tokio::test]
async fn test_method_not_found() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = make_request(1, "nonexistent/method", None);
    let response = handler.handle_message(msg).await.unwrap();

    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], error_codes::METHOD_NOT_FOUND);
}

#[tokio::test]
async fn test_invalid_jsonrpc_version() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = JsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: "1.0".to_string(),
        id: RequestId::Number(1),
        method: "tools/list".to_string(),
        params: None,
    });

    let response = handler.handle_message(msg).await.unwrap();
    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], error_codes::INVALID_REQUEST);
}

#[tokio::test]
async fn test_ping() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = make_request(1, "ping", None);
    let response = handler.handle_message(msg).await.unwrap();
    assert!(response.get("result").is_some());
}

#[tokio::test]
async fn test_notification_no_response() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg =
        JsonRpcMessage::Notification(JsonRpcNotification::new("initialized".to_string(), None));

    let response = handler.handle_message(msg).await;
    assert!(response.is_none());
}

#[tokio::test]
async fn test_shutdown() {
    let session = create_test_session();
    let handler = ProtocolHandler::new(session);

    let msg = make_request(1, "shutdown", None);
    let response = handler.handle_message(msg).await.unwrap();
    assert!(response.get("result").is_some());
    assert!(handler.shutdown_requested());
}
