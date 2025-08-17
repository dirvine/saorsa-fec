# Saorsa-PQC Migration Architecture Document

## 1. Executive Summary

This document outlines the comprehensive migration strategy to replace the current AES-256-GCM encryption implementation in saorsa-fec with the quantum-resistant saorsa-pqc library. The migration will transform our encryption from classical algorithms to post-quantum cryptography while maintaining backward compatibility, preserving convergent encryption capabilities, and ensuring zero-downtime transitions.

### 1.1 Business Context
- **Goal**: Quantum-resistant encryption for future-proof data protection
- **Timeline**: Phased migration over 3 releases (v0.4, v0.5, v0.6)
- **Stakeholders**: Saorsa Labs development team, end users with existing encrypted data
- **Success Criteria**: 100% migration with no data loss, maintained performance, quantum resistance

### 1.2 Technical Context
- **Current System**: AES-256-GCM with convergent encryption modes
- **Target System**: ML-KEM/ML-DSA hybrid encryption with saorsa-pqc
- **Integration**: Replace crypto module while preserving API compatibility
- **Performance**: Maintain current throughput (1-7GB/s) with post-quantum security

## 2. Current State Analysis

### 2.1 Existing Encryption Components

#### Redundant Types to Remove
```rust
// These will be completely replaced
pub struct EncryptionKey([u8; 32]);
pub struct CryptoEngine { last_nonce: Option<[u8; 12]> };
pub enum EncryptionAlgorithm { Aes256Gcm };

// These functions will be replaced
pub fn derive_convergent_key(content: &[u8], secret: Option<&[u8; 32]>) -> EncryptionKey;
pub fn generate_random_key() -> EncryptionKey;
impl CryptoEngine {
    pub fn encrypt(&mut self, data: &[u8], key: &EncryptionKey) -> Result<Vec<u8>>;
    pub fn decrypt(&self, encrypted_data: &[u8], key: &EncryptionKey) -> Result<Vec<u8>>;
}
```

#### Components to Preserve
```rust
// Keep these with updated implementations
pub enum EncryptionMode { Convergent, ConvergentWithSecret, RandomKey };
pub struct ConvergenceSecret([u8; 32]);
pub struct EncryptionMetadata { /* updated fields */ };
pub struct EncryptionConfig { /* updated for PQC */ };
```

### 2.2 Current Encryption Flow
1. **Key Derivation**: SHA-256 + HKDF for convergent keys, random for privacy
2. **Encryption**: AES-256-GCM with 12-byte nonces
3. **Storage**: Nonce + ciphertext format
4. **Decryption**: Extract nonce, decrypt with same key

## 3. Architectural Decisions

### 3.1 Hybrid Encryption Architecture

**Selected Pattern**: Classical + Post-Quantum Hybrid Encryption

```rust
// New hybrid encryption architecture
pub struct HybridCipher {
    classical: ClassicalCipher,    // AES-256-GCM for data encryption
    post_quantum: PostQuantumKem,  // ML-KEM for key encapsulation
    digital_signature: Option<PostQuantumSigner>, // ML-DSA for authentication
}
```

**Rationale**:
- **Performance**: Classical symmetric encryption for bulk data (maintains current speed)
- **Security**: Post-quantum KEM for key protection against quantum attacks
- **Compatibility**: Gradual migration path with hybrid support
- **Standards**: Follows NIST post-quantum cryptography recommendations

### 3.2 Technology Stack

| Layer | Current Technology | New Technology | Rationale |
|-------|-------------------|----------------|-----------|
| Symmetric Encryption | AES-256-GCM | AES-256-GCM + ChaCha20-Poly1305 | Maintain performance, add quantum-resistant option |
| Key Encapsulation | N/A | ML-KEM (512/768/1024) | NIST-standardized post-quantum KEM |
| Digital Signatures | N/A | ML-DSA (44/65/87) | NIST-standardized post-quantum signatures |
| Key Derivation | HKDF-SHA256 | HKDF-SHA256 + BLAKE3 | Maintain convergent encryption |
| Library | aes-gcm, hkdf | saorsa-pqc | Integrated post-quantum solution |

