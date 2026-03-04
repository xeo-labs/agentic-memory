//! Resource registration and dispatch for MCP resources.

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::session::SessionManager;
use crate::types::{
    McpError, McpResult, ReadResourceResult, ResourceDefinition, ResourceTemplateDefinition,
};

use super::{graph, node, session, templates, type_index};

/// Registry of all available MCP resources.
pub struct ResourceRegistry;

impl ResourceRegistry {
    /// List all resource URI templates.
    pub fn list_templates() -> Vec<ResourceTemplateDefinition> {
        templates::list_templates()
    }

    /// List all concrete (non-templated) resources.
    pub fn list_resources() -> Vec<ResourceDefinition> {
        #[cfg(feature = "v3")]
        let mut resources = templates::list_resources();
        #[cfg(not(feature = "v3"))]
        let resources = templates::list_resources();
        #[cfg(feature = "v3")]
        {
            resources.extend(crate::v3_resources::list_v3_resources());
        }
        resources
    }

    /// Read a resource by URI, dispatching to the appropriate handler.
    pub async fn read(
        uri: &str,
        session: &Arc<Mutex<SessionManager>>,
    ) -> McpResult<ReadResourceResult> {
        #[cfg(feature = "v3")]
        {
            return Self::read_with_v3(uri, session, None).await;
        }
        #[cfg(not(feature = "v3"))]
        {
            return Self::read_with_v3(uri, session).await;
        }
    }

    /// Read a resource by URI with optional V3 engine dispatch.
    #[allow(clippy::ptr_arg)]
    pub async fn read_with_v3(
        uri: &str,
        session: &Arc<Mutex<SessionManager>>,
        #[cfg(feature = "v3")] v3_engine: Option<&crate::tools::v3_tools::SharedEngine>,
    ) -> McpResult<ReadResourceResult> {
        #[cfg(feature = "v3")]
        if let Some(engine) = v3_engine {
            if let Some(v3_result) = crate::v3_resources::read_v3_resource(uri, engine).await {
                return v3_result;
            }
        }

        if let Some(id_str) = uri.strip_prefix("amem://node/") {
            let id: u64 = id_str
                .parse()
                .map_err(|_| McpError::InvalidParams(format!("Invalid node ID: {id_str}")))?;
            node::read_node(id, session).await
        } else if let Some(id_str) = uri.strip_prefix("amem://session/") {
            let id: u32 = id_str
                .parse()
                .map_err(|_| McpError::InvalidParams(format!("Invalid session ID: {id_str}")))?;
            session::read_session(id, session).await
        } else if let Some(type_name) = uri.strip_prefix("amem://types/") {
            type_index::read_type(type_name, session).await
        } else if uri == "amem://graph/stats" {
            graph::read_stats(session).await
        } else if uri == "amem://graph/recent" {
            graph::read_recent(session).await
        } else if uri == "amem://graph/important" {
            graph::read_important(session).await
        } else {
            Err(McpError::ResourceNotFound(uri.to_string()))
        }
    }
}
