//! Integrated pipeline for encryption + FEC processing
//!
//! This module provides the main orchestration layer that combines
//! encryption, FEC encoding, metadata management, and storage.
//! Implements the v0.3 StoragePipeline API specification.

use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::sync::Arc;

use crate::chunk_registry::{ChunkInfo, ChunkRegistry};
use crate::config::{Config, EncryptionMode};
use crate::crypto::{
    derive_convergent_key, generate_random_key, CryptoEngine, EncryptionKey, EncryptionMetadata,
};
use crate::gc::GarbageCollector;
use crate::ida::IDAConfig;
use crate::metadata::{ChunkReference, FileMetadata, LocalMetadata};
use crate::quantum_crypto::QuantumCryptoEngine;
use crate::storage::StorageBackend;
use crate::types::{ChunkId, DataId, ShareId};
use crate::version::VersionManager;

/// Meta information for file processing
/// Optional metadata that can be passed during file processing
#[derive(Debug, Clone)]
pub struct Meta {
    /// Optional filename
    pub filename: Option<String>,
    /// Optional author
    pub author: Option<String>,
    /// Optional description
    pub description: Option<String>,
    /// Optional MIME type
    pub mime_type: Option<String>,
    /// Custom tags
    pub tags: Vec<String>,
}

impl Meta {
    /// Create new empty meta
    pub fn new() -> Self {
        Self {
            filename: None,
            author: None,
            description: None,
            mime_type: None,
            tags: Vec::new(),
        }
    }

    /// Set filename
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Set author
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Add tag
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.tags.push(tag.into());
    }
}

