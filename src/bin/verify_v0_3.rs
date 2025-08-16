//! Quick verification that v0.3 API compiles and basic functionality works

use anyhow::Result;
use saorsa_fec::{Config, EncryptionMode, LocalStorage, Meta, StoragePipeline};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Verifying v0.3 API implementation...");

    // Test Config builder pattern
    let config = Config::default()
        .with_encryption_mode(EncryptionMode::Convergent)
        .with_fec_params(8, 2)
        .with_chunk_size(32 * 1024)
        .with_compression(true, 6);

    println!("✅ Config builder pattern works");
    println!("   - Encryption mode: {:?}", config.encryption_mode);
    println!(
        "   - FEC params: {} data + {} parity",
        config.data_shards, config.parity_shards
    );
    println!("   - Chunk size: {} KB", config.chunk_size / 1024);
    println!(
        "   - Compression: {} (level {})",
        config.compression_enabled, config.compression_level
    );

    // Test StoragePipeline creation
    let temp_dir = tempfile::TempDir::new()?;
    let backend = LocalStorage::new(temp_dir.path().to_path_buf()).await?;
    let pipeline = StoragePipeline::new(config, backend).await?;

    println!("✅ StoragePipeline creation works");

    // Test Meta creation
    let meta = Meta::new()
        .with_filename("test.txt")
        .with_author("Verification Script");

    println!("✅ Meta builder pattern works");
    println!("   - Filename: {:?}", meta.filename);
    println!("   - Author: {:?}", meta.author);

    // Test stats
    let stats = pipeline.stats();
    println!("✅ Pipeline statistics work");
    println!("   - Total chunks: {}", stats.total_chunks);
    println!("   - Encryption mode: {:?}", stats.encryption_mode);
    println!("   - FEC params: {:?}", stats.fec_params);

    println!("\n🎉 v0.3 API verification completed successfully!");
    println!("   All core components compile and basic functionality works.");

    Ok(())
}
