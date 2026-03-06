//! Main request dispatcher — receives JSON-RPC messages, routes to handlers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use serde_json::{json, Value};

use crate::prompts::PromptRegistry;
use crate::resources::ResourceRegistry;
use crate::session::SessionManager;
#[cfg(feature = "v3")]
use crate::tools::v3_tools::{self, SharedEngine};
use crate::tools::ToolRegistry;
use crate::types::*;
#[cfg(feature = "v3")]
use crate::v3_auto_capture::AutoCaptureMiddleware;
#[cfg(feature = "v3")]
use agentic_memory::v3::{EngineConfig, MemoryEngineV3};

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
    /// Tracks which session has already had its deterministic resume hook executed.
    last_resumed_session: Arc<Mutex<Option<u32>>>,
    tool_surface: ToolSurface,
    /// V3 engine for immortal capture/retrieval tools.
    #[cfg(feature = "v3")]
    v3_engine: SharedEngine,
    /// V3 middleware that captures tool calls/results deterministically.
    #[cfg(feature = "v3")]
    v3_auto_capture: Arc<AutoCaptureMiddleware>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolSurface {
    Full,
    Compact,
}

impl ToolSurface {
    fn from_env() -> Self {
        let raw = std::env::var("AMEM_MCP_TOOL_SURFACE")
            .ok()
            .or_else(|| std::env::var("MCP_TOOL_SURFACE").ok())
            .unwrap_or_else(|| "full".to_string());
        match raw.trim().to_ascii_lowercase().as_str() {
            "compact" => Self::Compact,
            _ => Self::Full,
        }
    }
}

impl ProtocolHandler {
    /// Create a new protocol handler with the given session manager.
    pub fn new(session: Arc<Mutex<SessionManager>>) -> Self {
        #[cfg(feature = "v3")]
        let v3_engine = init_v3_engine_from_env();
        #[cfg(feature = "v3")]
        let v3_auto_capture = Arc::new(AutoCaptureMiddleware::with_defaults(v3_engine.clone()));
        Self {
            session,
            capabilities: Arc::new(Mutex::new(NegotiatedCapabilities::default())),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            memory_mode: MemoryMode::Smart,
            auto_session_started: AtomicBool::new(false),
            last_resumed_session: Arc::new(Mutex::new(None)),
            tool_surface: ToolSurface::from_env(),
            #[cfg(feature = "v3")]
            v3_engine,
            #[cfg(feature = "v3")]
            v3_auto_capture,
        }
    }