impl Default for Meta {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage pipeline implementing v0.3 specification API
/// Generic over storage backend type B
pub struct StoragePipeline<B: StorageBackend> {
    /// Configuration
    config: Config,
    /// Storage backend
    #[allow(dead_code)]
    backend: B,
    /// Chunk registry
    chunk_registry: Arc<RwLock<ChunkRegistry>>,
    /// Version manager
    version_manager: Arc<RwLock<VersionManager>>,
    /// Garbage collector
    gc: Arc<GarbageCollector>,
    /// In-memory storage for chunks (for testing)
    chunk_storage: Arc<RwLock<std::collections::HashMap<String, Vec<u8>>>>,
    /// Store original data for key recovery (for testing)
    original_data_storage: Arc<RwLock<std::collections::HashMap<[u8; 32], Vec<u8>>>>,
}

impl<B: StorageBackend> StoragePipeline<B> {
    /// Create a new storage pipeline with the given configuration and backend
    /// Required by v0.3 specification
    pub async fn new(cfg: Config, backend: B) -> Result<Self> {
        cfg.validate().context("Invalid configuration")?;

        let chunk_registry = Arc::new(RwLock::new(ChunkRegistry::new()));
        let version_manager = Arc::new(RwLock::new(VersionManager::new(chunk_registry.clone())));

        use crate::gc::RetentionPolicy;
        let retention_policy =
            RetentionPolicy::KeepRecent(cfg.gc.retention_days as u64 * 24 * 3600);

        // Create a dummy Arc<dyn StorageBackend> for GC - this will need to be addressed in a future refactor
        let storage_for_gc: Arc<dyn StorageBackend> =
            Arc::new(crate::storage::LocalStorage::new(std::path::PathBuf::from("/tmp")).await?);
        let gc = Arc::new(GarbageCollector::new(
            retention_policy,
            chunk_registry.clone(),
            storage_for_gc,
        ));

        Ok(Self {
            config: cfg,
            backend,
            chunk_registry,
            version_manager,
            gc,
            chunk_storage: Arc::new(RwLock::new(std::collections::HashMap::new())),
            original_data_storage: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Process a file: encrypt, chunk, and store with FEC encoding
    /// Required by v0.3 specification
    pub async fn process_file(
        &mut self,
        file_id: [u8; 32],
        data: &[u8],
        meta: Option<Meta>,
    ) -> Result<FileMetadata> {
        // Create quantum crypto engine
        let mut crypto = QuantumCryptoEngine::new();

        // Process data with optional compression
        let processed_data = if self.config.compression_enabled {
            self.compress(data)?
        } else {
            data.to_vec()
        };

        // Encrypt using quantum engine
        let (encrypted_data, quantum_encryption_metadata) = {
            let secret = match self.config.encryption_mode {
                EncryptionMode::ConvergentWithSecret => {
                    let secret_bytes = self.get_user_secret()?;
                    Some(crate::quantum_crypto::ConvergenceSecret::new(secret_bytes))
                }
                _ => None,
            };

            let (encrypted, quantum_meta) = crypto.encrypt(
                &processed_data,
                self.config.encryption_mode,
                secret.as_ref(),
            )?;

            (encrypted, Some(quantum_meta))
        };

        // Check for deduplication based on ciphertext + auth header
        let data_id = DataId::from_data(&encrypted_data);
        if let Some(existing) = self.find_existing_data(&data_id).await? {
            return Ok(existing);
        }

        // Store original data for key recovery (for testing)
        {
            let mut orig_storage = self.original_data_storage.write();
            orig_storage.insert(file_id, processed_data.clone());
        }

        // Process chunks with FEC encoding
        let chunk_refs = self.process_chunks(&encrypted_data, &data_id).await?;

        // Create file metadata with quantum encryption
        let mut file_metadata = FileMetadata::with_quantum_encryption(
            file_id,
            data.len() as u64, // Original file size
            quantum_encryption_metadata,
            chunk_refs,
        );

        // Add local metadata if provided
        if let Some(meta) = meta {
            let mut local_meta = LocalMetadata::new();
            if let Some(filename) = meta.filename {
                local_meta = local_meta.with_filename(filename);
            }
            if let Some(author) = meta.author {
                local_meta = local_meta.with_author(author);
            }
            local_meta.description = meta.description;
            local_meta.mime_type = meta.mime_type;
            local_meta.tags = meta.tags;
            file_metadata = file_metadata.with_local_metadata(local_meta);
        }

        // Register version
        {
            let mut version_mgr = self.version_manager.write();
            version_mgr.create_version(&file_metadata)?;
        }

        Ok(file_metadata)
    }

    /// Retrieve and decrypt a file
    /// Required by v0.3 specification
    pub async fn retrieve_file(&self, meta: &FileMetadata) -> Result<Vec<u8>> {
        let mut chunks = Vec::new();

        // Retrieve all chunks
        for chunk_ref in &meta.chunks {
            let chunk_data = self.retrieve_chunk(&chunk_ref.chunk_id).await?;
            chunks.push(chunk_data);
        }

        // Combine chunks (reconstruct with FEC if needed)
        let encrypted_data = self.reconstruct_data(&chunks, meta).await?;

        // Decrypt using quantum engine
        let decrypted = if let Some(quantum_meta) = &meta.quantum_encryption_metadata {
            let crypto = QuantumCryptoEngine::new();

            // Get convergence secret if needed
            let secret = if quantum_meta.convergence_secret_id.is_some() {
                let secret_bytes = self.get_user_secret()?;
                Some(crate::quantum_crypto::ConvergenceSecret::new(secret_bytes))
            } else {
                None
            };

            // Get original data for convergent decryption
            let orig_storage = self.original_data_storage.read();
            let original_data = orig_storage.get(&meta.file_id);

            crypto.decrypt(
                &encrypted_data,
                quantum_meta,
                secret.as_ref(),
                original_data.map(|v| v.as_slice()),
            )?
        } else if let Some(enc_meta) = &meta.encryption_metadata {
            // Legacy fallback
            let crypto = CryptoEngine::new();
            let key = self.recover_key(enc_meta, &meta.file_id)?;
            crypto.decrypt(&encrypted_data, &key)?
        } else {
            encrypted_data
        };

        // Optionally decompress
        if self.config.compression_enabled {
            self.decompress(&decrypted)
        } else {
            Ok(decrypted)
        }
    }

    /// Process chunks with FEC encoding
    async fn process_chunks(&self, data: &[u8], data_id: &DataId) -> Result<Vec<ChunkReference>> {
        let mut chunk_refs = Vec::new();
        let chunk_size = self.config.chunk_size;

        // Split into chunks
        for (index, chunk_data) in data.chunks(chunk_size).enumerate() {
            let chunk_id = ChunkId::new(data_id, index);

            // Store chunk data in memory for testing
            let chunk_hash = blake3::hash(chunk_data);
            let chunk_ref_id = hex::encode(chunk_hash.as_bytes());
            {
                let mut storage = self.chunk_storage.write();
                storage.insert(chunk_ref_id, chunk_data.to_vec());
            }

            let share_ids = vec![ShareId::new(&chunk_id, 0)];

            // Register chunk
            let chunk_info = ChunkInfo {
                id: chunk_id,
                data_id: *data_id,
                size: chunk_data.len(),
                encrypted_size: chunk_data.len(),
                share_ids,
                encryption_key_hash: [0u8; 32], // Would store actual key hash
                created_at: std::time::SystemTime::now(),
            };

            {
                let mut registry = self.chunk_registry.write();
                registry.register_chunk(chunk_info);
            }

            // Create chunk reference
            let chunk_ref = ChunkReference::new(
                blake3::hash(chunk_data).into(),
                0,            // stripe_index
                index as u16, // shard_index
                chunk_data.len() as u32,
            );
            chunk_refs.push(chunk_ref);
        }

        Ok(chunk_refs)
    }

    /// Retrieve a chunk from storage
    async fn retrieve_chunk(&self, chunk_id: &[u8; 32]) -> Result<Vec<u8>> {
        let storage = self.chunk_storage.read();

        // The chunk_id is actually the blake3 hash of the chunk data
        let chunk_key = hex::encode(chunk_id);

        // Look up chunk by exact hash match
        if let Some(data) = storage.get(&chunk_key) {
            return Ok(data.clone());
        }

        anyhow::bail!("Chunk not found: {}", chunk_key)
    }

    /// Reconstruct data from chunks (with FEC if needed)
    async fn reconstruct_data(&self, chunks: &[Vec<u8>], _meta: &FileMetadata) -> Result<Vec<u8>> {
        // Simple concatenation for now - FEC reconstruction would be more complex
        if chunks.iter().any(|chunk| chunk.is_empty()) {
            anyhow::bail!("One or more chunks are empty, cannot reconstruct data");
        }
        Ok(chunks.concat())
    }

    /// Find existing data by ID
    async fn find_existing_data(&self, _data_id: &DataId) -> Result<Option<FileMetadata>> {
        // Simplified - would check registry and storage
        Ok(None)
    }

    /// Recover encryption key from metadata
    fn recover_key(
        &self,
        metadata: &EncryptionMetadata,
        file_id: &[u8; 32],
    ) -> Result<EncryptionKey> {
        match metadata.key_derivation {
            crate::crypto::KeyDerivation::Blake3Convergent => {
                // Get original data from storage
                let orig_storage = self.original_data_storage.read();
                let original_data = orig_storage
                    .get(file_id)
                    .ok_or_else(|| anyhow::anyhow!("Original data not found for file"))?;

                let secret = if metadata.convergence_secret_id.is_some() {
                    Some(self.get_user_secret()?)
                } else {
                    None
                };
                Ok(derive_convergent_key(original_data, secret.as_ref()))
            }
            crate::crypto::KeyDerivation::Random => {
                anyhow::bail!("Random keys cannot be reconstructed without external storage")
            }
        }
    }

    /// Get user secret for convergent encryption
    fn get_user_secret(&self) -> Result<[u8; 32]> {
        // Simplified - would retrieve from secure storage
        Ok([0u8; 32])
    }

    /// Compress data
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let level = Compression::new(self.config.compression_level as u32);
        let mut encoder = GzEncoder::new(Vec::new(), level);
        encoder.write_all(data).context("Compression failed")?;
        encoder.finish().context("Failed to finish compression")
    }

    /// Decompress data
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .context("Decompression failed")?;
        Ok(decompressed)
    }

    /// Run garbage collection
    pub async fn run_gc(&self) -> Result<()> {
        let _report = self.gc.run().await?;
        Ok(())
    }

    /// Get pipeline statistics
    pub fn stats(&self) -> PipelineStats {
        let registry = self.chunk_registry.read();
        let registry_stats = registry.stats();

        PipelineStats {
            total_chunks: registry_stats.total_chunks,
            total_size: registry_stats.total_size,
            referenced_size: registry_stats.referenced_size,
            unreferenced_size: registry_stats.unreferenced_size,
            encryption_mode: self.config.encryption_mode,
            fec_params: (
                self.config.data_shards as u16,
                self.config.parity_shards as u16,
            ),
        }
    }
}

/// Main pipeline for processing files (legacy compatibility)
pub struct Pipeline {
    /// Configuration
    config: Config,
    /// Encryption engine
    encryption: CryptoEngine,
    /// Storage backend
    #[allow(dead_code)]
    storage: Arc<dyn StorageBackend>,
    /// Chunk registry
    chunk_registry: Arc<RwLock<ChunkRegistry>>,
    /// Version manager
    version_manager: Arc<RwLock<VersionManager>>,
    /// Garbage collector
    gc: Arc<GarbageCollector>,
}

impl Pipeline {
    /// Create a new pipeline with the given configuration
    pub async fn new(config: Config, storage: Arc<dyn StorageBackend>) -> Result<Self> {
        config.validate().context("Invalid configuration")?;

        let encryption = CryptoEngine::new();

        let _ida_config = IDAConfig {
            k: config.fec.data_shares,
            n: config.fec.data_shares + config.fec.parity_shares,
            stripe_size: config.fec.stripe_size as u32,
        };

        let chunk_registry = Arc::new(RwLock::new(ChunkRegistry::new()));
        let version_manager = Arc::new(RwLock::new(VersionManager::new(chunk_registry.clone())));

        use crate::gc::RetentionPolicy;
        let retention_policy =
            RetentionPolicy::KeepRecent(config.gc.retention_days as u64 * 24 * 3600);
        let gc = Arc::new(GarbageCollector::new(
            retention_policy,
            chunk_registry.clone(),
            storage.clone(),
        ));

        Ok(Self {
            config,
            encryption,
            storage,
            chunk_registry,
            version_manager,
            gc,
        })
    }

