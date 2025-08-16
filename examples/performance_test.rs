// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Performance test for reed-solomon-simd implementation

use saorsa_fec::{FecBackend, FecParams, backends::pure_rust::PureRustBackend};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Reed-Solomon SIMD Performance Test");
    println!("===================================");

    let backend = PureRustBackend::new();
    println!("Using backend: {}", backend.name());
    println!("SIMD accelerated: {}", backend.is_accelerated());
    
    // Report CPU features
    println!("CPU features:");
    println!("  AVX2: {}", cfg!(target_feature = "avx2"));
    println!("  AVX: {}", cfg!(target_feature = "avx"));
    println!("  SSE4.1: {}", cfg!(target_feature = "sse4.1"));
    println!("  NEON: {}", cfg!(target_feature = "neon"));

    // Test different file sizes
    for &size in &[1_000_000, 10_000_000, 50_000_000] {
        let params = FecParams::from_content_size(size);
        let k = params.data_shares as usize;
        let m = params.parity_shares as usize;

        // Ensure even block size for reed-solomon-simd
        let block_size = (size / k) & !1;
        let actual_size = block_size * k;
        
        println!("\nTesting {}MB file (actual: {:.2}MB, {}/{})", 
                 size / 1_000_000, 
                 actual_size as f64 / 1_000_000.0,
                 k, k + m);

        // Create test data
        let data: Vec<Vec<u8>> = (0..k).map(|_| vec![0u8; block_size]).collect();
        let data_refs: Vec<&[u8]> = data.iter().map(|v| v.as_slice()).collect();

        // Benchmark encoding
        let start = Instant::now();
        let mut parity = vec![vec![]; m];
        
        for _ in 0..5 {
            backend.encode_blocks(&data_refs, &mut parity, params)?;
        }
        
        let duration = start.elapsed();
        let avg_duration = duration / 5;
        let throughput = (actual_size as f64) / avg_duration.as_secs_f64() / 1_000_000.0;
        
        println!("  Encoding: {:.2} MB/s ({:.2}ms)", throughput, avg_duration.as_millis());

        // Verify parity was generated
        assert!(!parity[0].is_empty(), "Parity data should be generated");
        assert_eq!(parity[0].len(), block_size, "Parity block size should match");
    }

    println!("\nTest completed successfully!");
    Ok(())
}