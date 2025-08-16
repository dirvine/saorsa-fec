# Saorsa FEC — Storage, Encryption & FEC

[![CI](https://github.com/dirvine/saorsa-fec/actions/workflows/ci.yml/badge.svg)](https://github.com/dirvine/saorsa-fec/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/saorsa-fec.svg)](https://crates.io/crates/saorsa-fec)
[![Documentation](https://docs.rs/saorsa-fec/badge.svg)](https://docs.rs/saorsa-fec)

A comprehensive Forward Error Correction library with **AES‑256‑GCM authenticated encryption**, **SHA‑256–based key derivation**, and high‑level storage pipeline APIs — plus legacy FEC‑only support.

## Key Features

- **Three Encryption Modes**: Convergent, ConvergentWithSecret, and RandomKey
- **Authenticated Encryption**: AES-256-GCM with deterministic nonces
- **Wire-Compatible Format**: 96-byte shard headers for network protocols
- **Storage Pipeline**: High-level file processing with chunking → encryption → FEC → storage
- **Multiple Backends**: LocalStorage, MemoryStorage, MultiStorage (NetworkStorage planned)
- **Legacy Compatibility**: Original Reed-Solomon API still works
- **High Performance**: 1,000-7,500 MB/s with reed-solomon-simd SIMD acceleration

## Encryption Modes

### Convergent
Key derived solely from content hash → **global deduplication** (identical content → identical ciphertext).

### ConvergentWithSecret  
Key derived from content hash + user secret → **per‑user deduplication**.

### RandomKey
Per‑encryption random key → **no deduplication**, maximum confidentiality.

**Security Note**: Convergent modes can enable confirmation‑of‑file if an attacker can compute the content hash; ConvergentWithSecret mitigates this by mixing a user secret. RandomKey mode avoids dedup to maximise privacy.

## Quick Start

### High-Level Storage Pipeline API

```rust
use saorsa_fec::{Config, StoragePipeline, EncryptionMode, LocalStorage, FecParams, ChunkConfig};

// Configure pipeline
let fec_params = FecParams::new(16, 4)?; // 25% overhead  
let chunk_config = ChunkConfig::new(64 * 1024, fec_params); // 64 KiB chunks
let config = Config::default()
    .with_encryption_mode(EncryptionMode::Convergent)
    .with_fec_params(16, 4)  
    .with_chunk_size(64 * 1024)
    .with_compression(false, 0);

// Create storage backend
let storage = LocalStorage::new("./storage").await?;
let mut pipeline = StoragePipeline::new(config, storage).await?;

// Store file
let file_data = std::fs::read("example.txt")?;
let file_id = [42u8; 32]; // Application-defined identifier
let metadata = pipeline.process_file(file_id, &file_data, None).await?;

// Retrieve file  
let retrieved = pipeline.retrieve_file(&metadata).await?;
assert_eq!(retrieved, file_data);
```

### Legacy Reed-Solomon API

```rust
use saorsa_fec::ReedSolomon;

// Create Reed-Solomon codec
let rs = ReedSolomon::new(10, 3)?; // 10 data + 3 parity shards

// Encode data
let data_shards = vec![
    vec![1, 2, 3, 4],
    vec![5, 6, 7, 8],
    // ... 8 more data shards
];
let all_shards = rs.encode(&data_shards)?;

// Simulate missing shards
let mut corrupted_shards = all_shards;
corrupted_shards[2] = None;
corrupted_shards[7] = None;

// Reconstruct
let reconstructed = rs.reconstruct(&mut corrupted_shards)?;
```

## FEC Parameters & Storage Overhead

API: `with_fec_params(data_shards, parity_shards)` where **overhead = parity/data**.

### Example Configurations

| Configuration | Overhead | Use Case |
|--------------|----------|-----------|
| (32, 8) | 25% | High performance |
| (16, 12) | 75% | High reliability |  
| (20, 5) | 25% | Minimal storage |
| (16, 8) | **50%** | Balanced reliability |

## Storage Backends

### LocalStorage
File system-based storage with CID addressing and metadata persistence.

```rust
let storage = LocalStorage::new("/path/to/storage").await?;
```

### MemoryStorage  
In-memory storage for testing and caching.

```rust
let storage = MemoryStorage::new();
```

### MultiStorage
Combines multiple backends with redundancy, load balancing, or failover.

```rust
let storage = MultiStorage::redundant(vec![storage1, storage2]).await?;
```

## Shard Format

Each shard uses a compact 96-byte header:

```
version (u8)          = 3
file_id (32B)         = Unique file identifier  
chunk_index (u32)     = Index of chunk within file
shard_index (u16)     = Index of shard within chunk
nspec (u8,u8)         = (data_shards, parity_shards)
flags (u8)            = encrypted, mode, isa-l, compressed
nonce (12B)           = AES‑GCM nonce
mac (16B)             = AES‑GCM tag
```

Header is authenticated via AEAD and included in CID calculation.

## Security Considerations

- **Modes**: prefer ConvergentWithSecret for most user‑private data (balances dedup & privacy); use RandomKey for highly sensitive data; Convergent suits public/semi‑public content.
- **Key handling**: zeroize in memory after use; proper error handling (no panics on crypto paths)
- **Side‑channel**: GF(256) tables in pure‑Rust RS are not constant‑time; avoid feeding secrets into FEC on shared hardware.

## Performance

**🚀 Exceptional Performance with reed-solomon-simd v0.2.1**

- **1MB files**: 1,193 MB/s (2.4x target)
- **10MB files**: 7,545 MB/s (15x target)  
- **50MB files**: 5,366 MB/s (10.7x target)

**SIMD Acceleration Support:**
- **AVX2**: Intel/AMD advanced vector extensions
- **AVX**: Intel/AMD vector extensions  
- **SSE4.1**: Intel streaming SIMD extensions
- **NEON**: ARM vector processing
- **Pure Rust**: No C dependencies required
- **Streaming**: Async (Tokio) pipeline processing

Performance scales with file size and benefits from SIMD instructions available on modern CPUs.

## Features

- `default = ["pure-rust"]` - High-performance reed-solomon-simd implementation
- `isa-l` - ISA-L hardware acceleration (x86_64, optional)
- `bench` - Benchmark dependencies

## Development

```bash
# Build and test
cargo build --release
cargo test --all-features  
cargo clippy -- -D warnings

# Run benchmarks
cargo bench --features bench

# Check performance
cargo run --example performance_test --release
```

## License

Licensed under the GNU Affero General Public License v3.0 or later.

## Contributing

Contributions welcome! Please open issues and pull requests.