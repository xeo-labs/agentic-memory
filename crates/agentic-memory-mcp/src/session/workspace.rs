//! Multi-context workspace manager for loading and querying multiple .amem files.

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agentic_memory::{AmemReader, MemoryGraph, QueryEngine, TextSearchParams};

use crate::types::{McpError, McpResult};

/// Role of a context within a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextRole {
    Primary,
    Secondary,
    Reference,
    Archive,
}

impl ContextRole {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "primary" => Some(Self::Primary),
            "secondary" => Some(Self::Secondary),
            "reference" => Some(Self::Reference),
            "archive" => Some(Self::Archive),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Reference => "reference",
            Self::Archive => "archive",
        }
    }
}

/// A loaded memory context within a workspace.
pub struct MemoryContext {
    pub id: String,
    pub role: ContextRole,
    pub path: String,
    pub label: Option<String>,
    pub graph: MemoryGraph,
}

/// A multi-memory workspace.
pub struct MemoryWorkspace {
    pub id: String,
    pub name: String,
    pub contexts: Vec<MemoryContext>,
    pub created_at: u64,
}

/// Result from querying across contexts.
#[derive(Debug)]
pub struct CrossContextResult {
    pub context_id: String,
    pub context_role: ContextRole,
    pub matches: Vec<CrossContextMatch>,
}

/// A single match from cross-context querying.
#[derive(Debug)]
pub struct CrossContextMatch {
    pub node_id: u64,
    pub content: String,
    pub event_type: String,
    pub confidence: f32,
    pub score: f32,
}

/// Comparison result across contexts.
#[derive(Debug)]
pub struct Comparison {
    pub item: String,
    pub found_in: Vec<String>,
    pub missing_from: Vec<String>,
    pub matches_per_context: Vec<(String, Vec<CrossContextMatch>)>,
}

/// Cross-reference result.
#[derive(Debug)]
pub struct CrossReference {
    pub item: String,
    pub present_in: Vec<String>,
    pub absent_from: Vec<String>,
}

/// Manages multiple memory workspaces.
#[derive(Default)]
pub struct WorkspaceManager {
    workspaces: HashMap<String, MemoryWorkspace>,
    next_id: u64,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new workspace.
    pub fn create(&mut self, name: &str) -> String {
        let id = format!("ws_{}", self.next_id);
        self.next_id += 1;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let workspace = MemoryWorkspace {
            id: id.clone(),
            name: name.to_string(),
            contexts: Vec::new(),
            created_at: now,
        };

        self.workspaces.insert(id.clone(), workspace);
        id
    }

    /// Add a context to a workspace by loading an .amem file.
    pub fn add_context(
        &mut self,
        workspace_id: &str,
        path: &str,
        role: ContextRole,
        label: Option<String>,
    ) -> McpResult<String> {
        let workspace = self.workspaces.get_mut(workspace_id).ok_or_else(|| {
            McpError::InvalidParams(format!("Workspace not found: {workspace_id}"))
        })?;

        // Load the .amem file
        let file_path = Path::new(path);
        if !file_path.exists() {
            return Err(McpError::InvalidParams(format!("File not found: {path}")));
        }

        let graph = AmemReader::read_from_file(file_path)
            .map_err(|e| McpError::AgenticMemory(format!("Failed to parse {path}: {e}")))?;

        let ctx_id = format!("ctx_{}_{}", workspace.contexts.len() + 1, workspace_id);

        let context = MemoryContext {
            id: ctx_id.clone(),
            role,
            path: path.to_string(),
            label: label.or_else(|| {
                file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            }),
            graph,
        };

        workspace.contexts.push(context);
        Ok(ctx_id)
    }

    /// List contexts in a workspace.
    pub fn list(&self, workspace_id: &str) -> McpResult<&[MemoryContext]> {
        let workspace = self.workspaces.get(workspace_id).ok_or_else(|| {
            McpError::InvalidParams(format!("Workspace not found: {workspace_id}"))
        })?;
        Ok(&workspace.contexts)
    }

