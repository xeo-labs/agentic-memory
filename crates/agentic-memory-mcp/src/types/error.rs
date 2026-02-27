//! Error types and JSON-RPC error codes for the MCP server.

use super::message::{JsonRpcError, JsonRpcErrorObject, RequestId, JSONRPC_VERSION};

/// Standard JSON-RPC 2.0 error codes.
pub mod error_codes {
    /// Invalid JSON was received.
    pub const PARSE_ERROR: i32 = -32700;
    /// The JSON sent is not a valid Request object.
    pub const INVALID_REQUEST: i32 = -32600;
    /// The method does not exist / is not available.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid method parameter(s).
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal JSON-RPC error.
    pub const INTERNAL_ERROR: i32 = -32603;
}

/// MCP-specific error codes (per MCP spec).
pub mod mcp_error_codes {
    /// The request was cancelled by the client.
    pub const REQUEST_CANCELLED: i32 = -32800;
    /// Content too large to process.
    pub const CONTENT_TOO_LARGE: i32 = -32801;
    /// Resource not found.
    pub const RESOURCE_NOT_FOUND: i32 = -32802;
    /// Tool not found.
    pub const TOOL_NOT_FOUND: i32 = -32803;
    /// Prompt not found.
    pub const PROMPT_NOT_FOUND: i32 = -32804;
    /// AgenticMemory specific: Node not found.
    pub const NODE_NOT_FOUND: i32 = -32850;
    /// AgenticMemory specific: Session not found.
    pub const SESSION_NOT_FOUND: i32 = -32851;
    /// AgenticMemory specific: Invalid graph operation.
    pub const INVALID_GRAPH_OP: i32 = -32852;

    /// Server: Unauthorized (missing or invalid bearer token).
    pub const UNAUTHORIZED: i32 = -32900;
    /// Server: User not found (multi-tenant, missing X-User-ID header).
    pub const USER_NOT_FOUND: i32 = -32901;
    /// Server: Rate limited.
    pub const RATE_LIMITED: i32 = -32902;
}

/// All errors that can occur in the MCP server.
#[derive(thiserror::Error, Debug)]
pub enum McpError {
    /// Invalid JSON received.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Request object is malformed.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Method does not exist.
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Parameters are invalid.
    #[error("Invalid params: {0}")]
    InvalidParams(String),

    /// Internal server error.
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Request was cancelled by the client.
    #[error("Request cancelled")]
    RequestCancelled,

    /// Content exceeds size limits.
    #[error("Content too large: {size} bytes exceeds {max} bytes")]
    ContentTooLarge {
        /// Actual size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },

    /// MCP resource not found.
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    /// MCP tool not found.
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// MCP prompt not found.
    #[error("Prompt not found: {0}")]
    PromptNotFound(String),

    /// AgenticMemory node not found.
    #[error("Node not found: {0}")]
    NodeNotFound(u64),

    /// AgenticMemory session not found.
    #[error("Session not found: {0}")]
    SessionNotFound(u32),

    /// Invalid graph operation.
    #[error("Invalid graph operation: {0}")]
    InvalidGraphOp(String),

    /// Transport-level error.
    #[error("Transport error: {0}")]
    Transport(String),

    /// I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Error from the AgenticMemory core library.
    #[error("AgenticMemory error: {0}")]
    AgenticMemory(String),

    /// Unauthorized — missing or invalid bearer token.
    #[error("Unauthorized")]
    Unauthorized,

    /// User not found — missing X-User-ID header in multi-tenant mode.
    #[error("User not found: {0}")]
    UserNotFound(String),
}

impl McpError {
    /// Returns true if this is a protocol-level error (should be a JSON-RPC error).
    /// Tool execution errors (node not found, invalid graph op, etc.) return false
    /// and should use `ToolCallResult::error()` with `isError: true` instead.
    pub fn is_protocol_error(&self) -> bool {
        matches!(
            self,
            McpError::ParseError(_)
                | McpError::InvalidRequest(_)
                | McpError::MethodNotFound(_)
                | McpError::ToolNotFound(_)
                | McpError::RequestCancelled
                | McpError::ContentTooLarge { .. }
                | McpError::ResourceNotFound(_)
                | McpError::PromptNotFound(_)
                | McpError::Unauthorized
                | McpError::UserNotFound(_)
        )
    }

    /// Return the JSON-RPC error code for this error type.
    pub fn code(&self) -> i32 {
        use error_codes::*;
        use mcp_error_codes::*;
        match self {
            McpError::ParseError(_) => PARSE_ERROR,
            McpError::InvalidRequest(_) => INVALID_REQUEST,
            McpError::MethodNotFound(_) => METHOD_NOT_FOUND,
            McpError::InvalidParams(_) => INVALID_PARAMS,
            McpError::InternalError(_) => INTERNAL_ERROR,
            McpError::RequestCancelled => REQUEST_CANCELLED,
            McpError::ContentTooLarge { .. } => CONTENT_TOO_LARGE,
            McpError::ResourceNotFound(_) => RESOURCE_NOT_FOUND,
            McpError::ToolNotFound(_) => TOOL_NOT_FOUND,
            McpError::PromptNotFound(_) => PROMPT_NOT_FOUND,
            McpError::NodeNotFound(_) => NODE_NOT_FOUND,
            McpError::SessionNotFound(_) => SESSION_NOT_FOUND,
            McpError::InvalidGraphOp(_) => INVALID_GRAPH_OP,
            McpError::Transport(_) => INTERNAL_ERROR,
            McpError::Io(_) => INTERNAL_ERROR,
            McpError::Json(_) => PARSE_ERROR,
            McpError::AgenticMemory(_) => INTERNAL_ERROR,
            McpError::Unauthorized => UNAUTHORIZED,
            McpError::UserNotFound(_) => USER_NOT_FOUND,
        }
    }

    /// Convert this error into a JSON-RPC error response.
    pub fn to_json_rpc_error(&self, id: RequestId) -> JsonRpcError {
        JsonRpcError {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            error: JsonRpcErrorObject {
                code: self.code(),
                message: self.to_string(),
                data: None,
            },
        }
    }
}

impl From<agentic_memory::AmemError> for McpError {
    fn from(e: agentic_memory::AmemError) -> Self {
        McpError::AgenticMemory(e.to_string())
    }
}

/// Convenience result type for MCP operations.
pub type McpResult<T> = Result<T, McpError>;