    /// Create a new protocol handler with a specific memory mode.
    pub fn with_mode(session: Arc<Mutex<SessionManager>>, mode: MemoryMode) -> Self {
        #[cfg(feature = "v3")]
        let v3_engine = init_v3_engine_from_env();
        #[cfg(feature = "v3")]
        let v3_auto_capture = Arc::new(AutoCaptureMiddleware::with_defaults(v3_engine.clone()));
        Self {
            session,
            capabilities: Arc::new(Mutex::new(NegotiatedCapabilities::with_mode(mode))),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            memory_mode: mode,
            auto_session_started: AtomicBool::new(false),
            last_resumed_session: Arc::new(Mutex::new(None)),
            tool_surface: ToolSurface::from_env(),
            #[cfg(feature = "v3")]
            v3_engine,
            #[cfg(feature = "v3")]
            v3_auto_capture,
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
                    drop(session);
                    self.ensure_resume_hook_for_active_session().await;
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
            tools: match self.tool_surface {
                ToolSurface::Full => ToolRegistry::list_tools(),
                ToolSurface::Compact => ToolRegistry::list_tools_compact(),
            },
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
        let tool_input = call_params
            .arguments
            .clone()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        self.ensure_resume_hook_for_active_session().await;

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

        #[cfg(feature = "v3")]
        let started = std::time::Instant::now();
        #[cfg(feature = "v3")]
        self.v3_auto_capture
            .on_tool_call(&call_params.name, &tool_input)
            .await;

        // Classify errors: protocol errors (ToolNotFound etc.) become JSON-RPC errors;
        // tool execution errors (NodeNotFound, InvalidGraphOp, etc.) become isError: true.
        let result = {
            #[cfg(feature = "v3")]
            let v3_try =
                v3_tools::dispatch_v3_tool(&call_params.name, tool_input.clone(), &self.v3_engine)
                    .await;

            #[cfg(not(feature = "v3"))]
            let v3_try: Option<McpResult<ToolCallResult>> = None;

            match v3_try {
                Some(Ok(r)) => r,
                Some(Err(e)) if e.is_protocol_error() => {
                    #[cfg(feature = "v3")]
                    self.v3_auto_capture
                        .on_tool_result(
                            &call_params.name,
                            tool_input.clone(),
                            json!({"error": e.to_string(), "protocol_error": true}),
                            started.elapsed().as_millis() as u64,
                            false,
                        )
                        .await;
                    return Err(e);
                }
                Some(Err(e)) => ToolCallResult::error(e.to_string()),
                None => match ToolRegistry::call(
                    &call_params.name,
                    call_params.arguments,
                    &self.session,
                )
                .await
                {
                    Ok(r) => r,
                    Err(e) if e.is_protocol_error() => {
                        #[cfg(feature = "v3")]
                        self.v3_auto_capture
                            .on_tool_result(
                                &call_params.name,
                                tool_input.clone(),
                                json!({"error": e.to_string(), "protocol_error": true}),
                                started.elapsed().as_millis() as u64,
                                false,
                            )
                            .await;
                        return Err(e);
                    }
                    Err(e) => ToolCallResult::error(e.to_string()),
                },
            }
        };

        #[cfg(feature = "v3")]
        self.v3_auto_capture
            .on_tool_result(
                &call_params.name,
                tool_input.clone(),
                serde_json::to_value(&result)
                    .unwrap_or_else(|_| json!({"error": "serialize_failed"})),
                started.elapsed().as_millis() as u64,
                !result.is_error.unwrap_or(false),
            )
            .await;

        self.auto_log_tool_turn(&call_params.name, Some(&tool_input), &result)
            .await;
        if call_params.name == "session_start" {
            self.ensure_resume_hook_for_active_session().await;
        }

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

        #[cfg(feature = "v3")]
        let result =
            ResourceRegistry::read_with_v3(&read_params.uri, &self.session, Some(&self.v3_engine))
                .await?;
        #[cfg(not(feature = "v3"))]
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
        let prompt_args = get_params
            .arguments
            .clone()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        self.ensure_resume_hook_for_active_session().await;

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
        self.auto_log_prompt_turn(&get_params.name, Some(&prompt_args))
            .await;

        serde_json::to_value(result).map_err(|e| McpError::InternalError(e.to_string()))
    }

    async fn ensure_resume_hook_for_active_session(&self) {
        if self.memory_mode == MemoryMode::Minimal {
            return;
        }

        let current_session = {
            let session = self.session.lock().await;
            session.current_session_id()
        };

        let already_resumed = {
            let guard = self.last_resumed_session.lock().await;
            guard.is_some_and(|sid| sid == current_session)
        };
        if already_resumed {
            return;
        }

        match ToolRegistry::call(
            "memory_session_resume",
            Some(json!({ "limit": 15 })),
            &self.session,
        )
        .await
        {
            Ok(_) => {
                let mut guard = self.last_resumed_session.lock().await;
                *guard = Some(current_session);
            }
            Err(e) => tracing::warn!(
                "Deterministic resume hook failed for session {}: {}",
                current_session,
                e
            ),
        }
    }

    async fn auto_log_tool_turn(
        &self,
        tool_name: &str,
        arguments: Option<&Value>,
        result: &ToolCallResult,
    ) {
        if tool_name == "conversation_log"
            || tool_name.starts_with("memory_")
            || tool_name.starts_with("session_")
        {
            return;
        }
        if self.memory_mode == MemoryMode::Minimal {
            return;
        }

        let Some(user_message) = summarize_substantive_input(arguments) else {
            return;
        };

        let success = !result.is_error.unwrap_or(false);
        let detail = first_text_content(&result.content)
            .map(|v| truncate_for_log(v, 220))
            .unwrap_or_else(|| "no textual output".to_string());
        let agent_response = format!("tool={tool_name} success={success} detail={detail}");

        let _ = crate::tools::conversation_log::execute(
            json!({
                "user_message": user_message,
                "agent_response": truncate_for_log(&agent_response, 320),
                "topic": "auto-tool-turn"
            }),
            &self.session,
        )
        .await;
    }

    async fn auto_log_prompt_turn(&self, prompt_name: &str, arguments: Option<&Value>) {
        if self.memory_mode == MemoryMode::Minimal {
            return;
        }
        let Some(user_message) = summarize_substantive_input(arguments) else {
            return;
        };
        let agent_response = format!("prompt={prompt_name} expanded");
        let _ = crate::tools::conversation_log::execute(
            json!({
                "user_message": user_message,
                "agent_response": agent_response,
                "topic": "auto-prompt-turn"
            }),
            &self.session,
        )
        .await;
    }
}

fn summarize_substantive_input(arguments: Option<&Value>) -> Option<String> {
    let args = arguments?;
    let mut snippets = Vec::new();
    collect_string_fields(
        args,
        "",
        &mut snippets,
        &[
            "query",
            "content",
            "message",
            "prompt",
            "information",
            "context",
            "topic",
            "reason",
            "instruction",
            "user_message",
        ],
        4,
    );
    if snippets.is_empty() {
        return None;
    }
    let joined = snippets.join(" | ");
    if is_trivial_text(&joined) {
        return None;
    }
    Some(truncate_for_log(&joined, 320))
}

fn collect_string_fields(
    value: &Value,
    path: &str,
    out: &mut Vec<String>,
    preferred_keys: &[&str],
    max_items: usize,
) {
    if out.len() >= max_items {
        return;
    }
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if out.len() >= max_items {
                    break;
                }
                let next = if path.is_empty() {
                    k.to_string()
                } else {
                    format!("{path}.{k}")
                };
                let key_match = preferred_keys
                    .iter()
                    .any(|needle| k.eq_ignore_ascii_case(needle) || next.ends_with(needle));
                if key_match {
                    if let Some(s) = v.as_str() {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            out.push(format!("{next}={}", truncate_for_log(trimmed, 120)));
                        }
                    }
                }
                collect_string_fields(v, &next, out, preferred_keys, max_items);
            }
        }
        Value::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                if out.len() >= max_items {
                    break;
                }
                collect_string_fields(
                    item,
                    &format!("{path}[{idx}]"),
                    out,
                    preferred_keys,
                    max_items,
                );
            }
        }
        _ => {}
    }
}

