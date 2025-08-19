//! v0.3 API demonstration
//!
//! Shows how to use the new StoragePipeline API with builder pattern configuration

use anyhow::Result;
use saorsa_fec::{Config, EncryptionMode, Meta, StoragePipeline};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<()> {
    // Create temporary directory for storage
    let temp_dir = TempDir::new()?;
    let storage_path = temp_dir.path().to_path_buf();

    // Create storage backend
    let backend = saorsa_fec::storage::LocalStorage::new(storage_path).await?;

    // Configure pipeline using v0.3 builder pattern
    let config = Config::default()
        .with_encryption_mode(EncryptionMode::Convergent)
        .with_fec_params(16, 4) // 16 data shards, 4 parity shards (25% overhead)
        .with_chunk_size(64 * 1024) // 64 KiB chunks as specified
        .with_compression(true, 6); // Enable compression level 6

    // Create storage pipeline
    let mut pipeline = StoragePipeline::new(config, backend).await?;

    // Prepare test data
    let file_id = [42u8; 32];
    let test_data = b"This is test data for the v0.3 StoragePipeline API demonstration!";

    // Create metadata
    let meta = Some(
        Meta::new()
            .with_filename("test_file.txt")
            .with_author("v0.3 Demo"),
    );

    println!("Processing file with {} bytes...", test_data.len());

    // Process the file
    let file_metadata = pipeline.process_file(file_id, test_data, meta).await?;

    println!("File processed successfully!");
    println!("- File ID: {:?}", hex::encode(file_metadata.file_id));
    println!("- Original size: {} bytes", file_metadata.file_size);
    println!("- Number of chunks: {}", file_metadata.chunks.len());

    if let Some(local_meta) = &file_metadata.local_metadata {
        if let Some(filename) = &local_meta.filename {
            println!("- Filename: {}", filename);
        }
        if let Some(author) = &local_meta.author {
            println!("- Author: {}", author);
        }
    }

    // Retrieve the file
    println!("\nRetrieving file...");
    let retrieved_data = pipeline.retrieve_file(&file_metadata).await?;

    // Verify data integrity
    if retrieved_data == test_data {
        println!("✅ Data integrity verified! Original and retrieved data match.");
    } else {
        println!("❌ Data integrity check failed!");
        return Err(anyhow::anyhow!("Data mismatch"));
    }

    // Show pipeline statistics
    let stats = pipeline.stats();
    println!("\nPipeline Statistics:");
    println!("- Total chunks: {}", stats.total_chunks);
    println!("- Total size: {} bytes", stats.total_size);
    println!("- Encryption mode: {:?}", stats.encryption_mode);
    println!(
        "- FEC parameters: {} data + {} parity shards",
        stats.fec_params.0, stats.fec_params.1
    );

    println!("\n🎉 v0.3 API demonstration completed successfully!");

    Ok(())
}