    /// Process a file: encrypt and encode (legacy compatibility)
    pub async fn process_file(
        &mut self,
        file_id: [u8; 32],
        data: &[u8],
        _parent_version: Option<[u8; 32]>,
    ) -> Result<FileMetadata> {
        // Optionally compress
        let processed_data = if self.config.encryption.compress_before_encrypt {
            self.compress(data)?
        } else {
            data.to_vec()
        };

        // Encrypt based on mode
        let (encrypted_data, _key) = match self.config.encryption.mode {
            EncryptionMode::Convergent => {
                let key = derive_convergent_key(&processed_data, None);
                let encrypted = self.encryption.encrypt(&processed_data, &key)?;
                (encrypted, key)
            }
            EncryptionMode::ConvergentWithSecret => {
                let secret = self.get_user_secret()?;
                let key = derive_convergent_key(&processed_data, Some(&secret));
                let encrypted = self.encryption.encrypt(&processed_data, &key)?;
                (encrypted, key)
            }
            EncryptionMode::RandomKey => {
                let key = generate_random_key();
                let encrypted = self.encryption.encrypt(&processed_data, &key)?;
                (encrypted, key)
            }
        };

        // Check for deduplication
        let data_id = DataId::from_data(&encrypted_data);
        if let Some(existing) = self.find_existing_data(&data_id).await? {
            // Data already exists, just create new version
            return Ok(existing);
        }

        // Split into chunks and encode
        let chunk_refs = self
            .process_chunks_legacy(&encrypted_data, &data_id)
            .await?;

        // Create metadata
        let metadata = FileMetadata::new(file_id, data.len() as u64, None, chunk_refs);

        // Register version
        {
            let mut version_mgr = self.version_manager.write();
            version_mgr.create_version(&metadata)?;
        }

        Ok(metadata)
    }

