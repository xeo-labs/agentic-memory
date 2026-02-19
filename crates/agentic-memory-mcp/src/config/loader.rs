//! Configuration loading from file, environment, and CLI arguments.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::McpResult;

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Path to the .amem memory file.
    pub memory_path: String,
    /// Transport type ("stdio" or "sse").
    #[serde(default = "default_transport")]
    pub transport: String,
    /// SSE listen address (only used when transport is "sse").
    #[serde(default = "default_sse_addr")]
    pub sse_addr: String,
    /// Auto-save interval in seconds.
    #[serde(default = "default_auto_save_interval")]
    pub auto_save_interval: u64,
    /// Log level.
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_transport() -> String {
    "stdio".to_string()
}

fn default_sse_addr() -> String {
    "127.0.0.1:3000".to_string()
}

fn default_auto_save_interval() -> u64 {
    30
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            memory_path: resolve_default_memory_path(),
            transport: default_transport(),
            sse_addr: default_sse_addr(),
            auto_save_interval: default_auto_save_interval(),
            log_level: default_log_level(),
        }
    }
}

/// Load configuration from a TOML file.
pub fn load_config(path: &str) -> McpResult<ServerConfig> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        crate::types::McpError::Io(std::io::Error::other(format!(
            "Failed to read config file {path}: {e}"
        )))
    })?;

    toml::from_str(&content)
        .map_err(|e| crate::types::McpError::InternalError(format!("Failed to parse config: {e}")))
}

/// Resolve the memory file path using priority order:
/// 1. Explicit path (CLI arg)
/// 2. AMEM_BRAIN environment variable
/// 3. .amem/brain.amem in current directory
/// 4. ~/.brain.amem (global default)
pub fn resolve_memory_path(explicit: Option<&str>) -> String {
    if let Some(path) = explicit {
        return path.to_string();
    }

    if let Ok(env_path) = std::env::var("AMEM_BRAIN") {
        return env_path;
    }

    let cwd_brain = PathBuf::from(".amem/brain.amem");
    if cwd_brain.exists() {
        return cwd_brain.display().to_string();
    }

    resolve_default_memory_path()
}

fn resolve_default_memory_path() -> String {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());

    format!("{home}/.brain.amem")
}
