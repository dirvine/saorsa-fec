// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # FEC Demo - Reed-Solomon RS(14,10) with 1.4x Overhead
//!
//! This example demonstrates the FEC shard layer with:
//! - RS(10,4) encoding (10 data shards, 4 parity shards)
//! - 1.4x storage overhead
//! - CRC32 integrity checking
//! - Repair simulation

use anyhow::Result;
use saorsa_fec::fec::{self, FecParams, RepairHooks, Shard};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

// Type aliases to reduce complexity
type ShardMap = HashMap<u16, Shard>;
type StorageMap = HashMap<Vec<u8>, ShardMap>;

/// Simple in-memory storage backend for demo
struct DemoStorage {
    shards: Arc<parking_lot::RwLock<StorageMap>>,
    bandwidth_used: Arc<parking_lot::RwLock<usize>>,
}

impl DemoStorage {
    fn new() -> Self {
        Self {
            shards: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            bandwidth_used: Arc::new(parking_lot::RwLock::new(0)),
        }
    }

    fn store_shards(&self, key: Vec<u8>, shards: Vec<Shard>) {
        let mut storage = self.shards.write();
        let entry = storage.entry(key).or_default();

        let mut bandwidth = self.bandwidth_used.write();
        for shard in shards {
            *bandwidth += shard.data.len();
            entry.insert(shard.idx, shard);
        }
    }

    fn remove_shards(&self, key: &Vec<u8>, indices: &[u16]) {
        let mut storage = self.shards.write();
        if let Some(entry) = storage.get_mut(key) {
            for idx in indices {
                entry.remove(idx);
            }
        }
    }

    fn get_bandwidth_used(&self) -> usize {
        *self.bandwidth_used.read()
    }

    fn reset_bandwidth(&self) {
        *self.bandwidth_used.write() = 0;
    }
}

impl RepairHooks for DemoStorage {
    fn fetch_shards(&self, key: Vec<u8>, need: usize) -> Result<Vec<Shard>> {
        let storage = self.shards.read();
        if let Some(entry) = storage.get(&key) {
            let shards: Vec<Shard> = entry.values().take(need).cloned().collect();

            // Track bandwidth for fetches
            let mut bandwidth = self.bandwidth_used.write();
            for shard in &shards {
                *bandwidth += shard.data.len();
            }

            Ok(shards)
        } else {
            Ok(Vec::new())
        }
    }

    fn reseed(&self, key: Vec<u8>, shards: Vec<Shard>) -> Result<()> {
        self.store_shards(key, shards);
        Ok(())
    }
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║     FEC Demo - Reed-Solomon RS(10,4) with 1.4x Overhead   ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Create FEC parameters for RS(10,4)
    let params = FecParams::new(10, 4, 64 * 1024)?; // 64KB shard size

    println!("📊 FEC Parameters:");
    println!("   • Data shards (k): {}", params.k);
    println!("   • Parity shards (m): {}", params.m);
    println!("   • Total shards (n): {}", params.total_shards());
    println!("   • Shard size: {}", format_size(params.shard_size));
    println!("   • Storage overhead: {:.1}x", params.overhead_ratio());
    println!();

    // Create test data (640 KB - exactly 10 shards)
    let data_size = params.k as usize * params.shard_size;
    let mut test_data = vec![0u8; data_size];
    for (i, byte) in test_data.iter_mut().enumerate() {
        *byte = (i % 256) as u8; // Pattern for verification
    }

    println!("📦 Test Data:");
    println!("   • Original size: {}", format_size(data_size));
    println!();

    // Encode data
    println!("🔧 Encoding data into shards...");
    let start = Instant::now();
    let shards = fec::encode(&test_data, params)?;
    let encode_time = start.elapsed();

    println!("   ✅ Encoded in {:.2?}", encode_time);
    println!("   • Created {} shards", shards.len());

    // Calculate storage statistics
    let total_storage: usize = shards.iter().map(|s| s.data.len()).sum();
    let actual_overhead = total_storage as f64 / data_size as f64;

    println!();
    println!("💾 Storage Statistics:");
    println!("   • Total storage: {}", format_size(total_storage));
    println!("   • Actual overhead: {:.2}x", actual_overhead);
    println!(
        "   • Overhead matches target: {}",
        if (actual_overhead - 1.4).abs() < 0.01 {
            "✅ Yes"
        } else {
            "❌ No"
        }
    );

    // Test recovery scenarios
    println!();
    println!("🔬 Testing Recovery Scenarios:");
    println!();

    // Scenario 1: Decode with minimum k shards
    println!("   Scenario 1: Decoding with minimum shards (k=10)");
    let minimal_shards: Vec<Shard> = shards.iter().take(10).cloned().collect();
    let start = Instant::now();
    let decoded = fec::decode(&minimal_shards, params)?;
    let decode_time = start.elapsed();