### 3.3 Security Level Mapping

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// NIST Level 1 (~128-bit classical security)
    Level1 { kem: MlKem512, sig: Option<MlDsa44> },
    /// NIST Level 3 (~192-bit classical security) 
    Level3 { kem: MlKem768, sig: Option<MlDsa65> },
    /// NIST Level 5 (~256-bit classical security)
    Level5 { kem: MlKem1024, sig: Option<MlDsa87> },
}
```

## 4. New Architecture Design

### 4.1 Core Components

#### 4.1.1 New Crypto Engine
```rust
//! New quantum-resistant crypto engine
use saorsa_pqc::{
    kem::{MlKem512, MlKem768, MlKem1024, KeyGen, Encaps, Decaps},
    sig::{MlDsa44, MlDsa65, MlDsa87, Signer, Verifier},
};

pub struct QuantumCryptoEngine {
    security_level: SecurityLevel,
    hybrid_mode: HybridMode,
    convergence_strategy: ConvergenceStrategy,
}

impl QuantumCryptoEngine {
    pub fn new(config: QuantumCryptoConfig) -> Result<Self>;
    
    /// Encrypt with post-quantum key encapsulation
    pub fn encrypt_hybrid(&mut self, data: &[u8], recipient_pk: &PublicKey) -> Result<EncryptedPacket>;
    
    /// Decrypt with post-quantum key decapsulation  
    pub fn decrypt_hybrid(&self, packet: &EncryptedPacket, private_key: &PrivateKey) -> Result<Vec<u8>>;
    
    /// Convergent encryption with post-quantum protection
    pub fn encrypt_convergent(&mut self, data: &[u8], secret: Option<&ConvergenceSecret>) -> Result<ConvergentPacket>;
    
    /// Decrypt convergent data
    pub fn decrypt_convergent(&self, packet: &ConvergentPacket, original_data: Option<&[u8]>, secret: Option<&ConvergenceSecret>) -> Result<Vec<u8>>;
}
```

#### 4.1.2 Hybrid Encryption Packet Format
```rust
/// Encrypted packet with hybrid post-quantum protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPacket {
    /// Security level used
    pub security_level: SecurityLevel,
    /// Encapsulated symmetric key (post-quantum protected)
    pub encapsulated_key: Vec<u8>,
    /// Encrypted data using symmetric key
    pub ciphertext: Vec<u8>,
    /// Nonce for symmetric encryption
    pub nonce: [u8; 12],
    /// Optional digital signature
    pub signature: Option<Vec<u8>>,
    /// Metadata
    pub metadata: QuantumEncryptionMetadata,
}

