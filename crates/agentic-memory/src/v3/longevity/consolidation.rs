//! Consolidation engine — scheduled compression through the memory hierarchy.
//!
//! Nightly:    Raw → Episodes (5:1 compression)
//! Weekly:     Episodes → Summaries (10:1)
//! Monthly:    Summaries → Patterns (20:1)
//! Quarterly:  Patterns → Traits (100:1)
//! Annual:     Traits → Identity review (human-in-the-loop)

use super::hierarchy::{MemoryHierarchy, MemoryLayer, MemoryRecord};
use super::significance::SignificanceScorer;
use super::store::{LongevityError, LongevityStore};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Consolidation schedule type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsolidationSchedule {
    Nightly,
    Weekly,
    Monthly,
    Quarterly,
    Annual,
    OnDemand,
}

impl ConsolidationSchedule {
    /// Which layer transition this schedule handles.
    pub fn layer_transition(&self) -> Option<(MemoryLayer, MemoryLayer)> {
        match self {
            Self::Nightly => Some((MemoryLayer::Raw, MemoryLayer::Episode)),
            Self::Weekly => Some((MemoryLayer::Episode, MemoryLayer::Summary)),
            Self::Monthly => Some((MemoryLayer::Summary, MemoryLayer::Pattern)),
            Self::Quarterly => Some((MemoryLayer::Pattern, MemoryLayer::Trait)),
            Self::Annual => Some((MemoryLayer::Trait, MemoryLayer::Identity)),
            Self::OnDemand => None,
        }
    }

    /// How old memories must be at the source layer to be eligible.
    pub fn age_threshold_hours(&self) -> u64 {
        match self {
            Self::Nightly => 24,
            Self::Weekly => 24 * 7,
            Self::Monthly => 24 * 30,
            Self::Quarterly => 24 * 90,
            Self::Annual => 24 * 365,
            Self::OnDemand => 0,
        }
    }
}

/// A consolidation task to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationTask {
    pub schedule: ConsolidationSchedule,
    pub from_layer: MemoryLayer,
    pub to_layer: MemoryLayer,
    pub project_id: String,
    pub max_memories: u32,
}

/// Result of a consolidation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationResult {
    pub task: ConsolidationTask,
    pub memories_processed: u32,
    pub memories_created: u32,
    pub memories_preserved: u32,
    pub compression_ratio: f64,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

/// The consolidation engine orchestrates all compression operations.
pub struct ConsolidationEngine {
    scorer: SignificanceScorer,
}

impl ConsolidationEngine {
    pub fn new() -> Self {
        Self {
            scorer: SignificanceScorer::new(),
        }
    }

    pub fn with_scorer(scorer: SignificanceScorer) -> Self {
        Self { scorer }
    }

