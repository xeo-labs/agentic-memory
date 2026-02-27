//! Main request dispatcher — receives JSON-RPC messages, routes to handlers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use serde_json::Value;

use crate::prompts::PromptRegistry;
use crate::resources::ResourceRegistry;
use crate::session::SessionManager;
use crate::tools::ToolRegistry;
use crate::types::*;

use super::negotiation::NegotiatedCapabilities;
use super::validator::validate_request;

/// The main protocol handler that dispatches incoming JSON-RPC messages.
pub struct ProtocolHandler {
    session: Arc<Mutex<SessionManager>>,
    capabilities: Arc<Mutex<NegotiatedCapabilities>>,
    shutdown_requested: Arc<AtomicBool>,
    memory_mode: MemoryMode,
    /// Tracks whether an auto-session was started so we can auto-end it.
    auto_session_started: AtomicBool,
}

impl ProtocolHandler {
    /// Create a new protocol handler with the given session manager.
    pub fn new(session: Arc<Mutex<SessionManager>>) -> Self {
        Self {
            session,
            capabilities: Arc::new(Mutex::new(NegotiatedCapabilities::default())),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            memory_mode: MemoryMode::Smart,
            auto_session_started: AtomicBool::new(false),
        }
    }

    /// Create a new protocol handler with a specific memory mode.
    pub fn with_mode(session: Arc<Mutex<SessionManager>>, mode: MemoryMode) -> Self {
        Self {
            session,
            capabilities: Arc::new(Mutex::new(NegotiatedCapabilities::with_mode(mode))),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            memory_mode: mode,
            auto_session_started: AtomicBool::new(false),
        }
    }