    /// Get a workspace reference.
    pub fn get(&self, workspace_id: &str) -> Option<&MemoryWorkspace> {
        self.workspaces.get(workspace_id)
    }

    /// Query across all contexts in a workspace.
    pub fn query_all(
        &self,
        workspace_id: &str,
        query: &str,
        max_per_context: usize,
    ) -> McpResult<Vec<CrossContextResult>> {
        let workspace = self.workspaces.get(workspace_id).ok_or_else(|| {
            McpError::InvalidParams(format!("Workspace not found: {workspace_id}"))
        })?;

        let engine = QueryEngine::new();
        let mut results = Vec::new();

        for ctx in &workspace.contexts {
            let text_matches = engine
                .text_search(
                    &ctx.graph,
                    ctx.graph.term_index.as_ref(),
                    ctx.graph.doc_lengths.as_ref(),
                    TextSearchParams {
                        query: query.to_string(),
                        max_results: max_per_context,
                        event_types: Vec::new(),
                        session_ids: Vec::new(),
                        min_score: 0.0,
                    },
                )
                .unwrap_or_default();

            let matches: Vec<CrossContextMatch> = text_matches
                .iter()
                .filter_map(|m| {
                    ctx.graph.get_node(m.node_id).map(|node| CrossContextMatch {
                        node_id: node.id,
                        content: node.content.clone(),
                        event_type: node.event_type.name().to_string(),
                        confidence: node.confidence,
                        score: m.score,
                    })
                })
                .collect();

            results.push(CrossContextResult {
                context_id: ctx.id.clone(),
                context_role: ctx.role,
                matches,
            });
        }

        Ok(results)
    }

    /// Compare a topic across all contexts.
    pub fn compare(
        &self,
        workspace_id: &str,
        item: &str,
        max_per_context: usize,
    ) -> McpResult<Comparison> {
        let results = self.query_all(workspace_id, item, max_per_context)?;
        let workspace = self.workspaces.get(workspace_id).unwrap();

        let mut found_in = Vec::new();
        let mut missing_from = Vec::new();
        let mut matches_per_context = Vec::new();

        for (i, ctx_result) in results.into_iter().enumerate() {
            let label = workspace.contexts[i]
                .label
                .clone()
                .unwrap_or_else(|| ctx_result.context_id.clone());

            if ctx_result.matches.is_empty() {
                missing_from.push(label);
            } else {
                found_in.push(label.clone());
                matches_per_context.push((label, ctx_result.matches));
            }
        }

        Ok(Comparison {
            item: item.to_string(),
            found_in,
            missing_from,
            matches_per_context,
        })
    }

    /// Cross-reference: find which contexts have/lack a topic.
    pub fn cross_reference(&self, workspace_id: &str, item: &str) -> McpResult<CrossReference> {
        let comparison = self.compare(workspace_id, item, 5)?;
        Ok(CrossReference {
            item: comparison.item,
            present_in: comparison.found_in,
            absent_from: comparison.missing_from,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_role_roundtrip() {
        assert_eq!(ContextRole::from_str("primary"), Some(ContextRole::Primary));
        assert_eq!(
            ContextRole::from_str("SECONDARY"),
            Some(ContextRole::Secondary)
        );
        assert_eq!(
            ContextRole::from_str("reference"),
            Some(ContextRole::Reference)
        );
        assert_eq!(ContextRole::from_str("archive"), Some(ContextRole::Archive));
        assert_eq!(ContextRole::from_str("unknown"), None);
    }

    #[test]
    fn test_workspace_create() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.create("test");
        assert!(id.starts_with("ws_"));
        assert!(mgr.get(&id).is_some());
        assert_eq!(mgr.get(&id).unwrap().name, "test");
    }

    #[test]
    fn test_workspace_not_found() {
        let mgr = WorkspaceManager::new();
        assert!(mgr.list("nonexistent").is_err());
    }

    #[test]
    fn test_workspace_file_not_found() {
        let mut mgr = WorkspaceManager::new();
        let id = mgr.create("test");
        let result = mgr.add_context(&id, "/nonexistent.amem", ContextRole::Primary, None);
        assert!(result.is_err());
    }
}
