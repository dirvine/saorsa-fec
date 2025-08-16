# Architecture Decision Records (ADR) - Saorsa FEC

## ADR-001: Reed-Solomon Implementation Choice

### Status
**ACCEPTED** - Implemented in v0.2.1

### Context
The v0.3 specification required 500+ MB/s Reed-Solomon encoding/decoding performance. The existing custom implementation achieved only ~150 MB/s, creating a significant performance gap.

### Decision
Replace custom Reed-Solomon implementation with reed-solomon-simd library.

### Alternatives Considered

1. **ISA-L Integration**
   - ✅ High performance (~500+ MB/s)
   - ❌ C dependency complexity
   - ❌ x86_64 architecture limitation
   - ❌ Build system complexity

2. **Custom SIMD Implementation**
   - ✅ Full control over implementation
   - ❌ High development cost
   - ❌ Cross-platform SIMD complexity
   - ❌ Maintenance burden

3. **reed-solomon-simd Library**
   - ✅ Pure Rust implementation
   - ✅ Cross-platform SIMD support
   - ✅ Active maintenance
   - ✅ Performance exceeds targets
   - ❌ External dependency

### Rationale
- reed-solomon-simd delivers 1,000-7,500 MB/s (2-15x target performance)
- Pure Rust eliminates C build complexity and cross-compilation issues
- SIMD support across x86_64 (AVX2, AVX, SSE4.1) and ARM64 (NEON)
- Well-maintained library with active development
- Reduces long-term maintenance burden

### Consequences
- **Positive**: Exceptional performance exceeding all targets
- **Positive**: Simplified build process and deployment
- **Positive**: Cross-platform compatibility
- **Negative**: External dependency (mitigated by library quality)
- **Requirement**: Even-sized blocks for optimal SIMD performance

---

## ADR-002: Storage Pipeline Architecture

### Status
**ACCEPTED** - Implemented in v0.3 API

### Context
Need for high-level API that combines encryption, chunking, FEC, and storage in a cohesive pipeline while maintaining flexibility for different storage backends.

### Decision
Implement StoragePipeline with pluggable storage backend architecture.

### Architecture
```rust
StoragePipeline<B: StorageBackend> {
    config: Config,
    backend: B,
    fec: Arc<dyn FecBackend>,
}
```

### Design Principles
1. **Separation of Concerns**: Each layer has a single responsibility
2. **Pluggable Backends**: Storage implementation is abstracted
3. **Configuration-Driven**: Behavior controlled through configuration
4. **Type Safety**: Generic traits ensure compile-time correctness

### Consequences
- **Positive**: Clean separation between processing and storage
- **Positive**: Easy to test with MemoryStorage backend
- **Positive**: Supports multiple storage implementations
- **Positive**: Configuration flexibility
- **Negative**: Some complexity in generic trait bounds

---

## ADR-003: Encryption Mode Strategy

### Status
**ACCEPTED** - Implemented in v0.3 API

### Context
Need to support different encryption strategies balancing deduplication benefits with privacy requirements.

### Decision
Implement three encryption modes with distinct use cases:

1. **Convergent**: Pure convergent encryption for maximum deduplication
2. **ConvergentWithSecret**: User-scoped convergent encryption
3. **RandomKey**: Random keys for maximum privacy

### Rationale
- **Convergent**: Enables global deduplication for public/semi-public content
- **ConvergentWithSecret**: Balances deduplication with privacy for user data
- **RandomKey**: Maximum privacy for sensitive data
- **Flexibility**: Users can choose appropriate trade-offs

### Implementation
```rust
enum EncryptionMode {
    Convergent,
    ConvergentWithSecret([u8; 32]),
    RandomKey,
}
```

### Consequences
- **Positive**: Covers all major use cases
- **Positive**: Clear security/privacy trade-offs
- **Positive**: User can choose appropriate mode
- **Negative**: Complexity in key management
- **Requirement**: User education on mode selection

---

## ADR-004: Content-Addressed Storage (CAS)

### Status
**ACCEPTED** - Implemented in v0.3 API

### Context
Need for deduplication, integrity verification, and efficient storage addressing.

### Decision
Use Blake3 hash of encrypted content plus authenticated header as Content ID (CID).

### CID Computation
```
CID = Blake3(encrypted_content || authenticated_header)
```

### Benefits
1. **Integrity**: Content corruption is detectable
2. **Deduplication**: Identical content has identical CID
3. **Convergent**: Deterministic addressing enables deduplication
4. **Performance**: Blake3 is extremely fast

### Consequences
- **Positive**: Automatic deduplication and integrity verification
- **Positive**: High-performance hashing with Blake3
- **Positive**: Deterministic content addressing
- **Negative**: CID reveals some information about content structure

---

## ADR-005: Error Handling Strategy

### Status
**ACCEPTED** - Implemented across all APIs

### Context
Need for consistent, comprehensive error handling across the entire library.

### Decision
Use `anyhow::Result<T>` for all fallible operations with custom error types for specific cases.

### Error Hierarchy
```rust
#[derive(thiserror::Error, Debug)]
pub enum FecError {
    #[error("Invalid parameters: k={k}, n={n}")]
    InvalidParameters { k: usize, n: usize },
    
    #[error("Size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },
    
    #[error("Insufficient shares: have {have}, need {need}")]
    InsufficientShares { have: usize, need: usize },
    
    #[error("Backend error: {0}")]
    Backend(String),
}
```

