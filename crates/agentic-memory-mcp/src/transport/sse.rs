//! SSE transport — HTTP server with auth, multi-tenant routing, and /health.

#[cfg(feature = "sse")]
use std::path::PathBuf;
#[cfg(feature = "sse")]
use std::sync::Arc;

#[cfg(feature = "sse")]
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Json as AxumJson, Response},
    routing::{get, post},
    Router,
};

#[cfg(feature = "sse")]
use tokio::sync::Mutex;

#[cfg(feature = "sse")]
use crate::protocol::ProtocolHandler;
#[cfg(feature = "sse")]
use crate::session::tenant::TenantRegistry;
#[cfg(feature = "sse")]
use crate::types::{McpResult, MemoryMode};

/// Server operating mode.
#[cfg(feature = "sse")]
pub enum ServerMode {
    /// Single-user: one brain file, one handler.
    Single(Arc<ProtocolHandler>),
    /// Multi-tenant: per-user brain files in a data directory.
    MultiTenant {
        data_dir: PathBuf,
        registry: Arc<Mutex<TenantRegistry>>,
        memory_mode: MemoryMode,
    },
}

/// Shared server state passed to all handlers via axum State.
#[cfg(feature = "sse")]
pub struct ServerState {
    pub token: Option<String>,
    pub mode: ServerMode,
}

/// SSE transport for web-based MCP clients.
#[cfg(feature = "sse")]
pub struct SseTransport {
    state: Arc<ServerState>,
}

#[cfg(feature = "sse")]
impl SseTransport {
    /// Create a single-user SSE transport (backward compatible).
    pub fn new(handler: ProtocolHandler) -> Self {
        Self {
            state: Arc::new(ServerState {
                token: None,
                mode: ServerMode::Single(Arc::new(handler)),
            }),
        }
    }

    /// Create an SSE transport with full configuration.
    pub fn with_config(token: Option<String>, mode: ServerMode) -> Self {
        Self {
            state: Arc::new(ServerState { token, mode }),
        }
    }

    /// Run the HTTP server on the given address.
    pub async fn run(&self, addr: &str) -> McpResult<()> {
        let state = self.state.clone();

        let app = Router::new()
            .route("/mcp", post(handle_request))
            .layer(middleware::from_fn_with_state(state.clone(), auth_layer))
            .route("/health", get(handle_health))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(crate::types::McpError::Io)?;

        tracing::info!("HTTP transport listening on {addr}");

        axum::serve(listener, app)
            .await
            .map_err(|e| crate::types::McpError::Transport(e.to_string()))?;

        Ok(())
    }
}

/// Auth middleware — checks Bearer token if configured.
/// /health is handled by a separate route that bypasses this layer.
#[cfg(feature = "sse")]
async fn auth_layer(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: middleware::Next,
) -> Response {
    if let Some(expected) = &state.token {
        let authorized = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|token| token == expected);

        if !authorized {
            return (
                StatusCode::UNAUTHORIZED,
                AxumJson(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32900,
                        "message": "Unauthorized"
                    }
                })),
            )
                .into_response();
        }
    }

    next.run(request).await
}

/// Handle JSON-RPC requests. In multi-tenant mode, routes by X-User-ID header.
#[cfg(feature = "sse")]
async fn handle_request(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    AxumJson(body): AxumJson<serde_json::Value>,
) -> Result<AxumJson<serde_json::Value>, Response> {
    let handler = match &state.mode {
        ServerMode::Single(handler) => handler.clone(),
        ServerMode::MultiTenant {
            data_dir: _,
            registry,
            memory_mode,
        } => {
            let user_id = headers
                .get("x-user-id")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        AxumJson(serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": null,
                            "error": {
                                "code": -32901,
                                "message": "Missing X-User-ID header (required in multi-tenant mode)"
                            }
                        })),
                    )
                        .into_response()
                })?;

            let session = {
                let mut reg = registry.lock().await;
                reg.get_or_create(user_id).map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        AxumJson(serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": null,
                            "error": {
                                "code": -32603,
                                "message": format!("Failed to open brain for user '{user_id}': {e}")
                            }
                        })),
                    )
                        .into_response()
                })?
            };

            Arc::new(ProtocolHandler::with_mode(session, *memory_mode))
        }
    };

    let msg: crate::types::JsonRpcMessage = serde_json::from_value(body).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            AxumJson(serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32700,
                    "message": "Parse error"
                }
            })),
        )
            .into_response()
    })?;

    match handler.handle_message(msg).await {
        Some(response) => Ok(AxumJson(response)),
        None => Ok(AxumJson(serde_json::Value::Null)),
    }
}

/// Health check endpoint — no auth required.
#[cfg(feature = "sse")]
async fn handle_health(State(state): State<Arc<ServerState>>) -> AxumJson<serde_json::Value> {
    let profile = std::env::var("AMEM_AUTONOMIC_PROFILE")
        .unwrap_or_else(|_| "desktop".to_string())
        .trim()
        .to_ascii_lowercase();
    let migration_policy = std::env::var("AMEM_STORAGE_MIGRATION_POLICY")
        .unwrap_or_else(|_| "auto-safe".to_string())
        .trim()
        .to_ascii_lowercase();
    let ledger_dir = std::env::var("AMEM_HEALTH_LEDGER_DIR")
        .ok()
        .or_else(|| std::env::var("AGENTRA_HEALTH_LEDGER_DIR").ok())
        .unwrap_or_else(|| "~/.agentra/health-ledger".to_string());

    let mut health = serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "autonomic": {
            "profile": profile,
            "migration_policy": migration_policy,
            "health_ledger_dir": ledger_dir,
        }
    });

    if let ServerMode::MultiTenant { registry, .. } = &state.mode {
        let reg = registry.lock().await;
        health["users"] = serde_json::json!(reg.count());
    }

    AxumJson(health)
}
