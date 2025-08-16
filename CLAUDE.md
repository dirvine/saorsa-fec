# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Saorsa FEC is a standalone, patent-free erasure coding library implementing systematic Reed-Solomon coding using Galois Field (GF256) arithmetic. It provides forward error correction for distributed data storage and transmission, allowing data reconstruction even when some fragments are lost.

## Essential Commands

### Build
```bash
cargo build                      # Standard debug build
cargo build --release           # Optimized release build
cargo build --features pure-rust # Build with pure Rust backend
cargo build --features isa-l    # Build with ISA-L acceleration (x86_64 only)
```

### Test
```bash
cargo test                      # Run all tests
cargo test --lib               # Run library tests only
cargo test proptest            # Run property-based tests
cargo test --features pure-rust # Test with pure Rust backend
cargo test -- --nocapture      # Show test output
cargo test test_name           # Run specific test
```

### Quality Checks
```bash
cargo fmt                      # Format code
cargo fmt --check             # Check formatting without changing
cargo clippy                  # Run linter
cargo clippy -- -D warnings   # Treat warnings as errors (CI standard)
```

### Benchmarks
```bash
cargo bench --features bench   # Run performance benchmarks
```

## Architecture

### Core Components

The crate follows a modular architecture with clear separation of concerns:

1. **Trait Layer** (`src/traits.rs`)
   - `Fec` trait: High-level async API for encoding/decoding
   - `FecBackend` trait: Low-level backend implementation interface
   - Allows pluggable implementations (pure-rust, ISA-L)

2. **Backend Implementations** (`src/backends/`)
   - `pure_rust.rs`: Default software implementation using GF(256) arithmetic
   - `isa_l.rs`: Hardware-accelerated implementation for x86_64
   - Selected at compile time via features

3. **Galois Field Math** (`src/gf256.rs`)
   - Core GF(256) operations using lookup tables (LOG/EXP)
   - Matrix operations for Reed-Solomon encoding
   - Systematic encoding preserves original data in first k shares

4. **IDA Implementation** (`src/ida.rs`)
   - Information Dispersal Algorithm orchestration
   - Stripe-based processing for large files
   - Automatic parameter selection based on content size
   - Metadata management for share tracking

### Data Flow

1. **Encoding**: Data → Stripes → GF256 Matrix Multiply → Systematic Shares (data + parity)
2. **Decoding**: Available Shares → Matrix Inversion → Reconstruction → Original Data

### Key Design Decisions

- **Systematic Encoding**: Original data preserved in first k shares for efficiency
- **25% Default Overhead**: Balanced trade-off between storage and fault tolerance
- **Stripe-Based Processing**: Enables streaming and memory-efficient operations
- **Async-First API**: Compatible with Tokio for network operations
- **No External Dependencies**: Pure Rust implementation is self-contained

## Testing Strategy

- **Unit Tests**: Core functionality validation
- **Property Tests**: Using proptest for invariant checking
- **Reconstruction Tests**: Verify data recovery with various loss patterns
- **Matrix Tests**: Validate GF(256) arithmetic correctness
- **Performance Benchmarks**: Track encoding/decoding speed

## Performance Considerations

- GF(256) uses lookup tables - fast but not constant-time (cache timing considerations)
- ISA-L backend provides 2-4x speedup on supported hardware
- Default stripe size (64KB) optimized for network transmission
- Matrix operations are the performance bottleneck

## Security Notes

- This provides error correction, NOT encryption
- GF(256) operations may leak timing information via cache
- No built-in authentication - combine with AEAD if needed
- Systematic encoding exposes original data in first k shares