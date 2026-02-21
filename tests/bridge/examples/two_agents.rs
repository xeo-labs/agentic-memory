//! Example: Two agents sharing a single .amem file.
//!
//! Agent A learns facts about the user, then Agent B reads them.
//! Demonstrates cross-agent memory persistence via the MCP protocol bridge.

use std::sync::Arc;
use tokio::sync::Mutex;

use agentic_memory_mcp::types::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, RequestId};
use agentic_memory_mcp::{ProtocolHandler, SessionManager};
use serde_json::json;

fn create_handler(path: &str) -> ProtocolHandler {
    let session = SessionManager::open(path).expect("Failed to open session");
    ProtocolHandler::new(Arc::new(Mutex::new(session)))
}

async fn init(handler: &ProtocolHandler) {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: RequestId::Number(0),
        method: "initialize".into(),
        params: Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "two_agents_example", "version": "1.0"}
        })),
    };
    handler.handle_message(JsonRpcMessage::Request(req)).await;

    let notif = JsonRpcNotification::new("initialized".into(), None);
    handler
        .handle_message(JsonRpcMessage::Notification(notif))
        .await;
}

async fn tool(handler: &ProtocolHandler, name: &str, args: serde_json::Value) -> serde_json::Value {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: RequestId::Number(1),
        method: "tools/call".into(),
        params: Some(json!({"name": name, "arguments": args})),
    };
    handler
        .handle_message(JsonRpcMessage::Request(req))
        .await
        .unwrap()
}

#[tokio::main]
async fn main() {
    let path = "/tmp/two_agents_demo.amem";

    println!("=== Agent A: Learning user preferences ===\n");
    {
        let handler = create_handler(path);
        init(&handler).await;

        tool(&handler, "session_start", json!({"session_id": 1})).await;

        let r1 = tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "User prefers Rust over Python",
                "confidence": 0.95
            }),
        )
        .await;
        println!("Agent A stored: User prefers Rust over Python");
        println!("  Response: {}\n", r1["result"]["content"][0]["text"]);

        let r2 = tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "User works on distributed systems",
                "confidence": 0.9
            }),
        )
        .await;
        println!("Agent A stored: User works on distributed systems");
        println!("  Response: {}\n", r2["result"]["content"][0]["text"]);

        tool(
            &handler,
            "session_end",
            json!({
                "create_episode": true,
                "summary": "Learned user language and domain preferences"
            }),
        )
        .await;
        println!("Agent A: Session ended with episode summary.\n");
    }

    println!("=== Agent B: Reading shared memory ===\n");
    {
        let handler = create_handler(path);
        init(&handler).await;

        let facts = tool(&handler, "memory_query", json!({"event_types": ["fact"]})).await;
        let text = facts["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();

        println!(
            "Agent B found {} facts from Agent A:",
            parsed["count"].as_u64().unwrap()
        );
        for node in parsed["nodes"].as_array().unwrap() {
            println!(
                "  - {} (confidence: {})",
                node["content"], node["confidence"]
            );
        }

        println!("\nAgent B: Successfully read Agent A's memories from shared .amem file!");
    }

    // Cleanup
    std::fs::remove_file(path).ok();
}
