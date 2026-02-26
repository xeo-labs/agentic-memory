//! Smart multi-index context retrieval engine.

use super::block::*;
use super::immortal_log::*;
use super::indexes::*;
use super::tiered::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Request for smart context retrieval
#[derive(Debug, Clone)]
pub struct RetrievalRequest {
    /// Natural language query
    pub query: String,
    /// Token budget for the context
    pub token_budget: u32,
    /// Retrieval strategy
    pub strategy: RetrievalStrategy,
    /// Minimum relevance score (0.0 - 1.0)
    pub min_relevance: f32,
}

/// Retrieval strategy
#[derive(Debug, Clone, Copy)]
pub enum RetrievalStrategy {
    /// Prioritize recent blocks
    Recency,
    /// Prioritize relevant blocks
    Relevance,
    /// Prioritize causal chains
    Causal,
    /// Balanced mix
    Balanced,
    /// Custom weights
    Custom {
        recency_weight: f32,
        relevance_weight: f32,
        causal_weight: f32,
    },
}

/// Result of smart retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    /// Assembled context (ordered blocks)
    pub blocks: Vec<Block>,
    /// Tokens used
    pub tokens_used: u32,
    /// Coverage metrics
    pub coverage: RetrievalCoverage,
    /// Blocks that didn't fit
    pub omitted: Vec<BlockHash>,
    /// Retrieval duration in ms
    pub retrieval_ms: u64,
}

/// Coverage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalCoverage {
    pub semantic: f32,
    pub temporal: f32,
    pub causal: f32,
}

/// Smart retrieval engine
pub struct SmartRetrievalEngine {}

impl SmartRetrievalEngine {
    pub fn new() -> Self {
        Self {}
    }

