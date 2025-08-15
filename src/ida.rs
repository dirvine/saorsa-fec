// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Information Dispersal Algorithm (IDA) implementation

use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use crate::{FecError, Result};

/// IDA configuration for different content sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IDAConfig {
    pub k: u16,     // Data shares required
    pub n: u16,     // Total shares (k + parity)
    pub stripe_size: u32, // Bytes per stripe
}

impl IDAConfig {
    /// Create configuration based on content size
    pub fn from_content_size(size: usize) -> Self {
        match size {
            0..=1_000_000 => Self {
                k: 8,
                n: 10,  // 25% overhead
                stripe_size: 64 * 1024, // 64KB stripes
            },
            1_000_001..=10_000_000 => Self {
                k: 16,
                n: 20,  // 25% overhead
                stripe_size: 128 * 1024, // 128KB stripes
            },
            _ => Self {
                k: 20,
                n: 25,  // 25% overhead
                stripe_size: 256 * 1024, // 256KB stripes
            },
        }
    }
    
    /// Calculate number of stripes for given data size
    pub fn num_stripes(&self, data_len: usize) -> usize {
        (data_len + self.stripe_size as usize - 1) / self.stripe_size as usize
    }
    
    /// Get redundancy ratio (n/k)
    pub fn redundancy(&self) -> f32 {
        self.n as f32 / self.k as f32
    }
}

/// IDA descriptor for a dispersed file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IDADescriptor {
    pub k: u16,               // Data slices required
    pub n: u16,               // Total slices
    pub stripe_size: u32,     // Bytes per stripe
    pub file_size: u64,       // Original file size
    pub code: String,         // "rs-gf256" for Reed-Solomon
    pub checksum: [u8; 32],   // BLAKE3 of original data
}

/// Metadata for an individual share
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareMetadata {
    pub file_id: [u8; 32],      // Content ID
    pub stripe_ix: u32,          // Which stripe (0-based)
    pub shard_ix: u16,           // Which shard within stripe (0-based)
    pub k: u16,                  // Data shares needed
    pub n: u16,                  // Total shares
    pub gen_row_seed: u64,       // Seed for deterministic parity
    pub chunk_hash: [u8; 32],    // Hash of this chunk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aead_tag: Option<[u8; 16]>, // For encrypted data
}

impl ShareMetadata {
    /// Create metadata for a share
    pub fn new(
        file_id: [u8; 32],
        stripe_ix: u32,
        shard_ix: u16,
        config: &IDAConfig,
        seed: u64,
    ) -> Self {
        Self {
            file_id,
            stripe_ix,
            shard_ix,
            k: config.k,
            n: config.n,
            gen_row_seed: seed,
            chunk_hash: [0; 32], // Will be filled after encoding
            aead_tag: None,
        }
    }
    
    /// Check if this is a data share (systematic)
    pub fn is_data_share(&self) -> bool {
        self.shard_ix < self.k
    }
    
    /// Check if this is a parity share
    pub fn is_parity_share(&self) -> bool {
        self.shard_ix >= self.k
    }
}

/// Stripe data for encoding/decoding
#[derive(Debug)]
pub struct Stripe {
    pub index: u32,
    pub data: Vec<u8>,
    pub padding: usize,
}

impl Stripe {
    /// Create a new stripe from data
    pub fn new(index: u32, data: Vec<u8>, stripe_size: usize) -> Self {
        let padding = if data.len() < stripe_size {
            stripe_size - data.len()
        } else {
            0
        };
        
        Self {
            index,
            data,
            padding,
        }
    }
    
    /// Pad stripe to full size
    pub fn padded(&self, stripe_size: usize) -> Vec<u8> {
        if self.padding == 0 {
            self.data.clone()
        } else {
            let mut padded = self.data.clone();
            padded.resize(stripe_size, 0);
            padded
        }
    }
    
    /// Remove padding from decoded stripe
    pub fn unpad(mut data: Vec<u8>, padding: usize) -> Vec<u8> {
        if padding > 0 && data.len() >= padding {
            data.truncate(data.len() - padding);
        }
        data
    }
}

