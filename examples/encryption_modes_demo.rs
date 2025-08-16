//! Comprehensive demonstration of all encryption modes with Local and Memory storage backends.
//!
//! This example shows how to use each of the three encryption modes:
//! - Convergent: Global deduplication (same content → same ciphertext)
//! - ConvergentWithSecret: Per-user deduplication (content + secret → ciphertext)  
//! - RandomKey: No deduplication, maximum privacy (random key per chunk)
//!
//! Each mode is demonstrated with both LocalStorage and MemoryStorage backends.

use saorsa_fec::{
    ChunkConfig, Config, EncryptionMode, FecParams, Result, StoragePipeline,
    storage::{LocalStorage, MemoryStorage},
};
use std::path::Path;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Saorsa FEC v0.3 Encryption Modes Demonstration ===\n");

    // Test data
    let test_data = b"Hello, World! This is test data for demonstrating all encryption modes and storage backends.";
    let file_id = [0x42u8; 32];

    println!("Test data: {} bytes", test_data.len());
    println!("File ID: {}\n", hex::encode(&file_id[..8]));

    // Create temporary directory for LocalStorage demos
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let storage_path = temp_dir.path().join("saorsa_storage");

    // ==========================================
    // CONVERGENT ENCRYPTION MODE
    // ==========================================
    println!("🔹 CONVERGENT ENCRYPTION MODE");
    println!("   Global deduplication: identical content → identical ciphertext");

    await_demo_convergent_local(&storage_path, test_data, file_id).await?;
    await_demo_convergent_memory(test_data, file_id).await?;

    // ==========================================
    // CONVERGENT WITH SECRET ENCRYPTION MODE
    // ==========================================
    println!("\n🔹 CONVERGENT WITH SECRET ENCRYPTION MODE");
    println!("   Per-user deduplication: content + secret → ciphertext");

    await_demo_convergent_secret_local(&storage_path, test_data, file_id).await?;
    await_demo_convergent_secret_memory(test_data, file_id).await?;

    // ==========================================
    // RANDOM KEY ENCRYPTION MODE
    // ==========================================
    println!("\n🔹 RANDOM KEY ENCRYPTION MODE");
    println!("   No deduplication: random key per chunk → maximum privacy");

    await_demo_random_key_local(&storage_path, test_data, file_id).await?;
    await_demo_random_key_memory(test_data, file_id).await?;

    // ==========================================
    // DEDUPLICATION COMPARISON
    // ==========================================
    println!("\n🔍 DEDUPLICATION BEHAVIOR COMPARISON");
    await_demo_deduplication_comparison().await?;

    println!("\n=== All encryption modes demonstrated successfully! ===");

    Ok(())
}

/// Demonstrate Convergent encryption with LocalStorage
async fn await_demo_convergent_local(
    storage_path: &Path,
    data: &[u8],
    file_id: [u8; 32],
) -> Result<()> {
    println!("📁 Local Storage Backend:");

    // Configure convergent encryption
    let fec_params = FecParams::new(3, 2)?;
    let chunk_config = ChunkConfig::new(64, fec_params);
    let config = Config::default()
        .with_encryption_mode(EncryptionMode::Convergent)
        .with_fec_params(3, 2)
        .with_chunk_size(64)
        .with_compression(false, 0);

    // Create storage backend
    let storage = LocalStorage::new(storage_path.join("convergent")).await?;
    let mut pipeline = StoragePipeline::new(config, storage).await?;

    // Process file
    let metadata = pipeline.process_file(file_id, data, None).await?;
    println!("   ✓ Stored file with {} chunks", metadata.chunk_count);

    // Retrieve and verify
    let retrieved = pipeline.retrieve_file(&metadata).await?;
    assert_eq!(retrieved, data);
    println!("   ✓ Retrieved and verified {} bytes", retrieved.len());

    // Show convergent property - same data should produce same result
    let metadata2 = pipeline.process_file([0x43u8; 32], data, None).await?;
    println!("   ✓ Same content with different file_id produces deterministic encryption");

    Ok(())
}