    /// Run a consolidation task against the longevity store.
    pub fn run(
        &self,
        store: &LongevityStore,
        task: &ConsolidationTask,
    ) -> Result<ConsolidationResult, LongevityError> {
        let start = Instant::now();

        // Calculate the cutoff date
        let cutoff = chrono::Utc::now()
            - chrono::Duration::hours(task.schedule.age_threshold_hours() as i64);
        let cutoff_str = cutoff.to_rfc3339();

        // Get eligible memories from the source layer
        let memories = store.get_old_memories(
            &task.project_id,
            task.from_layer,
            &cutoff_str,
            task.max_memories,
        )?;

        if memories.is_empty() {
            return Ok(ConsolidationResult {
                task: task.clone(),
                memories_processed: 0,
                memories_created: 0,
                memories_preserved: 0,
                compression_ratio: 1.0,
                duration_ms: start.elapsed().as_millis() as u64,
                errors: Vec::new(),
            });
        }

        // Separate immune memories (high significance)
        // Use the stored significance field (set by the scorer during sync)
        // as the primary signal, with score_simple as fallback
        let preservation_threshold = task.from_layer.preservation_threshold();
        let mut to_compress = Vec::new();
        let mut preserved = Vec::new();

        for memory in &memories {
            let score = memory.significance.max(self.scorer.score_simple(memory));
            if score > preservation_threshold {
                preserved.push(memory);
            } else {
                to_compress.push(memory);
            }
        }

        if to_compress.is_empty() {
            return Ok(ConsolidationResult {
                task: task.clone(),
                memories_processed: 0,
                memories_created: 0,
                memories_preserved: preserved.len() as u32,
                compression_ratio: 1.0,
                duration_ms: start.elapsed().as_millis() as u64,
                errors: Vec::new(),
            });
        }

        let mut errors = Vec::new();
        let mut created_count = 0u32;

        // Route to appropriate compression algorithm
        match (task.from_layer, task.to_layer) {
            (MemoryLayer::Raw, MemoryLayer::Episode) => {
                let groups = MemoryHierarchy::group_for_episodes(&memories);
                for group in &groups {
                    // Filter group to only compressible memories
                    let compressible: Vec<&&MemoryRecord> = group
                        .iter()
                        .filter(|m| {
                            m.significance.max(self.scorer.score_simple(m))
                                <= preservation_threshold
                        })
                        .collect();

                    if compressible.is_empty() {
                        continue;
                    }

                    let refs: Vec<&MemoryRecord> = compressible.iter().map(|m| **m).collect();
                    let episode_content = MemoryHierarchy::create_episode_summary(&refs);
                    let source_ids: Vec<String> = refs.iter().map(|m| m.id.clone()).collect();

                    let episode_id = generate_ulid();
                    let episode = MemoryRecord::new_compressed(
                        episode_id,
                        MemoryLayer::Episode,
                        episode_content,
                        source_ids,
                        task.project_id.clone(),
                    );

                    if let Err(e) = store.insert_memory(&episode) {
                        errors.push(format!("Failed to insert episode: {}", e));
                    } else {
                        created_count += 1;
                    }
                }
            }
            (MemoryLayer::Episode, MemoryLayer::Summary) => {
                // Group episodes into chunks of ~10 and summarize
                for chunk in to_compress.chunks(10) {
                    let refs: Vec<&MemoryRecord> = chunk.to_vec();
                    let summary_content = Self::create_summary(&refs);
                    let source_ids: Vec<String> = refs.iter().map(|m| m.id.clone()).collect();

                    let summary = MemoryRecord::new_compressed(
                        generate_ulid(),
                        MemoryLayer::Summary,
                        summary_content,
                        source_ids,
                        task.project_id.clone(),
                    );

                    if let Err(e) = store.insert_memory(&summary) {
                        errors.push(format!("Failed to insert summary: {}", e));
                    } else {
                        created_count += 1;
                    }
                }
            }
            (MemoryLayer::Summary, MemoryLayer::Pattern) => {
                let refs: Vec<&MemoryRecord> = to_compress.to_vec();
                let patterns = MemoryHierarchy::extract_patterns(&refs);
                let source_ids: Vec<String> = refs.iter().map(|m| m.id.clone()).collect();

                for pattern_content in patterns {
                    let pattern = MemoryRecord::new_compressed(
                        generate_ulid(),
                        MemoryLayer::Pattern,
                        pattern_content,
                        source_ids.clone(),
                        task.project_id.clone(),
                    );

                    if let Err(e) = store.insert_memory(&pattern) {
                        errors.push(format!("Failed to insert pattern: {}", e));
                    } else {
                        created_count += 1;
                    }
                }
            }
            (MemoryLayer::Pattern, MemoryLayer::Trait) => {
                let refs: Vec<&MemoryRecord> = to_compress.to_vec();
                let traits = MemoryHierarchy::distill_traits(&refs);
                let source_ids: Vec<String> = refs.iter().map(|m| m.id.clone()).collect();

                for trait_content in traits {
                    let trait_record = MemoryRecord::new_compressed(
                        generate_ulid(),
                        MemoryLayer::Trait,
                        trait_content,
                        source_ids.clone(),
                        task.project_id.clone(),
                    );

                    if let Err(e) = store.insert_memory(&trait_record) {
                        errors.push(format!("Failed to insert trait: {}", e));
                    } else {
                        created_count += 1;
                    }
                }
            }
            (MemoryLayer::Trait, MemoryLayer::Identity) => {
                // Identity layer requires human-in-the-loop. Just flag for review.
                errors.push("Identity consolidation requires human review".to_string());
            }
            _ => {
                errors.push(format!(
                    "Unsupported transition: {} → {}",
                    task.from_layer, task.to_layer
                ));
            }
        }

        // Delete source memories that were compressed (but NOT preserved ones)
        let compressed_ids: Vec<String> = to_compress.iter().map(|m| m.id.clone()).collect();
        if !compressed_ids.is_empty() && created_count > 0 {
            if let Err(e) = store.delete_memories(&compressed_ids) {
                errors.push(format!("Failed to delete source memories: {}", e));
            }
        }

        let memories_processed = to_compress.len() as u32;
        let compression_ratio = if created_count > 0 {
            memories_processed as f64 / created_count as f64
        } else {
            1.0
        };

        // Log the consolidation
        let log_id = generate_ulid();
        let duration_ms = start.elapsed().as_millis() as u64;
        store.log_consolidation(
            &log_id,
            task.from_layer,
            task.to_layer,
            memories_processed,
            created_count,
            compression_ratio,
            "algorithmic",
            duration_ms,
        )?;

        Ok(ConsolidationResult {
            task: task.clone(),
            memories_processed,
            memories_created: created_count,
            memories_preserved: preserved.len() as u32,
            compression_ratio,
            duration_ms,
            errors,
        })
    }

