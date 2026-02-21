//! Mock MCP client for integration testing.
#![allow(dead_code)]

use serde_json::{json, Value};

use agentic_memory_mcp::protocol::ProtocolHandler;
use agentic_memory_mcp::types::*;

/// A mock MCP client that sends requests and collects responses.
pub struct MockClient {
    handler: ProtocolHandler,
    next_id: i64,
}

impl MockClient {
    /// Create a new mock client wrapping a protocol handler.
    pub fn new(handler: ProtocolHandler) -> Self {
        Self {
            handler,
            next_id: 1,
        }
    }

    /// Send a request and return the response.
    pub async fn request(&mut self, method: &str, params: Option<Value>) -> Value {
        let id = self.next_id;
        self.next_id += 1;

        let msg = JsonRpcMessage::Request(JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Number(id),
            method: method.to_string(),
            params,
        });

        self.handler
            .handle_message(msg)
            .await
            .expect("Request should produce a response")
    }

    /// Send a notification (no response expected).
    pub async fn notify(&mut self, method: &str, params: Option<Value>) {
        let msg =
            JsonRpcMessage::Notification(JsonRpcNotification::new(method.to_string(), params));

        let response = self.handler.handle_message(msg).await;
        assert!(
            response.is_none(),
            "Notifications should not produce responses"
        );
    }

    /// Perform the standard MCP initialization handshake.
    pub async fn initialize(&mut self) -> Value {
        let response = self
            .request(
                "initialize",
                Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "mock-client", "version": "1.0.0" }
                })),
            )
            .await;

        // Send initialized notification
        self.notify("initialized", None).await;

        response
    }

    /// Call a tool and return the response.
    pub async fn call_tool(&mut self, name: &str, arguments: Option<Value>) -> Value {
        self.request(
            "tools/call",
            Some(json!({
                "name": name,
                "arguments": arguments.unwrap_or(json!({}))
            })),
        )
        .await
    }

    /// Read a resource by URI.
    pub async fn read_resource(&mut self, uri: &str) -> Value {
        self.request("resources/read", Some(json!({ "uri": uri })))
            .await
    }

    /// Get an expanded prompt.
    pub async fn get_prompt(&mut self, name: &str, arguments: Option<Value>) -> Value {
        self.request(
            "prompts/get",
            Some(json!({
                "name": name,
                "arguments": arguments.unwrap_or(json!({}))
            })),
        )
        .await
    }

    /// Send shutdown request.
    pub async fn shutdown(&mut self) -> Value {
        self.request("shutdown", None).await
    }
}
