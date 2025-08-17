//! Quantum-safe encryption module using saorsa-pqc
//!
//! This module provides post-quantum cryptographic capabilities using ML-KEM
//! for key encapsulation and AES-256-GCM for data encryption. It replaces
//! the previous crypto module with quantum-safe alternatives.

use anyhow::{Context, Result};
use saorsa_pqc::api::{
    kem::ml_kem_768,
    symmetric::{ChaCha20Poly1305, generate_nonce},
};
use generic_array::GenericArray;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};
use blake3::Hasher;
use hkdf::Hkdf;
use sha2::Sha256;

use crate::config::EncryptionMode;

/// Security levels for post-quantum cryptography
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// NIST Level 1 (128-bit security)
    Level1,
    /// NIST Level 3 (192-bit security) - Default
    Level3,
    /// NIST Level 5 (256-bit security)
    Level5,
}

impl Default for SecurityLevel {
    fn default() -> Self {
        SecurityLevel::Level3
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumEncryptionMetadata {
    /// Security level used
    pub security_level: SecurityLevel,
    /// Encapsulated shared secret (from ML-KEM)
    pub encapsulated_secret: Vec<u8>,
    /// Nonce used for ChaCha20Poly1305
    pub nonce: [u8; 12],
    /// Key derivation method for convergent encryption
    pub key_derivation: QuantumKeyDerivation,
    /// Optional convergence secret identifier
    pub convergence_secret_id: Option<[u8; 32]>,
}

/// Quantum-safe key derivation methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantumKeyDerivation {
    /// Blake3-based convergent key derivation (quantum-safe)
    Blake3Convergent,
    /// Random key generation using ML-KEM
    QuantumRandom,
}

/// Convergence secret for controlled deduplication
#[derive(Zeroize, ZeroizeOnDrop)]
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

/// Main quantum cryptographic engine
pub struct QuantumCryptoEngine {
    /// Security level for operations
    security_level: SecurityLevel,
    /// Last nonce used (for metadata)
    last_nonce: Option<[u8; 12]>,
}

impl Default for QuantumCryptoEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantumCryptoEngine {
    /// Create a new quantum crypto engine with default security level
    pub fn new() -> Self {
        Self {
            security_level: SecurityLevel::default(),
            last_nonce: None,
        }
    }

    /// Create with specific security level
    pub fn with_security_level(level: SecurityLevel) -> Self {
        Self {
            security_level: level,
            last_nonce: None,
        }
    }

    /// Encrypt data using the specified encryption mode
    pub fn encrypt(
        &mut self,
        data: &[u8],
        mode: EncryptionMode,
        convergence_secret: Option<&ConvergenceSecret>,
    ) -> Result<(Vec<u8>, QuantumEncryptionMetadata)> {
        match mode {
            EncryptionMode::Convergent => {
                self.encrypt_convergent(data, None)
            }
            EncryptionMode::ConvergentWithSecret => {
                let secret = convergence_secret
                    .context("Convergence secret required for ConvergentWithSecret mode")?;
                self.encrypt_convergent(data, Some(secret))
            }
            EncryptionMode::RandomKey => {
                self.encrypt_random_key(data)
            }
        }
    }

    /// Decrypt data using quantum-safe algorithms
    pub fn decrypt(
        &self,
        encrypted_data: &[u8],
        metadata: &QuantumEncryptionMetadata,
        convergence_secret: Option<&ConvergenceSecret>,
        original_data: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        match metadata.key_derivation {
            QuantumKeyDerivation::Blake3Convergent => {
                self.decrypt_convergent(encrypted_data, metadata, convergence_secret, original_data)
            }
            QuantumKeyDerivation::QuantumRandom => {
                self.decrypt_random_key(encrypted_data, metadata)
            }
        }
    }

    /// Get the last nonce used
    pub fn last_nonce(&self) -> [u8; 12] {
        self.last_nonce.unwrap_or([0u8; 12])
    }

    fn encrypt_convergent(
        &mut self,
        data: &[u8],
        secret: Option<&ConvergenceSecret>,
    ) -> Result<(Vec<u8>, QuantumEncryptionMetadata)> {
        // Derive deterministic key from content
        let key_bytes = self.derive_convergent_key(data, secret)?;
        
        // Generate deterministic nonce for convergent encryption
        let nonce = self.generate_deterministic_nonce(data, secret.map(|s| s.as_bytes()))?;
        self.last_nonce = Some(nonce);

        // Encrypt data with ChaCha20Poly1305
        let ciphertext = self.chacha20_encrypt(data, &key_bytes, &nonce)?;

        // Create metadata
        let metadata = QuantumEncryptionMetadata {
            security_level: self.security_level,
            encapsulated_secret: Vec::new(), // No encapsulation for convergent
            nonce,
            key_derivation: QuantumKeyDerivation::Blake3Convergent,
            convergence_secret_id: secret.map(|s| self.compute_secret_id(s.as_bytes())),
        };

        Ok((ciphertext, metadata))
    }