/// Convergent packet with deterministic post-quantum key derivation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergentPacket {
    /// Deterministically derived key fingerprint
    pub key_fingerprint: [u8; 32],
    /// Encrypted data
    pub ciphertext: Vec<u8>,
    /// Deterministic nonce
    pub nonce: [u8; 12],
    /// Convergence parameters
    pub convergence_params: ConvergenceParams,
    /// Metadata
    pub metadata: QuantumEncryptionMetadata,
}
```

#### 4.1.3 Updated Configuration
```rust
/// Quantum-resistant encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumCryptoConfig {
    /// Security level (affects key sizes and performance)
    pub security_level: SecurityLevel,
    /// Hybrid encryption mode
    pub hybrid_mode: HybridMode,
    /// Whether to use digital signatures
    pub use_signatures: bool,
    /// Convergent encryption settings
    pub convergent_config: ConvergentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HybridMode {
    /// Classical + Post-Quantum (recommended)
    ClassicalPlusPostQuantum,
    /// Post-Quantum only (future-proof)
    PostQuantumOnly,
    /// Classical only (migration/compatibility)
    ClassicalOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergentConfig {
    /// Enable convergent encryption
    pub enabled: bool,
    /// Key derivation algorithm
    pub kdf: ConvergentKdf,
    /// Deterministic nonce generation
    pub deterministic_nonces: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConvergentKdf {
    /// HKDF with SHA-256 (current)
    HkdfSha256,
    /// HKDF with BLAKE3 (enhanced)
    HkdfBlake3,
    /// Post-quantum safe KDF
    QuantumSafeKdf,
}
```

### 4.2 API Compatibility Layer

```rust
/// Compatibility wrapper for existing API
pub struct LegacyCryptoAdapter {
    quantum_engine: QuantumCryptoEngine,
    migration_state: MigrationState,
}

impl LegacyCryptoAdapter {
    /// Encrypt using legacy API but quantum backend
    pub fn encrypt(&mut self, data: &[u8], key: &LegacyEncryptionKey) -> Result<Vec<u8>> {
        // Convert legacy key to quantum format
        let quantum_key = self.convert_legacy_key(key)?;
        
        // Use quantum engine with compatibility mode
        match self.migration_state {
            MigrationState::Legacy => self.encrypt_legacy_format(data, &quantum_key),
            MigrationState::Hybrid => self.encrypt_hybrid_format(data, &quantum_key),
            MigrationState::QuantumOnly => self.encrypt_quantum_format(data, &quantum_key),
        }
    }
    
    /// Decrypt with automatic format detection
    pub fn decrypt(&self, encrypted_data: &[u8], key: &LegacyEncryptionKey) -> Result<Vec<u8>> {
        let format = self.detect_encryption_format(encrypted_data)?;
        
        match format {
            EncryptionFormat::Legacy => self.decrypt_legacy(encrypted_data, key),
            EncryptionFormat::Hybrid => self.decrypt_hybrid(encrypted_data, key),
            EncryptionFormat::Quantum => self.decrypt_quantum(encrypted_data, key),
        }
    }
}
```

## 5. Migration Strategy

### 5.1 Phase 1: Foundation (v0.4.0)

#### Goals
- Add saorsa-pqc dependency
- Implement hybrid encryption alongside existing crypto
- No breaking API changes

#### Implementation
```rust
// Add to Cargo.toml
[dependencies]
saorsa-pqc = "0.1"

// New module structure
mod crypto {
    mod legacy;           // Current AES-256-GCM implementation
    mod quantum;          // New saorsa-pqc implementation  
    mod hybrid;           // Hybrid encryption combining both
    mod migration;        // Migration utilities
    mod compatibility;    // API compatibility layer
}
```

#### Deliverables
- [ ] Quantum crypto engine implementation
- [ ] Hybrid encryption support
- [ ] Comprehensive test suite
- [ ] Performance benchmarks
- [ ] Migration detection utilities

### 5.2 Phase 2: Hybrid Deployment (v0.5.0)

#### Goals
- Enable hybrid encryption by default
- Provide migration tools for existing data
- Maintain backward compatibility

#### Implementation
```rust
/// Migration tool for existing data
pub struct DataMigrator {
    source_config: LegacyConfig,
    target_config: QuantumConfig,
    batch_size: usize,
}

impl DataMigrator {
    /// Migrate encrypted shards to quantum format
    pub async fn migrate_storage(&mut self, storage: &dyn StorageBackend) -> Result<MigrationReport> {
        let mut report = MigrationReport::new();
        
        // Scan for legacy encrypted shards
        let legacy_shards = storage.list_shards_by_format(EncryptionFormat::Legacy).await?;
        
        for shard_batch in legacy_shards.chunks(self.batch_size) {
            let migrated = self.migrate_shard_batch(shard_batch, storage).await?;
            report.add_migrated(migrated);
        }
        
        Ok(report)
    }
    
    /// Migrate individual shard
    async fn migrate_shard(&mut self, shard: &Shard, storage: &dyn StorageBackend) -> Result<Shard> {
        // 1. Decrypt with legacy engine
        let plaintext = self.legacy_engine.decrypt(&shard.data, &shard.metadata)?;
        
        // 2. Re-encrypt with quantum engine
        let quantum_encrypted = self.quantum_engine.encrypt(&plaintext, &self.target_config)?;
        
        // 3. Create new quantum shard
        let quantum_shard = Shard {
            cid: Cid::from_data(&quantum_encrypted),
            data: quantum_encrypted,
            metadata: self.create_quantum_metadata()?,
            header: self.create_quantum_header()?,
        };
        
        // 4. Store quantum shard
        storage.store_shard(&quantum_shard).await?;
        
        // 5. Mark legacy shard for deletion (after verification)
        storage.mark_for_deletion(&shard.cid).await?;
        
        Ok(quantum_shard)
    }
}
```

#### Deliverables
- [ ] Data migration tools
- [ ] Hybrid encryption enabled by default
- [ ] Migration progress tracking
- [ ] Rollback capabilities
- [ ] Performance monitoring

### 5.3 Phase 3: Quantum-Only (v0.6.0)

#### Goals
- Remove legacy encryption code
- Default to quantum-only encryption
- Clean up deprecated APIs

#### Implementation
```rust
// Remove legacy types and functions
// pub struct EncryptionKey([u8; 32]); // REMOVED
// pub struct CryptoEngine; // REMOVED  
// pub enum EncryptionAlgorithm { Aes256Gcm }; // REMOVED

// Replace with quantum-only implementation
pub use quantum::*;

/// Final quantum-only configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoConfig {
    pub security_level: SecurityLevel,
    pub encryption_mode: EncryptionMode,
    pub use_signatures: bool,
    pub convergent_config: ConvergentConfig,
}
```

#### Deliverables
- [ ] Legacy code removal
- [ ] Quantum-only defaults
- [ ] Updated documentation
- [ ] Final migration verification
- [ ] Performance optimization

## 6. Data Architecture

### 6.1 New Data Format

#### Quantum Shard Header (128 bytes)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumShardHeader {
    /// Version identifier
    pub version: u8,                     // 1 byte
    /// Security level used
    pub security_level: SecurityLevel,   // 1 byte
    /// Encryption mode
    pub encryption_mode: EncryptionMode, // 1 byte
    /// FEC parameters
    pub fec_params: (u8, u8),           // 2 bytes
    /// Data size
    pub data_size: u32,                 // 4 bytes
    /// Key encapsulation size
    pub encap_size: u16,                // 2 bytes
    /// Signature size (0 if not signed)
    pub sig_size: u16,                  // 2 bytes
    /// Convergence fingerprint (for convergent mode)
    pub convergence_fp: Option<[u8; 32]>, // 33 bytes (1 + 32)
    /// Reserved for future use
    pub reserved: [u8; 81],             // 81 bytes
}
```

#### Migration Metadata
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationMetadata {
    /// Original encryption format
    pub source_format: EncryptionFormat,
    /// Target encryption format  
    pub target_format: EncryptionFormat,
    /// Migration timestamp
    pub migrated_at: u64,
    /// Migration tool version
    pub migration_version: String,
    /// Verification hash
    pub verification_hash: [u8; 32],
}
```

### 6.2 Storage Layout

```
/var/lib/saorsa/
├── shards/
│   ├── legacy/          # Legacy AES-256-GCM shards
│   ├── hybrid/          # Hybrid encrypted shards
│   └── quantum/         # Quantum-only shards
├── keys/
│   ├── classical/       # Classical keys
│   ├── quantum/         # Post-quantum key pairs
│   └── migration/       # Migration key mappings
├── migration/
│   ├── progress/        # Migration progress tracking
│   ├── logs/           # Migration logs
│   └── verification/   # Post-migration verification
└── metadata/
    ├── encryption/     # Encryption metadata
    └── convergence/    # Convergence secret management
```

## 7. Security Architecture

### 7.1 Post-Quantum Key Management

```rust
/// Post-quantum key management
pub struct QuantumKeyManager {
    key_store: SecureKeyStore,
    rng: Box<dyn CryptoRng + RngCore>,
}

impl QuantumKeyManager {
    /// Generate new post-quantum key pair
    pub fn generate_keypair(&mut self, security_level: SecurityLevel) -> Result<(PublicKey, PrivateKey)> {
        match security_level {
            SecurityLevel::Level1 { .. } => {
                let mut rng = &mut self.rng;
                let (pk, sk) = MlKem512::keygen(&mut rng)?;
                Ok((PublicKey::Level1(pk), PrivateKey::Level1(sk)))
            }
            SecurityLevel::Level3 { .. } => {
                let mut rng = &mut self.rng;
                let (pk, sk) = MlKem768::keygen(&mut rng)?;
                Ok((PublicKey::Level3(pk), PrivateKey::Level3(sk)))
            }
            SecurityLevel::Level5 { .. } => {
                let mut rng = &mut self.rng;
                let (pk, sk) = MlKem1024::keygen(&mut rng)?;
                Ok((PublicKey::Level5(pk), PrivateKey::Level5(sk)))
            }
        }
    }
    
    /// Derive convergent post-quantum keys
    pub fn derive_convergent_keypair(
        &self, 
        content: &[u8], 
        secret: Option<&ConvergenceSecret>
    ) -> Result<(PublicKey, PrivateKey)> {
        // Use quantum-safe key derivation for convergent encryption
        let seed = self.derive_quantum_safe_seed(content, secret)?;
        let mut deterministic_rng = DeterministicRng::from_seed(seed);
        
        // Generate deterministic key pair for convergent encryption
        match self.security_level {
            SecurityLevel::Level1 { .. } => {
                let (pk, sk) = MlKem512::keygen(&mut deterministic_rng)?;
                Ok((PublicKey::Level1(pk), PrivateKey::Level1(sk)))
            }
            _ => todo!("Other security levels")
        }
    }
}
```

### 7.2 Security Measures

#### Encryption Security
- **Post-Quantum Resistance**: ML-KEM provides security against quantum attacks
- **Hybrid Security**: Classical + post-quantum provides defense in depth
- **Forward Secrecy**: New key pairs for each encryption operation
- **Key Separation**: Separate keys for encryption and authentication

#### Convergent Encryption Security
```rust
/// Secure convergent encryption with post-quantum protection
impl QuantumCryptoEngine {
    /// Encrypt convergent with deterministic post-quantum key derivation
    pub fn encrypt_convergent_secure(
        &mut self, 
        data: &[u8], 
        secret: Option<&ConvergenceSecret>
    ) -> Result<ConvergentPacket> {
        // 1. Derive deterministic seed from content + secret
        let seed = self.derive_convergent_seed(data, secret)?;
        
        // 2. Generate deterministic post-quantum key pair
        let (pk, sk) = self.key_manager.derive_convergent_keypair_from_seed(seed)?;
        
        // 3. Use standard post-quantum encryption
        self.encrypt_with_keypair(data, &pk, &sk)
    }
    
    fn derive_convergent_seed(&self, data: &[u8], secret: Option<&ConvergenceSecret>) -> Result<[u8; 32]> {
        let mut kdf = HkdfSha256::new(None, data);
        
        if let Some(secret) = secret {
            kdf = HkdfSha256::new(Some(secret.as_bytes()), data);
        }
        
        let mut seed = [0u8; 32];
        kdf.expand(b"saorsa-pqc:convergent:seed:v1", &mut seed)
           .map_err(|_| CryptoError::KeyDerivationFailed)?;
           
        Ok(seed)
    }
}
```

## 8. Performance Considerations

### 8.1 Performance Targets

| Operation | Current (AES-256-GCM) | Target (Hybrid PQC) | Acceptable Range |
|-----------|----------------------|---------------------|------------------|
| Key Generation | ~0.1ms | ~2-5ms | < 10ms |
| Encryption (1MB) | ~1ms | ~5-10ms | < 20ms |
| Decryption (1MB) | ~1ms | ~5-10ms | < 20ms |
| Key Encapsulation | N/A | ~0.5-2ms | < 5ms |
| Key Decapsulation | N/A | ~0.5-2ms | < 5ms |

### 8.2 Optimization Strategies

#### Parallel Processing
```rust
/// Parallel post-quantum encryption for large data
impl QuantumCryptoEngine {
    pub async fn encrypt_parallel(
        &mut self, 
        data: &[u8], 
        chunk_size: usize,
        parallelism: usize
    ) -> Result<Vec<EncryptedChunk>> {
        let chunks: Vec<&[u8]> = data.chunks(chunk_size).collect();
        let mut handles = Vec::new();
        
        for chunk in chunks {
            let engine = self.clone();
            let chunk = chunk.to_vec();
            
            let handle = tokio::spawn(async move {
                engine.encrypt(&chunk)
            });
            
            handles.push(handle);
            
            // Limit concurrent operations
            if handles.len() >= parallelism {
                // Wait for some to complete
                let (result, _idx, remaining) = futures::future::select_all(handles).await;
                handles = remaining;
                
                result??;
            }
        }
        
        // Wait for remaining operations
        for handle in handles {
            handle.await??;
        }
        
        Ok(encrypted_chunks)
    }
}
```

#### Caching Strategy
```rust
/// Performance caching for post-quantum operations
pub struct QuantumCryptoCache {
    key_pairs: LruCache<[u8; 32], (PublicKey, PrivateKey)>,
    encapsulations: LruCache<[u8; 32], Vec<u8>>,
    convergent_keys: LruCache<[u8; 32], ConvergentKey>,
}

impl QuantumCryptoCache {
    /// Cache convergent key derivation results
    pub fn get_or_derive_convergent_key(
        &mut self, 
        content_hash: [u8; 32], 
        secret: Option<&ConvergenceSecret>
    ) -> Result<ConvergentKey> {
        if let Some(key) = self.convergent_keys.get(&content_hash) {
            return Ok(key.clone());
        }
        
        let key = self.engine.derive_convergent_key_uncached(content_hash, secret)?;
        self.convergent_keys.put(content_hash, key.clone());
        
        Ok(key)
    }
}
```

## 9. Testing Strategy

### 9.1 Test Categories

#### Unit Tests
```rust
#[cfg(test)]
mod quantum_crypto_tests {
    use super::*;
    
    #[test]
    fn test_quantum_encryption_roundtrip() {
        let mut engine = QuantumCryptoEngine::new(QuantumCryptoConfig::default()).unwrap();
        let data = b"Test data for quantum encryption";
        
        // Generate key pair
        let (pk, sk) = engine.key_manager.generate_keypair(SecurityLevel::Level1).unwrap();
        
        // Encrypt
        let encrypted = engine.encrypt_hybrid(data, &pk).unwrap();
        
        // Decrypt
        let decrypted = engine.decrypt_hybrid(&encrypted, &sk).unwrap();
        
        assert_eq!(data, decrypted.as_slice());
    }
    
    #[test]
    fn test_convergent_determinism() {
        let engine = QuantumCryptoEngine::new(QuantumCryptoConfig::default()).unwrap();
        let data = b"Convergent test data";
        
        // Encrypt same data twice
        let encrypted1 = engine.encrypt_convergent(data, None).unwrap();
        let encrypted2 = engine.encrypt_convergent(data, None).unwrap();
        
        // Should produce identical ciphertexts for convergent encryption
        assert_eq!(encrypted1.key_fingerprint, encrypted2.key_fingerprint);
        assert_eq!(encrypted1.nonce, encrypted2.nonce);
    }
    
    #[test]
    fn test_security_level_compatibility() {
        // Test that different security levels can decrypt each other's data
        // when using compatible algorithms
    }
    
    #[test]
    fn test_migration_compatibility() {
        // Test that migrated data can be decrypted correctly
        let legacy_engine = LegacyCryptoEngine::new();
        let quantum_engine = QuantumCryptoEngine::new(QuantumCryptoConfig::default()).unwrap();
        let migrator = DataMigrator::new(legacy_engine, quantum_engine);
        
        let data = b"Test migration data";
        
        // Encrypt with legacy
        let legacy_encrypted = migrator.legacy_engine.encrypt(data).unwrap();
        
        // Migrate
        let quantum_encrypted = migrator.migrate_encrypted_data(&legacy_encrypted).unwrap();
        
        // Decrypt with quantum
        let decrypted = migrator.quantum_engine.decrypt(&quantum_encrypted).unwrap();
        
        assert_eq!(data, decrypted.as_slice());
    }
}
```

#### Integration Tests
```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_full_storage_pipeline() {
        let config = Config::default()
            .with_encryption_mode(EncryptionMode::Convergent)
            .with_quantum_security(SecurityLevel::Level1);
            
        let mut pipeline = StoragePipeline::new(config).await.unwrap();
        
        let data = vec![0u8; 1024 * 1024]; // 1MB test data
        
        // Store with quantum encryption
        let meta = pipeline.store(&data).await.unwrap();
        
        // Retrieve and verify
        let retrieved = pipeline.retrieve(&meta.cid).await.unwrap();
        
        assert_eq!(data, retrieved);
    }
    
    #[tokio::test]
    async fn test_migration_workflow() {
        // Test complete migration from legacy to quantum
        let storage = MemoryStorage::new();
        
        // Store data with legacy encryption
        let legacy_config = Config::legacy();
        let mut legacy_pipeline = StoragePipeline::new(legacy_config).await.unwrap();
        let data = b"Legacy encryption test data".to_vec();
        let legacy_meta = legacy_pipeline.store(&data).await.unwrap();
        
        // Migrate to quantum
        let quantum_config = Config::quantum();
        let migrator = DataMigrator::new(legacy_config, quantum_config).await.unwrap();
        let migration_report = migrator.migrate_storage(&storage).await.unwrap();
        
        assert_eq!(migration_report.migrated_count, 1);
        assert_eq!(migration_report.errors.len(), 0);
        
        // Verify with quantum pipeline
        let quantum_pipeline = StoragePipeline::new(quantum_config).await.unwrap();
        let retrieved = quantum_pipeline.retrieve(&legacy_meta.cid).await.unwrap();
        
        assert_eq!(data, retrieved);
    }
}
```

#### Property-Based Tests
```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn test_quantum_encryption_roundtrip_property(
            data in prop::collection::vec(any::<u8>(), 0..10000),
            security_level in prop_oneof![
                Just(SecurityLevel::Level1),
                Just(SecurityLevel::Level3),
                Just(SecurityLevel::Level5),
            ]
        ) {
            let mut engine = QuantumCryptoEngine::new(
                QuantumCryptoConfig { security_level, ..Default::default() }
            ).unwrap();
            
            let (pk, sk) = engine.key_manager.generate_keypair(security_level).unwrap();
            let encrypted = engine.encrypt_hybrid(&data, &pk).unwrap();
            let decrypted = engine.decrypt_hybrid(&encrypted, &sk).unwrap();
            
            prop_assert_eq!(data, decrypted);
        }
        
        #[test]
        fn test_convergent_determinism_property(
            data in prop::collection::vec(any::<u8>(), 1..10000),
        ) {
            let engine = QuantumCryptoEngine::new(QuantumCryptoConfig::default()).unwrap();
            
            let encrypted1 = engine.encrypt_convergent(&data, None).unwrap();
            let encrypted2 = engine.encrypt_convergent(&data, None).unwrap();
            
            prop_assert_eq!(encrypted1.key_fingerprint, encrypted2.key_fingerprint);
            prop_assert_eq!(encrypted1.nonce, encrypted2.nonce);
        }
    }
}
```

### 9.2 Performance Benchmarks

```rust
#[cfg(test)]
mod benchmarks {
    use super::*;
    use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
    
