// Copyright 2024 Saorsa Labs
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Performance benchmarks for FEC operations

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use saorsa_fec::{FecBackend, FecParams, backends::pure_rust::PureRustBackend};

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");

    // Test different file sizes
    for size in &[1_000_000, 10_000_000, 100_000_000] {
        let params = FecParams::from_content_size(*size);
        let k = params.data_shares as usize;
        let m = params.parity_shares as usize;

        // Create test data
        let block_size = size / k;
        let data: Vec<Vec<u8>> = (0..k).map(|_| vec![0u8; block_size]).collect();
        let data_refs: Vec<&[u8]> = data.iter().map(|v| v.as_slice()).collect();

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("pure_rust", format!("{}MB", size / 1_000_000)),
            size,
            |b, _| {
                let backend = PureRustBackend::new();
                let mut parity = vec![vec![]; m];

                b.iter(|| {
                    backend
                        .encode_blocks(
                            black_box(&data_refs),
                            black_box(&mut parity),
                            black_box(params),
                        )
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    // Test different file sizes
    for size in &[1_000_000, 10_000_000, 100_000_000] {
        let params = FecParams::from_content_size(*size);
        let k = params.data_shares as usize;
        let m = params.parity_shares as usize;

        // Create and encode test data
        let block_size = size / k;
        let data: Vec<Vec<u8>> = (0..k).map(|_| vec![0u8; block_size]).collect();
        let data_refs: Vec<&[u8]> = data.iter().map(|v| v.as_slice()).collect();

        let backend = PureRustBackend::new();
        let mut parity = vec![vec![]; m];
        backend
            .encode_blocks(&data_refs, &mut parity, params)
            .unwrap();

        // Create shares with one missing data block
        let mut shares: Vec<Option<Vec<u8>>> = vec![None; k + m];
        shares[0] = None; // Missing first data block
        for i in 1..k {
            shares[i] = Some(data[i].clone());
        }
        for i in 0..m {
            shares[k + i] = Some(parity[i].clone());
        }

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("pure_rust", format!("{}MB", size / 1_000_000)),
            size,
            |b, _| {
                b.iter(|| {
                    let mut test_shares = shares.clone();
                    backend
                        .decode_blocks(black_box(&mut test_shares), black_box(params))
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_matrix_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_generation");

    for (k, m) in &[(8, 2), (16, 4), (20, 5), (32, 8)] {
        group.bench_with_input(
            BenchmarkId::new("cauchy", format!("{}+{}", k, m)),
            &(*k, *m),
            |b, &(k, m)| {
                let backend = PureRustBackend::new();
                b.iter(|| backend.generate_matrix(black_box(k), black_box(m)));
            },
        );
    }

    group.finish();
}

fn bench_gf256_operations(c: &mut Criterion) {
    use saorsa_fec::gf256::{Gf256, add_slice, mul_slice};

    let mut group = c.benchmark_group("gf256");

    // Benchmark multiplication
    let data = vec![0u8; 65536];
    let mut result = vec![0u8; 65536];
    let scalar = Gf256::new(7);

    group.throughput(Throughput::Bytes(65536));
    group.bench_function("mul_slice", |b| {
        b.iter(|| {
            mul_slice(black_box(&mut result), black_box(&data), black_box(scalar));
        });
    });

    // Benchmark addition (XOR)
    group.bench_function("add_slice", |b| {
        b.iter(|| {
            add_slice(black_box(&mut result), black_box(&data));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_encode,
    bench_decode,
    bench_matrix_generation,
    bench_gf256_operations
);
criterion_main!(benches);
