//! Six-layer memory hierarchy: Raw → Episode → Summary → Pattern → Trait → Identity.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The six cognitive compression layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MemoryLayer {
    /// Layer 0: Raw events — individual messages, tool calls, file operations
    Raw = 0,
    /// Layer 1: Episodes — grouped events from a session or topic
    Episode = 1,
    /// Layer 2: Summaries — key-point distillations from episodes
    Summary = 2,
    /// Layer 3: Patterns — recurring behaviors, preferences, habits
    Pattern = 3,
    /// Layer 4: Traits — stable identity attributes ("prefers Rust over Go")
    Trait = 4,
    /// Layer 5: Identity — core cognitive essence (never auto-compressed)
    Identity = 5,
}

impl MemoryLayer {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Raw),
            1 => Some(Self::Episode),
            2 => Some(Self::Summary),
            3 => Some(Self::Pattern),
            4 => Some(Self::Trait),
            5 => Some(Self::Identity),
            _ => None,
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Raw => "event",
            Self::Episode => "episode",
            Self::Summary => "summary",
            Self::Pattern => "pattern",
            Self::Trait => "trait",
            Self::Identity => "identity",
        }
    }

    /// Target compression ratio when consolidating TO this layer.
    pub fn target_compression_ratio(&self) -> f64 {
        match self {
            Self::Raw => 1.0,
            Self::Episode => 5.0,
            Self::Summary => 10.0,
            Self::Pattern => 20.0,
            Self::Trait => 100.0,
            Self::Identity => 1000.0,
        }
    }

    /// Minimum significance to be immune from consolidation at this layer.
    pub fn preservation_threshold(&self) -> f64 {
        match self {
            Self::Raw => 0.8,
            Self::Episode => 0.7,
            Self::Summary => 0.6,
            Self::Pattern => 0.5,
            Self::Trait => 0.4,
            Self::Identity => 0.0, // Never auto-compressed
        }
    }

    /// Next layer up in the compression hierarchy.
    pub fn next_layer(&self) -> Option<Self> {
        match self {
            Self::Raw => Some(Self::Episode),
            Self::Episode => Some(Self::Summary),
            Self::Summary => Some(Self::Pattern),
            Self::Pattern => Some(Self::Trait),
            Self::Trait => Some(Self::Identity),
            Self::Identity => None,
        }
    }

    pub fn all() -> &'static [MemoryLayer] {
        &[
            Self::Raw,
            Self::Episode,
            Self::Summary,
            Self::Pattern,
            Self::Trait,
            Self::Identity,
        ]
    }
}

impl fmt::Display for MemoryLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.content_type())
    }
}

/// A memory record stored in the longevity SQLite database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// ULID (time-sortable unique ID)
    pub id: String,
    /// Which cognitive layer
    pub layer: MemoryLayer,
    /// The memory content (JSON)
    pub content: serde_json::Value,
    /// Content type tag
    pub content_type: String,
    /// Float32 embedding vector (optional)
    pub embedding: Option<Vec<f32>>,
    /// Which model generated the embedding
    pub embedding_model: Option<String>,
    /// Computed significance score (0.0 - 1.0)
    pub significance: f64,
    /// How many times this memory has been retrieved
    pub access_count: u64,
    /// Last time this memory was accessed (ISO 8601)
    pub last_accessed: Option<String>,
    /// When this memory was created (ISO 8601)
    pub created_at: String,
    /// Source memory IDs that were compressed into this (JSON array)
    pub original_ids: Option<Vec<String>>,
    /// Session that created this memory
    pub session_id: Option<String>,
    /// Project isolation key (canonical-path hash)
    pub project_id: String,
    /// Extensible metadata (JSON)
    pub metadata: Option<serde_json::Value>,
    /// Which encryption key protects this (NULL = plaintext)
    pub encryption_key_id: Option<String>,
    /// Schema version when this record was created
    pub schema_version: u32,
}

