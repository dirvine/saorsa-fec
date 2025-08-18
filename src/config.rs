//! Configuration for the encryption and FEC pipeline
//!
//! This module provides configuration options for encryption modes,
//! storage settings, and FEC parameters. The v0.3 specification requires
//! a builder pattern for configuration.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Encryption mode selection for the v0.3 API
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EncryptionMode {
    /// Pure convergent encryption (deduplication across all users)
    Convergent,
    /// Convergent encryption with secret (controlled deduplication)
    ConvergentWithSecret,
    /// Random key encryption (no deduplication)
    RandomKey,
}


/// Main configuration for the Saorsa FEC system
/// Supports builder pattern as specified in v0.3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Encryption mode
    pub encryption_mode: EncryptionMode,
    /// FEC data shards (k)
    pub data_shards: u8,
    /// FEC parity shards (n-k)
    pub parity_shards: u8,
    /// Chunk size in bytes (default ~64 KiB)
    pub chunk_size: usize,
    /// Whether compression is enabled
    pub compression_enabled: bool,
    /// Compression level (1-9)
    pub compression_level: u8,
    /// Legacy fields for backward compatibility
    pub encryption: EncryptionConfig,
    pub fec: FecConfig,
    pub storage: StorageConfig,
    pub gc: GcConfig,
    pub version: VersionConfig,
}

impl Config {
    /// Create a new configuration with default settings
    /// Required by v0.3 specification
    pub fn new() -> Self {
        Self {
            encryption_mode: EncryptionMode::Convergent,
            data_shards: 16,
            parity_shards: 4,
            chunk_size: 64 * 1024, // 64 KiB as specified
            compression_enabled: true,
            compression_level: 6,
            // Legacy fields
            encryption: EncryptionConfig::default(),
            fec: FecConfig::default(),
            storage: StorageConfig::default(),
            gc: GcConfig::default(),
            version: VersionConfig::default(),
        }
    }

    /// Set encryption mode (v0.3 builder pattern)
    pub fn with_encryption_mode(mut self, mode: EncryptionMode) -> Self {
        self.encryption_mode = mode;
        self
    }

    /// Set FEC parameters (v0.3 builder pattern)
    /// overhead = parity_shards / data_shards
    pub fn with_fec_params(mut self, data_shards: u8, parity_shards: u8) -> Self {
        self.data_shards = data_shards;
        self.parity_shards = parity_shards;
        // Update legacy fields for compatibility
        self.fec.data_shares = data_shards as u16;
        self.fec.parity_shares = parity_shards as u16;
        self
    }

    /// Set chunk size (v0.3 builder pattern)
    pub fn with_chunk_size(mut self, bytes: usize) -> Self {
        self.chunk_size = bytes;
        // Update legacy field
        self.fec.stripe_size = bytes;
        self
    }

    /// Set compression settings (v0.3 builder pattern)
    pub fn with_compression(mut self, on: bool, level: u8) -> Self {
        self.compression_enabled = on;
        self.compression_level = level.clamp(1, 9);
        // Update legacy fields
        self.encryption.compress_before_encrypt = on;
        self.encryption.compression_level = level as u32;
        self
    }

    /// Create a high-performance configuration
    pub fn high_performance() -> Self {
        Self {
            encryption_mode: EncryptionMode::Convergent,
            data_shards: 16,
            parity_shards: 4,
            chunk_size: 128 * 1024,
            compression_enabled: true,
            compression_level: 3,
            encryption: EncryptionConfig {
                mode: EncryptionMode::Convergent,
                compress_before_encrypt: true,
                compression_level: 3,
            },
            fec: FecConfig {
                data_shares: 16,
                parity_shares: 4,
                stripe_size: 128 * 1024,
                auto_params: true,
            },
            storage: StorageConfig {
                backend: StorageBackend::Local {
                    path: "/var/lib/saorsa".into(),
                },
                cache_size: 1024 * 1024 * 1024,
                parallel_operations: 8,
            },
            gc: GcConfig {
                enabled: true,
                retention_days: 30,
                min_free_space_gb: 10,
                run_interval: Duration::from_secs(3600),
            },
            version: VersionConfig {
                max_versions: 100,
                auto_tag_interval: 10,
                diff_compression: true,
            },
        }
    }

    /// Create a high-reliability configuration
    pub fn high_reliability() -> Self {
        Self {
            encryption_mode: EncryptionMode::RandomKey,
            data_shards: 10,
            parity_shards: 10,
            chunk_size: 64 * 1024,
            compression_enabled: true,
            compression_level: 6,
            encryption: EncryptionConfig {
                mode: EncryptionMode::RandomKey,
                compress_before_encrypt: true,
                compression_level: 6,
            },
            fec: FecConfig {
                data_shares: 10,
                parity_shares: 10,
                stripe_size: 64 * 1024,
                auto_params: false,
            },
            storage: StorageConfig {
                backend: StorageBackend::Multi {
                    backends: vec![
                        StorageBackend::Local {
                            path: "/var/lib/saorsa/primary".into(),
                        },
                        StorageBackend::Local {
                            path: "/var/lib/saorsa/backup".into(),
                        },
                    ],
                },
                cache_size: 512 * 1024 * 1024,
                parallel_operations: 4,
            },
            gc: GcConfig {
                enabled: true,
                retention_days: 90,
                min_free_space_gb: 50,
                run_interval: Duration::from_secs(7200),
            },
            version: VersionConfig {
                max_versions: 1000,
                auto_tag_interval: 1,
                diff_compression: true,
            },
        }
    }