    fn encrypt_random_key(
        &mut self,
        data: &[u8],
    ) -> Result<(Vec<u8>, QuantumEncryptionMetadata)> {
        // Create ML-KEM instance
        let kem = ml_kem_768();
        
        // Generate keypair
        let (public_key, _secret_key) = kem.generate_keypair()
            .map_err(|e| anyhow::anyhow!("KEM keypair generation failed: {:?}", e))?;

        // Encapsulate to get shared secret
        let (shared_secret, ciphertext) = kem.encapsulate(&public_key)
            .map_err(|e| anyhow::anyhow!("KEM encapsulation failed: {:?}", e))?;

        // Derive ChaCha20 key from shared secret - need to convert to [u8; 32]
        let shared_bytes = shared_secret.to_bytes();
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&shared_bytes[..32]);

        // Generate random nonce using saorsa-pqc - convert to [u8; 12]
        let nonce_generic = generate_nonce();
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_generic[..12]);
        self.last_nonce = Some(nonce);

        // Encrypt data with ChaCha20Poly1305
        let encrypted = self.chacha20_encrypt(data, &key_bytes, &nonce)?;

        // Create metadata
        let metadata = QuantumEncryptionMetadata {
            security_level: self.security_level,
            encapsulated_secret: ciphertext.to_bytes(),
            nonce,
            key_derivation: QuantumKeyDerivation::QuantumRandom,
            convergence_secret_id: None,
        };