/// Split data into stripes for encoding
pub fn create_stripes(data: &[u8], config: &IDAConfig) -> Vec<Stripe> {
    let stripe_size = config.stripe_size as usize;
    let mut stripes = Vec::new();
    let mut offset = 0;
    let mut index = 0;
    
    while offset < data.len() {
        let end = (offset + stripe_size).min(data.len());
        let stripe_data = data[offset..end].to_vec();
        stripes.push(Stripe::new(index, stripe_data, stripe_size));
        offset = end;
        index += 1;
    }
    
    stripes
}

/// Reconstruct data from decoded stripes
pub fn reconstruct_data(stripes: Vec<Stripe>, original_size: usize) -> Result<Bytes> {
    let mut data = BytesMut::with_capacity(original_size);
    
    // Sort stripes by index
    let mut sorted_stripes = stripes;
    sorted_stripes.sort_by_key(|s| s.index);
    
    // Concatenate stripes
    for (i, stripe) in sorted_stripes.iter().enumerate() {
        if stripe.index != i as u32 {
            return Err(FecError::SizeMismatch {
                expected: i,
                actual: stripe.index as usize,
            });
        }
        
        // All stripes contain actual data without padding
        // The padding field just indicates how much padding would be needed
        // to make it a full stripe
        data.extend_from_slice(&stripe.data);
    }
    
    // Verify we got the expected size
    if data.len() != original_size {
        return Err(FecError::SizeMismatch {
            expected: original_size,
            actual: data.len(),
        });
    }
    
    Ok(data.freeze())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ida_config_selection() {
        let small = IDAConfig::from_content_size(500_000);
        assert_eq!(small.k, 8);
        assert_eq!(small.n, 10);
        
        let medium = IDAConfig::from_content_size(5_000_000);
        assert_eq!(medium.k, 16);
        assert_eq!(medium.n, 20);
        
        let large = IDAConfig::from_content_size(50_000_000);
        assert_eq!(large.k, 20);
        assert_eq!(large.n, 25);
    }
    
    #[test]
    fn test_stripe_creation() {
        let data = vec![0u8; 1000];
        let config = IDAConfig {
            k: 3,
            n: 5,
            stripe_size: 256,
        };
        
        let stripes = create_stripes(&data, &config);
        assert_eq!(stripes.len(), 4); // 1000 / 256 = 3.9, so 4 stripes
        
        // First 3 stripes should be full
        for i in 0..3 {
            assert_eq!(stripes[i].data.len(), 256);
            assert_eq!(stripes[i].padding, 0);
        }
        
        // Last stripe should have padding
        assert_eq!(stripes[3].data.len(), 232); // 1000 - 768
        assert_eq!(stripes[3].padding, 24); // 256 - 232
    }
    
    #[test]
    fn test_data_reconstruction() {
        let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let config = IDAConfig {
            k: 2,
            n: 3,
            stripe_size: 4,
        };
        
        let stripes = create_stripes(&original, &config);
        
        // Verify stripe creation
        assert_eq!(stripes.len(), 3); // 10 bytes / 4 stripe_size = 3 stripes
        assert_eq!(stripes[0].data.len(), 4);
        assert_eq!(stripes[1].data.len(), 4);
        assert_eq!(stripes[2].data.len(), 2); // Last stripe has only 2 bytes
        assert_eq!(stripes[2].padding, 2); // And 2 bytes of padding
        
        let reconstructed = reconstruct_data(stripes, original.len()).unwrap();
        
        assert_eq!(reconstructed.as_ref(), &original);
    }
    
    #[test]
    fn test_share_metadata() {
        let file_id = [0u8; 32];
        let config = IDAConfig::from_content_size(1_000_000);
        
        let data_share = ShareMetadata::new(file_id, 0, 0, &config, 12345);
        assert!(data_share.is_data_share());
        assert!(!data_share.is_parity_share());
        
        let parity_share = ShareMetadata::new(file_id, 0, 8, &config, 12345);
        assert!(!parity_share.is_data_share());
        assert!(parity_share.is_parity_share());
    }
}