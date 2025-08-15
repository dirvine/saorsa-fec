// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Saorsa FEC - Patent-free Erasure Coding
//!
//! This crate provides systematic Reed-Solomon erasure coding for the Saorsa P2P network,
//! implementing the Information Dispersal Algorithm (IDA) without patent encumbrance.
//!
//! ## Features
//! - Systematic encoding (original data in first k shares)
//! - Deterministic parity generation
//! - GF(256) arithmetic for efficiency
//! - Optional ISA-L acceleration on x86_64
//! - On-demand parity generation for repair

use std::fmt;
use thiserror::Error;

pub mod backends;
pub mod gf256;
pub mod ida;
pub mod traits;

pub use ida::{IDAConfig, IDADescriptor, ShareMetadata};
pub use traits::{Fec, FecBackend};

/// Errors that can occur during FEC operations
#[derive(Debug, Error)]
pub enum FecError {
    #[error("Invalid parameters: k={k}, n={n}")]
    InvalidParameters { k: usize, n: usize },

    #[error("Insufficient shares for reconstruction: have {have}, need {need}")]
    InsufficientShares { have: usize, need: usize },

    #[error("Share index out of bounds: {index} >= {max}")]
    InvalidShareIndex { index: usize, max: usize },

    #[error("Data size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },

    #[error("Matrix is not invertible")]
    SingularMatrix,

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FecError>;

/// FEC parameters for encoding/decoding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecParams {
    /// Number of data shares (k)
    pub data_shares: u16,
    /// Number of parity shares (n - k)
    pub parity_shares: u16,
    /// Size of each symbol in bytes
    pub symbol_size: u32,
}

impl FecParams {
    /// Create new FEC parameters
    pub fn new(data_shares: u16, parity_shares: u16) -> Result<Self> {
        if data_shares == 0 || parity_shares == 0 {
            return Err(FecError::InvalidParameters {
                k: data_shares as usize,
                n: (data_shares + parity_shares) as usize,
            });
        }

        // GF(256) limits us to 255 total shares
        if data_shares as u32 + parity_shares as u32 > 255 {
            return Err(FecError::InvalidParameters {
                k: data_shares as usize,
                n: (data_shares + parity_shares) as usize,
            });
        }

        Ok(Self {
            data_shares,
            parity_shares,
            symbol_size: 64 * 1024, // 64KB default
        })
    }

    /// Get total number of shares (n)
    pub fn total_shares(&self) -> u16 {
        self.data_shares + self.parity_shares
    }

    /// Calculate parameters based on content size
    pub fn from_content_size(size: usize) -> Self {
        match size {
            0..=1_000_000 => Self::new(8, 2).unwrap(), // 25% overhead
            1_000_001..=10_000_000 => Self::new(16, 4).unwrap(), // 25% overhead
            _ => Self::new(20, 5).unwrap(),            // 25% overhead
        }
    }
}

impl fmt::Display for FecParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FEC({}/{}, {}KB symbols)",
            self.data_shares,
            self.total_shares(),
            self.symbol_size / 1024
        )
    }
}

/// Main FEC encoder/decoder
#[derive(Debug)]
pub struct FecCodec {
    params: FecParams,
    #[allow(dead_code)]
    backend: Box<dyn FecBackend>,
}

impl FecCodec {
    /// Create a new FEC codec with the given parameters
    pub fn new(params: FecParams) -> Result<Self> {
        let backend = backends::create_backend()?;
        Ok(Self { params, backend })
    }

    /// Create with specific backend
    pub fn with_backend(params: FecParams, backend: Box<dyn FecBackend>) -> Self {
        Self { params, backend }
    }

    /// Encode data into shares
    pub fn encode(&self, data: &[u8]) -> Result<Vec<Vec<u8>>> {
        let k = self.params.data_shares as usize;
        let m = self.params.parity_shares as usize;

        // Split data into k blocks
        let block_size = (data.len() + k - 1) / k;
        let mut data_blocks = vec![vec![0u8; block_size]; k];

        for (i, chunk) in data.chunks(block_size).enumerate() {
            if i < k {
                data_blocks[i][..chunk.len()].copy_from_slice(chunk);
            }
        }

        let data_refs: Vec<&[u8]> = data_blocks.iter().map(|v| v.as_slice()).collect();

        // Generate parity blocks
        let mut parity_blocks = vec![vec![]; m];
        self.backend
            .encode_blocks(&data_refs, &mut parity_blocks, self.params)?;

        // Combine data and parity blocks
        let mut shares = data_blocks;
        shares.extend(parity_blocks);

        Ok(shares)
    }

    /// Decode from available shares
    pub fn decode(&self, shares: &[Option<Vec<u8>>]) -> Result<Vec<u8>> {
        let k = self.params.data_shares as usize;

        // Clone shares for decoding
        let mut work_shares = shares.to_vec();

        // Decode
        self.backend.decode_blocks(&mut work_shares, self.params)?;

        // Reconstruct original data from first k shares
        let mut data = Vec::new();
        for i in 0..k {
            if let Some(block) = &work_shares[i] {
                data.extend_from_slice(block);
            } else {
                return Err(FecError::InsufficientShares { have: 0, need: k });
            }
        }

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fec_params_validation() {
        assert!(FecParams::new(0, 10).is_err());
        assert!(FecParams::new(10, 0).is_err());
        assert!(FecParams::new(200, 100).is_err()); // > 255 total
        assert!(FecParams::new(10, 5).is_ok());
    }

    #[test]
    fn test_content_size_params() {
        let small = FecParams::from_content_size(500_000);
        assert_eq!(small.data_shares, 8);
        assert_eq!(small.parity_shares, 2);

        let medium = FecParams::from_content_size(5_000_000);
        assert_eq!(medium.data_shares, 16);
        assert_eq!(medium.parity_shares, 4);

        let large = FecParams::from_content_size(50_000_000);
        assert_eq!(large.data_shares, 20);
        assert_eq!(large.parity_shares, 5);
    }
}
