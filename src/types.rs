//! Common types used throughout the Saorsa FEC system

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DataId([u8; 32]);

impl DataId {
    /// Create a DataId from raw bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Create a DataId from data content
    pub fn from_data(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        Self(*hash.as_bytes())
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for DataId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..8]))
    }
}

/// Unique identifier for a chunk
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId {
    data_id: DataId,
    index: usize,
}

impl ChunkId {
    /// Create a new chunk ID
    pub fn new(data_id: &DataId, index: usize) -> Self {
        Self {
            data_id: *data_id,
            index,
        }
    }

    /// Get the data ID
    pub fn data_id(&self) -> &DataId {
        &self.data_id
    }

    /// Get the chunk index
    pub fn index(&self) -> usize {
        self.index
    }
}

impl fmt::Display for ChunkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.data_id, self.index)
    }
}

/// Unique identifier for a share
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShareId {
    chunk_id: ChunkId,
    index: usize,
}

impl ShareId {
    /// Create a new share ID
    pub fn new(chunk_id: &ChunkId, index: usize) -> Self {
        Self {
            chunk_id: *chunk_id,
            index,
        }
    }

    /// Get the chunk ID
    pub fn chunk_id(&self) -> &ChunkId {
        &self.chunk_id
    }

    /// Get the share index
    pub fn index(&self) -> usize {
        self.index
    }
}

impl fmt::Display for ShareId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.chunk_id, self.index)
    }
}

/// Version identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionId([u8; 32]);

impl VersionId {
    /// Create a new version ID
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for VersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0[..8]))
    }
}
