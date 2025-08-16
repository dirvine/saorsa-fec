//! Version management for tracking file history and changes
//!
//! This module provides a version tree structure for tracking file versions,
//! enabling efficient diff computation and chunk deduplication.

use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::chunk_registry::ChunkRegistry;
use crate::metadata::FileMetadata;

/// Type alias for chunk diff result
type ChunkDiff = (Vec<[u8; 32]>, Vec<[u8; 32]>);

/// Node in the version tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionNode {
    /// Hash of the FileMetadata for this version
    pub metadata_hash: [u8; 32],
    /// Parent version if this is not the first version
    pub parent: Option<Box<VersionNode>>,
    /// Chunks added in this version
    pub chunks_added: Vec<[u8; 32]>,
    /// Chunks removed in this version
    pub chunks_removed: Vec<[u8; 32]>,
    /// Optional local version information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_info: Option<LocalVersionInfo>,
}

impl VersionNode {
    /// Create a new version node
    pub fn new(metadata_hash: [u8; 32]) -> Self {
        Self {
            metadata_hash,
            parent: None,
            chunks_added: Vec::new(),
            chunks_removed: Vec::new(),
            local_info: None,
        }
    }

    /// Set parent version
    pub fn with_parent(mut self, parent: VersionNode) -> Self {
        self.parent = Some(Box::new(parent));
        self
    }

    /// Add chunks that were added in this version
    pub fn with_added_chunks(mut self, chunks: Vec<[u8; 32]>) -> Self {
        self.chunks_added = chunks;
        self
    }

    /// Add chunks that were removed in this version
    pub fn with_removed_chunks(mut self, chunks: Vec<[u8; 32]>) -> Self {
        self.chunks_removed = chunks;
        self
    }

    /// Get depth of this node in version tree
    pub fn depth(&self) -> usize {
        match &self.parent {
            Some(parent) => parent.depth() + 1,
            None => 0,
        }
    }

    /// Get all ancestor metadata hashes
    pub fn ancestors(&self) -> Vec<[u8; 32]> {
        let mut result = Vec::new();
        let mut current = self.parent.as_deref();

        while let Some(node) = current {
            result.push(node.metadata_hash);
            current = node.parent.as_deref();
        }

        result
    }
}

/// Local version information (not content-addressed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalVersionInfo {
    /// Unix timestamp when version was created
    pub created_at: u64,
    /// Optional version tag or label
    pub tag: Option<String>,
    /// Optional commit message
    pub message: Option<String>,
    /// Author of this version
    pub author: Option<String>,
}

impl LocalVersionInfo {
    /// Create new local version info with current timestamp
    pub fn new() -> Self {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            created_at,
            tag: None,
            message: None,
            author: None,
        }
    }

    /// Set version tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Set commit message
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

impl Default for LocalVersionInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Diff between two versions
#[derive(Debug, Clone)]
pub struct VersionDiff {
    /// Chunks added in newer version
    pub added: Vec<[u8; 32]>,
    /// Chunks removed in newer version
    pub removed: Vec<[u8; 32]>,
    /// Chunks that remained unchanged
    pub unchanged: Vec<[u8; 32]>,
    /// Total size change in bytes
    pub size_delta: i64,
}

/// Version manager for tracking file history
pub struct VersionManager {
    /// All versions indexed by metadata hash
    versions: HashMap<[u8; 32], VersionNode>,
    /// Reference to chunk registry for tracking
    chunk_registry: Arc<RwLock<ChunkRegistry>>,
    /// File ID to latest version mapping
    file_versions: HashMap<[u8; 32], [u8; 32]>,
}

impl VersionManager {
    /// Create a new version manager
    pub fn new(chunk_registry: Arc<RwLock<ChunkRegistry>>) -> Self {
        Self {
            versions: HashMap::new(),
            chunk_registry,
            file_versions: HashMap::new(),
        }
    }

