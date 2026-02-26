//! Content-addressed, immutable blocks — the fundamental unit of V3 storage.

use blake3::Hasher;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A content-addressed, immutable block.
/// Once written, never modified. Never deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// BLAKE3 hash of content (also serves as ID)
    pub hash: BlockHash,

    /// Hash of previous block (integrity chain)
    pub prev_hash: BlockHash,

    /// Sequence number (monotonic, gap-free)
    pub sequence: u64,

    /// When this block was created
    pub timestamp: DateTime<Utc>,

    /// Block type
    pub block_type: BlockType,

    /// The actual content
    pub content: BlockContent,

    /// Size in bytes (for budgeting)
    pub size_bytes: u32,
}

/// 32-byte BLAKE3 hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockHash(pub [u8; 32]);

impl BlockHash {
    pub fn compute(data: &[u8]) -> Self {
        Self(*blake3::hash(data).as_bytes())
    }

    pub fn zero() -> Self {
        Self([0u8; 32])
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        let bytes = hex::decode(s).ok()?;
        if bytes.len() != 32 {
            return None;
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Some(Self(arr))
    }
}

impl std::fmt::Display for BlockHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Types of blocks we store
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    UserMessage,
    AssistantMessage,
    SystemMessage,
    ToolCall,
    ToolResult,
    FileOperation,
    Decision,
    SessionBoundary,
    Error,
    Checkpoint,
    Custom,
}

