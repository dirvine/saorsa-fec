//! Integrated pipeline for encryption + FEC processing
//!
//! This module provides the main orchestration layer that combines
//! encryption, FEC encoding, metadata management, and storage.

use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::sync::Arc;

use crate::chunk_registry::{ChunkInfo, ChunkRegistry};
use crate::config::{Config, EncryptionMode};
use crate::crypto::{CryptoEngine, EncryptionKey};
use crate::gc::GarbageCollector;
use crate::ida::IDAConfig;
use crate::metadata::{ChunkReference, FileMetadata};
use crate::storage::StorageBackend;
use crate::types::{ChunkId, DataId, ShareId};
use crate::version::VersionManager;

/// Main pipeline for processing files
pub struct Pipeline {
    /// Configuration
    config: Config,
    /// Encryption engine
    encryption: CryptoEngine,
    /// Storage backend
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

    /// Process a file: encrypt and encode
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
        let (encrypted_data, key) = match self.config.encryption.mode {
            EncryptionMode::Convergent => {
                use crate::crypto::derive_convergent_key;
                let key = derive_convergent_key(&processed_data, None);
                let encrypted = self.encryption.encrypt(&processed_data, &key)?;
                (encrypted, key)
            }
            EncryptionMode::ConvergentWithSecret => {
                use crate::crypto::derive_convergent_key;
                let secret = self.get_user_secret()?;
                let key = derive_convergent_key(&processed_data, Some(&secret));
                let encrypted = self.encryption.encrypt(&processed_data, &key)?;
                (encrypted, key)
            }
            EncryptionMode::RandomKey => {
                let mut key_bytes = [0u8; 32];
                use rand::RngCore;
                rand::thread_rng().fill_bytes(&mut key_bytes);
                let key = EncryptionKey::new(key_bytes);
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
        let chunk_refs = self.process_chunks(&encrypted_data, &data_id, &key).await?;

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
        let key = self.recover_key(&metadata.chunks[0].chunk_id)?;
        let decrypted = self.encryption.decrypt(&encrypted_data, &key)?;

        // Optionally decompress
        if self.config.encryption.compress_before_encrypt {
            self.decompress(&decrypted)
        } else {
            Ok(decrypted)
        }
    }

    /// Process chunks with FEC encoding
    async fn process_chunks(
        &self,
        data: &[u8],
        data_id: &DataId,
        key: &EncryptionKey,
    ) -> Result<Vec<ChunkReference>> {
        let mut chunk_refs = Vec::new();
        let chunk_size = self.config.fec.stripe_size;

        for (index, chunk_data) in data.chunks(chunk_size).enumerate() {
            let chunk_id = ChunkId::new(data_id, index);

            // For now, store chunk directly (FEC encoding would be more complex)
            let chunk_hash = blake3::hash(chunk_data);
            self.storage
                .put_chunk(chunk_hash.as_bytes(), chunk_data)
                .await?;

            let share_ids = vec![ShareId::new(&chunk_id, 0)];

            // Register chunk
            let chunk_info = ChunkInfo {
                id: chunk_id,
                data_id: *data_id,
                size: chunk_data.len(),
                encrypted_size: chunk_data.len(),
                share_ids,
                encryption_key_hash: blake3::hash(key.as_bytes()).into(),
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
    async fn retrieve_chunk(&self, chunk_id: &[u8; 32]) -> Result<Vec<u8>> {
        // For simplicity, retrieve from storage directly
        self.storage.get_chunk(chunk_id).await
    }

    /// Store a share
    #[allow(dead_code)]
    async fn store_share(&self, share_id: &ShareId, data: &[u8]) -> Result<()> {
        let id = blake3::hash(format!("{}", share_id).as_bytes()).into();
        self.storage.put_chunk(&id, data).await
    }

    /// Find existing data by ID
    async fn find_existing_data(&self, _data_id: &DataId) -> Result<Option<FileMetadata>> {
        // Simplified - would check registry and storage
        Ok(None)
    }

    /// Recover encryption key for a chunk
    fn recover_key(&self, _chunk_id: &[u8; 32]) -> Result<EncryptionKey> {
        // Simplified - would retrieve from secure storage
        let mut key_bytes = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut key_bytes);
        Ok(EncryptionKey::new(key_bytes))
    }

    /// Get user secret for convergent encryption
    fn get_user_secret(&self) -> Result<[u8; 32]> {
        // Simplified - would retrieve from secure storage
        Ok([0u8; 32])
    }

    /// Compress data
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
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
            encryption_mode: self.config.encryption.mode,
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