    /// Main retrieval function
    pub fn retrieve(
        &self,
        request: RetrievalRequest,
        _log: &ImmortalLog,
        storage: &TieredStorage,
        temporal: &temporal::TemporalIndex,
        semantic: &semantic::SemanticIndex,
        causal: &causal::CausalIndex,
        entity: &entity::EntityIndex,
        _procedural: &procedural::ProceduralIndex,
    ) -> RetrievalResult {
        let start = std::time::Instant::now();

        // Step 1: Gather candidates from all indexes
        let mut candidates: HashMap<u64, f32> = HashMap::new();

        // Semantic search
        let semantic_results = semantic.search_by_text(&request.query, 100);
        for result in &semantic_results {
            let score = result.score * self.get_weight(&request.strategy, "semantic");
            *candidates.entry(result.block_sequence).or_insert(0.0) += score;
        }

        // Temporal search (recent)
        let recent_results = temporal.query_recent(3600); // Last hour
        for (i, result) in recent_results.iter().enumerate() {
            let recency_score = 1.0 - (i as f32 / recent_results.len().max(1) as f32);
            let score = recency_score * self.get_weight(&request.strategy, "temporal");
            *candidates.entry(result.block_sequence).or_insert(0.0) += score;
        }

        // Entity search (extract entities from query)
        for word in request.query.split_whitespace() {
            if word.contains('/') || word.contains('.') {
                let entity_results = entity.query_entity(word);
                for result in entity_results {
                    let score = 0.8 * self.get_weight(&request.strategy, "entity");
                    *candidates.entry(result.block_sequence).or_insert(0.0) += score;
                }
            }
        }

        // Step 2: Sort by score
        let mut sorted: Vec<(u64, f32)> = candidates.into_iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Step 3: Filter by minimum relevance
        sorted.retain(|(_, score)| *score >= request.min_relevance);

        // Step 4: Fit within token budget
        let mut selected_blocks = Vec::new();
        let mut tokens_used = 0u32;
        let mut omitted = Vec::new();

        for (seq, _score) in &sorted {
            if let Some(block) = storage.get(*seq) {
                let block_tokens = self.estimate_tokens(&block);
                if tokens_used + block_tokens <= request.token_budget {
                    tokens_used += block_tokens;
                    selected_blocks.push(block);
                } else {
                    omitted.push(block.hash);
                }
            }
        }

        // Step 5: Causal expansion (add context for decisions)
        let decision_blocks: Vec<u64> = selected_blocks
            .iter()
            .filter(|b| matches!(b.block_type, BlockType::Decision))
            .map(|b| b.sequence)
            .collect();

        for decision_seq in &decision_blocks {
            let ancestors = causal.get_ancestors(*decision_seq, 3);
            for result in ancestors {
                if !selected_blocks
                    .iter()
                    .any(|b| b.sequence == result.block_sequence)
                {
                    if let Some(block) = storage.get(result.block_sequence) {
                        let block_tokens = self.estimate_tokens(&block);
                        if tokens_used + block_tokens <= request.token_budget {
                            tokens_used += block_tokens;
                            selected_blocks.push(block);
                        }
                    }
                }
            }
        }

        // Step 6: Sort by sequence (chronological order)
        selected_blocks.sort_by_key(|b| b.sequence);

        // Step 7: Calculate coverage
        let coverage = RetrievalCoverage {
            semantic: (selected_blocks.len() as f32 / 100.0).min(1.0),
            temporal: (selected_blocks
                .iter()
                .filter(|b| {
                    chrono::Utc::now()
                        .signed_duration_since(b.timestamp)
                        .num_hours()
                        < 24
                })
                .count() as f32
                / 50.0)
                .min(1.0),
            causal: (decision_blocks.len() as f32 / 10.0).min(1.0),
        };

        RetrievalResult {
            blocks: selected_blocks,
            tokens_used,
            coverage,
            omitted,
            retrieval_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn get_weight(&self, strategy: &RetrievalStrategy, index_type: &str) -> f32 {
        match strategy {
            RetrievalStrategy::Recency => match index_type {
                "temporal" => 1.0,
                "semantic" => 0.3,
                _ => 0.2,
            },
            RetrievalStrategy::Relevance => match index_type {
                "semantic" => 1.0,
                "entity" => 0.8,
                _ => 0.2,
            },
            RetrievalStrategy::Causal => match index_type {
                "causal" => 1.0,
                "semantic" => 0.5,
                _ => 0.2,
            },
            RetrievalStrategy::Balanced => 0.5,
            RetrievalStrategy::Custom {
                recency_weight,
                relevance_weight,
                causal_weight,
            } => match index_type {
                "temporal" => *recency_weight,
                "semantic" => *relevance_weight,
                "causal" => *causal_weight,
                _ => 0.3,
            },
        }
    }

    fn estimate_tokens(&self, block: &Block) -> u32 {
        let content_size = match &block.content {
            BlockContent::Text { text, .. } => text.len(),
            BlockContent::Tool {
                tool_name,
                input,
                output,
                ..
            } => {
                tool_name.len()
                    + serde_json::to_string(input).map(|s| s.len()).unwrap_or(0)
                    + output
                        .as_ref()
                        .and_then(|o| serde_json::to_string(o).ok())
                        .map(|s| s.len())
                        .unwrap_or(0)
            }
            BlockContent::File { path, diff, .. } => {
                path.len() + diff.as_ref().map(|d| d.len()).unwrap_or(0)
            }
            BlockContent::Decision {
                decision,
                reasoning,
                ..
            } => decision.len() + reasoning.as_ref().map(|r| r.len()).unwrap_or(0),
            BlockContent::Boundary { summary, .. } => summary.len(),
            BlockContent::Error {
                message,
                resolution,
                ..
            } => message.len() + resolution.as_ref().map(|r| r.len()).unwrap_or(0),
            BlockContent::Checkpoint {
                working_context, ..
            } => working_context.len(),
            BlockContent::Binary { data, .. } => data.len(),
        };

        ((content_size / 4) + 10) as u32
    }
}

impl Default for SmartRetrievalEngine {
    fn default() -> Self {
        Self::new()
    }
}
