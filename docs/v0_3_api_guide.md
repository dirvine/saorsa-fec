# Saorsa FEC v0.3 API Guide

This document describes the v0.3 StoragePipeline API, which provides a high-level interface for encrypted, chunked storage with forward error correction.

## Overview

The v0.3 API introduces:
- **Builder pattern configuration** for clean, fluent setup
- **Generic storage pipeline** that works with any storage backend
- **Integrated encryption and FEC** with automatic chunking
- **Content-addressed storage** with deduplication support
- **Metadata management** for files and chunks

## Core Components

### Config Builder Pattern

```rust
use saorsa_fec::{Config, EncryptionMode};

let config = Config::default()
    .with_encryption_mode(EncryptionMode::Convergent)
    .with_fec_params(16, 4)      // 16 data + 4 parity shards (25% overhead)
    .with_chunk_size(64 * 1024)  // 64 KiB chunks (default)
    .with_compression(true, 6);  // Enable compression at level 6
```

#### Available Configuration Options

- **`with_encryption_mode(mode)`**: Set encryption behavior
  - `EncryptionMode::Convergent`: Pure convergent encryption (global deduplication)
  - `EncryptionMode::ConvergentWithSecret`: Convergent with user secret (controlled deduplication)
  - `EncryptionMode::RandomKey`: Random keys (no deduplication, maximum privacy)

- **`with_fec_params(data_shards, parity_shards)`**: Configure forward error correction
  - `data_shards`: Number of data chunks (k in Reed-Solomon)
  - `parity_shards`: Number of parity chunks (n-k in Reed-Solomon)
  - Overhead ratio = `parity_shards / data_shards`

- **`with_chunk_size(bytes)`**: Set chunk size in bytes
  - Default: ~64 KiB as specified in v0.3
  - Larger chunks = fewer metadata overhead, less granular deduplication
  - Smaller chunks = more metadata overhead, better deduplication

- **`with_compression(enabled, level)`**: Configure compression
  - `enabled`: Whether to compress before encryption
  - `level`: Compression level (1-9, where 9 is maximum compression)

### StoragePipeline API

```rust
use saorsa_fec::{StoragePipeline, Config, LocalStorage, Meta};

// Create storage backend
let backend = LocalStorage::new("/path/to/storage").await?;

// Create pipeline
let config = Config::default().with_encryption_mode(EncryptionMode::Convergent);
let mut pipeline = StoragePipeline::new(config, backend).await?;

// Process a file
let file_id = [42u8; 32];  // Unique file identifier
let data = b"Hello, World!";
let meta = Some(Meta::new().with_filename("hello.txt"));

let file_metadata = pipeline.process_file(file_id, data, meta).await?;

// Retrieve the file
let retrieved_data = pipeline.retrieve_file(&file_metadata).await?;
assert_eq!(retrieved_data, data);
```

#### Core Methods

- **`StoragePipeline::new(cfg, backend)`**: Create new pipeline with configuration and storage backend
- **`process_file(file_id, data, meta)`**: Store a file with optional metadata
  - Returns `FileMetadata` containing chunk references and encryption info
  - Automatically handles compression, encryption, chunking, and FEC encoding
- **`retrieve_file(metadata)`**: Retrieve and decrypt a file from its metadata
  - Automatically handles FEC reconstruction, decryption, and decompression

### File Metadata

The v0.3 API provides rich metadata support:

```rust
use saorsa_fec::Meta;

let meta = Meta::new()
    .with_filename("document.pdf")
    .with_author("Alice")
    .add_tag("important")
    .add_tag("work");

meta.description = Some("Project documentation".to_string());
meta.mime_type = Some("application/pdf".to_string());
```

## Example Usage

### Basic File Storage

```rust
use saorsa_fec::{Config, EncryptionMode, StoragePipeline, LocalStorage, Meta};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup
    let backend = LocalStorage::new("./storage").await?;
    let config = Config::default()
        .with_encryption_mode(EncryptionMode::Convergent)
        .with_fec_params(16, 4)
        .with_chunk_size(64 * 1024);
    
    let mut pipeline = StoragePipeline::new(config, backend).await?;
    
    // Store file
    let file_id = blake3::hash(b"unique-file-identifier").into();
    let data = std::fs::read("document.pdf")?;
    let meta = Some(Meta::new()
        .with_filename("document.pdf")
        .with_author("Alice"));
    
    let metadata = pipeline.process_file(file_id, &data, meta).await?;
    println!("Stored file with {} chunks", metadata.chunks.len());
    
    // Retrieve file
    let retrieved = pipeline.retrieve_file(&metadata).await?;
    assert_eq!(retrieved, data);
    
    Ok(())
}
```

### High-Reliability Configuration

```rust
// Configure for maximum redundancy
let config = Config::default()
    .with_encryption_mode(EncryptionMode::RandomKey)  // Maximum privacy
    .with_fec_params(10, 10)  // 100% overhead for high reliability
    .with_chunk_size(32 * 1024)  // Smaller chunks for better recovery
    .with_compression(true, 9);  // Maximum compression
```

