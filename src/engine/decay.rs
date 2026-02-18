//! Confidence decay and access tracking.

use crate::types::{CognitiveEvent, EventType};

/// Calculate the decay score for a node.
///
/// Formula: base_importance * recency_factor * access_factor
///
/// - base_importance: 1.0 for FACT/DECISION/CORRECTION, 0.8 for INFERENCE/SKILL, 0.6 for EPISODE
/// - recency_factor: exp(-lambda * days_since_last_access) where lambda = 0.01
/// - access_factor: min(1.0, log2(access_count + 1) / 10.0)
///
/// The result is clamped to [0.0, 1.0].
pub fn calculate_decay(event: &CognitiveEvent, current_time: u64) -> f32 {
    let base_importance = match event.event_type {
        EventType::Fact | EventType::Decision | EventType::Correction => 1.0f32,
        EventType::Inference | EventType::Skill => 0.8,
        EventType::Episode => 0.6,
    };

    // days since last access
    let micros_per_day: f64 = 86_400_000_000.0;
    let elapsed_micros = current_time.saturating_sub(event.last_accessed) as f64;
    let days = elapsed_micros / micros_per_day;

    let lambda = 0.01f64;
    let recency_factor = (-lambda * days).exp() as f32;

    let access_factor = ((event.access_count as f32 + 1.0).log2() / 10.0).min(1.0);

    (base_importance * recency_factor * access_factor).clamp(0.0, 1.0)
}