    /// Returns true once a shutdown request has been handled.
    pub fn shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::Relaxed)
    }

    /// Handle an incoming JSON-RPC message and optionally return a response.
    pub async fn handle_message(&self, msg: JsonRpcMessage) -> Option<Value> {
        match msg {
            JsonRpcMessage::Request(req) => Some(self.handle_request(req).await),
            JsonRpcMessage::Notification(notif) => {
                self.handle_notification(notif).await;
                None
            }
            _ => {
                // Responses and errors from the client are unexpected
                tracing::warn!("Received unexpected message type from client");
                None
            }
        }
    }

    /// Cleanup on transport close (EOF). Auto-ends session if one was started.
    pub async fn cleanup(&self) {
        if !self.auto_session_started.load(Ordering::Relaxed) {
            return;
        }

        let mut session = self.session.lock().await;
        let sid = session.current_session_id();
        match session.end_session_with_episode(sid, "Session ended: MCP connection closed") {
            Ok(episode_id) => {
                tracing::info!("Auto-ended session {sid} on EOF, episode node {episode_id}");
            }
            Err(e) => {
                tracing::warn!("Failed to auto-end session on EOF: {e}");
                if let Err(save_err) = session.save() {
                    tracing::error!("Failed to save on EOF cleanup: {save_err}");
                }
            }
        }
        self.auto_session_started.store(false, Ordering::Relaxed);
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> Value {
        // Validate JSON-RPC structure
        if let Err(e) = validate_request(&request) {
            return serde_json::to_value(e.to_json_rpc_error(request.id)).unwrap_or_default();
        }

        let id = request.id.clone();
        let result = self.dispatch_request(&request).await;

        match result {
            Ok(value) => serde_json::to_value(JsonRpcResponse::new(id, value)).unwrap_or_default(),
            Err(e) => serde_json::to_value(e.to_json_rpc_error(id)).unwrap_or_default(),
        }
    }

    async fn dispatch_request(&self, request: &JsonRpcRequest) -> McpResult<Value> {
        match request.method.as_str() {
            // Lifecycle
            "initialize" => self.handle_initialize(request.params.clone()).await,
            "shutdown" => self.handle_shutdown().await,

            // Tools
            "tools/list" => self.handle_tools_list().await,
            "tools/call" => self.handle_tools_call(request.params.clone()).await,

            // Resources
            "resources/list" => self.handle_resources_list().await,
            "resources/templates/list" => self.handle_resource_templates_list().await,
            "resources/read" => self.handle_resources_read(request.params.clone()).await,
            "resources/subscribe" => Ok(Value::Object(serde_json::Map::new())),
            "resources/unsubscribe" => Ok(Value::Object(serde_json::Map::new())),

            // Prompts
            "prompts/list" => self.handle_prompts_list().await,
            "prompts/get" => self.handle_prompts_get(request.params.clone()).await,

            // Ping
            "ping" => Ok(Value::Object(serde_json::Map::new())),

            _ => Err(McpError::MethodNotFound(request.method.clone())),
        }
    }

    async fn handle_notification(&self, notification: JsonRpcNotification) {
        match notification.method.as_str() {
            "initialized" => {
                let mut caps = self.capabilities.lock().await;
                if let Err(e) = caps.mark_initialized() {
                    tracing::error!("Failed to mark initialized: {e}");
                }

                // Auto-start session when client confirms connection (smart/full mode).
                if self.memory_mode != MemoryMode::Minimal {
                    let mut session = self.session.lock().await;
                    match session.start_session(None) {
                        Ok(sid) => {
                            self.auto_session_started.store(true, Ordering::Relaxed);
                            tracing::info!(
                                "Auto-started session {sid} (mode={:?})",
                                self.memory_mode
                            );
                        }
                        Err(e) => {
                            tracing::error!("Failed to auto-start session: {e}");
                        }
                    }
                }
            }
            "notifications/cancelled" | "$/cancelRequest" => {
                tracing::info!("Received cancellation notification");
            }
            _ => {
                tracing::debug!("Unknown notification: {}", notification.method);
            }
        }
    }

    async fn handle_initialize(&self, params: Option<Value>) -> McpResult<Value> {
        let init_params: InitializeParams = params
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| McpError::InvalidParams(e.to_string()))?
            .ok_or_else(|| McpError::InvalidParams("Initialize params required".to_string()))?;

        let mut caps = self.capabilities.lock().await;
        let result = caps.negotiate(init_params)?;

        serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
    }

    async fn handle_shutdown(&self) -> McpResult<Value> {
        tracing::info!("Shutdown requested");

        let mut session = self.session.lock().await;

        // Auto-end session with episode summary if one was auto-started.
        if self.auto_session_started.swap(false, Ordering::Relaxed) {
            let sid = session.current_session_id();
            match session.end_session_with_episode(sid, "Session ended: MCP client shutdown") {
                Ok(episode_id) => {
                    tracing::info!("Auto-ended session {sid}, episode node {episode_id}");
                }
                Err(e) => {
                    tracing::warn!("Failed to auto-end session on shutdown: {e}");
                    session.save()?;
                }
            }
        } else {
            session.save()?;
        }

        self.shutdown_requested.store(true, Ordering::Relaxed);
        Ok(Value::Object(serde_json::Map::new()))
    }

    async fn handle_tools_list(&self) -> McpResult<Value> {
        let result = ToolListResult {
            tools: ToolRegistry::list_tools(),
            next_cursor: None,
        };
        serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
    }

    async fn handle_tools_call(&self, params: Option<Value>) -> McpResult<Value> {
        let call_params: ToolCallParams = params
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| McpError::InvalidParams(e.to_string()))?
            .ok_or_else(|| McpError::InvalidParams("Tool call params required".to_string()))?;

        {
            let mut session = self.session.lock().await;
            if let Err(e) =
                session.capture_tool_call(&call_params.name, call_params.arguments.as_ref())
            {
                tracing::warn!(
                    "Auto-capture skipped for tool {} due to error: {}",
                    call_params.name,
                    e
                );
            }
        }

        // Classify errors: protocol errors (ToolNotFound etc.) become JSON-RPC errors;
        // tool execution errors (NodeNotFound, InvalidGraphOp, etc.) become isError: true.
        let result =
            match ToolRegistry::call(&call_params.name, call_params.arguments, &self.session).await
            {
                Ok(r) => r,
                Err(e) if e.is_protocol_error() => return Err(e),
                Err(e) => ToolCallResult::error(e.to_string()),
            };

        serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
    }

    async fn handle_resources_list(&self) -> McpResult<Value> {
        let result = ResourceListResult {
            resources: ResourceRegistry::list_resources(),
            next_cursor: None,
        };
        serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
    }

    async fn handle_resource_templates_list(&self) -> McpResult<Value> {
        let result = ResourceTemplateListResult {
            resource_templates: ResourceRegistry::list_templates(),
            next_cursor: None,
        };
        serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
    }

    async fn handle_resources_read(&self, params: Option<Value>) -> McpResult<Value> {
        let read_params: ResourceReadParams = params
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| McpError::InvalidParams(e.to_string()))?
            .ok_or_else(|| McpError::InvalidParams("Resource read params required".to_string()))?;

        let result = ResourceRegistry::read(&read_params.uri, &self.session).await?;

        serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
    }

    async fn handle_prompts_list(&self) -> McpResult<Value> {
        let result = PromptListResult {
            prompts: PromptRegistry::list_prompts(),
            next_cursor: None,
        };
        serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
    }

    async fn handle_prompts_get(&self, params: Option<Value>) -> McpResult<Value> {
        let get_params: PromptGetParams = params
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| McpError::InvalidParams(e.to_string()))?
            .ok_or_else(|| McpError::InvalidParams("Prompt get params required".to_string()))?;

        {
            let mut session = self.session.lock().await;
            if let Err(e) =
                session.capture_prompt_request(&get_params.name, get_params.arguments.as_ref())
            {
                tracing::warn!(
                    "Auto-capture skipped for prompt {} due to error: {}",
                    get_params.name,
                    e
                );
            }
        }

        let result =
            PromptRegistry::get(&get_params.name, get_params.arguments, &self.session).await?;

        serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
    }
}
