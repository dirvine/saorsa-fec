// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Pure Rust Reed-Solomon implementation using systematic Cauchy encoding

use crate::{
    gf256::{self, Gf256},
    FecBackend, FecError, FecParams, Result,
};
use std::sync::Arc;

/// Pure Rust Reed-Solomon backend
#[derive(Debug)]
pub struct PureRustBackend {
    /// Cached encoding matrices for common parameters
    matrix_cache: Arc<parking_lot::RwLock<Vec<CachedMatrix>>>,
}

#[derive(Debug)]
struct CachedMatrix {
    k: usize,
    m: usize,
    matrix: Vec<Vec<Gf256>>,
}

impl PureRustBackend {
    pub fn new() -> Self {
        Self {
            matrix_cache: Arc::new(parking_lot::RwLock::new(Vec::new())),
        }
    }
    
    fn get_or_create_matrix(&self, k: usize, m: usize) -> Vec<Vec<Gf256>> {
        // Check cache
        {
            let cache = self.matrix_cache.read();
            for cached in cache.iter() {
                if cached.k == k && cached.m == m {
                    return cached.matrix.clone();
                }
            }
        }
        
        // Generate new matrix
        let matrix = gf256::generate_cauchy_matrix(k, m);
        
        // Cache it
        {
            let mut cache = self.matrix_cache.write();
            cache.push(CachedMatrix {
                k,
                m,
                matrix: matrix.clone(),
            });
        }
        
        matrix
    }
    
    fn encode_systematic(&self, data_blocks: &[&[u8]], parity_out: &mut [Vec<u8>], k: usize, m: usize) -> Result<()> {
        if data_blocks.len() != k {
            return Err(FecError::InvalidParameters {
                k: data_blocks.len(),
                n: k + m,
            });
        }
        
        if parity_out.len() != m {
            return Err(FecError::InvalidParameters {
                k,
                n: k + parity_out.len(),
            });
        }
        
        let block_size = data_blocks[0].len();
        for block in data_blocks {
            if block.len() != block_size {
                return Err(FecError::SizeMismatch {
                    expected: block_size,
                    actual: block.len(),
                });
            }
        }
        
        let matrix = self.get_or_create_matrix(k, m);
        
        // Generate parity blocks using the bottom m rows of the matrix
        for (i, parity_block) in parity_out.iter_mut().enumerate() {
            parity_block.resize(block_size, 0);
            parity_block.fill(0);
            
            // Multiply row by data blocks
            for (j, data_block) in data_blocks.iter().enumerate() {
                let coefficient = matrix[k + i][j];
                if coefficient.0 != 0 {
                    let mut temp = vec![0u8; block_size];
                    gf256::mul_slice(&mut temp, data_block, coefficient);
                    gf256::add_slice(parity_block, &temp);
                }
            }
        }
        
        Ok(())
    }
    
    fn decode_systematic(&self, shares: &mut [Option<Vec<u8>>], k: usize) -> Result<()> {
        let n = shares.len();
        let m = n - k;
        
        // Find k available shares
        let mut available_indices = Vec::new();
        let mut available_data = Vec::new();
        
        for (i, share) in shares.iter().enumerate() {
            if let Some(data) = share {
                available_indices.push(i);
                available_data.push(data.clone());
                if available_indices.len() == k {
                    break;
                }
            }
        }
        
        if available_indices.len() < k {
            return Err(FecError::InsufficientShares {
                have: available_indices.len(),
                need: k,
            });
        }
        
        // Check if we have all data shares (fast path)
        let have_all_data = (0..k).all(|i| shares[i].is_some());
        if have_all_data {
            return Ok(()); // Nothing to decode
        }
        
        // Build decoding matrix from available shares
        let full_matrix = self.get_or_create_matrix(k, m);
        let mut decode_matrix = vec![vec![Gf256::ZERO; k]; k];
        
        for (i, &idx) in available_indices.iter().enumerate() {
            if idx < k {
                // Data share - copy from identity portion
                decode_matrix[i][idx] = Gf256::ONE;
            } else {
                // Parity share - copy from Cauchy portion
                for j in 0..k {
                    decode_matrix[i][j] = full_matrix[idx][j];
                }
            }
        }
        
        // Invert the decode matrix
        let inv_matrix = gf256::invert_matrix(&decode_matrix)
            .ok_or(FecError::SingularMatrix)?;
        
        // Reconstruct missing data shares
        let block_size = available_data[0].len();
        
        for data_idx in 0..k {
            if shares[data_idx].is_none() {
                let mut reconstructed = vec![0u8; block_size];
                
                // Find which row of inverse matrix to use
                let inv_row = &inv_matrix[data_idx];
                
                // Multiply by available shares
                for (i, data) in available_data.iter().enumerate() {
                    let coefficient = inv_row[i];
                    if coefficient.0 != 0 {
                        let mut temp = vec![0u8; block_size];
                        gf256::mul_slice(&mut temp, data, coefficient);
                        gf256::add_slice(&mut reconstructed, &temp);
                    }
                }
                
                shares[data_idx] = Some(reconstructed);
            }
        }
        
        Ok(())
    }
}