    /// Create a new version from metadata
    pub fn create_version(&mut self, metadata: &FileMetadata) -> Result<VersionNode> {
        let metadata_hash = metadata.compute_id();

        // Find parent version if it exists
        let parent_node = if let Some(parent_hash) = metadata.parent_version {
            Some(
                self.versions
                    .get(&parent_hash)
                    .cloned()
                    .context("Parent version not found")?,
            )
        } else {
            // Check if this is an update to an existing file
            self.find_previous_version(&metadata.file_id).cloned()
        };

        // Compute chunks added/removed
        let (added, removed) = if let Some(ref parent) = parent_node {
            self.compute_chunk_diff(metadata, parent)?
        } else {
            // First version - all chunks are new
            let added = metadata.chunks.iter().map(|c| c.chunk_id).collect();
            (added, Vec::new())
        };

        // Create version node
        let mut node = VersionNode::new(metadata_hash)
            .with_added_chunks(added.clone())
            .with_removed_chunks(removed.clone());

        if let Some(parent) = parent_node {
            node = node.with_parent(parent);
        }

        // Update chunk registry
        {
            let mut registry = self.chunk_registry.write();
            registry.increment_refs(&metadata.chunks)?;
            if !removed.is_empty() {
                registry.decrement_refs(&removed)?;
            }
        }

        // Store version
        self.versions.insert(metadata_hash, node.clone());
        self.file_versions.insert(metadata.file_id, metadata_hash);

        Ok(node)
    }

    /// Find the previous version of a file
    pub fn find_previous_version(&self, file_id: &[u8; 32]) -> Option<&VersionNode> {
        self.file_versions
            .get(file_id)
            .and_then(|hash| self.versions.get(hash))
    }

    /// Get version history for a file
    pub fn get_history(&self, file_id: &[u8; 32]) -> Vec<VersionNode> {
        let mut history = Vec::new();

        if let Some(latest_hash) = self.file_versions.get(file_id)
            && let Some(mut node) = self.versions.get(latest_hash).cloned()
        {
            history.push(node.clone());

            while let Some(parent) = node.parent {
                history.push(parent.as_ref().clone());
                node = parent.as_ref().clone();
            }
        }

        history.reverse(); // Oldest first
        history
    }

    /// Compute diff between two versions
    pub fn diff(&self, v1: &VersionNode, v2: &VersionNode) -> Result<VersionDiff> {
        // Get all chunks for each version
        let chunks1 = self.get_version_chunks(v1)?;
        let chunks2 = self.get_version_chunks(v2)?;

        let set1: HashSet<_> = chunks1.iter().copied().collect();
        let set2: HashSet<_> = chunks2.iter().copied().collect();

        let added: Vec<_> = set2.difference(&set1).copied().collect();
        let removed: Vec<_> = set1.difference(&set2).copied().collect();
        let unchanged: Vec<_> = set1.intersection(&set2).copied().collect();

        // Calculate size delta
        let registry = self.chunk_registry.read();
        let size_added: i64 = added
            .iter()
            .filter_map(|id| registry.get_chunk_size(id))
            .map(|s| s as i64)
            .sum();
        let size_removed: i64 = removed
            .iter()
            .filter_map(|id| registry.get_chunk_size(id))
            .map(|s| s as i64)
            .sum();

        Ok(VersionDiff {
            added,
            removed,
            unchanged,
            size_delta: size_added - size_removed,
        })
    }

    /// Get specific version by hash
    pub fn get_version(&self, hash: &[u8; 32]) -> Option<&VersionNode> {
        self.versions.get(hash)
    }

    /// Remove a version (careful - this affects chunk references)
    pub fn remove_version(&mut self, hash: &[u8; 32]) -> Result<()> {
        let node = self.versions.remove(hash).context("Version not found")?;

        // Update chunk references
        let mut registry = self.chunk_registry.write();
        if !node.chunks_removed.is_empty() {
            // Re-increment refs for chunks that were marked as removed
            // (since we're removing the version that removed them)
            for chunk_id in &node.chunks_removed {
                registry.increment_ref(chunk_id)?;
            }
        }
        if !node.chunks_added.is_empty() {
            // Decrement refs for chunks that were added
            registry.decrement_refs(&node.chunks_added)?;
        }

        Ok(())
    }

    /// Tag a version with a name
    pub fn tag_version(&mut self, hash: &[u8; 32], tag: impl Into<String>) -> Result<()> {
        let version = self.versions.get_mut(hash).context("Version not found")?;

        if version.local_info.is_none() {
            version.local_info = Some(LocalVersionInfo::new());
        }

        if let Some(info) = &mut version.local_info {
            info.tag = Some(tag.into());
        }

        Ok(())
    }

    /// Get all tagged versions
    pub fn get_tagged_versions(&self) -> Vec<(&str, &VersionNode)> {
        self.versions
            .values()
            .filter_map(|v| {
                v.local_info
                    .as_ref()
                    .and_then(|info| info.tag.as_ref())
                    .map(|tag| (tag.as_str(), v))
            })
            .collect()
    }