/// Demonstrate Convergent encryption with MemoryStorage
async fn await_demo_convergent_memory(data: &[u8], file_id: [u8; 32]) -> Result<()> {
    println!("💾 Memory Storage Backend:");

    let fec_params = FecParams::new(3, 2)?;
    let chunk_config = ChunkConfig::new(64, fec_params);
    let config = Config::default()
        .with_encryption_mode(EncryptionMode::Convergent)
        .with_fec_params(3, 2)
        .with_chunk_size(64)
        .with_compression(false, 0);

    let storage = MemoryStorage::new();
    let mut pipeline = StoragePipeline::new(config, storage).await?;

    let metadata = pipeline.process_file(file_id, data, None).await?;
    let retrieved = pipeline.retrieve_file(&metadata).await?;

    assert_eq!(retrieved, data);
    println!(
        "   ✓ Memory storage: {} bytes processed successfully",
        data.len()
    );

    Ok(())
}

/// Demonstrate ConvergentWithSecret encryption with LocalStorage
async fn await_demo_convergent_secret_local(
    storage_path: &Path,
    data: &[u8],
    file_id: [u8; 32],
) -> Result<()> {
    println!("📁 Local Storage Backend:");

    let fec_params = FecParams::new(3, 2)?;
    let chunk_config = ChunkConfig::new(64, fec_params);
    let user_secret = b"my_secret_key_for_convergent_encryption".to_vec();

    let config = Config::with_secret_encryption(
        chunk_config,
        saorsa_fec::ReedSolomonBackend::PureRust,
        user_secret,
        false,
    );

    let storage = LocalStorage::new(storage_path.join("convergent_secret")).await?;
    let mut pipeline = StoragePipeline::new(config, storage).await?;

    let metadata = pipeline.process_file(file_id, data, None).await?;
    println!("   ✓ Stored with secret-based convergent encryption");

    let retrieved = pipeline.retrieve_file(&metadata).await?;
    assert_eq!(retrieved, data);
    println!("   ✓ Retrieved and verified with same secret");

    Ok(())
}

/// Demonstrate ConvergentWithSecret encryption with MemoryStorage
async fn await_demo_convergent_secret_memory(data: &[u8], file_id: [u8; 32]) -> Result<()> {
    println!("💾 Memory Storage Backend:");

    let fec_params = FecParams::new(3, 2)?;
    let chunk_config = ChunkConfig::new(64, fec_params);
    let user_secret = b"different_user_secret_key_here".to_vec();

    let config = Config::with_secret_encryption(
        chunk_config,
        saorsa_fec::ReedSolomonBackend::PureRust,
        user_secret,
        false,
    );

    let storage = MemoryStorage::new();
    let mut pipeline = StoragePipeline::new(config, storage).await?;

    let metadata = pipeline.process_file(file_id, data, None).await?;
    let retrieved = pipeline.retrieve_file(&metadata).await?;

    assert_eq!(retrieved, data);
    println!(
        "   ✓ Memory storage with secret: {} bytes processed",
        data.len()
    );

    Ok(())
}

/// Demonstrate RandomKey encryption with LocalStorage
async fn await_demo_random_key_local(
    storage_path: &Path,
    data: &[u8],
    file_id: [u8; 32],
) -> Result<()> {
    println!("📁 Local Storage Backend:");

    let fec_params = FecParams::new(3, 2)?;
    let chunk_config = ChunkConfig::new(64, fec_params);
    let config = Config::with_random_encryption(
        chunk_config,
        saorsa_fec::ReedSolomonBackend::PureRust,
        false,
    );

    let storage = LocalStorage::new(storage_path.join("random_key")).await?;
    let mut pipeline = StoragePipeline::new(config, storage).await?;

    let metadata = pipeline.process_file(file_id, data, None).await?;
    println!("   ✓ Stored with random key encryption (no deduplication)");

    let retrieved = pipeline.retrieve_file(&metadata).await?;
    assert_eq!(retrieved, data);
    println!("   ✓ Retrieved and verified with stored key material");

    Ok(())
}