/// Block content variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockContent {
    /// Text content (messages)
    Text {
        text: String,
        role: Option<String>,
        tokens: Option<u32>,
    },

    /// Tool invocation
    Tool {
        tool_name: String,
        input: serde_json::Value,
        output: Option<serde_json::Value>,
        duration_ms: Option<u64>,
        success: bool,
    },

    /// File operation
    File {
        path: String,
        operation: FileOperation,
        content_hash: Option<BlockHash>,
        lines: Option<u32>,
        diff: Option<String>,
    },

    /// Decision record
    Decision {
        decision: String,
        reasoning: Option<String>,
        evidence_blocks: Vec<BlockHash>,
        confidence: Option<f32>,
    },

    /// Session boundary
    Boundary {
        boundary_type: BoundaryType,
        context_tokens_before: u32,
        context_tokens_after: u32,
        summary: String,
        continuation_hint: Option<String>,
    },

    /// Error record
    Error {
        error_type: String,
        message: String,
        resolution: Option<String>,
        resolved: bool,
    },

    /// Checkpoint (periodic state snapshot)
    Checkpoint {
        active_files: Vec<String>,
        working_context: String,
        pending_tasks: Vec<String>,
    },

    /// Raw bytes (for binary content)
    Binary {
        #[serde(with = "base64_serde")]
        data: Vec<u8>,
        mime_type: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOperation {
    Create,
    Read,
    Update,
    Delete,
    Rename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryType {
    SessionStart,
    SessionEnd,
    Compaction,
    ContextPressure,
    UserRequested,
    Checkpoint,
}

impl Block {
    /// Create a new block
    pub fn new(
        prev_hash: BlockHash,
        sequence: u64,
        block_type: BlockType,
        content: BlockContent,
    ) -> Self {
        let timestamp = Utc::now();
        let content_bytes = serde_json::to_vec(&content).unwrap_or_default();
        let size_bytes = content_bytes.len() as u32;

        // Hash includes: prev_hash + sequence + timestamp + content
        let mut hasher = Hasher::new();
        hasher.update(&prev_hash.0);
        hasher.update(&sequence.to_le_bytes());
        hasher.update(timestamp.to_rfc3339().as_bytes());
        hasher.update(&content_bytes);
        let hash = BlockHash(*hasher.finalize().as_bytes());

        Self {
            hash,
            prev_hash,
            sequence,
            timestamp,
            block_type,
            content,
            size_bytes,
        }
    }

    /// Verify block integrity
    pub fn verify(&self) -> bool {
        let content_bytes = serde_json::to_vec(&self.content).unwrap_or_default();

        let mut hasher = Hasher::new();
        hasher.update(&self.prev_hash.0);
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        hasher.update(&content_bytes);
        let computed = BlockHash(*hasher.finalize().as_bytes());

        computed == self.hash
    }

    /// Get a short summary of block content (for display).
    pub fn content_summary(&self) -> String {
        let full = match &self.content {
            BlockContent::Text { text, role, .. } => {
                format!("[{}] {}", role.as_deref().unwrap_or("text"), text)
            }
            BlockContent::Tool { tool_name, success, .. } => {
                format!("tool:{} ({})", tool_name, if *success { "ok" } else { "err" })
            }
            BlockContent::File { path, operation, .. } => {
                format!("{:?} {}", operation, path)
            }
            BlockContent::Decision { decision, confidence, .. } => {
                format!("Decision({:.0}%): {}", confidence.unwrap_or(0.0) * 100.0, decision)
            }
            BlockContent::Boundary { boundary_type, summary, .. } => {
                format!("{:?}: {}", boundary_type, summary)
            }
            BlockContent::Error { error_type, message, resolved, .. } => {
                format!("{}:{} [{}]", error_type, message, if *resolved { "resolved" } else { "open" })
            }
            BlockContent::Checkpoint { working_context, .. } => {
                format!("Checkpoint: {}", working_context)
            }
            BlockContent::Binary { mime_type, data } => {
                format!("Binary({}, {} bytes)", mime_type, data.len())
            }
        };
        // Truncate to 200 chars
        if full.len() > 200 {
            format!("{}...", &full[..200])
        } else {
            full
        }
    }

    /// Extract text from block content (for indexing)
    pub fn extract_text(&self) -> Option<String> {
        match &self.content {
            BlockContent::Text { text, .. } => Some(text.clone()),
            BlockContent::Decision { decision, reasoning, .. } => {
                Some(format!("{} {}", decision, reasoning.as_deref().unwrap_or("")))
            }
            BlockContent::Tool { tool_name, .. } => Some(tool_name.clone()),
            BlockContent::File { path, .. } => Some(path.clone()),
            BlockContent::Error { message, .. } => Some(message.clone()),
            BlockContent::Boundary { summary, .. } => Some(summary.clone()),
            BlockContent::Checkpoint { working_context, .. } => Some(working_context.clone()),
            _ => None,
        }
    }
}

// Base64 serialization for binary data
mod base64_serde {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(data))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_creation_and_verify() {
        let block = Block::new(
            BlockHash::zero(),
            0,
            BlockType::UserMessage,
            BlockContent::Text {
                text: "Hello world".to_string(),
                role: Some("user".to_string()),
                tokens: Some(3),
            },
        );

        assert!(block.verify());
        assert_eq!(block.sequence, 0);
        assert_eq!(block.prev_hash, BlockHash::zero());
    }

    #[test]
    fn test_block_hash_hex_roundtrip() {
        let hash = BlockHash::compute(b"test data");
        let hex = hash.to_hex();
        let recovered = BlockHash::from_hex(&hex).unwrap();
        assert_eq!(hash, recovered);
    }

    #[test]
    fn test_block_integrity_chain() {
        let b0 = Block::new(
            BlockHash::zero(),
            0,
            BlockType::UserMessage,
            BlockContent::Text {
                text: "First".to_string(),
                role: None,
                tokens: None,
            },
        );

        let b1 = Block::new(
            b0.hash,
            1,
            BlockType::AssistantMessage,
            BlockContent::Text {
                text: "Second".to_string(),
                role: None,
                tokens: None,
            },
        );

        assert!(b0.verify());
        assert!(b1.verify());
        assert_eq!(b1.prev_hash, b0.hash);
    }

    #[test]
    fn test_block_serialization() {
        let block = Block::new(
            BlockHash::zero(),
            0,
            BlockType::Decision,
            BlockContent::Decision {
                decision: "Use V3 architecture".to_string(),
                reasoning: Some("Better persistence".to_string()),
                evidence_blocks: vec![],
                confidence: Some(0.95),
            },
        );

        let json = serde_json::to_string(&block).unwrap();
        let recovered: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.hash, block.hash);
    }
}
