//! Encryption key rotation and lifecycle management.
//!
//! Keys follow the lifecycle: Active → Retired → Archived.
//! Old memories are lazily re-encrypted when accessed.

use super::store::{LongevityError, LongevityStore};
use serde::{Deserialize, Serialize};

/// Encryption key status in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyStatus {
    /// Currently used for new encryptions
    Active,
    /// Kept for decryption only, no new encryptions
    Retired,
    /// All data re-encrypted, kept in history
    Archived,
}

impl KeyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Retired => "retired",
            Self::Archived => "archived",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "retired" => Some(Self::Retired),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

/// Key lifecycle event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyLifecycle {
    pub key_id: String,
    pub algorithm: String,
    pub status: KeyStatus,
    pub created_at: String,
    pub retired_at: Option<String>,
}

/// Encryption key rotation manager.
pub struct EncryptionRotator;

impl EncryptionRotator {
    /// Generate and store a new active key, retiring the current one.
    pub fn rotate_key(
        store: &LongevityStore,
        algorithm: &str,
    ) -> Result<String, LongevityError> {
        // Retire current active key
        if let Some(current) = store.get_active_encryption_key()? {
            store.retire_encryption_key(&current.key_id)?;
        }

        // Generate new key ID and placeholder blob
        // In production, this would use proper key generation (ChaCha20/AES-256)
        let key_id = ulid::Ulid::new().to_string();
        let key_blob = blake3::hash(key_id.as_bytes()).as_bytes().to_vec();

        store.store_encryption_key(&key_id, algorithm, "active", &key_blob)?;

        Ok(key_id)
    }

    /// Get the current active key lifecycle info.
    pub fn current_key(store: &LongevityStore) -> Result<Option<KeyLifecycle>, LongevityError> {
        let key = store.get_active_encryption_key()?;
        Ok(key.map(|k| KeyLifecycle {
            key_id: k.key_id,
            algorithm: k.algorithm,
            status: KeyStatus::Active,
            created_at: k.created_at,
            retired_at: k.retired_at,
        }))
    }

    /// Count memories still encrypted with a specific key.
    pub fn memories_with_key(
        _store: &LongevityStore,
        _key_id: &str,
    ) -> Result<u64, LongevityError> {
        // This would query memories WHERE encryption_key_id = key_id
        // For now, return 0 as encryption is not yet active
        Ok(0)
    }
}
