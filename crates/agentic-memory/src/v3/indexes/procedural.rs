//! Procedural index for ordered action sequences and workflow tracking.

use super::{Index, IndexResult};
use crate::v3::block::{Block, BlockContent, BlockHash, BlockType, BoundaryType};
use std::collections::HashMap;

/// Workflow step record
#[derive(Debug, Clone)]
pub struct WorkflowStep {
    pub sequence: u64,
    pub step_type: String,
    pub description: String,
}

/// A tracked workflow
#[derive(Debug, Clone)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub start_sequence: u64,
    pub end_sequence: Option<u64>,
    pub steps: Vec<WorkflowStep>,
}

/// Procedural index for sessions and workflows.
pub struct ProceduralIndex {
    /// Session ID -> ordered list of block sequences
    sessions: HashMap<String, Vec<u64>>,

    /// Block -> session it belongs to
    block_to_session: HashMap<u64, String>,

    /// Current session ID
    current_session: String,

    /// Block hashes
    hashes: HashMap<u64, BlockHash>,

    /// Tracked workflows
    workflows: Vec<Workflow>,
}

impl ProceduralIndex {
    pub fn new() -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        Self {
            sessions: HashMap::new(),
            block_to_session: HashMap::new(),
            current_session: session_id,
            hashes: HashMap::new(),
            workflows: Vec::new(),
        }
    }

    /// Get all blocks in a session, in order
    pub fn get_session(&self, session_id: &str) -> Vec<IndexResult> {
        self.sessions
            .get(session_id)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|&seq| {
                        self.hashes.get(&seq).map(|&hash| IndexResult {
                            block_sequence: seq,
                            block_hash: hash,
                            score: 1.0,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get current session blocks
    pub fn get_current_session(&self) -> Vec<IndexResult> {
        self.get_session(&self.current_session.clone())
    }

    /// Get current session ID
    pub fn current_session_id(&self) -> &str {
        &self.current_session
    }

    /// Get all session IDs
    pub fn get_sessions(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }

    /// Get the last N blocks in current session
    pub fn get_recent_steps(&self, n: usize) -> Vec<IndexResult> {
        self.sessions
            .get(&self.current_session)
            .map(|blocks| {
                blocks
                    .iter()
                    .rev()
                    .take(n)
                    .filter_map(|&seq| {
                        self.hashes.get(&seq).map(|&hash| IndexResult {
                            block_sequence: seq,
                            block_hash: hash,
                            score: 1.0,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Start a new workflow
    pub fn start_workflow(&mut self, name: &str, start_sequence: u64) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.workflows.push(Workflow {
            id: id.clone(),
            name: name.to_string(),
            start_sequence,
            end_sequence: None,
            steps: Vec::new(),
        });
        id
    }

    /// End a workflow
    pub fn end_workflow(&mut self, workflow_id: &str, end_sequence: u64) {
        if let Some(workflow) = self.workflows.iter_mut().find(|w| w.id == workflow_id) {
            workflow.end_sequence = Some(end_sequence);
        }
    }

    /// Add step to current workflow
    pub fn add_workflow_step(&mut self, sequence: u64, step_type: &str, description: &str) {
        if let Some(workflow) = self.workflows.last_mut() {
            if workflow.end_sequence.is_none() {
                workflow.steps.push(WorkflowStep {
                    sequence,
                    step_type: step_type.to_string(),
                    description: description.to_string(),
                });
            }
        }
    }

    /// Get workflow by ID
    pub fn get_workflow(&self, workflow_id: &str) -> Option<&Workflow> {
        self.workflows.iter().find(|w| w.id == workflow_id)
    }

    /// Get all workflows
    pub fn get_all_workflows(&self) -> &[Workflow] {
        &self.workflows
    }
}

impl Default for ProceduralIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl Index for ProceduralIndex {
    fn index(&mut self, block: &Block) {
        self.hashes.insert(block.sequence, block.hash);

        // Check for session boundary
        if let BlockContent::Boundary { boundary_type, .. } = &block.content {
            match boundary_type {
                BoundaryType::SessionStart | BoundaryType::Compaction => {
                    self.current_session = uuid::Uuid::new_v4().to_string();
                }
                _ => {}
            }
        }

        // Add to current session
        self.sessions
            .entry(self.current_session.clone())
            .or_default()
            .push(block.sequence);
        self.block_to_session
            .insert(block.sequence, self.current_session.clone());

        // Auto-detect workflow steps
        match block.block_type {
            BlockType::ToolCall => {
                if let BlockContent::Tool { tool_name, .. } = &block.content {
                    self.add_workflow_step(block.sequence, "tool_call", tool_name);
                }
            }
            BlockType::FileOperation => {
                if let BlockContent::File {
                    path, operation, ..
                } = &block.content
                {
                    self.add_workflow_step(
                        block.sequence,
                        "file_op",
                        &format!("{:?} {}", operation, path),
                    );
                }
            }
            BlockType::Decision => {
                if let BlockContent::Decision { decision, .. } = &block.content {
                    self.add_workflow_step(block.sequence, "decision", decision);
                }
            }
            _ => {}
        }
    }

    fn remove(&mut self, sequence: u64) {
        self.hashes.remove(&sequence);
        if let Some(session_id) = self.block_to_session.remove(&sequence) {
            if let Some(blocks) = self.sessions.get_mut(&session_id) {
                blocks.retain(|&s| s != sequence);
            }
        }
    }

    fn rebuild(&mut self, blocks: impl Iterator<Item = Block>) {
        self.sessions.clear();
        self.block_to_session.clear();
        self.hashes.clear();
        self.workflows.clear();
        self.current_session = uuid::Uuid::new_v4().to_string();
        for block in blocks {
            self.index(&block);
        }
    }
}