    /// Create a minimal storage configuration
    pub fn minimal_storage() -> Self {
        Self {
            encryption_mode: EncryptionMode::Convergent,
            data_shards: 20,
            parity_shards: 2,
            chunk_size: 32 * 1024,
            compression_enabled: true,
            compression_level: 9,
            encryption: EncryptionConfig {
                mode: EncryptionMode::Convergent,
                compress_before_encrypt: true,
                compression_level: 9,
            },
            fec: FecConfig {
                data_shares: 20,
                parity_shares: 2,
                stripe_size: 32 * 1024,
                auto_params: true,
            },
            storage: StorageConfig {
                backend: StorageBackend::Local {
                    path: "/var/lib/saorsa".into(),
                },
                cache_size: 64 * 1024 * 1024,
                parallel_operations: 2,
            },
            gc: GcConfig {
                enabled: true,
                retention_days: 7,
                min_free_space_gb: 1,
                run_interval: Duration::from_secs(1800),
            },
            version: VersionConfig {
                max_versions: 10,
                auto_tag_interval: 0,
                diff_compression: true,
            },
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.fec.data_shares == 0 {
            anyhow::bail!("Data shares must be greater than 0");
        }
        if self.fec.parity_shares == 0 {
            anyhow::bail!("Parity shares must be greater than 0");
        }
        if self.fec.data_shares + self.fec.parity_shares > 255 {
            anyhow::bail!("Total shares cannot exceed 255");
        }
        if self.fec.stripe_size == 0 {
            anyhow::bail!("Stripe size must be greater than 0");
        }
        if self.storage.cache_size == 0 {
            anyhow::bail!("Cache size must be greater than 0");
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }

}
/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Encryption mode to use
    pub mode: EncryptionMode,
    /// Whether to compress before encryption
    pub compress_before_encrypt: bool,
    /// Compression level (1-9)
    pub compression_level: u32,
}
impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            mode: EncryptionMode::Convergent,
            compress_before_encrypt: true,
            compression_level: 6,
        }
    }
}

/// FEC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FecConfig {
    /// Number of data shares
    pub data_shares: u16,
    /// Number of parity shares
    pub parity_shares: u16,
    /// Size of each stripe in bytes
    pub stripe_size: usize,
    /// Automatically adjust parameters based on content
    pub auto_params: bool,
}

impl Default for FecConfig {
    fn default() -> Self {
        Self {
            data_shares: 16,
            parity_shares: 4,
            stripe_size: 64 * 1024,
            auto_params: true,
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage backend to use
    pub backend: StorageBackend,
    /// Cache size in bytes
    pub cache_size: usize,
    /// Number of parallel storage operations
    pub parallel_operations: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Local {
                path: "/var/lib/saorsa".into(),
            },
            cache_size: 256 * 1024 * 1024,
            parallel_operations: 4,
        }
    }
}

/// Storage backend type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    /// Local filesystem storage
    Local {
        /// Base path for storage
        path: String,
    },
    /// Network storage
    Network {
        /// List of node addresses
        nodes: Vec<String>,
        /// Replication factor
        replication: usize,
    },
    /// Multiple backends
    Multi {
        /// List of backends
        backends: Vec<StorageBackend>,
    },
}

/// Garbage collection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcConfig {
    /// Whether GC is enabled
    pub enabled: bool,
    /// Number of days to retain unreferenced chunks
    pub retention_days: u32,
    /// Minimum free space in GB before triggering GC
    pub min_free_space_gb: u32,
    /// How often to run GC
    pub run_interval: Duration,
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 30,
            min_free_space_gb: 10,
            run_interval: Duration::from_secs(3600),
        }
    }
}

/// Version management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionConfig {
    /// Maximum number of versions to keep
    pub max_versions: usize,
    /// Auto-tag every N versions (0 = disabled)
    pub auto_tag_interval: usize,
    /// Use compression for version diffs
    pub diff_compression: bool,
}

impl Default for VersionConfig {
    fn default() -> Self {
        Self {
            max_versions: 100,
            auto_tag_interval: 10,
            diff_compression: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_high_performance() {
        let config = Config::high_performance();
        assert!(config.validate().is_ok());
        assert_eq!(config.fec.data_shares, 16);
        assert_eq!(config.fec.parity_shares, 4);
    }

    #[test]
    fn test_config_high_reliability() {
        let config = Config::high_reliability();
        assert!(config.validate().is_ok());
        assert_eq!(config.fec.data_shares, 10);
        assert_eq!(config.fec.parity_shares, 10);
    }

    #[test]
    fn test_config_minimal_storage() {
        let config = Config::minimal_storage();
        assert!(config.validate().is_ok());
        assert_eq!(config.fec.data_shares, 20);
        assert_eq!(config.fec.parity_shares, 2);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();

        config.fec.data_shares = 0;
        assert!(config.validate().is_err());

        config.fec.data_shares = 200;
        config.fec.parity_shares = 100;
        assert!(config.validate().is_err());

        config.fec.data_shares = 16;
        config.fec.parity_shares = 4;
        config.fec.stripe_size = 0;
        assert!(config.validate().is_err());
    }
}
