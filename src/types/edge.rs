//! Edge types and the core edge struct.

use serde::Serialize;

use super::now_micros;

/// The type of relationship between two cognitive events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[repr(u8)]
pub enum EdgeType {
    /// This event happened because of that event.
    CausedBy = 0,
    /// This event provides evidence for that event.
    Supports = 1,
    /// This event conflicts with that event.
    Contradicts = 2,
    /// This event replaces that event (newer corrects older).
    Supersedes = 3,
    /// Semantic similarity without causal/logical relationship.
    RelatedTo = 4,
    /// This event belongs to a larger episode or cluster.
    PartOf = 5,
    /// Chronological ordering within a session.
    TemporalNext = 6,
}

impl EdgeType {
    /// Convert a u8 value to an EdgeType, returning None for invalid values.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::CausedBy),
            1 => Some(Self::Supports),
            2 => Some(Self::Contradicts),
            3 => Some(Self::Supersedes),
            4 => Some(Self::RelatedTo),
            5 => Some(Self::PartOf),
            6 => Some(Self::TemporalNext),
            _ => None,
        }
    }

    /// Return a human-readable name for this edge type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::CausedBy => "caused_by",
            Self::Supports => "supports",
            Self::Contradicts => "contradicts",
            Self::Supersedes => "supersedes",
            Self::RelatedTo => "related_to",
            Self::PartOf => "part_of",
            Self::TemporalNext => "temporal_next",
        }
    }

    /// Parse an edge type from a string name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "caused_by" | "causedby" => Some(Self::CausedBy),
            "supports" => Some(Self::Supports),
            "contradicts" => Some(Self::Contradicts),
            "supersedes" => Some(Self::Supersedes),
            "related_to" | "relatedto" => Some(Self::RelatedTo),
            "part_of" | "partof" => Some(Self::PartOf),
            "temporal_next" | "temporalnext" => Some(Self::TemporalNext),
            _ => None,
        }
    }
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A directed relationship between two cognitive events.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Edge {
    /// Source node ID (origin of the relationship).
    pub source_id: u64,
    /// Target node ID (destination of the relationship).
    pub target_id: u64,
    /// Type of relationship.
    pub edge_type: EdgeType,
    /// Strength of relationship (0.0 = weak, 1.0 = strong).
    pub weight: f32,
    /// When this edge was created (Unix epoch microseconds).
    pub created_at: u64,
}

impl Edge {
    /// Create a new edge with weight clamped to [0.0, 1.0].
    pub fn new(source_id: u64, target_id: u64, edge_type: EdgeType, weight: f32) -> Self {
        Self {
            source_id,
            target_id,
            edge_type,
            weight: weight.clamp(0.0, 1.0),
            created_at: now_micros(),
        }
    }

    /// Create a new edge with an explicit timestamp.
    pub fn with_timestamp(
        source_id: u64,
        target_id: u64,
        edge_type: EdgeType,
        weight: f32,
        created_at: u64,
    ) -> Self {
        Self {
            source_id,
            target_id,
            edge_type,
            weight: weight.clamp(0.0, 1.0),
            created_at,
        }
    }
}