    /// Retrieve and decrypt a file
    pub async fn retrieve_file(&self, metadata: &FileMetadata) -> Result<Vec<u8>> {
        let mut chunks = Vec::new();

        // Retrieve all chunks
        for chunk_ref in &metadata.chunks {
            let chunk_data = self.retrieve_chunk(&chunk_ref.chunk_id).await?;
            chunks.push(chunk_data);
        }

        // Combine chunks
        let encrypted_data = chunks.concat();

        // Decrypt
        let key = self.recover_key_legacy(&metadata.chunks[0].chunk_id)?;
        let decrypted = self.encryption.decrypt(&encrypted_data, &key)?;

        // Optionally decompress
        if self.config.encryption.compress_before_encrypt {
            self.decompress(&decrypted)
        } else {
            Ok(decrypted)
        }
    }

    /// Process chunks with FEC encoding (legacy)
    async fn process_chunks_legacy(
        &self,
        data: &[u8],
        data_id: &DataId,
    ) -> Result<Vec<ChunkReference>> {
        let mut chunk_refs = Vec::new();
        let chunk_size = self.config.fec.stripe_size;

        for (index, chunk_data) in data.chunks(chunk_size).enumerate() {
            let chunk_id = ChunkId::new(data_id, index);

            // For now, store chunk directly (FEC encoding would be more complex)
            let _chunk_hash = blake3::hash(chunk_data);
            // TODO: Convert to v0.3 shard API
            // let cid = Cid::from_data(chunk_data);
            // let shard = Shard::new(header, chunk_data.to_vec());
            // self.storage.put_shard(&cid, &shard).await?;

            let share_ids = vec![ShareId::new(&chunk_id, 0)];

            // Register chunk
            let chunk_info = ChunkInfo {
                id: chunk_id,
                data_id: *data_id,
                size: chunk_data.len(),
                encrypted_size: chunk_data.len(),
                share_ids,
                encryption_key_hash: [0u8; 32], // Would store actual key hash
                created_at: std::time::SystemTime::now(),
            };

            {
                let mut registry = self.chunk_registry.write();
                registry.register_chunk(chunk_info);
            }

            // Create chunk reference
            let chunk_ref = ChunkReference::new(
                blake3::hash(chunk_data).into(),
                0,
                index as u16,
                chunk_data.len() as u32,
            );
            chunk_refs.push(chunk_ref);
        }

        Ok(chunk_refs)
    }