    fn bench_quantum_encryption(c: &mut Criterion) {
        let mut group = c.benchmark_group("quantum_encryption");
        
        for size in [1024, 64*1024, 1024*1024, 10*1024*1024].iter() {
            group.bench_with_input(
                BenchmarkId::new("encrypt", size),
                size,
                |b, &size| {
                    let mut engine = QuantumCryptoEngine::new(QuantumCryptoConfig::default()).unwrap();
                    let data = vec![0u8; size];
                    let (pk, _) = engine.key_manager.generate_keypair(SecurityLevel::Level1).unwrap();
                    
                    b.iter(|| {
                        engine.encrypt_hybrid(&data, &pk).unwrap()
                    });
                }
            );
        }
        
        group.finish();
    }
    
    fn bench_convergent_encryption(c: &mut Criterion) {
        let mut group = c.benchmark_group("convergent_encryption");
        
        for size in [1024, 64*1024, 1024*1024].iter() {
            group.bench_with_input(
                BenchmarkId::new("convergent", size),
                size,
                |b, &size| {
                    let engine = QuantumCryptoEngine::new(QuantumCryptoConfig::default()).unwrap();
                    let data = vec![0u8; size];
                    
                    b.iter(|| {
                        engine.encrypt_convergent(&data, None).unwrap()
                    });
                }
            );
        }
        
        group.finish();
    }
    