fn first_text_content(content: &[ToolContent]) -> Option<&str> {
    for item in content {
        if let ToolContent::Text { text } = item {
            if !text.trim().is_empty() {
                return Some(text.trim());
            }
        }
    }
    None
}

fn truncate_for_log(input: &str, max_chars: usize) -> String {
    let mut out = input.trim().to_string();
    if out.chars().count() <= max_chars {
        return out;
    }
    out = out.chars().take(max_chars).collect();
    out.push('…');
    out
}

fn is_trivial_text(input: &str) -> bool {
    let lower = input.trim().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "ok" | "okay" | "thanks" | "thank you" | "got it" | "great"
    )
}

#[cfg(feature = "v3")]
fn init_v3_engine_from_env() -> SharedEngine {
    let data_dir = std::env::var("AMEM_V3_DATA_DIR")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| ".agentic/memory-v3".to_string());
    let cfg = EngineConfig {
        data_dir: std::path::PathBuf::from(data_dir),
        ..EngineConfig::default()
    };

    let engine = MemoryEngineV3::open_with_recovery(cfg.clone())
        .or_else(|_| MemoryEngineV3::open(cfg))
        .ok();
    if engine.is_none() {
        tracing::warn!("V3 engine unavailable; V3 tool calls will return initialization errors");
    }
    Arc::new(Mutex::new(engine))
}