impl MemoryRecord {
    /// Create a new raw-layer memory record.
    pub fn new_raw(
        id: String,
        content: serde_json::Value,
        project_id: String,
        session_id: Option<String>,
    ) -> Self {
        Self {
            id,
            layer: MemoryLayer::Raw,
            content,
            content_type: "event".to_string(),
            embedding: None,
            embedding_model: None,
            significance: 0.5,
            access_count: 0,
            last_accessed: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            original_ids: None,
            session_id,
            project_id,
            metadata: None,
            encryption_key_id: None,
            schema_version: 1,
        }
    }

    /// Create a compressed memory from source memories.
    pub fn new_compressed(
        id: String,
        layer: MemoryLayer,
        content: serde_json::Value,
        source_ids: Vec<String>,
        project_id: String,
    ) -> Self {
        Self {
            id,
            layer,
            content,
            content_type: layer.content_type().to_string(),
            embedding: None,
            embedding_model: None,
            significance: 0.5,
            access_count: 0,
            last_accessed: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            original_ids: Some(source_ids),
            session_id: None,
            project_id,
            metadata: None,
            encryption_key_id: None,
            schema_version: 1,
        }
    }

    /// Extract searchable text from the memory content.
    pub fn extract_text(&self) -> String {
        match &self.content {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(map) => {
                let mut parts = Vec::new();
                if let Some(serde_json::Value::String(text)) = map.get("text") {
                    parts.push(text.as_str());
                }
                if let Some(serde_json::Value::String(summary)) = map.get("summary") {
                    parts.push(summary.as_str());
                }
                if let Some(serde_json::Value::String(decision)) = map.get("decision") {
                    parts.push(decision.as_str());
                }
                if let Some(serde_json::Value::String(pattern)) = map.get("pattern") {
                    parts.push(pattern.as_str());
                }
                parts.join(" ")
            }
            other => other.to_string(),
        }
    }
}

/// Statistics for the memory hierarchy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HierarchyStats {
    pub raw_count: u64,
    pub episode_count: u64,
    pub summary_count: u64,
    pub pattern_count: u64,
    pub trait_count: u64,
    pub identity_count: u64,
    pub raw_bytes: u64,
    pub episode_bytes: u64,
    pub summary_bytes: u64,
    pub pattern_bytes: u64,
    pub trait_bytes: u64,
    pub identity_bytes: u64,
    pub total_count: u64,
    pub total_bytes: u64,
}

impl HierarchyStats {
    pub fn count_for_layer(&self, layer: MemoryLayer) -> u64 {
        match layer {
            MemoryLayer::Raw => self.raw_count,
            MemoryLayer::Episode => self.episode_count,
            MemoryLayer::Summary => self.summary_count,
            MemoryLayer::Pattern => self.pattern_count,
            MemoryLayer::Trait => self.trait_count,
            MemoryLayer::Identity => self.identity_count,
        }
    }

    pub fn bytes_for_layer(&self, layer: MemoryLayer) -> u64 {
        match layer {
            MemoryLayer::Raw => self.raw_bytes,
            MemoryLayer::Episode => self.episode_bytes,
            MemoryLayer::Summary => self.summary_bytes,
            MemoryLayer::Pattern => self.pattern_bytes,
            MemoryLayer::Trait => self.trait_bytes,
            MemoryLayer::Identity => self.identity_bytes,
        }
    }
}

/// The memory hierarchy manager provides operations across layers.
pub struct MemoryHierarchy;

impl MemoryHierarchy {
    /// Group raw memories by session and topic similarity for episode creation.
    pub fn group_for_episodes(memories: &[MemoryRecord]) -> Vec<Vec<&MemoryRecord>> {
        let mut groups: Vec<Vec<&MemoryRecord>> = Vec::new();
        let mut current_group: Vec<&MemoryRecord> = Vec::new();
        let mut current_session: Option<&str> = None;

        for memory in memories {
            let session = memory.session_id.as_deref();
            if session != current_session && !current_group.is_empty() {
                groups.push(std::mem::take(&mut current_group));
            }
            current_session = session;
            current_group.push(memory);
        }
        if !current_group.is_empty() {
            groups.push(current_group);
        }

        // Split large groups into chunks of ~20 events
        let mut result = Vec::new();
        for group in groups {
            if group.len() > 30 {
                for chunk in group.chunks(20) {
                    result.push(chunk.to_vec());
                }
            } else {
                result.push(group);
            }
        }
        result
    }