    /// Compute chunk differences between metadata and parent
    fn compute_chunk_diff(
        &self,
        metadata: &FileMetadata,
        parent: &VersionNode,
    ) -> Result<ChunkDiff> {
        let parent_chunks = self.get_version_chunks(parent)?;
        let parent_set: HashSet<_> = parent_chunks.into_iter().collect();

        let current_chunks: HashSet<_> = metadata.chunks.iter().map(|c| c.chunk_id).collect();

        let added = current_chunks.difference(&parent_set).copied().collect();

        let removed = parent_set.difference(&current_chunks).copied().collect();

        Ok((added, removed))
    }

    /// Get all chunks for a version (traversing up the tree)
    fn get_version_chunks(&self, version: &VersionNode) -> Result<Vec<[u8; 32]>> {
        let mut chunks = HashSet::new();
        let mut current = Some(version);

        while let Some(node) = current {
            // Add chunks from this version
            for chunk_id in &node.chunks_added {
                chunks.insert(*chunk_id);
            }

            // Remove chunks that were removed
            for chunk_id in &node.chunks_removed {
                chunks.remove(chunk_id);
            }

            current = node.parent.as_deref();
        }

        Ok(chunks.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metadata(file_id: [u8; 32], chunk_ids: Vec<[u8; 32]>) -> FileMetadata {
        use crate::metadata::ChunkReference;

        let chunks: Vec<ChunkReference> = chunk_ids
            .into_iter()
            .enumerate()
            .map(|(i, id)| ChunkReference::new(id, 0, i as u16, 1024))
            .collect();

        FileMetadata::new(file_id, 1024 * chunks.len() as u64, None, chunks)
    }

    #[test]
    fn test_version_node_depth() {
        let v1 = VersionNode::new([1u8; 32]);
        assert_eq!(v1.depth(), 0);

        let v2 = VersionNode::new([2u8; 32]).with_parent(v1.clone());
        assert_eq!(v2.depth(), 1);

        let v3 = VersionNode::new([3u8; 32]).with_parent(v2);
        assert_eq!(v3.depth(), 2);
    }

    #[test]
    fn test_version_ancestors() {
        let v1 = VersionNode::new([1u8; 32]);
        let v2 = VersionNode::new([2u8; 32]).with_parent(v1.clone());
        let v3 = VersionNode::new([3u8; 32]).with_parent(v2.clone());

        let ancestors = v3.ancestors();
        assert_eq!(ancestors.len(), 2);
        assert_eq!(ancestors[0], [2u8; 32]);
        assert_eq!(ancestors[1], [1u8; 32]);
    }

    #[test]
    fn test_version_manager_create() {
        let registry = Arc::new(RwLock::new(ChunkRegistry::new()));
        let mut manager = VersionManager::new(registry);

        let metadata = create_test_metadata([10u8; 32], vec![[1u8; 32], [2u8; 32]]);
        let version = manager.create_version(&metadata).unwrap();

        assert_eq!(version.chunks_added.len(), 2);
        assert_eq!(version.chunks_removed.len(), 0);
        assert!(version.parent.is_none());
    }

    #[test]
    fn test_version_history() {
        let registry = Arc::new(RwLock::new(ChunkRegistry::new()));
        let mut manager = VersionManager::new(registry);

        let file_id = [10u8; 32];

        // Create first version
        let metadata1 = create_test_metadata(file_id, vec![[1u8; 32]]);
        let v1 = manager.create_version(&metadata1).unwrap();

        // Create second version with parent
        let metadata2 =
            create_test_metadata(file_id, vec![[1u8; 32], [2u8; 32]]).with_parent(v1.metadata_hash);
        manager.create_version(&metadata2).unwrap();

        let history = manager.get_history(&file_id);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_version_tagging() {
        let registry = Arc::new(RwLock::new(ChunkRegistry::new()));
        let mut manager = VersionManager::new(registry);

        let metadata = create_test_metadata([10u8; 32], vec![[1u8; 32]]);
        let version = manager.create_version(&metadata).unwrap();

        manager.tag_version(&version.metadata_hash, "v1.0").unwrap();

        let tagged = manager.get_tagged_versions();
        assert_eq!(tagged.len(), 1);
        assert_eq!(tagged[0].0, "v1.0");
    }
}
