# Changelog

All notable changes to this project will be documented in this file.

## [0.4.5] - 2024-01-19

### Changed
- Relaxed Clippy settings from ultra-pedantic to realistic configuration
- Updated CI workflow to use targeted Clippy flags for better developer experience
- Improved code quality checks while maintaining practicality

### Fixed
- Fixed bool assertion comparisons in tests
- Fixed manual div_ceil implementations to use built-in method
- Removed unused enumerate indices in test loops

### Developer Experience
- Clippy now uses practical linting levels:
  - Errors (-D) for: correctness, suspicious, complexity
  - Warnings (-W) for: performance, style
- Removed overly pedantic linting that doesn't add value
- Maintained high code quality standards

## [0.4.4] - 2024-01-19

### Changed
- Updated to reed-solomon-simd v3.0 for improved performance
- Removed WASM support temporarily due to reed-solomon-simd v3 compatibility

### Fixed
- Fixed incorrect share indexing in Reed-Solomon encoding (now uses 1-based indexing)
- Removed unused sqlx dependency to eliminate potential security vulnerabilities
- Fixed property tests for reed-solomon-simd v3 API

## Previous Versions

See git history for changes in earlier versions.