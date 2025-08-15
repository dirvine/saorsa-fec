// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Core traits for FEC operations

use async_trait::async_trait;
use bytes::Bytes;
use crate::{FecParams, Result};
use std::fmt;

/// Core FEC trait for encoding and decoding operations
#[async_trait]
pub trait Fec: Send + Sync {
    /// Encode data into shares using systematic encoding
    /// 
    /// Returns `k + parity` shares where the first `k` shares contain the original data
    async fn encode(&self, data: &[u8], params: FecParams) -> Result<Vec<Bytes>>;
    
    /// Decode data from any k shares
    /// 
    /// The shares slice should have exactly `total_shares` elements,
    /// with `None` for missing shares and `Some(data)` for available shares
    async fn decode(&self, shares: &[Option<Bytes>], params: FecParams) -> Result<Bytes>;
    
    /// Generate additional parity shares on-demand
    /// 
    /// Used for repair when telemetry indicates share loss
    async fn mint_parity(
        &self,
        data: &[u8],
        params: FecParams,
        extra_parity: usize,
        seed: u64,
    ) -> Result<Vec<Bytes>>;
    
    /// Verify shares are valid without full reconstruction
    async fn verify_shares(&self, shares: &[Option<Bytes>], params: FecParams) -> Result<bool>;
}

/// Backend trait for different FEC implementations
pub trait FecBackend: Send + Sync + fmt::Debug {
    /// Encode data blocks into parity blocks
    fn encode_blocks(&self, data: &[&[u8]], parity: &mut [Vec<u8>], params: FecParams) -> Result<()>;
    
    /// Decode from available shares
    fn decode_blocks(&self, shares: &mut [Option<Vec<u8>>], params: FecParams) -> Result<()>;
    
    /// Generate encoding matrix
    fn generate_matrix(&self, k: usize, m: usize) -> Vec<Vec<u8>>;
    
    /// Check if backend supports hardware acceleration
    fn is_accelerated(&self) -> bool {
        false
    }
    
    /// Get backend name for debugging
    fn name(&self) -> &'static str;
}