### Benefits
- **Ergonomic**: anyhow provides excellent error composition
- **Informative**: Structured error messages with context
- **Debuggable**: Full error chain with source information
- **Consistent**: Uniform error handling across all APIs

### Consequences
- **Positive**: Excellent error diagnostics for users
- **Positive**: Easy error composition and propagation
- **Positive**: Good debugging experience
- **Negative**: Some dependency on anyhow ecosystem

---

## ADR-006: Async/Await Architecture

### Status
**ACCEPTED** - Implemented in v0.3 StoragePipeline

### Context
Storage operations are inherently I/O bound and benefit from async processing.

### Decision
Use async/await throughout the StoragePipeline API with Tokio compatibility.

### Design
```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn put_chunk(&self, id: &[u8; 32], data: &[u8]) -> anyhow::Result<()>;
    async fn get_chunk(&self, id: &[u8; 32]) -> anyhow::Result<Vec<u8>>;
}
```

### Benefits
- **Performance**: Non-blocking I/O for storage operations
- **Scalability**: Can handle many concurrent operations
- **Ecosystem**: Compatible with Tokio ecosystem
- **Future-Ready**: Supports network storage backends

### Consequences
- **Positive**: Excellent I/O performance and concurrency
- **Positive**: Natural fit for network storage
- **Positive**: Integration with async ecosystem
- **Negative**: Some complexity in testing
- **Requirement**: Tokio runtime for async operations

---

## ADR-007: Configuration Builder Pattern

### Status
**ACCEPTED** - Implemented in v0.3 Config API

### Context
Need for flexible, discoverable configuration with sensible defaults.

### Decision
Implement fluent builder pattern for configuration with compile-time validation.

### Implementation
```rust
impl Config {
    pub fn with_encryption_mode(mut self, mode: EncryptionMode) -> Self {
        self.encryption_mode = mode;
        self
    }
    
    pub fn with_fec_params(mut self, data_shards: u16, parity_shards: u16) -> Result<Self> {
        self.fec_params = FecParams::new(data_shards, parity_shards)?;
        Ok(self)
    }
}
```

### Benefits
- **Discoverable**: IDE completion shows available options
- **Flexible**: Can set any combination of options
- **Safe**: Compile-time validation where possible
- **Ergonomic**: Fluent interface is natural to use

### Consequences
- **Positive**: Excellent developer experience
- **Positive**: Self-documenting configuration
- **Positive**: Compile-time safety
- **Negative**: Some verbosity in configuration

---

## ADR-008: Metadata Management Strategy

### Status
**ACCEPTED** - Implemented in v0.3 Meta system

### Context
Need to associate rich metadata with stored files while maintaining content addressing.

### Decision
Separate content addressing from metadata storage with optional metadata association.

### Design
```rust
pub struct FileMetadata {
    pub file_id: [u8; 32],
    pub chunks: Vec<ChunkReference>,
    pub meta: Option<Meta>,
    // ... other fields
}

pub struct Meta {
    pub filename: Option<String>,
    pub author: Option<String>,
    pub tags: Vec<String>,
    // ... other metadata fields
}
```

### Rationale
- Content addressing is based only on content, not metadata
- Metadata can be changed without affecting storage
- Optional metadata doesn't impact core functionality
- Rich metadata supports application requirements

### Consequences
- **Positive**: Content addressing is pure and deterministic
- **Positive**: Metadata can be updated independently
- **Positive**: Rich metadata support for applications
- **Negative**: Additional complexity in metadata management

---

## ADR-009: Testing Strategy

### Status
**ACCEPTED** - Implemented across codebase

### Context
Need for comprehensive testing covering functionality, performance, and reliability.

### Decision
Multi-layered testing strategy with unit tests, integration tests, property-based tests, and benchmarks.

### Testing Layers
1. **Unit Tests**: Individual component functionality
2. **Integration Tests**: End-to-end pipeline testing
3. **Property Tests**: Invariant validation with random inputs
4. **Benchmarks**: Performance regression testing
5. **Example Tests**: Documentation and usage validation

### Tools
- **Rust Test Framework**: Standard unit and integration tests
- **Proptest**: Property-based testing for invariants
- **Criterion**: Performance benchmarking
- **Pretty Assertions**: Enhanced test output

### Consequences
- **Positive**: High confidence in correctness
- **Positive**: Performance regression detection
- **Positive**: Documentation validation
- **Negative**: Longer CI/CD times

---

## ADR-010: Versioning and Compatibility Strategy

### Status
**ACCEPTED** - Applied to v0.2.1 release

### Context
Need for stable API evolution while maintaining backward compatibility.

### Decision
Semantic versioning with clear compatibility guarantees and migration paths.

### Versioning Rules
- **Major (x.0.0)**: Breaking API changes
- **Minor (0.x.0)**: New features, backward compatible
- **Patch (0.0.x)**: Bug fixes, no API changes

### Compatibility Guarantees
- **Public API**: No breaking changes within major version
- **Storage Format**: Forward and backward compatible within major version
- **Configuration**: Additive changes only within major version

### Migration Strategy
- Legacy APIs maintained for one major version
- Clear migration guides for breaking changes
- Automated migration tools where possible

### Consequences
- **Positive**: Predictable API evolution
- **Positive**: User confidence in upgrades
- **Positive**: Clear compatibility expectations
- **Negative**: Some constraints on API design

---

**Document Version**: 1.0  
**Last Updated**: 2025-01-16  
**Review Frequency**: Quarterly  
**Authority**: Technical Architecture Committee