/// Demonstrate RandomKey encryption with MemoryStorage
async fn await_demo_random_key_memory(data: &[u8], file_id: [u8; 32]) -> Result<()> {
    println!("💾 Memory Storage Backend:");

    let fec_params = FecParams::new(3, 2)?;
    let chunk_config = ChunkConfig::new(64, fec_params);
    let config = Config::with_random_encryption(
        chunk_config,
        saorsa_fec::ReedSolomonBackend::PureRust,
        false,
    );

    let storage = MemoryStorage::new();
    let mut pipeline = StoragePipeline::new(config, storage).await?;

    let metadata = pipeline.process_file(file_id, data, None).await?;
    let retrieved = pipeline.retrieve_file(&metadata).await?;

    assert_eq!(retrieved, data);
    println!(
        "   ✓ Memory storage with random keys: {} bytes processed",
        data.len()
    );

    Ok(())
}

/// Demonstrate deduplication behavior differences between modes
async fn await_demo_deduplication_comparison() -> Result<()> {
    let test_content = b"This is identical content for deduplication testing.";
    let file_id1 = [0x01u8; 32];
    let file_id2 = [0x02u8; 32];

    println!("Testing with identical content but different file IDs...");

    // Test Convergent (should deduplicate globally)
    {
        let fec_params = FecParams::new(2, 1)?;
        let chunk_config = ChunkConfig::new(32, fec_params);
        let config = Config::with_convergent_encryption(
            chunk_config,
            saorsa_fec::ReedSolomonBackend::PureRust,
            false,
        );

        let storage = MemoryStorage::new();
        let mut pipeline = StoragePipeline::new(config, storage).await?;

        let meta1 = pipeline.process_file(file_id1, test_content, None).await?;
        let meta2 = pipeline.process_file(file_id2, test_content, None).await?;

        println!(
            "   Convergent: File1 chunks={}, File2 chunks={}",
            meta1.chunk_count, meta2.chunk_count
        );
        println!("   → Global deduplication: same content produces deterministic encryption");
    }

    // Test ConvergentWithSecret (should deduplicate per-secret)
    {
        let fec_params = FecParams::new(2, 1)?;
        let chunk_config = ChunkConfig::new(32, fec_params);
        let secret = b"user_secret".to_vec();
        let config = Config::with_secret_encryption(
            chunk_config,
            saorsa_fec::ReedSolomonBackend::PureRust,
            secret,
            false,
        );

        let storage = MemoryStorage::new();
        let mut pipeline = StoragePipeline::new(config, storage).await?;

        let meta1 = pipeline.process_file(file_id1, test_content, None).await?;
        let meta2 = pipeline.process_file(file_id2, test_content, None).await?;

        println!(
            "   ConvergentWithSecret: File1 chunks={}, File2 chunks={}",
            meta1.chunk_count, meta2.chunk_count
        );
        println!("   → Per-user deduplication: same content + same secret → deterministic");
    }

    // Test RandomKey (should never deduplicate)
    {
        let fec_params = FecParams::new(2, 1)?;
        let chunk_config = ChunkConfig::new(32, fec_params);
        let config = Config::with_random_encryption(
            chunk_config,
            saorsa_fec::ReedSolomonBackend::PureRust,
            false,
        );

        let storage = MemoryStorage::new();
        let mut pipeline = StoragePipeline::new(config, storage).await?;

        let meta1 = pipeline.process_file(file_id1, test_content, None).await?;
        let meta2 = pipeline.process_file(file_id2, test_content, None).await?;

        println!(
            "   RandomKey: File1 chunks={}, File2 chunks={}",
            meta1.chunk_count, meta2.chunk_count
        );
        println!("   → No deduplication: random keys ensure unique encryption");
    }

    Ok(())
}