### Storage-Optimized Configuration

```rust
// Configure for minimal storage overhead
let config = Config::default()
    .with_encryption_mode(EncryptionMode::Convergent)  // Enable deduplication
    .with_fec_params(20, 2)  // Only 10% overhead
    .with_chunk_size(128 * 1024)  // Larger chunks
    .with_compression(true, 9);  // Maximum compression
```

## Architecture Details

### Chunking and CID Computation

1. **Input Processing**: Optional compression of original data
2. **Encryption**: Applied based on encryption mode
3. **Chunking**: Split encrypted data into configurable-size chunks
4. **CID Generation**: Computed over ciphertext + authenticated header
5. **FEC Encoding**: Applied to each chunk for redundancy
6. **Storage**: Chunks stored in backend with content-addressed IDs

### Metadata Structure

Each file produces a `FileMetadata` containing:
- **File ID**: Blake3 hash of original content
- **File Size**: Original uncompressed size in bytes
- **Encryption Metadata**: Algorithm, key derivation, nonce
- **Chunk References**: List of chunk IDs, stripe/shard indices, sizes
- **Local Metadata**: Filename, author, tags (doesn't affect content addressing)

### Deduplication

Content-addressed storage enables automatic deduplication:
- **Convergent Encryption**: Same plaintext → same ciphertext → same storage
- **Chunk-Level**: Common chunks shared across different files
- **CID-Based**: Lookup by content identifier for instant deduplication detection

## Storage Backends

The v0.3 API supports pluggable storage backends:

### LocalStorage
```rust
use saorsa_fec::LocalStorage;

let backend = LocalStorage::new("/path/to/storage").await?;
// Stores chunks as files in directory hierarchy
```

### Custom Backends
Implement the `StorageBackend` trait for custom storage:

```rust
use saorsa_fec::StorageBackend;
use async_trait::async_trait;

struct MyBackend;

#[async_trait]
impl StorageBackend for MyBackend {
    async fn put_chunk(&self, id: &[u8; 32], data: &[u8]) -> anyhow::Result<()> {
        // Store chunk implementation
        todo!()
    }
    
    async fn get_chunk(&self, id: &[u8; 32]) -> anyhow::Result<Vec<u8>> {
        // Retrieve chunk implementation
        todo!()
    }
    
    // ... other required methods
}
```

## Security Considerations

- **Convergent Encryption**: Enables deduplication but reveals duplicate content
- **Random Key Encryption**: Maximum privacy but no deduplication
- **Convergent with Secret**: Balanced approach with controlled deduplication scope
- **Authenticated Encryption**: AES-256-GCM provides confidentiality and integrity
- **Key Derivation**: Blake3-based for convergent keys, secure random for others

## Performance Guidelines

**🚀 High-Performance reed-solomon-simd (v0.2.1)**

Saorsa FEC now achieves exceptional performance with reed-solomon-simd integration:

- **1MB files**: 1,193 MB/s (2.4x v0.3 target)
- **10MB files**: 7,545 MB/s (15x v0.3 target)  
- **50MB files**: 5,366 MB/s (10.7x v0.3 target)

**SIMD Acceleration**: Automatically utilizes available CPU vector instructions:
- **AVX2**: Intel/AMD advanced vector extensions
- **AVX**: Intel/AMD vector extensions  
- **SSE4.1**: Intel streaming SIMD extensions
- **NEON**: ARM vector processing

**Configuration Tips:**
- **Chunk Size**: Balance between metadata overhead and deduplication granularity
  - Even sizes recommended (e.g., 64KiB, 128KiB) for optimal SIMD performance
- **FEC Parameters**: Higher redundancy = better reliability but more storage
- **Compression**: Test with your data - not all content compresses well
- **Backend Choice**: Local storage for speed, network for distribution

## Error Handling

All operations return `anyhow::Result<T>` for comprehensive error handling:

```rust
match pipeline.process_file(file_id, data, meta).await {
    Ok(metadata) => println!("Success: {} chunks", metadata.chunks.len()),
    Err(e) => eprintln!("Error: {:#}", e),
}
```

## Migration from Earlier Versions

The v0.3 API is designed for new applications. For existing users:
- Legacy `Pipeline` struct remains available for backward compatibility
- Consider migrating to `StoragePipeline` for new features
- Configuration can be gradually migrated to builder pattern

## Future Roadmap

- Network storage backends
- Advanced deduplication strategies  
- Additional encryption algorithms
- GPU acceleration exploration
- Streaming API improvements

## Recent Updates

**v0.2.1 (Current)**
- ✅ reed-solomon-simd integration for 10-15x performance improvement
- ✅ SIMD acceleration (AVX2, AVX, SSE4.1, NEON)
- ✅ Achieves 1,000-7,500 MB/s encoding throughput
- ✅ Pure Rust implementation (no C dependencies)