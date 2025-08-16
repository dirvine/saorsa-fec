//! Encryption module providing convergent and standard encryption for Saorsa FEC
//!
//! This module implements AES-256-GCM encryption with multiple modes:
//! - Convergent encryption for deduplication across all users
//! - Convergent with secret for controlled deduplication
//! - Random key for maximum privacy

use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use anyhow::{Context, Result};
use blake3::Hasher;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Whether encryption is enabled
    pub enabled: bool,
    /// Encryption mode to use
    pub mode: EncryptionMode,
    /// Optional convergence secret for controlled deduplication
    #[serde(skip_serializing, skip_deserializing)]
    pub convergence_secret: Option<ConvergenceSecret>,
}

/// Secret used for convergent encryption with controlled deduplication
#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct ConvergenceSecret([u8; 32]);

impl ConvergenceSecret {
    /// Create a new convergence secret
    pub fn new(secret: [u8; 32]) -> Self {
        Self(secret)
    }

    /// Get the secret as bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Encryption mode selection
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EncryptionMode {
    /// Pure convergent encryption (deduplication across all users)
    Convergent,
    /// Convergent encryption with secret (controlled deduplication)
    ConvergentWithSecret,
    /// Random key encryption (no deduplication)
    RandomKey,
}

/// Encryption algorithm selection
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    /// AES-256-GCM authenticated encryption
    Aes256Gcm,
}

/// Key derivation method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyDerivation {
    /// Blake3-based convergent key derivation
    Blake3Convergent,
    /// Random key generation
    Random,
}

/// Metadata about how data was encrypted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionMetadata {
    /// Algorithm used for encryption
    pub algorithm: EncryptionAlgorithm,
    /// Key derivation method used
    pub key_derivation: KeyDerivation,
    /// ID of convergence secret if used (Blake3 hash of secret)
    pub convergence_secret_id: Option<[u8; 16]>,
    /// Nonce used for encryption
    pub nonce: [u8; 12],
}

/// Encryption key wrapper with secure handling
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct EncryptionKey([u8; 32]);

impl EncryptionKey {
    /// Create a new encryption key
    pub fn new(key: [u8; 32]) -> Self {
        Self(key)
    }

    /// Get the key as bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Main encryption engine
pub struct CryptoEngine {
    /// Last nonce used (for metadata)
    last_nonce: Option<[u8; 12]>,
}

impl CryptoEngine {
    /// Create a new crypto engine
    pub fn new() -> Self {
        Self { last_nonce: None }
    }

    /// Encrypt data using the specified key
    pub fn encrypt(&mut self, data: &[u8], key: &EncryptionKey) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_bytes()));
        let nonce_bytes = Aes256Gcm::generate_nonce(&mut OsRng);
        self.last_nonce = Some(nonce_bytes.into());

        let ciphertext = cipher
            .encrypt(&nonce_bytes, data)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        // Prepend nonce to ciphertext for storage
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt data using the specified key
    pub fn decrypt(&self, encrypted_data: &[u8], key: &EncryptionKey) -> Result<Vec<u8>> {
        if encrypted_data.len() < 12 {
            anyhow::bail!("Encrypted data too short to contain nonce");
        }

        let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_bytes()));
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("Decryption failed"))?;

        Ok(plaintext)
    }

    /// Get the last nonce used
    pub fn last_nonce(&self) -> [u8; 12] {
        self.last_nonce.unwrap_or([0u8; 12])
    }

    /// Reconstruct encryption key from metadata
    pub fn reconstruct_key(
        &self,
        metadata: &Option<EncryptionMetadata>,
        original_data: Option<&[u8]>,
        convergence_secret: Option<&ConvergenceSecret>,
    ) -> Result<EncryptionKey> {
        let metadata = metadata
            .as_ref()
            .context("No encryption metadata available")?;

        match metadata.key_derivation {
            KeyDerivation::Blake3Convergent => {
                let data = original_data
                    .context("Original data required for convergent key reconstruction")?;

                let secret = if metadata.convergence_secret_id.is_some() {
                    convergence_secret.map(|s| s.as_bytes())
                } else {
                    None
                };

                Ok(derive_convergent_key(data, secret))
            }
            KeyDerivation::Random => {
                anyhow::bail!("Random keys cannot be reconstructed without external storage")
            }
        }
    }
}

impl Default for CryptoEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Derive a convergent encryption key from content
pub fn derive_convergent_key(content: &[u8], secret: Option<&[u8; 32]>) -> EncryptionKey {
    let mut hasher = Hasher::new();

    // Domain separation
    hasher.update(b"saorsa-fec-v1-key");

    // Include secret if provided
    if let Some(s) = secret {
        hasher.update(s);
    }

    // Include content for convergence
    hasher.update(content);

    let hash = hasher.finalize();
    EncryptionKey::new(*hash.as_bytes())
}

/// Generate a random encryption key
pub fn generate_random_key() -> EncryptionKey {
    let mut key = [0u8; 32];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut key);
    EncryptionKey::new(key)
}

/// Compute convergence secret ID
pub fn compute_secret_id(secret: &ConvergenceSecret) -> [u8; 16] {
    let hash = blake3::hash(secret.as_bytes());
    let mut id = [0u8; 16];
    id.copy_from_slice(&hash.as_bytes()[..16]);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_roundtrip() {
        let mut engine = CryptoEngine::new();
        let data = b"Hello, World!";
        let key = derive_convergent_key(data, None);

        let encrypted = engine.encrypt(data, &key).unwrap();
        assert_ne!(encrypted, data);
        assert!(encrypted.len() > data.len() + 12); // Nonce + tag overhead

        let decrypted = engine.decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_convergent_key_deterministic() {
        let data = b"Test data";
        let key1 = derive_convergent_key(data, None);
        let key2 = derive_convergent_key(data, None);

        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_convergent_key_with_secret() {
        let data = b"Test data";
        let secret = ConvergenceSecret::new([42u8; 32]);

        let key_with_secret = derive_convergent_key(data, Some(secret.as_bytes()));
        let key_without = derive_convergent_key(data, None);

        assert_ne!(key_with_secret.as_bytes(), key_without.as_bytes());
    }

    #[test]
    fn test_random_key_uniqueness() {
        let key1 = generate_random_key();
        let key2 = generate_random_key();

        assert_ne!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_decrypt_invalid_data() {
        let engine = CryptoEngine::new();
        let key = generate_random_key();

        // Too short
        let result = engine.decrypt(&[0u8; 10], &key);
        assert!(result.is_err());

        // Invalid ciphertext
        let result = engine.decrypt(&[0u8; 30], &key);
        assert!(result.is_err());
    }

    #[test]
    fn test_encryption_metadata_serialization() {
        let metadata = EncryptionMetadata {
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key_derivation: KeyDerivation::Blake3Convergent,
            convergence_secret_id: Some([1u8; 16]),
            nonce: [2u8; 12],
        };

        let serialized = bincode::serialize(&metadata).unwrap();
        let deserialized: EncryptionMetadata = bincode::deserialize(&serialized).unwrap();

        assert_eq!(
            deserialized.convergence_secret_id,
            metadata.convergence_secret_id
        );
        assert_eq!(deserialized.nonce, metadata.nonce);
    }
}
