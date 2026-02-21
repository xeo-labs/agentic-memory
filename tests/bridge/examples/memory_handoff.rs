//! Example: Session handoff between agents.
//!
//! Agent A starts debugging a problem, then hands off to Agent B
//! who continues in the same session. Demonstrates seamless continuity.

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
            "clientInfo": {"name": "handoff_example", "version": "1.0"}
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

fn extract_id(resp: &serde_json::Value) -> u64 {
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    parsed["node_id"].as_u64().unwrap()
}

#[tokio::main]
async fn main() {
    let path = "/tmp/handoff_demo.amem";

    println!("=== Agent A: Starting debug session ===\n");
    {
        let handler = create_handler(path);
        init(&handler).await;

        tool(&handler, "session_start", json!({"session_id": 42})).await;

        let r1 = tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "Users report 500 errors on /api/checkout"
            }),
        )
        .await;
        let fact_id = extract_id(&r1);
        println!(
            "Agent A: Observed - 500 errors on /api/checkout (node {})",
            fact_id
        );

        tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "inference",
                "content": "Database connection pool may be exhausted under load",
                "edges": [{"target_id": fact_id, "edge_type": "caused_by"}]
            }),
        )
        .await;
        println!("Agent A: Inferred - DB connection pool exhaustion");

        // Hand off without episode (session stays open)
        tool(&handler, "session_end", json!({"create_episode": false})).await;
        println!("\nAgent A: Handing off to Agent B (session 42 persisted)\n");
    }

    println!("=== Agent B: Continuing debug session ===\n");
    {
        let handler = create_handler(path);
        init(&handler).await;

        // Resume session 42
        tool(&handler, "session_start", json!({"session_id": 42})).await;

        // Read what Agent A discovered
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: RequestId::Number(1),
            method: "resources/read".into(),
            params: Some(json!({"uri": "amem://session/42"})),
        };
        let resource_resp = handler
            .handle_message(JsonRpcMessage::Request(req))
            .await
            .unwrap();
        let text = resource_resp["result"]["contents"][0]["text"]
            .as_str()
            .unwrap();
        let session_data: serde_json::Value = serde_json::from_str(text).unwrap();
        println!(
            "Agent B: Found {} nodes from Agent A's investigation",
            session_data["node_count"]
        );

        for node in session_data["nodes"].as_array().unwrap() {
            println!(
                "  [{:>10}] {}",
                node["event_type"].as_str().unwrap(),
                node["content"].as_str().unwrap()
            );
        }

        // Agent B adds resolution
        tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "decision",
                "content": "Increased DB pool from 10 to 50 connections"
            }),
        )
        .await;
        println!("\nAgent B: Applied fix - increased DB pool to 50");

        tool(
            &handler,
            "memory_add",
            json!({
                "event_type": "fact",
                "content": "500 errors resolved after pool increase"
            }),
        )
        .await;
        println!("Agent B: Confirmed - 500 errors resolved");

        // Close session with summary
        tool(
            &handler,
            "session_end",
            json!({
                "create_episode": true,
                "summary": "Debug session: 500 errors on checkout caused by DB pool exhaustion. Fixed by increasing pool from 10 to 50."
            }),
        )
        .await;
        println!("\nAgent B: Session 42 closed with episode summary.\n");
    }

    // Verify everything
    println!("=== Verification: Full memory state ===\n");
    {
        let handler = create_handler(path);
        init(&handler).await;

        let stats = tool(&handler, "memory_stats", json!({})).await;
        let text = stats["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        println!("Total nodes: {}", parsed["node_count"]);
        println!("Total edges: {}", parsed["edge_count"]);
        println!("File size: {} bytes", parsed["file_size_bytes"]);
    }

    // Cleanup
    std::fs::remove_file(path).ok();
}
