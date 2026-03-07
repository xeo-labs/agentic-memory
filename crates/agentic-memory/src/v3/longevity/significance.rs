//! Seven-factor significance scorer for memory importance.

use super::hierarchy::MemoryRecord;
use serde::{Deserialize, Serialize};

/// Individual significance factors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignificanceFactor {
    pub name: String,
    pub value: f64,
    pub weight: f64,
    pub contribution: f64,
}

/// Thresholds for significance-based decisions.
#[derive(Debug, Clone, Copy)]
pub enum SignificanceThreshold {
    /// > 0.8: Immune from compression at current layer
    Immune,
    /// 0.5 - 0.8: Normal consolidation schedule
    Normal,
    /// 0.2 - 0.5: Accelerated consolidation
    Accelerated,
    /// < 0.2: Candidate for safe forgetting
    Forgettable,
}

impl SignificanceThreshold {
    pub fn from_score(score: f64) -> Self {
        if score > 0.8 {
            Self::Immune
        } else if score > 0.5 {
            Self::Normal
        } else if score > 0.2 {
            Self::Accelerated
        } else {
            Self::Forgettable
        }
    }
}

/// Detailed breakdown of a significance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignificanceBreakdown {
    pub final_score: f64,
    pub threshold: String,
    pub factors: Vec<SignificanceFactor>,
}

/// The significance scorer computes how important a memory is.
pub struct SignificanceScorer {
    /// Decay constant for recency (lambda). Default 0.01 (~70 day half-life).
    pub recency_lambda: f64,
    /// Weights for each factor (must sum to 1.0)
    pub weights: SignificanceWeights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignificanceWeights {
    pub recency: f64,
    pub access_frequency: f64,
    pub referential_weight: f64,
    pub causal_depth: f64,
    pub emotional_valence: f64,
    pub contradiction_signal: f64,
    pub uniqueness: f64,
}

impl Default for SignificanceWeights {
    fn default() -> Self {
        Self {
            recency: 0.15,
            access_frequency: 0.20,
            referential_weight: 0.25,
            causal_depth: 0.15,
            emotional_valence: 0.10,
            contradiction_signal: 0.10,
            uniqueness: 0.05,
        }
    }
}

/// Context needed to compute significance for a memory.
pub struct ScoringContext {
    /// How many other memories reference this one
    pub reference_count: u32,
    /// Maximum reference count across all memories (for normalization)
    pub max_reference_count: u32,
    /// Depth in the causal chain (0 = leaf, higher = root cause)
    pub causal_depth: u32,
    /// Maximum causal depth (for normalization)
    pub max_causal_depth: u32,
    /// Whether this memory involves corrections/supersessions
    pub is_contradiction: bool,
    /// Whether the user manually marked this as important
    pub user_marked: bool,
    /// Average cosine similarity to nearest neighbors (for uniqueness)
    pub avg_neighbor_similarity: f64,
    /// Maximum access count across all memories (for normalization)
    pub max_access_count: u64,
}

impl Default for ScoringContext {
    fn default() -> Self {
        Self {
            reference_count: 0,
            max_reference_count: 1,
            causal_depth: 0,
            max_causal_depth: 1,
            is_contradiction: false,
            user_marked: false,
            avg_neighbor_similarity: 0.5,
            max_access_count: 1,
        }
    }
}

impl Default for SignificanceScorer {
    fn default() -> Self {
        Self {
            recency_lambda: 0.01,
            weights: SignificanceWeights::default(),
        }
    }
}

impl SignificanceScorer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_weights(weights: SignificanceWeights) -> Self {
        Self {
            recency_lambda: 0.01,
            weights,
        }
    }