    /// Run all applicable consolidation tasks for a project.
    pub fn run_all(
        &self,
        store: &LongevityStore,
        project_id: &str,
    ) -> Result<Vec<ConsolidationResult>, LongevityError> {
        let schedules = [
            ConsolidationSchedule::Nightly,
            ConsolidationSchedule::Weekly,
            ConsolidationSchedule::Monthly,
            ConsolidationSchedule::Quarterly,
        ];

        let mut results = Vec::new();
        for schedule in &schedules {
            if let Some((from, to)) = schedule.layer_transition() {
                let task = ConsolidationTask {
                    schedule: *schedule,
                    from_layer: from,
                    to_layer: to,
                    project_id: project_id.to_string(),
                    max_memories: 1000,
                };
                results.push(self.run(store, &task)?);
            }
        }
        Ok(results)
    }

    /// Create a summary from episodes (algorithmic, no LLM).
    fn create_summary(episodes: &[&MemoryRecord]) -> serde_json::Value {
        let mut all_decisions = Vec::new();
        let mut all_files = Vec::new();
        let mut total_events = 0u64;
        let mut session_ids = Vec::new();

        for episode in episodes {
            if let serde_json::Value::Object(ref map) = episode.content {
                if let Some(serde_json::Value::Array(decisions)) = map.get("decisions") {
                    for d in decisions {
                        if let serde_json::Value::String(s) = d {
                            all_decisions.push(s.clone());
                        }
                    }
                }
                if let Some(serde_json::Value::Array(files)) = map.get("files_touched") {
                    for f in files {
                        if let serde_json::Value::String(s) = f {
                            if !all_files.contains(s) {
                                all_files.push(s.clone());
                            }
                        }
                    }
                }
                if let Some(serde_json::Value::Number(n)) = map.get("event_count") {
                    total_events += n.as_u64().unwrap_or(0);
                }
                if let Some(serde_json::Value::String(sid)) = map.get("session_id") {
                    if !session_ids.contains(sid) {
                        session_ids.push(sid.clone());
                    }
                }
            }
        }

        let time_range = format!(
            "{} to {}",
            episodes
                .first()
                .map(|e| e.created_at.as_str())
                .unwrap_or("?"),
            episodes
                .last()
                .map(|e| e.created_at.as_str())
                .unwrap_or("?"),
        );

        serde_json::json!({
            "summary": format!(
                "Summary of {} episodes ({} events) across {} sessions",
                episodes.len(), total_events, session_ids.len()
            ),
            "episode_count": episodes.len(),
            "total_events": total_events,
            "time_range": time_range,
            "sessions": session_ids,
            "key_decisions": all_decisions.iter().take(10).collect::<Vec<_>>(),
            "files_touched": all_files.iter().take(20).collect::<Vec<_>>(),
        })
    }
}

impl Default for ConsolidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a ULID string for record IDs.
fn generate_ulid() -> String {
    ulid::Ulid::new().to_string()
}