    criterion_group!(benches, bench_quantum_encryption, bench_convergent_encryption);
    criterion_main!(benches);
}
```

## 10. Risk Assessment & Mitigation

### 10.1 Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| **Performance Degradation** | Medium | High | Parallel processing, caching, hybrid encryption |
| **Migration Data Loss** | Low | Critical | Extensive testing, rollback capabilities, verification |
| **Compatibility Issues** | Medium | Medium | Compatibility layer, gradual migration, extensive testing |
| **Key Management Complexity** | High | Medium | Automated key management, clear documentation |
| **Increased Storage Requirements** | High | Low | Compression, efficient encoding, storage optimization |

### 10.2 Security Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| **Quantum Algorithm Vulnerabilities** | Low | High | Multiple algorithm support, hybrid approach |
| **Implementation Flaws** | Medium | Critical | Security audits, formal verification, extensive testing |
| **Side-Channel Attacks** | Medium | Medium | Constant-time operations, secure memory handling |
| **Key Compromise** | Low | Critical | Key rotation, forward secrecy, secure key storage |

### 10.3 Operational Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| **Migration Downtime** | Low | High | Rolling updates, compatibility layer |
| **Increased Complexity** | High | Medium | Clear documentation, training, tooling |
| **Support Burden** | Medium | Medium | Automated migration tools, monitoring |

## 11. Implementation Timeline

### 11.1 Development Schedule

#### Phase 1: Foundation (8 weeks)
- **Week 1-2**: Add saorsa-pqc dependency, basic integration
- **Week 3-4**: Implement quantum crypto engine
- **Week 5-6**: Hybrid encryption implementation
- **Week 7-8**: Unit tests and compatibility layer

#### Phase 2: Migration Tools (6 weeks)
- **Week 1-2**: Data migration utilities
- **Week 3-4**: Migration progress tracking and rollback
- **Week 5-6**: Integration testing and documentation

#### Phase 3: Production Deployment (4 weeks)  
- **Week 1-2**: Performance optimization and caching
- **Week 3-4**: Final testing and production release

### 11.2 Milestones

- [ ] **M1**: Quantum crypto engine implementation complete
- [ ] **M2**: Hybrid encryption working end-to-end
- [ ] **M3**: Migration tools functional
- [ ] **M4**: Performance benchmarks meet targets
- [ ] **M5**: Security audit complete
- [ ] **M6**: Production deployment successful

## 12. Success Metrics

### 12.1 Technical Metrics
- **Zero data loss** during migration
- **Performance within 2x** of current implementation
- **100% test coverage** for crypto modules
- **Zero critical security vulnerabilities**

### 12.2 Operational Metrics
- **Migration completion rate** > 99%
- **Rollback success rate** 100% when needed
- **Support ticket volume** < 5% increase
- **User satisfaction** maintained

## 13. Conclusion

This migration to saorsa-pqc will provide quantum-resistant encryption while maintaining the performance and functionality of the current system. The phased approach minimizes risk while ensuring a smooth transition to post-quantum cryptography.

The hybrid encryption strategy provides both immediate quantum resistance and a migration path that preserves existing data and user workflows. With proper testing, monitoring, and rollback capabilities, this migration will future-proof the saorsa-fec encryption system against quantum threats.

## 14. Appendices

### A. Glossary
- **ML-KEM**: Module Lattice Key Encapsulation Mechanism (NIST standardized)
- **ML-DSA**: Module Lattice Digital Signature Algorithm (NIST standardized)
- **KEM**: Key Encapsulation Mechanism
- **PQC**: Post-Quantum Cryptography
- **NIST**: National Institute of Standards and Technology

### B. References
- [NIST Post-Quantum Cryptography Standards](https://csrc.nist.gov/Projects/post-quantum-cryptography)
- [saorsa-pqc Documentation](https://docs.rs/saorsa-pqc)
- [Hybrid Encryption Best Practices](https://tools.ietf.org/html/draft-ietf-tls-hybrid-design)

### C. Migration Checklist
- [ ] Backup all existing encrypted data
- [ ] Test migration tools on non-production data
- [ ] Verify performance benchmarks
- [ ] Complete security audit
- [ ] Train support team on new system
- [ ] Prepare rollback procedures
- [ ] Monitor migration progress
- [ ] Verify data integrity post-migration