    /// Compute significance score for a memory with full context.
    pub fn score(&self, memory: &MemoryRecord, ctx: &ScoringContext) -> SignificanceBreakdown {
        // User-marked memories always get max score
        if ctx.user_marked {
            return SignificanceBreakdown {
                final_score: 1.0,
                threshold: "immune".to_string(),
                factors: vec![SignificanceFactor {
                    name: "user_marked".to_string(),
                    value: 1.0,
                    weight: 1.0,
                    contribution: 1.0,
                }],
            };
        }

        let mut factors = Vec::with_capacity(7);

        // 1. Recency Factor: e^(-λ × days_since_creation)
        let recency = self.compute_recency(memory);
        factors.push(SignificanceFactor {
            name: "recency".to_string(),
            value: recency,
            weight: self.weights.recency,
            contribution: recency * self.weights.recency,
        });

        // 2. Access Frequency: log(access_count + 1) / log(max_access + 1)
        let access = self.compute_access_frequency(memory, ctx);
        factors.push(SignificanceFactor {
            name: "access_frequency".to_string(),
            value: access,
            weight: self.weights.access_frequency,
            contribution: access * self.weights.access_frequency,
        });

        // 3. Referential Weight: normalized reference count
        let referential = self.compute_referential_weight(ctx);
        factors.push(SignificanceFactor {
            name: "referential_weight".to_string(),
            value: referential,
            weight: self.weights.referential_weight,
            contribution: referential * self.weights.referential_weight,
        });

        // 4. Causal Depth: normalized depth in decision chains
        let causal = self.compute_causal_depth(ctx);
        factors.push(SignificanceFactor {
            name: "causal_depth".to_string(),
            value: causal,
            weight: self.weights.causal_depth,
            contribution: causal * self.weights.causal_depth,
        });

        // 5. Emotional Valence: content-based emotional detection
        let emotional = self.compute_emotional_valence(memory);
        factors.push(SignificanceFactor {
            name: "emotional_valence".to_string(),
            value: emotional,
            weight: self.weights.emotional_valence,
            contribution: emotional * self.weights.emotional_valence,
        });

        // 6. Contradiction Signal: involvement in supersession chains
        let contradiction = if ctx.is_contradiction { 0.8 } else { 0.1 };
        factors.push(SignificanceFactor {
            name: "contradiction_signal".to_string(),
            value: contradiction,
            weight: self.weights.contradiction_signal,
            contribution: contradiction * self.weights.contradiction_signal,
        });

        // 7. Uniqueness: 1.0 - avg_neighbor_similarity (outliers preserved longer)
        let uniqueness = (1.0 - ctx.avg_neighbor_similarity).clamp(0.0, 1.0);
        factors.push(SignificanceFactor {
            name: "uniqueness".to_string(),
            value: uniqueness,
            weight: self.weights.uniqueness,
            contribution: uniqueness * self.weights.uniqueness,
        });

        let final_score: f64 = factors.iter().map(|f| f.contribution).sum();
        let final_score = final_score.clamp(0.0, 1.0);

        let threshold = match SignificanceThreshold::from_score(final_score) {
            SignificanceThreshold::Immune => "immune",
            SignificanceThreshold::Normal => "normal",
            SignificanceThreshold::Accelerated => "accelerated",
            SignificanceThreshold::Forgettable => "forgettable",
        };

        SignificanceBreakdown {
            final_score,
            threshold: threshold.to_string(),
            factors,
        }
    }

    /// Simple scoring with just the memory and no external context.
    /// Uses only recency, access frequency, and emotional valence.
    pub fn score_simple(&self, memory: &MemoryRecord) -> f64 {
        let recency = self.compute_recency(memory);
        let access = if memory.access_count > 0 {
            (memory.access_count as f64 + 1.0).ln() / 10.0_f64.ln()
        } else {
            0.0
        }
        .min(1.0);
        let emotional = self.compute_emotional_valence(memory);

        let score = 0.4 * recency + 0.4 * access + 0.2 * emotional;
        score.clamp(0.0, 1.0)
    }

    fn compute_recency(&self, memory: &MemoryRecord) -> f64 {
        let now = chrono::Utc::now();
        let created = chrono::DateTime::parse_from_rfc3339(&memory.created_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or(now);

        let days = (now - created).num_hours() as f64 / 24.0;
        (-self.recency_lambda * days).exp()
    }

    fn compute_access_frequency(&self, memory: &MemoryRecord, ctx: &ScoringContext) -> f64 {
        if ctx.max_access_count == 0 {
            return 0.0;
        }
        let numerator = (memory.access_count as f64 + 1.0).ln();
        let denominator = (ctx.max_access_count as f64 + 1.0).ln();
        if denominator == 0.0 {
            0.0
        } else {
            (numerator / denominator).min(1.0)
        }
    }

    fn compute_referential_weight(&self, ctx: &ScoringContext) -> f64 {
        if ctx.max_reference_count == 0 {
            return 0.0;
        }
        (ctx.reference_count as f64) / (ctx.max_reference_count as f64)
    }

    fn compute_causal_depth(&self, ctx: &ScoringContext) -> f64 {
        if ctx.max_causal_depth == 0 {
            return 0.0;
        }
        (ctx.causal_depth as f64) / (ctx.max_causal_depth as f64)
    }

    fn compute_emotional_valence(&self, memory: &MemoryRecord) -> f64 {
        let text = memory.extract_text().to_lowercase();

        // Simple keyword-based emotional detection
        let positive_markers = [
            "love",
            "great",
            "excellent",
            "perfect",
            "amazing",
            "beautiful",
            "brilliant",
            "fantastic",
            "wonderful",
            "excited",
            "happy",
            "important",
            "critical",
            "essential",
            "breakthrough",
            "success",
        ];
        let negative_markers = [
            "hate",
            "terrible",
            "awful",
            "frustrated",
            "angry",
            "annoyed",
            "broken",
            "failed",
            "bug",
            "error",
            "crash",
            "disaster",
            "urgent",
            "blocker",
            "regression",
        ];
        let emphasis_markers = [
            "!",
            "IMPORTANT",
            "CRITICAL",
            "NOTE",
            "WARNING",
            "NEVER",
            "ALWAYS",
            "MUST",
            "!!!",
        ];

        let mut score: f64 = 0.0;

        for marker in &positive_markers {
            if text.contains(marker) {
                score += 0.15;
            }
        }
        for marker in &negative_markers {
            if text.contains(marker) {
                score += 0.15; // Negative emotions are also significant
            }
        }
        for marker in &emphasis_markers {
            if memory.extract_text().contains(marker) {
                score += 0.1;
            }
        }

        score.min(1.0)
    }
}
