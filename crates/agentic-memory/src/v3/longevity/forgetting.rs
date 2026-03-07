//! Safe forgetting protocol — controlled memory deletion.
//!
//! Only memories with significance < 0.2 that pass all verification checks
//! are eligible for forgetting. Never auto-deletes without confirmation.

use super::hierarchy::{MemoryLayer, MemoryRecord};
use super::significance::SignificanceScorer;
use super::store::{LongevityError, LongevityStore};
use serde::{Deserialize, Serialize};

/// Verdict for a forgetting candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgettingVerdict {
    pub memory_id: String,
    pub significance: f64,
    pub eligible: bool,
    pub reason: String,
    pub layer: String,
    pub age_days: f64,
}

/// The forgetting protocol evaluates and executes safe memory deletion.
pub struct ForgettingProtocol {
    _scorer: SignificanceScorer,
    /// Minimum significance below which forgetting is considered
    threshold: f64,
    /// Minimum age in days before forgetting is considered
    min_age_days: f64,
}

impl ForgettingProtocol {
    pub fn new() -> Self {
        Self {
            _scorer: SignificanceScorer::new(),
            threshold: 0.2,
            min_age_days: 30.0,
        }
    }

    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    pub fn with_min_age(mut self, days: f64) -> Self {
        self.min_age_days = days;
        self
    }

    /// Evaluate which memories are candidates for forgetting.
    pub fn evaluate_candidates(
        &self,
        store: &LongevityStore,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<ForgettingVerdict>, LongevityError> {
        // Get low-significance memories
        let candidates = store.query_by_significance(project_id, 0.0, self.threshold, limit)?;

        let now = chrono::Utc::now();
        let mut verdicts = Vec::new();

        for memory in &candidates {
            let created = chrono::DateTime::parse_from_rfc3339(&memory.created_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or(now);
            let age_days = (now - created).num_hours() as f64 / 24.0;

            let eligible = self.check_eligibility(memory, age_days);

            let reason = if !eligible.0 {
                eligible.1.clone()
            } else {
                "Eligible for safe forgetting".to_string()
            };

            verdicts.push(ForgettingVerdict {
                memory_id: memory.id.clone(),
                significance: memory.significance,
                eligible: eligible.0,
                reason,
                layer: memory.layer.to_string(),
                age_days,
            });
        }

        Ok(verdicts)
    }

    /// Execute forgetting for confirmed candidates.
    /// Only deletes memories that pass all safety checks.
    pub fn execute(
        &self,
        store: &LongevityStore,
        memory_ids: &[String],
    ) -> Result<ForgettingResult, LongevityError> {
        let mut forgotten = Vec::new();
        let mut skipped = Vec::new();

        for id in memory_ids {
            if let Some(memory) = store.get_memory(id)? {
                let created = chrono::DateTime::parse_from_rfc3339(&memory.created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());
                let age_days = (chrono::Utc::now() - created).num_hours() as f64 / 24.0;

                let (eligible, reason) = self.check_eligibility(&memory, age_days);
                if eligible {
                    store.delete_memories(std::slice::from_ref(id))?;
                    forgotten.push(id.clone());
                } else {
                    skipped.push((id.clone(), reason));
                }
            } else {
                skipped.push((id.clone(), "Memory not found".to_string()));
            }
        }

        Ok(ForgettingResult {
            forgotten_count: forgotten.len() as u32,
            skipped_count: skipped.len() as u32,
            forgotten_ids: forgotten,
            skipped,
        })
    }

    fn check_eligibility(&self, memory: &MemoryRecord, age_days: f64) -> (bool, String) {
        // Rule 1: Must be below significance threshold
        if memory.significance > self.threshold {
            return (
                false,
                format!(
                    "Significance {:.2} exceeds threshold {:.2}",
                    memory.significance, self.threshold
                ),
            );
        }

        // Rule 2: Must be old enough
        if age_days < self.min_age_days {
            return (
                false,
                format!(
                    "Age {:.1} days below minimum {:.1} days",
                    age_days, self.min_age_days
                ),
            );
        }

        // Rule 3: Identity layer is never auto-forgotten
        if memory.layer == MemoryLayer::Identity {
            return (false, "Identity layer cannot be auto-forgotten".to_string());
        }

        // Rule 4: Trait layer requires extra low significance
        if memory.layer == MemoryLayer::Trait && memory.significance > 0.1 {
            return (
                false,
                format!(
                    "Trait significance {:.2} too high (need < 0.1)",
                    memory.significance
                ),
            );
        }

        // Rule 5: Recently accessed memories are protected
        if memory.access_count > 5 {
            return (
                false,
                format!(
                    "Accessed {} times — too frequently used",
                    memory.access_count
                ),
            );
        }

        // Rule 6: Memories with original_ids (compressed) are protected if referenced
        if memory.original_ids.is_some() && !memory.original_ids.as_ref().unwrap().is_empty() {
            // Compressed memories have provenance — keep them
            return (false, "Compressed memory with provenance chain".to_string());
        }

        (true, "Eligible".to_string())
    }
}

impl Default for ForgettingProtocol {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a forgetting execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgettingResult {
    pub forgotten_count: u32,
    pub skipped_count: u32,
    pub forgotten_ids: Vec<String>,
    pub skipped: Vec<(String, String)>,
}