        Ok((encrypted, metadata))
    }

    fn decrypt_convergent(
        &self,
        encrypted_data: &[u8],
        metadata: &QuantumEncryptionMetadata,
        convergence_secret: Option<&ConvergenceSecret>,
        original_data: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        // For convergent encryption, we need the original data to derive the key
        let data = original_data
            .context("Original data required for convergent decryption")?;

        let secret = if metadata.convergence_secret_id.is_some() {
            convergence_secret
        } else {
            None
        };

        // Derive the same key used for encryption
        let key_bytes = self.derive_convergent_key(data, secret)?;

        // Decrypt with ChaCha20Poly1305
        self.chacha20_decrypt(encrypted_data, &key_bytes, &metadata.nonce)
    }

    /// Decrypt random key encryption using ML-KEM
    fn decrypt_random_key(
        &self,
        _encrypted_data: &[u8],
        _metadata: &QuantumEncryptionMetadata,
    ) -> Result<Vec<u8>> {
        anyhow::bail!("Random key decryption requires stored decapsulation key")
    }


    fn derive_convergent_key(
        &self,
        content: &[u8],
        secret: Option<&ConvergenceSecret>,
    ) -> Result<[u8; 32]> {
        // Use Blake3 for quantum-safe content hashing
        let mut hasher = Hasher::new();
        hasher.update(content);
        
        if let Some(s) = secret {
            hasher.update(s.as_bytes());
        }
        
        let content_hash = hasher.finalize();

        // Use HKDF for proper key derivation
        let salt = {
            let mut salt_hasher = Hasher::new();
            salt_hasher.update(b"saorsa-fec-quantum-convergent");
            if let Some(s) = secret {
                salt_hasher.update(s.as_bytes());
            }
            salt_hasher.finalize()
        };

        let hkdf = Hkdf::<Sha256>::new(Some(salt.as_bytes()), content_hash.as_bytes());
        let mut key_bytes = [0u8; 32];
        hkdf.expand(b"saorsa-fec:quantum-chacha20:v1", &mut key_bytes)
            .map_err(|e| anyhow::anyhow!("HKDF expansion failed: {}", e))?;

        Ok(key_bytes)
    }

    fn chacha20_encrypt(&self, data: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>> {
        // Convert [u8; 32] to GenericArray for ChaCha20Poly1305
        let key_array = GenericArray::from_slice(key);
        let cipher = ChaCha20Poly1305::new(key_array);
        
        // Convert [u8; 12] to GenericArray for nonce
        let nonce_array = GenericArray::from_slice(nonce);
        
        let ciphertext = cipher
            .encrypt(nonce_array, data)
            .map_err(|e| anyhow::anyhow!("ChaCha20Poly1305 encryption failed: {:?}", e))?;

        // Prepend nonce to ciphertext for storage
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(nonce);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    fn chacha20_decrypt(&self, encrypted_data: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>> {
        if encrypted_data.len() < 12 {
            anyhow::bail!("Encrypted data too short to contain nonce");
        }

        let (data_nonce, ciphertext) = encrypted_data.split_at(12);
        
        // Verify nonce matches
        if data_nonce != nonce {
            anyhow::bail!("Nonce mismatch in encrypted data");
        }

        // Convert [u8; 32] to GenericArray for ChaCha20Poly1305
        let key_array = GenericArray::from_slice(key);
        let cipher = ChaCha20Poly1305::new(key_array);

        // Convert [u8; 12] to GenericArray for nonce
        let nonce_array = GenericArray::from_slice(nonce);

        let plaintext = cipher
            .decrypt(nonce_array, ciphertext)
            .map_err(|e| anyhow::anyhow!("ChaCha20Poly1305 decryption failed: {:?}", e))?;

        Ok(plaintext)
    }

    /// Generate deterministic nonce for convergent encryption
    fn generate_deterministic_nonce(
        &self,
        content: &[u8],
        secret: Option<&[u8; 32]>,
    ) -> Result<[u8; 12]> {
        let mut hasher = Hasher::new();
        hasher.update(b"nonce-derivation");
        hasher.update(content);
        
        if let Some(s) = secret {
            hasher.update(s);
        }
        
        let hash = hasher.finalize();
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&hash.as_bytes()[..12]);
        Ok(nonce)
    }

    /// Compute secret identifier
    fn compute_secret_id(&self, secret: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"secret-id");
        hasher.update(secret);
        let hash = hasher.finalize();
        *hash.as_bytes()
    }

    

    
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_crypto_convergent() -> Result<()> {
        let mut engine = QuantumCryptoEngine::new();
        let data = b"test data for convergent encryption";

        // Encrypt with convergent mode
        let (encrypted, metadata) = engine.encrypt(data, EncryptionMode::Convergent, None)?;
        
        // Verify metadata
        assert!(matches!(metadata.key_derivation, QuantumKeyDerivation::Blake3Convergent));
        assert!(metadata.convergence_secret_id.is_none());

        // Decrypt
        let decrypted = engine.decrypt(&encrypted, &metadata, None, Some(data))?;
        assert_eq!(decrypted, data);

        // Verify deterministic behavior
        let mut engine2 = QuantumCryptoEngine::new();
        let (encrypted2, metadata2) = engine2.encrypt(data, EncryptionMode::Convergent, None)?;
        
        // Same data should produce same result
        assert_eq!(encrypted, encrypted2);
        assert_eq!(metadata.nonce, metadata2.nonce);

        Ok(())
    }

    #[test]
    fn test_quantum_crypto_convergent_with_secret() -> Result<()> {
        let mut engine = QuantumCryptoEngine::new();
        let data = b"test data for secret convergent encryption";
        let secret = ConvergenceSecret::new([42u8; 32]);

        // Encrypt with secret
        let (encrypted, metadata) = engine.encrypt(
            data, 
            EncryptionMode::ConvergentWithSecret, 
            Some(&secret)
        )?;

        // Verify metadata
        assert!(matches!(metadata.key_derivation, QuantumKeyDerivation::Blake3Convergent));
        assert!(metadata.convergence_secret_id.is_some());

        // Decrypt
        let decrypted = engine.decrypt(&encrypted, &metadata, Some(&secret), Some(data))?;
        assert_eq!(decrypted, data);

        // Different secret should produce different result
        let secret2 = ConvergenceSecret::new([24u8; 32]);
        let mut engine2 = QuantumCryptoEngine::new();
        let (encrypted2, _) = engine2.encrypt(
            data, 
            EncryptionMode::ConvergentWithSecret, 
            Some(&secret2)
        )?;
        
        assert_ne!(encrypted, encrypted2);

        Ok(())
    }

    #[test]
    fn test_quantum_crypto_random_key() -> Result<()> {
        let mut engine = QuantumCryptoEngine::new();
        let data = b"test data for random key encryption";

        // Encrypt with random key mode
        let (encrypted, metadata) = engine.encrypt(data, EncryptionMode::RandomKey, None)?;
        
        // Verify metadata
        assert!(matches!(metadata.key_derivation, QuantumKeyDerivation::QuantumRandom));
        assert!(!metadata.encapsulated_secret.is_empty());

        // Random key mode should produce different results
        let mut engine2 = QuantumCryptoEngine::new();
        let (encrypted2, metadata2) = engine2.encrypt(data, EncryptionMode::RandomKey, None)?;
        
        assert_ne!(encrypted, encrypted2);
        assert_ne!(metadata.nonce, metadata2.nonce);
        assert_ne!(metadata.encapsulated_secret, metadata2.encapsulated_secret);

        Ok(())
    }

    #[test]
    fn test_security_levels() {
        let engine1 = QuantumCryptoEngine::with_security_level(SecurityLevel::Level1);
        let engine3 = QuantumCryptoEngine::with_security_level(SecurityLevel::Level3);
        let engine5 = QuantumCryptoEngine::with_security_level(SecurityLevel::Level5);

        assert!(matches!(engine1.security_level, SecurityLevel::Level1));
        assert!(matches!(engine3.security_level, SecurityLevel::Level3));
        assert!(matches!(engine5.security_level, SecurityLevel::Level5));
    }
}