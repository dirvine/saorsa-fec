# Saorsa FEC - Forward Error Correction

[![CI](https://github.com/dirvine/saorsa-foundation/actions/workflows/ci.yml/badge.svg)](https://github.com/dirvine/saorsa-foundation/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/saorsa-fec.svg)](https://crates.io/crates/saorsa-fec)
[![Documentation](https://docs.rs/saorsa-fec/badge.svg)](https://docs.rs/saorsa-fec)

Patent-free erasure coding using systematic Reed-Solomon for the Saorsa P2P network.

## Overview

Saorsa FEC provides forward error correction capabilities for distributed data storage and transmission. It implements systematic Reed-Solomon encoding using Galois Field arithmetic, allowing data to be reconstructed even when some fragments are lost or corrupted.

## Features

- **Systematic Reed-Solomon Encoding**: Original data remains intact, parity data is appended
- **Configurable Redundancy**: Adjust data/parity shard ratios for different fault tolerance levels  
- **Pure Rust Implementation**: Default backend with no external dependencies
- **Optional ISA-L Backend**: Hardware-accelerated implementation for x86_64 platforms
- **Async Support**: Tokio-compatible async operations
- **Property-Based Testing**: Extensive validation using proptest and quickcheck

## Usage

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

## Relationship to Saorsa Ecosystem

Saorsa FEC is a foundational crate in the Saorsa P2P ecosystem:

```
┌─────────────────────┐
│   Communitas App    │ ← End-user applications
└─────────────────────┘
           │
┌─────────────────────┐
│    Saorsa Core      │ ← P2P networking library
└─────────────────────┘
           │
┌─────────────────────┐
│    Saorsa MLS       │ ← Message Layer Security
└─────────────────────┘
           │
┌─────────────────────┐
│    Saorsa RSPS      │ ← Root-Scoped Provider Summaries
└─────────────────────┘
           │
┌─────────────────────┐
│    Saorsa FEC       │ ← Forward Error Correction (this crate)
└─────────────────────┘
           │
┌─────────────────────┐
│   Saorsa Types      │ ← Common types and utilities
└─────────────────────┘
```

### Dependencies and Usage

- **Zero Dependencies on Other Saorsa Crates**: This crate is dependency-free within the Saorsa ecosystem
- **Used By**: 
  - `saorsa-core` - For distributed data storage resilience
  - `saorsa-rsps` - For DHT data fragment recovery
  - Applications requiring data redundancy and fault tolerance

### Integration Points

- **DHT Storage**: Enables chunk recovery when nodes leave the network
- **Message Transmission**: Provides redundancy for large message payloads
- **Backup Systems**: Allows reconstruction of data from partial backups
- **Network Resilience**: Maintains data availability during network partitions

## Performance

The crate includes comprehensive benchmarks for different scenarios:

```bash
cargo bench --features bench
```

Typical performance on modern hardware:
- Encoding: ~2-5 GB/s depending on shard configuration
- Reconstruction: ~1-3 GB/s for typical loss scenarios
- ISA-L backend provides 2-4x speedup on x86_64

## Security Considerations

- **No Encryption**: This crate only provides error correction, not confidentiality
- **Timing Attack Resistance**: Galois field operations use constant-time implementations
- **Memory Safety**: Pure Rust implementation with comprehensive testing
- **Audit Status**: Part of the security-audited Saorsa ecosystem

## Development

### Testing

```bash
# Run all tests
cargo test --all-features

# Property-based tests
cargo test proptest

# Benchmarks
cargo bench
```

### Features

- `default = ["pure-rust"]` - Default pure Rust implementation
- `pure-rust` - Software-only implementation
- `isa-l` - Hardware-accelerated backend for x86_64
- `bench` - Enable benchmark dependencies

## License

Licensed under the GNU Affero General Public License v3.0 or later.

## Contributing

Contributions are welcome! Please see the [main Saorsa repository](https://github.com/dirvine/saorsa-foundation) for contribution guidelines.

## Related Crates

- [`saorsa-types`](https://crates.io/crates/saorsa-types) - Common types and utilities
- [`saorsa-core`](https://crates.io/crates/saorsa-core) - P2P networking foundation
- [`saorsa-rsps`](https://crates.io/crates/saorsa-rsps) - DHT provider summaries
- [`saorsa-mls`](https://crates.io/crates/saorsa-mls) - Message Layer Security