    /// Create an episode summary from a group of raw memories.
    /// Uses pure algorithmic approach (no LLM). LLM-enhanced version is separate.
    pub fn create_episode_summary(memories: &[&MemoryRecord]) -> serde_json::Value {
        let mut decisions = Vec::new();
        let mut files_touched = Vec::new();
        let mut key_points = Vec::new();
        let mut tools_used = Vec::new();

        for memory in memories {
            if let serde_json::Value::Object(ref map) = memory.content {
                // Extract decisions
                if let Some(serde_json::Value::String(d)) = map.get("decision") {
                    decisions.push(d.clone());
                }
                // Extract file paths
                if let Some(serde_json::Value::String(p)) = map.get("path") {
                    if !files_touched.contains(p) {
                        files_touched.push(p.clone());
                    }
                }
                // Extract tool names
                if let Some(serde_json::Value::String(t)) = map.get("tool_name") {
                    if !tools_used.contains(t) {
                        tools_used.push(t.clone());
                    }
                }
                // Extract text snippets for key points
                if let Some(serde_json::Value::String(text)) = map.get("text") {
                    if text.len() > 20 && text.len() < 500 {
                        key_points.push(text.clone());
                    }
                }
            }
        }

        // Limit key points to most representative
        key_points.truncate(5);

        let session_id = memories
            .first()
            .and_then(|m| m.session_id.clone())
            .unwrap_or_default();

        let time_range = format!(
            "{} to {}",
            memories.first().map(|m| m.created_at.as_str()).unwrap_or("?"),
            memories.last().map(|m| m.created_at.as_str()).unwrap_or("?")
        );

        serde_json::json!({
            "summary": format!("Session episode with {} events", memories.len()),
            "event_count": memories.len(),
            "session_id": session_id,
            "time_range": time_range,
            "decisions": decisions,
            "files_touched": files_touched,
            "tools_used": tools_used,
            "key_points": key_points,
        })
    }

    /// Extract patterns from a set of summaries.
    /// Pure algorithmic: frequency analysis of themes, files, decisions.
    pub fn extract_patterns(summaries: &[&MemoryRecord]) -> Vec<serde_json::Value> {
        use std::collections::HashMap;

        let mut file_frequency: HashMap<String, u32> = HashMap::new();
        let mut tool_frequency: HashMap<String, u32> = HashMap::new();
        let mut decision_themes: Vec<String> = Vec::new();

        for summary in summaries {
            if let serde_json::Value::Object(ref map) = summary.content {
                // Count file frequencies
                if let Some(serde_json::Value::Array(files)) = map.get("files_touched") {
                    for f in files {
                        if let serde_json::Value::String(path) = f {
                            *file_frequency.entry(path.clone()).or_default() += 1;
                        }
                    }
                }
                // Count tool frequencies
                if let Some(serde_json::Value::Array(tools)) = map.get("tools_used") {
                    for t in tools {
                        if let serde_json::Value::String(tool) = t {
                            *tool_frequency.entry(tool.clone()).or_default() += 1;
                        }
                    }
                }
                // Collect decision themes
                if let Some(serde_json::Value::Array(decisions)) = map.get("decisions") {
                    for d in decisions {
                        if let serde_json::Value::String(dec) = d {
                            decision_themes.push(dec.clone());
                        }
                    }
                }
            }
        }

        let mut patterns = Vec::new();

        // File focus patterns (files accessed > 3 times across summaries)
        let frequent_files: Vec<_> = file_frequency
            .into_iter()
            .filter(|(_, count)| *count >= 3)
            .collect();
        if !frequent_files.is_empty() {
            patterns.push(serde_json::json!({
                "pattern_type": "file_focus",
                "description": format!("Frequently works with: {}", frequent_files.iter().map(|(f, _)| f.as_str()).collect::<Vec<_>>().join(", ")),
                "files": frequent_files.iter().map(|(f, c)| serde_json::json!({"file": f, "frequency": c})).collect::<Vec<_>>(),
                "confidence": 0.7,
                "source_count": summaries.len(),
            }));
        }

        // Tool usage patterns
        let frequent_tools: Vec<_> = tool_frequency
            .into_iter()
            .filter(|(_, count)| *count >= 3)
            .collect();
        if !frequent_tools.is_empty() {
            patterns.push(serde_json::json!({
                "pattern_type": "tool_preference",
                "description": format!("Frequently uses: {}", frequent_tools.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>().join(", ")),
                "tools": frequent_tools.iter().map(|(t, c)| serde_json::json!({"tool": t, "frequency": c})).collect::<Vec<_>>(),
                "confidence": 0.6,
                "source_count": summaries.len(),
            }));
        }

        // Decision count pattern
        if decision_themes.len() >= 5 {
            patterns.push(serde_json::json!({
                "pattern_type": "decision_maker",
                "description": format!("{} decisions made across {} episodes", decision_themes.len(), summaries.len()),
                "sample_decisions": decision_themes.iter().take(5).collect::<Vec<_>>(),
                "confidence": 0.5,
                "source_count": summaries.len(),
            }));
        }

        patterns
    }

