//! Fusion Demo: Prove that kernel fusion provides 2-3× speedup

use blawk_kdb::Column;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn generate_test_data(size: usize) -> Column {
    let data: Vec<f64> = (0..size).map(|i| 100.0 + (i as f64 * 0.01)).collect();
    Column::F64(data)
}

fn bench_fusion(c: &mut Criterion) {
    let sizes = vec![1000, 10_000, 100_000, 1_000_000];

    let mut group = c.benchmark_group("dlog_fusion");

    for size in sizes {
        let data = generate_test_data(size);

        // Benchmark NON-FUSED (3 allocations)
        group.bench_with_input(BenchmarkId::new("non_fused", size), &size, |b, _| {
            b.iter(|| black_box(data.dlog_non_fused(1)))
        });

        // Benchmark FUSED (1 allocation)
        group.bench_with_input(BenchmarkId::new("fused", size), &size, |b, _| {
            b.iter(|| black_box(data.dlog_fused(1)))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_fusion);
criterion_main!(benches);