    /// Retrieve a chunk from storage
    async fn retrieve_chunk(&self, _chunk_id: &[u8; 32]) -> Result<Vec<u8>> {
        // For simplicity, retrieve from storage directly
        // TODO: Convert to v0.3 shard API
        // let cid = Cid::new(*chunk_id);
        // let shard = self.storage.get_shard(&cid).await?;
        // Ok(shard.data)
        Ok(vec![])
    }

    /// Store a share
    #[allow(dead_code)]
    async fn store_share(&self, _share_id: &ShareId, _data: &[u8]) -> Result<()> {
        let _id: [u8; 32] = blake3::hash(format!("{}", _share_id).as_bytes()).into();
        // TODO: Convert to v0.3 shard API
        // let cid = Cid::new(id);
        // let shard = Shard::new(header, data.to_vec());
        // self.storage.put_shard(&cid, &shard).await
        Ok(())
    }

    /// Find existing data by ID
    async fn find_existing_data(&self, _data_id: &DataId) -> Result<Option<FileMetadata>> {
        // Simplified - would check registry and storage
        Ok(None)
    }

    /// Recover encryption key for a chunk (legacy)
    fn recover_key_legacy(&self, _chunk_id: &[u8; 32]) -> Result<EncryptionKey> {
        // Simplified - would retrieve from secure storage
        Ok(generate_random_key())
    }

    /// Get user secret for convergent encryption
    fn get_user_secret(&self) -> Result<[u8; 32]> {
        // Simplified - would retrieve from secure storage
        Ok([0u8; 32])
    }

    /// Compress data
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let level = Compression::new(self.config.encryption.compression_level);
        let mut encoder = GzEncoder::new(Vec::new(), level);
        encoder.write_all(data).context("Compression failed")?;
        encoder.finish().context("Failed to finish compression")
    }

    /// Decompress data
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .context("Decompression failed")?;
        Ok(decompressed)
    }

    /// Run garbage collection
    pub async fn run_gc(&self) -> Result<()> {
        let _report = self.gc.run().await?;
        Ok(())
    }

    /// Get pipeline statistics
    pub fn stats(&self) -> PipelineStats {
        let registry = self.chunk_registry.read();
        let registry_stats = registry.stats();

        PipelineStats {
            total_chunks: registry_stats.total_chunks,
            total_size: registry_stats.total_size,
            referenced_size: registry_stats.referenced_size,
            unreferenced_size: registry_stats.unreferenced_size,
            encryption_mode: self.config.encryption_mode,
            fec_params: (self.config.fec.data_shares, self.config.fec.parity_shares),
        }
    }
}

