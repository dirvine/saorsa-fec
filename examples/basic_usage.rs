// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Basic usage example for Saorsa FEC

use saorsa_fec::{FecBackend, FecParams, backends::pure_rust::PureRustBackend};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a simple example with 4 data blocks and 2 parity blocks
    let k = 4; // data blocks
    let m = 2; // parity blocks
    let block_size = 1024;

    // Create test data
    let data: Vec<Vec<u8>> = (0..k).map(|i| vec![i as u8; block_size]).collect();

    let data_refs: Vec<&[u8]> = data.iter().map(|v| v.as_slice()).collect();

    // Create backend and parameters
    let backend = PureRustBackend::new();
    let params = FecParams::new(k as u16, m as u16).unwrap();

    // Encode: create parity blocks
    let mut parity = vec![vec![]; m];
    backend.encode_blocks(&data_refs, &mut parity, params)?;

    println!("Created {} data blocks and {} parity blocks", k, m);
    println!("Each block is {} bytes", block_size);

    // Simulate losing one data block
    let mut shares: Vec<Option<Vec<u8>>> = vec![None; k + m];
    shares[0] = None; // Lost first data block
    for i in 1..k {
        shares[i] = Some(data[i].clone());
    }
    for i in 0..m {
        shares[k + i] = Some(parity[i].clone());
    }

    println!("Lost data block 0, attempting reconstruction...");

    // Reconstruct the missing block
    backend.decode_blocks(&mut shares, params)?;

    // Verify reconstruction
    if let Some(ref reconstructed) = shares[0] {
        if reconstructed == &data[0] {
            println!("✓ Successfully reconstructed lost data block!");
        } else {
            println!("✗ Reconstruction failed - data mismatch");
        }
    } else {
        println!("✗ Reconstruction failed - no data returned");
    }

    Ok(())
}
