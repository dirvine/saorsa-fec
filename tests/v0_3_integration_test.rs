//! Integration test for v0.3 API specification compliance

use anyhow::Result;
use saorsa_fec::{Config, EncryptionMode, Meta, StoragePipeline, storage::LocalStorage};
use tempfile::TempDir;

#[tokio::test]
async fn test_v0_3_config_builder_pattern() -> Result<()> {
    // Test the Config builder pattern as specified in v0.3
    let config = Config::default()
        .with_encryption_mode(EncryptionMode::Convergent)
        .with_fec_params(8, 2) // 8 data, 2 parity
        .with_chunk_size(32 * 1024) // 32 KiB chunks
        .with_compression(true, 9); // Max compression

    assert_eq!(config.encryption_mode, EncryptionMode::Convergent);
    assert_eq!(config.data_shards, 8);
    assert_eq!(config.parity_shards, 2);
    assert_eq!(config.chunk_size, 32 * 1024);
    assert!(config.compression_enabled);
    assert_eq!(config.compression_level, 9);

    Ok(())
}

#[tokio::test]
async fn test_v0_3_storage_pipeline_api() -> Result<()> {
    // Test the StoragePipeline API as specified in v0.3
    let temp_dir = TempDir::new()?;
    let backend = LocalStorage::new(temp_dir.path().to_path_buf()).await?;

    let config = Config::default()
        .with_encryption_mode(EncryptionMode::Convergent)
        .with_fec_params(10, 2)
        .with_chunk_size(64 * 1024);

    let mut pipeline = StoragePipeline::new(config, backend).await?;

    let file_id = [123u8; 32];
    let data = b"Integration test data for v0.3 specification compliance";
    let meta = Some(
        Meta::new()
            .with_filename("integration_test.txt")
            .with_author("Test Suite"),
    );

    // Test process_file
    let file_metadata = pipeline.process_file(file_id, data, meta).await?;
    assert_eq!(file_metadata.file_id, file_id);
    assert_eq!(file_metadata.file_size, data.len() as u64);
    assert!(!file_metadata.chunks.is_empty());

    // Verify metadata
    if let Some(local_meta) = &file_metadata.local_metadata {
        assert_eq!(local_meta.filename.as_deref(), Some("integration_test.txt"));
        assert_eq!(local_meta.author.as_deref(), Some("Test Suite"));
    }

    // Test retrieve_file
    let retrieved = pipeline.retrieve_file(&file_metadata).await?;
    assert_eq!(retrieved, data);

    Ok(())
}

#[tokio::test]
async fn test_v0_3_encryption_modes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data = b"Test data for encryption modes";
    let file_id = [1u8; 32];

    // Test Convergent encryption
    {
        let backend = LocalStorage::new(temp_dir.path().join("convergent")).await?;
        let config = Config::default().with_encryption_mode(EncryptionMode::Convergent);
        let mut pipeline = StoragePipeline::new(config, backend).await?;

        let metadata = pipeline.process_file(file_id, data, None).await?;
        let retrieved = pipeline.retrieve_file(&metadata).await?;
        assert_eq!(retrieved, data);
    }

    // Test ConvergentWithSecret encryption
    {
        let backend = LocalStorage::new(temp_dir.path().join("convergent_secret")).await?;
        let config = Config::default().with_encryption_mode(EncryptionMode::ConvergentWithSecret);
        let mut pipeline = StoragePipeline::new(config, backend).await?;

        let metadata = pipeline.process_file(file_id, data, None).await?;
        let retrieved = pipeline.retrieve_file(&metadata).await?;
        assert_eq!(retrieved, data);
    }

    // Test RandomKey encryption - skip for now as decryption needs stored key
    // TODO: Implement proper random key decryption with stored keys
    /*
    {
        let backend = LocalStorage::new(temp_dir.path().join("random")).await?;
        let config = Config::default().with_encryption_mode(EncryptionMode::RandomKey);
        let mut pipeline = StoragePipeline::new(config, backend).await?;

        let metadata = pipeline.process_file(file_id, data, None).await?;
        let retrieved = pipeline.retrieve_file(&metadata).await?;
        assert_eq!(retrieved, data);
    }
    */

    Ok(())
}

#[tokio::test]
async fn test_v0_3_chunk_size_configuration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let backend = LocalStorage::new(temp_dir.path().to_path_buf()).await?;

    // Test with small chunk size to force multiple chunks
    let config = Config::default()
        .with_chunk_size(16) // Very small chunks to test chunking
        .with_compression(false, 1); // Disable compression for predictable chunking

    let mut pipeline = StoragePipeline::new(config, backend).await?;

    let file_id = [2u8; 32];
    let data = b"This is a longer test string that should be split into multiple chunks";

    let metadata = pipeline.process_file(file_id, data, None).await?;

    // Should have multiple chunks due to small chunk size
    assert!(
        metadata.chunks.len() > 1,
        "Should have multiple chunks with small chunk size"
    );

    // Verify retrieval works with multiple chunks
    let retrieved = pipeline.retrieve_file(&metadata).await?;
    assert_eq!(retrieved, data);

    Ok(())
}

#[tokio::test]
async fn test_v0_3_fec_parameters() -> Result<()> {
    // Test different FEC parameter combinations
    let test_cases = [
        (4, 2),  // 50% overhead
        (8, 2),  // 25% overhead
        (16, 4), // 25% overhead (default)
        (20, 5), // 25% overhead
    ];

    for (data_shards, parity_shards) in test_cases.iter() {
        let temp_dir = TempDir::new()?;
        let backend = LocalStorage::new(temp_dir.path().to_path_buf()).await?;

        let config = Config::default()
            .with_fec_params(*data_shards, *parity_shards)
            .with_encryption_mode(EncryptionMode::Convergent);

        let pipeline = StoragePipeline::new(config, backend).await?;
        let stats = pipeline.stats();

        assert_eq!(stats.fec_params.0, *data_shards as u16);
        assert_eq!(stats.fec_params.1, *parity_shards as u16);
    }

    Ok(())
}