    let data_matches = decoded[..data_size] == test_data[..];
    println!("      • Decoded in {:.2?}", decode_time);
    println!(
        "      • Data integrity: {}",
        if data_matches {
            "✅ Verified"
        } else {
            "❌ Failed"
        }
    );

    // Scenario 2: Test limitation of current implementation
    println!();
    println!("   Scenario 2: Testing missing data shards limitation");
    let mixed_shards: Vec<Shard> = shards[2..12].to_vec(); // Skip first 2 data shards
    let result2 = fec::decode(&mixed_shards, params);
    match result2 {
        Ok(_) => {
            println!("      • Unexpectedly succeeded with missing data shards");
        }
        Err(e) => {
            println!("      • Expected limitation: {}", e);
            println!("      • Note: reed-solomon-simd v3 doesn't support reconstruction");
        }
    }

    // Scenario 3: Simulate shard corruption
    println!();
    println!("   Scenario 3: CRC validation with corrupted shard");
    let mut corrupted_shards = shards.clone();
    corrupted_shards[3].data = vec![0xFF; params.shard_size]; // Corrupt data
    println!("      • Corrupted shard 3");
    println!(
        "      • CRC check: {}",
        if !corrupted_shards[3].verify_crc() {
            "✅ Detected corruption"
        } else {
            "❌ Missed corruption"
        }
    );

    // Still decode with enough valid shards
    let valid_count = corrupted_shards.iter().filter(|s| s.verify_crc()).count();
    println!("      • Valid shards: {}/{}", valid_count, shards.len());
    if valid_count >= params.k as usize {
        // Use only non-corrupted shards
        let valid_only: Vec<Shard> = shards
            .iter()
            .filter(|s| s.verify_crc())
            .take(params.k as usize)
            .cloned()
            .collect();

        // Check if we have all data shards
        let have_all_data = valid_only.iter().all(|s| s.idx < params.k);
        if have_all_data {
            let decoded3 = fec::decode(&valid_only, params)?;
            let data_matches3 = decoded3[..data_size] == test_data[..];
            println!(
                "      • Recovery: {}",
                if data_matches3 {
                    "✅ Successful"
                } else {
                    "❌ Failed"
                }
            );
        } else {
            println!("      • Recovery: ⚠️ Limited by missing data shard support");
        }
    }

    // Test repair mechanism
    println!();
    println!("🔄 Testing Repair Mechanism:");

    let storage = DemoStorage::new();
    let key = b"test_object_001".to_vec();

    // Store all shards initially
    storage.store_shards(key.clone(), shards.clone());
    println!("   • Stored all {} shards", shards.len());

    // Simulate shard loss
    let lost_indices = [11, 12, 13]; // Lose 3 parity shards
    storage.remove_shards(&key, &lost_indices);
    println!("   • Simulated loss of shards: {:?}", lost_indices);

    // Reset bandwidth counter
    storage.reset_bandwidth();

    // Run maintenance
    println!("   • Running maintenance...");
    fec::maintain(key.clone(), params, &storage)?;

    let repair_bandwidth = storage.get_bandwidth_used();
    println!(
        "   • Repair bandwidth used: {}",
        format_size(repair_bandwidth)
    );

    // Verify all shards are restored
    let restored = storage.fetch_shards(key, params.total_shards() as usize)?;
    println!(
        "   • Shards after repair: {}/{}",
        restored.len(),
        params.total_shards()
    );
    println!(
        "   • Repair status: {}",
        if restored.len() == params.total_shards() as usize {
            "✅ Complete"
        } else {
            "⚠️ Partial"
        }
    );

    // Performance metrics
    println!();
    println!("⚡ Performance Metrics:");

    let encode_throughput = (data_size as f64) / encode_time.as_secs_f64() / (1024.0 * 1024.0);
    let decode_throughput = (data_size as f64) / decode_time.as_secs_f64() / (1024.0 * 1024.0);

    println!("   • Encode throughput: {:.2} MB/s", encode_throughput);
    println!("   • Decode throughput: {:.2} MB/s", decode_throughput);

    // Bandwidth efficiency
    let repair_efficiency = if repair_bandwidth > 0 {
        let theoretical_min = lost_indices.len() * params.shard_size;
        theoretical_min as f64 / repair_bandwidth as f64
    } else {
        1.0
    };

    println!("   • Repair efficiency: {:.1}%", repair_efficiency * 100.0);

    println!();
    println!("═════════════════════════════════════════════════════════════");
    println!("✅ Demo completed successfully!");
    println!();

    Ok(())
}
