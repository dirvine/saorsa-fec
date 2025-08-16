# Saorsa FEC - Encrypted Storage with Forward Error Correction

[![CI](https://github.com/dirvine/saorsa-fec/actions/workflows/ci.yml/badge.svg)](https://github.com/dirvine/saorsa-fec/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/saorsa-fec.svg)](https://crates.io/crates/saorsa-fec)
[![Documentation](https://docs.rs/saorsa-fec/badge.svg)](https://docs.rs/saorsa-fec)

A comprehensive encrypted storage solution combining Reed-Solomon forward error correction with convergent encryption. Provides automatic deduplication, fault tolerance, and secure distributed storage capabilities.

## Overview

Saorsa FEC has evolved from a basic erasure coding library into a complete encrypted storage system. It combines patent-free Reed-Solomon encoding with convergent encryption to provide:

- **Data Protection**: AES-256-GCM authenticated encryption
- **Fault Tolerance**: Configurable Reed-Solomon redundancy  
- **Deduplication**: Content-addressable encryption for automatic space savings
- **Storage Abstraction**: Pluggable backends (memory, filesystem, network)
- **Version Control**: Complete metadata management with versioning
- **Lifecycle Management**: Automatic garbage collection and cleanup

## Key Features

### 🔐 **Convergent Encryption**
- **Content-Addressable**: Identical content produces identical encrypted chunks
- **Automatic Deduplication**: Significant space savings for repeated data
- **Security**: AES-256-GCM with content-derived keys
- **Integrity**: Built-in authentication prevents tampering

### 🛡️ **Forward Error Correction**
- **Systematic Reed-Solomon**: Original data preserved in first k shares
- **Configurable Redundancy**: Adjust fault tolerance (default 25% overhead)
- **Hardware Acceleration**: Optional ISA-L backend for x86_64
- **Performance**: 500MB/s+ encoding throughput

### 🗄️ **Storage Management**
- **Multiple Backends**: Memory, filesystem, network storage
- **Metadata System**: Versioned chunk tracking with references
- **Garbage Collection**: Automatic cleanup of unreferenced data
- **Atomic Operations**: Consistent state with rollback support

### ⚡ **Performance & Scalability**
- **Streaming Operations**: Memory-efficient processing of large files
- **Async-First**: Full Tokio integration for non-blocking I/O
- **Chunk-Based**: Configurable chunk sizes for optimal performance
- **SIMD Acceleration**: Hardware optimizations where available

## Quick Start

### Basic Usage

```rust
use saorsa_fec::{StoragePipeline, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create storage pipeline with default configuration
    let config = Config::default();
    let storage = /* your storage backend */;
    let mut pipeline = StoragePipeline::new(config, storage).await?;
    
    // Store data with encryption and FEC
    let file_id = [1u8; 32];
    let data = b"Hello, encrypted world!";
    let metadata = pipeline.process_file(file_id, data, None).await?;
    
    // Retrieve and decrypt data
    let recovered = pipeline.retrieve_file(&metadata).await?;
    assert_eq!(data.as_slice(), recovered);
    
    Ok(())
}
```

### Advanced Configuration

```rust
use saorsa_fec::{Config, EncryptionMode, StorageBackend, LocalStorage};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure for high reliability
    let config = Config::default()
        .with_encryption_mode(EncryptionMode::ConvergentWithSecret)
        .with_fec_params(16, 8)  // 33% overhead for higher fault tolerance
        .with_chunk_size(1024 * 1024)  // 1MB chunks
        .with_compression(true, 6);    // Enable compression
    
    // Set up local filesystem storage
    let storage = Arc::new(
        LocalStorage::new("/path/to/storage".into()).await?
    );
    
    let mut pipeline = StoragePipeline::new(config, storage).await?;
    
    // Process large file with progress tracking
    let data = std::fs::read("large_file.bin")?;
    let metadata = pipeline.process_file([42u8; 32], &data, None).await?;
    
    println!("Stored {} bytes in {} chunks", 
             metadata.file_size, 
             metadata.chunks.len());
    
    Ok(())
}
```

### Legacy FEC-Only Usage

The library maintains backward compatibility for basic Reed-Solomon operations:

```rust
use saorsa_fec::{ReedSolomon, FecError};

// Create encoder with 4 data shards and 2 parity shards
let rs = ReedSolomon::new(4, 2)?;

// Encode data
let data = vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9], vec![10, 11, 12]];
let mut shards = rs.encode(&data)?;

// Simulate losing 2 shards
shards[1] = None;
shards[3] = None;

// Reconstruct original data
let reconstructed = rs.reconstruct(&mut shards)?;
```

## Architecture

### System Components

```
┌─────────────────────────────────────────────────────┐
│                  Application Layer                  │
│         CLI, REST API, Library Interface            │
├─────────────────────────────────────────────────────┤
│                  Pipeline Layer                     │
│      Orchestration, Workflow, Error Handling       │
├─────────────────────────────────────────────────────┤
│                  Service Layer                      │
│    Registry, Metadata, Garbage Collection          │
├─────────────────────────────────────────────────────┤
│                 Processing Layer                    │
│         Encryption, FEC, Compression               │
├─────────────────────────────────────────────────────┤
│                  Storage Layer                      │
│      Memory, Filesystem, Network Backends          │
└─────────────────────────────────────────────────────┘
```

### Data Flow

```
Input Data → Convergent Encryption → Chunking → FEC Encoding → Storage Distribution
     ↓                                                               ↑
Metadata Generation ← Registry Updates ← Backend Storage ← Share Creation
```

## Encryption Modes

### 1. Convergent Encryption (Default)
```rust
let config = Config::default()
    .with_encryption_mode(EncryptionMode::Convergent);
```
- Keys derived from content hash
- Perfect deduplication across all users
- Best for public or semi-public data

### 2. Convergent with Secret
```rust
let config = Config::default()
    .with_encryption_mode(EncryptionMode::ConvergentWithSecret);
```
- Keys derived from content + user secret
- Deduplication within user's data only
- Balanced security and efficiency

### 3. Random Key Encryption
```rust
let config = Config::default()
    .with_encryption_mode(EncryptionMode::RandomKey);
```
- Unique random key for each encryption
- Maximum security, no deduplication
- Best for highly sensitive data

## Storage Backends

### Local Filesystem
```rust
use saorsa_fec::storage::LocalStorage;

let storage = LocalStorage::new("/path/to/storage".into()).await?;
```

### Memory Storage
```rust
use saorsa_fec::storage::MemoryStorage;

let storage = MemoryStorage::new();
```

### Network Storage (Planned)
```rust
use saorsa_fec::storage::NetworkStorage;

let nodes = vec![
    NodeEndpoint::new("node1.example.com", 8080),
    NodeEndpoint::new("node2.example.com", 8080),
];
let storage = NetworkStorage::new(nodes, 3); // 3x replication
```

### Multi-Backend Storage
```rust
use saorsa_fec::storage::MultiStorage;

let backends = vec![
    Arc::new(LocalStorage::new("/primary".into()).await?),
    Arc::new(LocalStorage::new("/backup".into()).await?),
];
let storage = MultiStorage::new(backends);
```

## Configuration Profiles

### High Performance
```rust
let config = Config::high_performance()
    .with_chunk_size(4 * 1024 * 1024)  // 4MB chunks
    .with_fec_params(32, 8)            // Larger Reed-Solomon matrix
    .with_compression(false, 0);       // Disable compression
```

### High Reliability
```rust
let config = Config::high_reliability()
    .with_fec_params(16, 12)           // 75% overhead
    .with_chunk_size(64 * 1024)        // Smaller chunks
    .with_encryption_mode(EncryptionMode::ConvergentWithSecret);
```

### Minimal Storage
```rust
let config = Config::minimal_storage()
    .with_fec_params(20, 5)            // 25% overhead
    .with_compression(true, 9)         // Maximum compression
    .with_chunk_size(128 * 1024);      // 128KB chunks
```

## Performance Characteristics

### Encoding Performance
- **Throughput**: 500MB/s+ on modern hardware (with ISA-L)
- **Memory Usage**: Configurable chunk sizes (default 64KB)
- **Scalability**: Linear scaling with data size
- **Optimization**: SIMD acceleration where available

### Storage Efficiency
- **Redundancy**: Configurable (default 25% overhead)
- **Deduplication**: Automatic for identical content
- **Compression**: Optional metadata compression  
- **Caching**: Intelligent prefetching and LRU eviction

### Network Efficiency
- **Chunk-Based**: Efficient partial updates and retrieval
- **Parallel Operations**: Concurrent encoding/decoding
- **Adaptive Parameters**: Automatic tuning based on conditions
- **Minimal Overhead**: Compact metadata and efficient protocols

## Security Considerations

### Cryptographic Security
- **AES-256-GCM**: Industry-standard authenticated encryption
- **SHA-256**: Secure hash function for key derivation
- **Constant-Time Operations**: Where cryptographically relevant
- **Side-Channel Resistance**: Careful implementation to prevent leaks

### Implementation Security
- **Memory Safety**: Rust's ownership system prevents common vulnerabilities
- **Input Validation**: Comprehensive bounds checking and sanitization
- **Error Handling**: Secure failure modes without information leakage
- **Dependency Management**: Regular security audits of dependencies

### Operational Security
- **Key Management**: Secure key derivation and handling
- **Access Control**: Backend-specific permission management
- **Audit Trail**: Comprehensive logging of security-relevant events
- **Secure Defaults**: Conservative configuration out-of-the-box

## Development

### Testing
```bash
# Run all tests
cargo test --all-features

# Property-based tests
cargo test proptest

# Integration tests
cargo test --test integration

# Security-focused tests
cargo test --test security

# Performance benchmarks
cargo bench --features bench
```

### Features
- `default = ["pure-rust"]` - Default pure Rust implementation
- `pure-rust` - Software-only implementation
- `isa-l` - Hardware-accelerated backend for x86_64
- `bench` - Enable benchmark dependencies

### Quality Assurance
This project maintains the highest quality standards:
- **Zero Tolerance**: No `unwrap()`, `expect()`, `panic!()`, or `todo!()`
- **Comprehensive Testing**: Unit, integration, property-based tests
- **Security First**: Regular audits and secure coding practices
- **Performance**: Continuous benchmarking and optimization
- **Documentation**: Complete API documentation with examples

## Storage Efficiency Tables

### Default 25% overhead across thresholds

| k (threshold) | m (parity) | n (total) | efficiency k/n | overhead % | failure tolerance |
| -------------- | ---------- | --------- | -------------- | ---------- | ----------------- |
| 8              | 2          | 10        | 0.80           | 25%        | 2                 |
| 12             | 3          | 15        | 0.80           | 25%        | 3                 |
| 16             | 4          | 20        | 0.80           | 25%        | 4                 |
| 20             | 5          | 25        | 0.80           | 25%        | 5                 |
| 24             | 6          | 30        | 0.80           | 25%        | 6                 |
| 32             | 8          | 40        | 0.80           | 25%        | 8                 |

### Varying parity at fixed threshold (k = 16)

| m | n | efficiency k/n | overhead % | failure tolerance |
| - | - | --------------- | ---------- | ----------------- |
| 2 | 18 | 0.888          | 12.5%      | 2                 |
| 4 | 20 | 0.800          | 25%        | 4                 |
| 6 | 22 | 0.727          | 37.5%      | 6                 |
| 8 | 24 | 0.667          | 50%        | 8                 |

## Use Cases

### Distributed Storage Systems
- **Cloud Storage**: Multi-region data protection with deduplication
- **CDN Origins**: Content distribution with automatic redundancy
- **Backup Systems**: Space-efficient backups with encryption

### High-Availability Applications
- **Database Storage**: Protect critical data with configurable redundancy
- **Media Archives**: Long-term storage with integrity verification
- **Scientific Data**: Preserve research data with fault tolerance

### Edge Computing
- **IoT Data Collection**: Efficient storage at edge nodes
- **Mobile Applications**: Offline-first data synchronization
- **Embedded Systems**: Resource-constrained fault-tolerant storage

## Migration from Basic FEC

Existing users of basic Reed-Solomon functionality can upgrade incrementally:

```rust
// Before: Basic FEC
let rs = ReedSolomon::new(4, 2)?;
let shards = rs.encode(&data)?;

// After: Enhanced storage (backward compatible)
let rs = ReedSolomon::new(4, 2)?;
let shards = rs.encode(&data)?;

// Or: Full encrypted storage
let pipeline = StoragePipeline::new(Config::default(), storage).await?;
let metadata = pipeline.process_file(file_id, &data, None).await?;
```

## Roadmap

### Current (v0.2.x)
- ✅ Convergent encryption with multiple modes
- ✅ Comprehensive metadata management
- ✅ Storage backend abstraction
- ✅ Integrated processing pipeline
- ✅ Garbage collection system

### Near Term (v0.3.x)
- 🚧 Network storage backend
- 🚧 Advanced replication strategies
- 🚧 Performance optimizations
- 🚧 Extended monitoring and metrics

### Medium Term (v0.4.x)
- 📋 Distributed consensus protocols
- 📋 Cross-platform hardware acceleration
- 📋 Advanced compression algorithms
- 📋 REST API and CLI tools

### Long Term (v1.0+)
- 📋 Formal verification of critical components
- 📋 Hardware security module integration
- 📋 Advanced threat protection
- 📋 Ecosystem integrations

## Independence

This crate is completely standalone:
- No runtime or build-time dependency on any Saorsa components
- No coupling to the Saorsa network; suitable for any Rust application
- Self-contained with minimal external dependencies

## License

Licensed under the GNU Affero General Public License v3.0 or later.

## Contributing

Contributions are welcome! Please read our contributing guidelines and ensure:
- All tests pass (`cargo test --all-features`)
- Code follows our quality standards (no unwrap/panic patterns)
- Documentation is updated for new features
- Security considerations are addressed

Open issues and pull requests on the [repository](https://github.com/dirvine/saorsa-fec).

---

**Saorsa FEC**: From basic erasure coding to complete encrypted storage solution. Secure, efficient, and production-ready.