impl FecBackend for PureRustBackend {
    fn encode_blocks(&self, data: &[&[u8]], parity: &mut [Vec<u8>], params: FecParams) -> Result<()> {
        self.encode_systematic(
            data,
            parity,
            params.data_shares as usize,
            params.parity_shares as usize,
        )
    }
    
    fn decode_blocks(&self, shares: &mut [Option<Vec<u8>>], params: FecParams) -> Result<()> {
        self.decode_systematic(shares, params.data_shares as usize)
    }
    
    fn generate_matrix(&self, k: usize, m: usize) -> Vec<Vec<u8>> {
        let gf_matrix = self.get_or_create_matrix(k, m);
        gf_matrix
            .iter()
            .map(|row| row.iter().map(|elem| elem.0).collect())
            .collect()
    }
    
    fn name(&self) -> &'static str {
        "pure-rust-rs"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encode_decode_small() {
        let backend = PureRustBackend::new();
        let params = FecParams::new(3, 2).unwrap();
        
        // Create test data
        let data1 = vec![1, 2, 3, 4];
        let data2 = vec![5, 6, 7, 8];
        let data3 = vec![9, 10, 11, 12];
        let data_blocks: Vec<&[u8]> = vec![&data1, &data2, &data3];
        
        // Encode
        let mut parity = vec![vec![]; 2];
        backend.encode_blocks(&data_blocks, &mut parity, params).unwrap();
        
        assert_eq!(parity[0].len(), 4);
        assert_eq!(parity[1].len(), 4);
        
        // Create shares array with one missing data share
        let mut shares: Vec<Option<Vec<u8>>> = vec![
            None,                    // Missing first data share
            Some(data2.clone()),
            Some(data3.clone()),
            Some(parity[0].clone()),
            Some(parity[1].clone()),
        ];
        
        // Decode
        backend.decode_blocks(&mut shares, params).unwrap();
        
        // Verify reconstruction
        assert_eq!(shares[0].as_ref().unwrap(), &data1);
    }
    
    #[test]
    fn test_systematic_property() {
        let backend = PureRustBackend::new();
        let params = FecParams::new(4, 2).unwrap();
        
        // Create test data
        let data: Vec<Vec<u8>> = (0..4)
            .map(|i| vec![i as u8; 100])
            .collect();
        let data_refs: Vec<&[u8]> = data.iter().map(|v| v.as_slice()).collect();
        
        // Encode
        let mut parity = vec![vec![]; 2];
        backend.encode_blocks(&data_refs, &mut parity, params).unwrap();
        
        // Verify we can reconstruct from any 4 shares
        for missing_idx in 0..6 {
            let mut shares: Vec<Option<Vec<u8>>> = (0..6).map(|i| {
                if i == missing_idx {
                    None
                } else if i < 4 {
                    Some(data[i].clone())
                } else {
                    Some(parity[i - 4].clone())
                }
            }).collect();
            
            backend.decode_blocks(&mut shares, params).unwrap();
            
            // Verify all data shares are reconstructed
            for i in 0..4 {
                assert_eq!(shares[i].as_ref().unwrap(), &data[i]);
            }
        }
    }
}