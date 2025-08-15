# Saorsa FEC - Forward Error Correction

[![CI](https://github.com/dirvine/saorsa-fec/actions/workflows/ci.yml/badge.svg)](https://github.com/dirvine/saorsa-fec/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/saorsa-fec.svg)](https://crates.io/crates/saorsa-fec)
[![Documentation](https://docs.rs/saorsa-fec/badge.svg)](https://docs.rs/saorsa-fec)

Patent-free erasure coding using systematic Reed-Solomon. Standalone, production-ready, and independent of the Saorsa network or any other Saorsa crates.

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

## Independence

This crate is completely standalone:

- No runtime or build-time dependency on any Saorsa components
- No coupling to the Saorsa network; suitable for any Rust application needing erasure coding

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

- **No Encryption**: This crate provides error correction, not confidentiality. Encrypt data before applying FEC if privacy is required.
- **Side-channel considerations**: GF(256) operations in the pure-Rust backend use lookup tables (`LOG`/`EXP`). These are not strictly constant-time and may leak through cache effects on shared hardware. Avoid feeding secret-dependent inputs in adversarial multi-tenant environments, or use a side-channel-hardened backend.
- **Memory Safety**: Pure Rust implementation with comprehensive testing; no `unsafe` in the core library.
- **Audit Status**: Independent crate; no formal third-party audit yet.

### Storage efficiency (sharded vs original)

Let `k` be the number of data shares required (threshold), `m` the parity shares, and `n = k + m` the total shares.

- **efficiency** = `k / n`
- **overhead %** = `100 × m / k`
- **storage multiplier** = `n / k`

Padding from block/stripe sizing is ≤ `k - 1` bytes (block mode) or `< stripe_size` (IDA stripes) and is negligible for large files.

#### Default 25% overhead across thresholds (as used by this crate)

| k (threshold) | m (parity) | n (total) | efficiency k/n | overhead % | failure tolerance |
| -------------- | ---------- | --------- | -------------- | ---------- | ----------------- |
| 8              | 2          | 10        | 0.80           | 25%        | 2                 |
| 12             | 3          | 15        | 0.80           | 25%        | 3                 |
| 16             | 4          | 20        | 0.80           | 25%        | 4                 |
| 20             | 5          | 25        | 0.80           | 25%        | 5                 |
| 24             | 6          | 30        | 0.80           | 25%        | 6                 |
| 32             | 8          | 40        | 0.80           | 25%        | 8                 |

#### Varying parity at a fixed threshold (example: k = 16)

| m | n | efficiency k/n | overhead % | failure tolerance |
| - | - | --------------- | ---------- | ----------------- |
| 2 | 18 | 0.888          | 12.5%      | 2                 |
| 4 | 20 | 0.800          | 25%        | 4                 |
| 6 | 22 | 0.727          | 37.5%      | 6                 |
| 8 | 24 | 0.667          | 50%        | 8                 |

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

Contributions are welcome! Open issues and pull requests on the repository.

<!-- Intentionally no cross-links to other Saorsa crates to emphasize independence -->