    /// Distill traits from a set of patterns. Returns trait descriptions.
    /// Pure algorithmic fallback. LLM-enhanced version is separate.
    pub fn distill_traits(patterns: &[&MemoryRecord]) -> Vec<serde_json::Value> {
        let mut traits = Vec::new();

        // Aggregate pattern data
        let mut all_files = Vec::new();
        let mut all_tools = Vec::new();
        let mut total_decisions = 0u32;

        for pattern in patterns {
            if let serde_json::Value::Object(ref map) = pattern.content {
                if let Some(serde_json::Value::String(ptype)) = map.get("pattern_type") {
                    match ptype.as_str() {
                        "file_focus" => {
                            if let Some(serde_json::Value::Array(files)) = map.get("files") {
                                for f in files {
                                    if let Some(name) = f.get("file").and_then(|v| v.as_str()) {
                                        all_files.push(name.to_string());
                                    }
                                }
                            }
                        }
                        "tool_preference" => {
                            if let Some(serde_json::Value::Array(tools)) = map.get("tools") {
                                for t in tools {
                                    if let Some(name) = t.get("tool").and_then(|v| v.as_str()) {
                                        all_tools.push(name.to_string());
                                    }
                                }
                            }
                        }
                        "decision_maker" => {
                            total_decisions += 1;
                        }
                        _ => {}
                    }
                }
            }
        }

        if !all_files.is_empty() {
            // Deduplicate and find most common
            let mut file_counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
            for f in &all_files {
                *file_counts.entry(f.as_str()).or_default() += 1;
            }
            let mut top_files: Vec<_> = file_counts.into_iter().collect();
            top_files.sort_by(|a, b| b.1.cmp(&a.1));
            top_files.truncate(5);

            traits.push(serde_json::json!({
                "trait_type": "focus_area",
                "description": format!("Primary focus: {}", top_files.iter().map(|(f, _)| *f).collect::<Vec<_>>().join(", ")),
                "evidence_count": patterns.len(),
                "confidence": 0.7,
            }));
        }

        if !all_tools.is_empty() {
            traits.push(serde_json::json!({
                "trait_type": "tool_affinity",
                "description": format!("Preferred tools: {}", all_tools.iter().take(5).cloned().collect::<Vec<_>>().join(", ")),
                "evidence_count": patterns.len(),
                "confidence": 0.6,
            }));
        }

        if total_decisions > 0 {
            traits.push(serde_json::json!({
                "trait_type": "decisiveness",
                "description": format!("Active decision maker ({} decisions observed)", total_decisions),
                "evidence_count": total_decisions,
                "confidence": 0.5,
            }));
        }

        traits
    }
}
