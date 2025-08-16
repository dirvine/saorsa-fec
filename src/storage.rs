//! Storage backend abstraction for chunk storage
//!
//! This module provides a trait for different storage implementations
//! (local filesystem, network, cloud) and concrete implementations.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Abstract storage backend interface
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store a chunk with the given ID
    async fn put_chunk(&self, id: &[u8; 32], data: &[u8]) -> Result<()>;

    /// Retrieve a chunk by ID
    async fn get_chunk(&self, id: &[u8; 32]) -> Result<Vec<u8>>;

    /// Delete a chunk by ID
    async fn delete_chunk(&self, id: &[u8; 32]) -> Result<()>;

    /// Check if a chunk exists
    async fn has_chunk(&self, id: &[u8; 32]) -> Result<bool>;

    /// List all chunk IDs in storage
    async fn list_chunks(&self) -> Result<Vec<[u8; 32]>>;
}

/// Local filesystem storage implementation
pub struct LocalStorage {
    /// Base directory for chunk storage
    base_path: PathBuf,
    /// Number of directory levels for sharding
    shard_levels: usize,
}

impl LocalStorage {
    /// Create a new local storage backend
    pub async fn new(base_path: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_path)
            .await
            .context("Failed to create storage directory")?;

        Ok(Self {
            base_path,
            shard_levels: 2, // Use 2 levels of sharding by default
        })
    }

    /// Get the path for a chunk
    fn chunk_path(&self, id: &[u8; 32]) -> PathBuf {
        let hex = hex::encode(id);

        // Create sharded path (e.g., ab/cd/abcdef...)
        let mut path = self.base_path.clone();

        for level in 0..self.shard_levels {
            if hex.len() > level * 2 + 2 {
                path = path.join(&hex[level * 2..level * 2 + 2]);
            }
        }

        path.join(format!("{}.chunk", hex))
    }

    /// Ensure parent directory exists
    async fn ensure_parent(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directory")?;
        }
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for LocalStorage {
    async fn put_chunk(&self, id: &[u8; 32], data: &[u8]) -> Result<()> {
        let path = self.chunk_path(id);

        // Ensure parent directory exists
        self.ensure_parent(&path).await?;

        // Write chunk atomically using temp file
        let temp_path = path.with_extension("tmp");

        let mut file = fs::File::create(&temp_path)
            .await
            .context("Failed to create temp file")?;

        file.write_all(data)
            .await
            .context("Failed to write chunk data")?;

        file.sync_all().await.context("Failed to sync file")?;

        // Atomic rename
        fs::rename(temp_path, path)
            .await
            .context("Failed to rename temp file")?;

        Ok(())
    }

    async fn get_chunk(&self, id: &[u8; 32]) -> Result<Vec<u8>> {
        let path = self.chunk_path(id);

        let mut file = fs::File::open(&path)
            .await
            .with_context(|| format!("Failed to open chunk file: {:?}", path))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .await
            .context("Failed to read chunk data")?;

        Ok(data)
    }

    async fn delete_chunk(&self, id: &[u8; 32]) -> Result<()> {
        let path = self.chunk_path(id);

        if path.exists() {
            fs::remove_file(path)
                .await
                .context("Failed to delete chunk file")?;
        }

        Ok(())
    }

    async fn has_chunk(&self, id: &[u8; 32]) -> Result<bool> {
        let path = self.chunk_path(id);
        Ok(path.exists())
    }

    async fn list_chunks(&self) -> Result<Vec<[u8; 32]>> {
        let mut chunks = Vec::new();

        // Walk directory tree
        let mut stack = vec![self.base_path.clone()];

        while let Some(dir) = stack.pop() {
            let mut entries = fs::read_dir(&dir)
                .await
                .with_context(|| format!("Failed to read directory: {:?}", dir))?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if let Some(name) = path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        if name_str.ends_with(".chunk") {
                            // Extract hex ID from filename
                            let hex = name_str.trim_end_matches(".chunk");
                            if let Ok(id_bytes) = hex::decode(hex) {
                                if id_bytes.len() == 32 {
                                    let mut id = [0u8; 32];
                                    id.copy_from_slice(&id_bytes);
                                    chunks.push(id);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(chunks)
    }
}

/// Network storage node endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeEndpoint {
    /// Node address (IP or hostname)
    pub address: String,
    /// Node port
    pub port: u16,
    /// Optional node ID
    pub node_id: Option<[u8; 32]>,
}

/// Network-based storage implementation
pub struct NetworkStorage {
    /// List of storage nodes
    nodes: Vec<NodeEndpoint>,
    /// Replication factor
    replication: usize,
}

impl NetworkStorage {
    /// Create a new network storage backend
    pub fn new(nodes: Vec<NodeEndpoint>, replication: usize) -> Self {
        Self { nodes, replication }
    }

    /// Select nodes for storing a chunk
    fn select_nodes(&self, chunk_id: &[u8; 32]) -> Vec<&NodeEndpoint> {
        // Simple deterministic selection based on chunk ID
        let mut selected = Vec::new();
        let target_count = self.replication.min(self.nodes.len());

        // Use different parts of the hash to select unique nodes
        for i in 0..target_count {
            let hash_offset = i * 4;
            let index = if hash_offset + 3 < chunk_id.len() {
                u32::from_le_bytes([
                    chunk_id[hash_offset],
                    chunk_id[hash_offset + 1],
                    chunk_id[hash_offset + 2],
                    chunk_id[hash_offset + 3],
                ]) as usize
            } else {
                // Use XOR of all bytes if we run out of unique positions
                chunk_id
                    .iter()
                    .enumerate()
                    .map(|(j, &b)| (j + i) * b as usize)
                    .sum::<usize>()
            };

            let mut node_index = index % self.nodes.len();
            let mut attempts = 0;

            // Find a node we haven't selected yet
            while selected.iter().any(|n| *n == &self.nodes[node_index])
                && attempts < self.nodes.len()
            {
                node_index = (node_index + 1) % self.nodes.len();
                attempts += 1;
            }

            if attempts < self.nodes.len() {
                selected.push(&self.nodes[node_index]);
            }
        }

        selected
    }
}

#[async_trait]
impl StorageBackend for NetworkStorage {
    async fn put_chunk(&self, id: &[u8; 32], _data: &[u8]) -> Result<()> {
        let nodes = self.select_nodes(id);

        if nodes.is_empty() {
            anyhow::bail!("No nodes available for storage");
        }

        // Store to selected nodes
        let mut success_count = 0;

        for node in nodes {
            // In a real implementation, this would make network calls
            // For now, we'll simulate success
            tracing::debug!("Storing chunk to node: {}:{}", node.address, node.port);
            success_count += 1;
        }

        if success_count == 0 {
            anyhow::bail!("Failed to store chunk to any node");
        }

        Ok(())
    }

    async fn get_chunk(&self, id: &[u8; 32]) -> Result<Vec<u8>> {
        let nodes = self.select_nodes(id);

        for node in nodes {
            // Try to retrieve from each node
            // In a real implementation, this would make network calls
            tracing::debug!("Retrieving chunk from node: {}:{}", node.address, node.port);

            // Simulate successful retrieval
            return Ok(vec![0u8; 1024]); // Placeholder data
        }

        anyhow::bail!("Chunk not found on any node")
    }

    async fn delete_chunk(&self, id: &[u8; 32]) -> Result<()> {
        let nodes = self.select_nodes(id);

        for node in nodes {
            // Delete from each node
            tracing::debug!("Deleting chunk from node: {}:{}", node.address, node.port);
        }

        Ok(())
    }

    async fn has_chunk(&self, id: &[u8; 32]) -> Result<bool> {
        let nodes = self.select_nodes(id);

        for node in nodes {
            // Check each node
            tracing::debug!("Checking chunk on node: {}:{}", node.address, node.port);
            return Ok(true); // Simulate found
        }

        Ok(false)
    }

    async fn list_chunks(&self) -> Result<Vec<[u8; 32]>> {
        // This would require querying all nodes and deduplicating
        Ok(Vec::new())
    }
}

/// Multi-backend storage that tries multiple backends
pub struct MultiStorage {
    /// Ordered list of storage backends
    backends: Vec<Arc<dyn StorageBackend>>,
}

impl MultiStorage {
    /// Create a new multi-backend storage
    pub fn new(backends: Vec<Arc<dyn StorageBackend>>) -> Self {
        Self { backends }
    }
}

use std::sync::Arc;

#[async_trait]
impl StorageBackend for MultiStorage {
    async fn put_chunk(&self, id: &[u8; 32], data: &[u8]) -> Result<()> {
        let mut last_error = None;

        // Try to store in all backends
        for backend in &self.backends {
            if let Err(e) = backend.put_chunk(id, data).await {
                tracing::warn!("Failed to store chunk in backend: {}", e);
                last_error = Some(e);
            }
        }

        if let Some(e) = last_error {
            Err(e)
        } else {
            Ok(())
        }
    }

    async fn get_chunk(&self, id: &[u8; 32]) -> Result<Vec<u8>> {
        // Try each backend in order
        for backend in &self.backends {
            match backend.get_chunk(id).await {
                Ok(data) => return Ok(data),
                Err(e) => {
                    tracing::debug!("Backend failed to get chunk: {}", e);
                }
            }
        }

        anyhow::bail!("Chunk not found in any backend")
    }

    async fn delete_chunk(&self, id: &[u8; 32]) -> Result<()> {
        // Delete from all backends
        for backend in &self.backends {
            if let Err(e) = backend.delete_chunk(id).await {
                tracing::warn!("Failed to delete chunk from backend: {}", e);
            }
        }
        Ok(())
    }

    async fn has_chunk(&self, id: &[u8; 32]) -> Result<bool> {
        // Check if any backend has the chunk
        for backend in &self.backends {
            if backend.has_chunk(id).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn list_chunks(&self) -> Result<Vec<[u8; 32]>> {
        let mut all_chunks = HashSet::new();

        // Collect from all backends
        for backend in &self.backends {
            if let Ok(chunks) = backend.list_chunks().await {
                all_chunks.extend(chunks);
            }
        }

        Ok(all_chunks.into_iter().collect())
    }
}

use std::collections::HashSet;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_storage_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        let chunk_id = [42u8; 32];
        let data = b"Hello, World!";

        // Store chunk
        storage.put_chunk(&chunk_id, data).await.unwrap();

        // Verify it exists
        assert!(storage.has_chunk(&chunk_id).await.unwrap());

        // Retrieve chunk
        let retrieved = storage.get_chunk(&chunk_id).await.unwrap();
        assert_eq!(retrieved, data);

        // Delete chunk
        storage.delete_chunk(&chunk_id).await.unwrap();
        assert!(!storage.has_chunk(&chunk_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_local_storage_list() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        // Store multiple chunks
        let chunks = vec![[1u8; 32], [2u8; 32], [3u8; 32]];

        for chunk_id in &chunks {
            storage.put_chunk(chunk_id, b"data").await.unwrap();
        }

        // List chunks
        let listed = storage.list_chunks().await.unwrap();
        assert_eq!(listed.len(), 3);

        for chunk_id in chunks {
            assert!(listed.contains(&chunk_id));
        }
    }

    #[test]
    fn test_network_storage_node_selection() {
        let nodes = vec![
            NodeEndpoint {
                address: "node1".to_string(),
                port: 8080,
                node_id: None,
            },
            NodeEndpoint {
                address: "node2".to_string(),
                port: 8080,
                node_id: None,
            },
            NodeEndpoint {
                address: "node3".to_string(),
                port: 8080,
                node_id: None,
            },
        ];

        let storage = NetworkStorage::new(nodes, 2);

        let chunk_id = [42u8; 32];
        let selected = storage.select_nodes(&chunk_id);

        assert_eq!(selected.len(), 2);

        // Should select same nodes for same chunk ID
        let selected2 = storage.select_nodes(&chunk_id);
        assert_eq!(selected, selected2);

        // Different chunk should select different nodes (probably)
        let chunk_id2 = [99u8; 32];
        let selected3 = storage.select_nodes(&chunk_id2);
        // May or may not be different, but should be deterministic
        assert_eq!(selected3.len(), 2);
    }

    #[tokio::test]
    async fn test_multi_storage() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();

        let backend1 = Arc::new(
            LocalStorage::new(temp_dir1.path().to_path_buf())
                .await
                .unwrap(),
        );
        let backend2 = Arc::new(
            LocalStorage::new(temp_dir2.path().to_path_buf())
                .await
                .unwrap(),
        );

        let multi = MultiStorage::new(vec![backend1.clone(), backend2.clone()]);

        let chunk_id = [42u8; 32];
        let data = b"Test data";

        // Store through multi-backend
        multi.put_chunk(&chunk_id, data).await.unwrap();

        // Verify both backends have the chunk
        assert!(backend1.has_chunk(&chunk_id).await.unwrap());
        assert!(backend2.has_chunk(&chunk_id).await.unwrap());

        // Delete from first backend
        backend1.delete_chunk(&chunk_id).await.unwrap();

        // Multi-backend should still find it in second backend
        let retrieved = multi.get_chunk(&chunk_id).await.unwrap();
        assert_eq!(retrieved, data);
    }
}
