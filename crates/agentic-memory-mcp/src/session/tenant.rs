//! Multi-tenant session registry — lazy-loads per-user brain files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use super::autosave::spawn_maintenance;
use super::SessionManager;
use crate::types::McpResult;

/// Registry of per-user sessions for multi-tenant mode.
pub struct TenantRegistry {
    data_dir: PathBuf,
    sessions: HashMap<String, Arc<Mutex<SessionManager>>>,
}

impl TenantRegistry {
    /// Create a new tenant registry backed by the given data directory.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            sessions: HashMap::new(),
        }
    }

    /// Get or create a session for the given user ID.
    ///
    /// On first access, creates `{data_dir}/{user_id}.amem` and opens a session.
    pub fn get_or_create(&mut self, user_id: &str) -> McpResult<Arc<Mutex<SessionManager>>> {
        if let Some(session) = self.sessions.get(user_id) {
            return Ok(session.clone());
        }

        // Ensure data directory exists
        std::fs::create_dir_all(&self.data_dir).map_err(|e| {
            crate::types::McpError::InternalError(format!(
                "Failed to create data dir {}: {e}",
                self.data_dir.display()
            ))
        })?;

        let brain_path = self.data_dir.join(format!("{user_id}.amem"));
        let path_str = brain_path.display().to_string();

        tracing::info!("Opening brain for user '{user_id}': {path_str}");

        let session = SessionManager::open(&path_str)?;
        let maintenance_interval = session.maintenance_interval();
        let session = Arc::new(Mutex::new(session));
        let _maintenance_task = spawn_maintenance(session.clone(), maintenance_interval);
        self.sessions.insert(user_id.to_string(), session.clone());

        Ok(session)
    }

    /// Number of active tenant sessions.
    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}
