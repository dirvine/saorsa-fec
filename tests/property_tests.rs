// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Property-based tests for FEC implementation

use proptest::prelude::*;
use saorsa_fec::{
    backends::pure_rust::PureRustBackend,
    FecBackend, FecParams,
};
use std::collections::HashSet;

/// Generate valid FEC parameters
fn fec_params_strategy() -> impl Strategy<Value = FecParams> {
    (2u16..=20, 1u16..=10)
        .prop_filter("k + m <= 255", |(k, m)| k + m <= 255)
        .prop_map(|(k, m)| FecParams::new(k, m).unwrap())
}

/// Generate test data of various sizes
fn test_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 100..=10000)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    
    #[test]
    fn encode_decode_roundtrip(
        params in fec_params_strategy(),
        data in test_data_strategy(),
    ) {
        let backend = PureRustBackend::new();
        let k = params.data_shares as usize;
        let m = params.parity_shares as usize;
        
        // Split data into k blocks
        let block_size = (data.len() + k - 1) / k;
        let mut blocks = vec![vec![0u8; block_size]; k];
        
        for (i, chunk) in data.chunks(block_size).enumerate() {
            if i < k {
                blocks[i][..chunk.len()].copy_from_slice(chunk);
            }
        }
        
        let block_refs: Vec<&[u8]> = blocks.iter().map(|v| v.as_slice()).collect();
        
        // Encode
        let mut parity = vec![vec![]; m];
        backend.encode_blocks(&block_refs, &mut parity, params).unwrap();
        
        // Create full shares array
        let mut shares: Vec<Option<Vec<u8>>> = blocks.into_iter().map(Some).collect();
        shares.extend(parity.into_iter().map(Some));
        
        // Try decoding with exactly k shares (any combination)
        let indices: Vec<usize> = (0..k+m).collect();
        for combo in indices.windows(k) {
            let mut test_shares = vec![None; k + m];
            for &idx in combo {
                test_shares[idx] = shares[idx].clone();
            }
            
            backend.decode_blocks(&mut test_shares, params).unwrap();
            
            // Verify data blocks match
            for i in 0..k {
                assert_eq!(test_shares[i], shares[i]);
            }
        }
    }
    
    #[test]
    fn systematic_encoding_preserves_data(
        params in fec_params_strategy(),
        data in test_data_strategy(),
    ) {
        let backend = PureRustBackend::new();
        let k = params.data_shares as usize;
        let m = params.parity_shares as usize;
        
        // Create data blocks
        let block_size = (data.len() + k - 1) / k;
        let mut blocks = vec![vec![0u8; block_size]; k];
        
        for (i, chunk) in data.chunks(block_size).enumerate() {
            if i < k {
                blocks[i][..chunk.len()].copy_from_slice(chunk);
            }
        }
        
        let original_blocks = blocks.clone();
        let block_refs: Vec<&[u8]> = blocks.iter().map(|v| v.as_slice()).collect();
        
        // Encode
        let mut parity = vec![vec![]; m];
        backend.encode_blocks(&block_refs, &mut parity, params).unwrap();
        
        // Verify data blocks are unchanged (systematic property)
        for i in 0..k {
            assert_eq!(blocks[i], original_blocks[i], 
                "Systematic encoding should not modify data blocks");
        }
    }
    
    #[test]
    fn deterministic_parity_generation(
        params in fec_params_strategy(),
        data in test_data_strategy(),
        _seed in any::<u64>(),
    ) {
        let backend = PureRustBackend::new();
        let k = params.data_shares as usize;
        let m = params.parity_shares as usize;
        
        // Create data blocks
        let block_size = (data.len() + k - 1) / k;
        let mut blocks = vec![vec![0u8; block_size]; k];
        
        for (i, chunk) in data.chunks(block_size).enumerate() {
            if i < k {
                blocks[i][..chunk.len()].copy_from_slice(chunk);
            }
        }
        
        let block_refs: Vec<&[u8]> = blocks.iter().map(|v| v.as_slice()).collect();
        
        // Generate parity twice with same data
        let mut parity1 = vec![vec![]; m];
        backend.encode_blocks(&block_refs, &mut parity1, params).unwrap();
        
        let mut parity2 = vec![vec![]; m];
        backend.encode_blocks(&block_refs, &mut parity2, params).unwrap();
        
        // Verify parity is identical
        for i in 0..m {
            assert_eq!(parity1[i], parity2[i], 
                "Parity generation should be deterministic");
        }
    }
    
    #[test]
    fn any_k_shares_reconstruct(
        params in fec_params_strategy(),
        missing_indices in prop::collection::vec(0usize..255, 0..=10),
    ) {
        let backend = PureRustBackend::new();
        let k = params.data_shares as usize;
        let m = params.parity_shares as usize;
        let n = k + m;
        
        // Create simple test data
        let data: Vec<Vec<u8>> = (0..k)
            .map(|i| vec![i as u8; 100])
            .collect();
        let data_refs: Vec<&[u8]> = data.iter().map(|v| v.as_slice()).collect();
        
        // Encode
        let mut parity = vec![vec![]; m];
        backend.encode_blocks(&data_refs, &mut parity, params).unwrap();
        
        // Create set of missing indices (up to m shares)
        let missing: HashSet<usize> = missing_indices.into_iter()
            .filter(|&i| i < n)
            .take(m)
            .collect();
        
        // Create shares with missing data
        let mut shares: Vec<Option<Vec<u8>>> = (0..n).map(|i| {
            if missing.contains(&i) {
                None
            } else if i < k {
                Some(data[i].clone())
            } else {
                Some(parity[i - k].clone())
            }
        }).collect();
        
        // Decode
        backend.decode_blocks(&mut shares, params).unwrap();
        
        // Verify all data shares are reconstructed
        for i in 0..k {
            assert_eq!(shares[i].as_ref().unwrap(), &data[i],
                "Share {} should be correctly reconstructed", i);
        }
    }
    
    #[test]
    fn insufficient_shares_fails(
        params in fec_params_strategy(),
    ) {
        let backend = PureRustBackend::new();
        let k = params.data_shares as usize;
        let m = params.parity_shares as usize;
        let n = k + m;
        
        // Create minimal test data
        let data: Vec<Vec<u8>> = (0..k)
            .map(|i| vec![i as u8; 10])
            .collect();
        let data_refs: Vec<&[u8]> = data.iter().map(|v| v.as_slice()).collect();
        
        // Encode
        let mut parity = vec![vec![]; m];
        backend.encode_blocks(&data_refs, &mut parity, params).unwrap();
        
        // Create shares with too many missing (k-1 available)
        let mut shares: Vec<Option<Vec<u8>>> = vec![None; n];
        for i in 0..k-1 {
            shares[i] = Some(data[i].clone());
        }
        
        // Decode should fail
        let result = backend.decode_blocks(&mut shares, params);
        assert!(result.is_err(), "Decoding with insufficient shares should fail");
    }
}