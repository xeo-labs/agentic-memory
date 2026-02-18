//! Cognitive event types and the core event struct.

use serde::Serialize;

use super::{now_micros, DEFAULT_DIMENSION, MAX_CONTENT_SIZE};
use crate::types::error::{AmemError, AmemResult};

/// The type of cognitive event stored in a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[repr(u8)]
pub enum EventType {
    /// Something the agent learned about the world or the user.
    Fact = 0,
    /// A choice the agent made and the reasoning behind it.
    Decision = 1,
    /// A conclusion the agent drew from multiple facts.
    Inference = 2,
    /// Something the agent previously believed that was corrected.
    Correction = 3,
    /// A learned pattern for how to accomplish something.
    Skill = 4,
    /// A compressed summary of an entire interaction session.
    Episode = 5,
}

impl EventType {
    /// Convert a u8 value to an EventType, returning None for invalid values.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Fact),
            1 => Some(Self::Decision),
            2 => Some(Self::Inference),
            3 => Some(Self::Correction),
            4 => Some(Self::Skill),
            5 => Some(Self::Episode),
            _ => None,
        }
    }

    /// Return a human-readable name for this event type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::Inference => "inference",
            Self::Correction => "correction",
            Self::Skill => "skill",
            Self::Episode => "episode",
        }
    }

    /// Parse an event type from a string name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "fact" => Some(Self::Fact),
            "decision" => Some(Self::Decision),
            "inference" => Some(Self::Inference),
            "correction" => Some(Self::Correction),
            "skill" => Some(Self::Skill),
            "episode" => Some(Self::Episode),
            _ => None,
        }
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A single cognitive event — the atomic unit of agent memory.
#[derive(Debug, Clone, Serialize)]
pub struct CognitiveEvent {
    /// Unique identifier (assigned sequentially).
    pub id: u64,
    /// Type of cognitive event.
    pub event_type: EventType,
    /// When this event was created (Unix epoch microseconds).
    pub created_at: u64,
    /// Which interaction session produced this event.
    pub session_id: u32,
    /// How certain the agent is about this (0.0 = no confidence, 1.0 = certain).
    pub confidence: f32,
    /// How many times this node has been accessed/traversed.
    pub access_count: u32,
    /// When this node was last accessed (Unix epoch microseconds).
    pub last_accessed: u64,
    /// Computed importance decay (higher = more important, decreases over time without access).
    pub decay_score: f32,
    /// The actual content of this event (UTF-8 text, will be compressed in file).
    pub content: String,
    /// Feature vector for similarity operations (dimension = DEFAULT_DIMENSION).
    #[serde(skip_serializing)]
    pub feature_vec: Vec<f32>,
}

impl CognitiveEvent {
    /// Validate this event's fields.
    pub fn validate(&self, dimension: usize) -> AmemResult<()> {
        if self.content.len() > MAX_CONTENT_SIZE {
            return Err(AmemError::ContentTooLarge {
                size: self.content.len(),
                max: MAX_CONTENT_SIZE,
            });
        }
        if !self.feature_vec.is_empty() && self.feature_vec.len() != dimension {
            return Err(AmemError::DimensionMismatch {
                expected: dimension,
                got: self.feature_vec.len(),
            });
        }
        Ok(())
    }
}

/// Builder for constructing CognitiveEvent instances ergonomically.
pub struct CognitiveEventBuilder {
    event_type: EventType,
    content: String,
    session_id: u32,
    confidence: f32,
    feature_vec: Vec<f32>,
    created_at: Option<u64>,
}

impl CognitiveEventBuilder {
    /// Create a new builder with the required fields.
    pub fn new(event_type: EventType, content: impl Into<String>) -> Self {
        Self {
            event_type,
            content: content.into(),
            session_id: 0,
            confidence: 1.0,
            feature_vec: Vec::new(),
            created_at: None,
        }
    }

    /// Set the session ID.
    pub fn session_id(mut self, session_id: u32) -> Self {
        self.session_id = session_id;
        self
    }

    /// Set the confidence value (clamped to [0.0, 1.0]).
    pub fn confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set the feature vector.
    pub fn feature_vec(mut self, vec: Vec<f32>) -> Self {
        self.feature_vec = vec;
        self
    }

    /// Set the creation timestamp.
    pub fn created_at(mut self, ts: u64) -> Self {
        self.created_at = Some(ts);
        self
    }

    /// Build the CognitiveEvent. The id will be 0 (assigned by graph on insertion).
    pub fn build(self) -> CognitiveEvent {
        let now = self.created_at.unwrap_or_else(now_micros);
        let mut feature_vec = self.feature_vec;
        if feature_vec.is_empty() {
            feature_vec = vec![0.0; DEFAULT_DIMENSION];
        }
        CognitiveEvent {
            id: 0,
            event_type: self.event_type,
            created_at: now,
            session_id: self.session_id,
            confidence: self.confidence.clamp(0.0, 1.0),
            access_count: 0,
            last_accessed: now,
            decay_score: 1.0,
            content: self.content,
            feature_vec,
        }
    }
}