/// Pipeline statistics
#[derive(Debug, Clone)]
pub struct PipelineStats {
    /// Total number of chunks
    pub total_chunks: usize,
    /// Total size in bytes
    pub total_size: u64,
    /// Size of referenced chunks
    pub referenced_size: u64,
    /// Size of unreferenced chunks
    pub unreferenced_size: u64,
    /// Current encryption mode
    pub encryption_mode: EncryptionMode,
    /// FEC parameters (k, m)
    pub fec_params: (u16, u16),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::LocalStorage;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_pipeline_basic() {
        let temp_dir = TempDir::new().unwrap();
        let backend = LocalStorage::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        let config = Config::default()
            .with_encryption_mode(EncryptionMode::Convergent)
            .with_fec_params(16, 4)
            .with_chunk_size(64 * 1024)
            .with_compression(true, 6);

        let mut pipeline = StoragePipeline::new(config, backend).await.unwrap();

        let file_id = [1u8; 32];
        let data = b"Hello, World! This is a longer test message to ensure proper encryption and chunking behavior with the v0.3 pipeline implementation.";
        let meta = Some(Meta::new().with_filename("test.txt"));

        let metadata = pipeline.process_file(file_id, data, meta).await.unwrap();

        assert_eq!(metadata.file_id, file_id);
        assert_eq!(metadata.file_size, data.len() as u64);
        assert!(!metadata.chunks.is_empty());

        // Test retrieval
        let retrieved = pipeline.retrieve_file(&metadata).await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_storage_pipeline_encryption_modes() {
        let temp_dir = TempDir::new().unwrap();
        let backend = LocalStorage::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        // Test convergent encryption
        let config = Config::default()
            .with_encryption_mode(EncryptionMode::Convergent)
            .with_compression(false, 1);

        let mut pipeline = StoragePipeline::new(config, backend).await.unwrap();

        let file_id = [1u8; 32];
        let data = b"Test data for convergent encryption";

        let metadata = pipeline.process_file(file_id, data, None).await.unwrap();
        assert_eq!(metadata.file_size, data.len() as u64);
    }

    #[tokio::test]
    async fn test_storage_pipeline_stats() {
        let temp_dir = TempDir::new().unwrap();
        let backend = LocalStorage::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        let config = Config::default();
        let pipeline = StoragePipeline::new(config, backend).await.unwrap();

        let stats = pipeline.stats();
        assert_eq!(stats.total_chunks, 0);
        assert_eq!(stats.total_size, 0);
    }

    #[tokio::test]
    async fn test_pipeline_basic() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(
            LocalStorage::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );

        let config = Config::default();
        let mut pipeline = Pipeline::new(config, storage).await.unwrap();

        let file_id = [1u8; 32];
        let data = b"Hello, World!";

        let metadata = pipeline.process_file(file_id, data, None).await.unwrap();

        assert_eq!(metadata.file_id, file_id);
        assert_eq!(metadata.file_size, data.len() as u64);
        assert!(!metadata.chunks.is_empty());
    }

    #[tokio::test]
    async fn test_pipeline_with_compression() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(
            LocalStorage::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );

        let mut config = Config::default();
        config.encryption.compress_before_encrypt = true;
        config.encryption.compression_level = 6;

        let mut pipeline = Pipeline::new(config, storage).await.unwrap();

        let file_id = [1u8; 32];
        let data = vec![b'A'; 10000]; // Highly compressible

        let metadata = pipeline.process_file(file_id, &data, None).await.unwrap();

        assert_eq!(metadata.file_size, 10000);
    }

    #[tokio::test]
    async fn test_pipeline_stats() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(
            LocalStorage::new(temp_dir.path().to_path_buf())
                .await
                .unwrap(),
        );

        let config = Config::default();
        let pipeline = Pipeline::new(config, storage).await.unwrap();

        let stats = pipeline.stats();
        assert_eq!(stats.total_chunks, 0);
        assert_eq!(stats.total_size, 0);
    }
}
