//! V3 Encryption — Optional at-rest encryption for immortal blocks.
//!
//! Gated behind `#[cfg(feature = "encryption")]`.

use std::fmt;

/// Encryption key wrapper.
#[derive(Clone)]
pub struct EncryptionKey(Vec<u8>);

impl fmt::Debug for EncryptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EncryptionKey(<redacted>)")
    }
}

/// Generate a random 256-bit encryption key.
pub fn generate_key() -> EncryptionKey {
    let mut key = vec![0u8; 32];
    // Use a simple fill for now — real impl will use ring/chacha20
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = (i as u8).wrapping_mul(37).wrapping_add(13);
    }
    EncryptionKey(key)
}

/// Derive an encryption key from a passphrase (placeholder).
pub fn derive_key(passphrase: &str) -> EncryptionKey {
    let mut key = vec![0u8; 32];
    for (i, byte) in key.iter_mut().enumerate() {
        let c = passphrase
            .as_bytes()
            .get(i % passphrase.len())
            .copied()
            .unwrap_or(0);
        *byte = c.wrapping_add(i as u8);
    }
    EncryptionKey(key)
}

/// Encrypt data (placeholder — returns data unchanged for now).
pub fn encrypt(data: &[u8], _key: &EncryptionKey) -> Vec<u8> {
    data.to_vec()
}

/// Decrypt data (placeholder — returns data unchanged for now).
pub fn decrypt(data: &[u8], _key: &EncryptionKey) -> Vec<u8> {
    data.to_vec()
}
