//! Kernel microbenchmarks - pure slice operations, no CSV
//!
//! Run with: cargo bench --bench kernels
//!
//! Metrics:
//! - ns/element
//! - throughput (GB/s for memory-bound ops)
//! - fused vs unfused comparison

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// Tight loop kernels (kdb-style)
#[inline(always)]
fn log_kernel_tight(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![0.0; n];

    for i in 0..n {
        unsafe {
            *out.get_unchecked_mut(i) = (*x.get_unchecked(i)).ln();
        }
    }
    out
}

#[inline(always)]
fn shift_kernel_tight(x: &[f64], lag: usize) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![0.0; n];

    // First lag elements are NaN
    for i in 0..lag.min(n) {
        unsafe {
            *out.get_unchecked_mut(i) = f64::NAN;
        }
    }

    // Copy shifted values
    for i in lag..n {
        unsafe {
            *out.get_unchecked_mut(i) = *x.get_unchecked(i - lag);
        }
    }
    out
}

#[inline(always)]
fn sub_kernel_tight(a: &[f64], b: &[f64]) -> Vec<f64> {
    let n = a.len();
    debug_assert_eq!(n, b.len());

    let mut out = vec![0.0; n];

    for i in 0..n {
        unsafe {
            *out.get_unchecked_mut(i) = *a.get_unchecked(i) - *b.get_unchecked(i);
        }
    }
    out
}

#[inline(always)]
fn dlog_fused_kernel_tight(x: &[f64], lag: usize) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![0.0; n];

    // First lag elements are NaN (no prior value)
    for i in 0..lag.min(n) {
        unsafe {
            *out.get_unchecked_mut(i) = f64::NAN;
        }
    }

    // Fused log returns: ln(x[i]) - ln(x[i-lag]) in one pass
    for i in lag..n {
        unsafe {
            let curr = *x.get_unchecked(i);
            let prev = *x.get_unchecked(i - lag);
            *out.get_unchecked_mut(i) = curr.ln() - prev.ln();
        }
    }
    out
}

// Non-fused dlog for comparison
#[inline(always)]
fn dlog_non_fused_tight(x: &[f64], lag: usize) -> Vec<f64> {
    let log_x = log_kernel_tight(x);
    let log_x_lag = shift_kernel_tight(&log_x, lag);
    sub_kernel_tight(&log_x, &log_x_lag)
}

fn bench_log(c: &mut Criterion) {
    let mut group = c.benchmark_group("log_kernel");

    for size in [1_000, 10_000, 100_000, 1_000_000].iter() {
        let data: Vec<f64> = (0..*size).map(|i| 100.0 + (i as f64) * 0.01).collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let result = log_kernel_tight(black_box(&data));
                black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_shift(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_kernel");

    for size in [1_000, 10_000, 100_000, 1_000_000].iter() {
        let data: Vec<f64> = (0..*size).map(|i| i as f64).collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let result = shift_kernel_tight(black_box(&data), black_box(1));
                black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_sub(c: &mut Criterion) {
    let mut group = c.benchmark_group("sub_kernel");

    for size in [1_000, 10_000, 100_000, 1_000_000].iter() {
        let a: Vec<f64> = (0..*size).map(|i| i as f64 + 100.0).collect();
        let b: Vec<f64> = (0..*size).map(|i| i as f64).collect();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b_bench, _| {
            b_bench.iter(|| {
                let result = sub_kernel_tight(black_box(&a), black_box(&b));
                black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_dlog_fused_vs_unfused(c: &mut Criterion) {
    let mut group = c.benchmark_group("dlog_comparison");

    let size = 1_000_000;
    let data: Vec<f64> = (0..size).map(|i| 100.0 + (i as f64) * 0.01).collect();

    group.throughput(Throughput::Elements(size as u64));

    group.bench_function("dlog_fused", |b| {
        b.iter(|| {
            let result = dlog_fused_kernel_tight(black_box(&data), black_box(1));
            black_box(result);
        });
    });

    group.bench_function("dlog_non_fused", |b| {
        b.iter(|| {
            let result = dlog_non_fused_tight(black_box(&data), black_box(1));
            black_box(result);
        });
    });

    group.finish();
}

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    let size = 1_000_000;
    let data: Vec<f64> = (0..size).map(|i| 100.0 + (i as f64) * 0.01).collect();
    let bytes = (size * std::mem::size_of::<f64>()) as u64;

    group.throughput(Throughput::Bytes(bytes));

    group.bench_function("log_1M", |b| {
        b.iter(|| {
            let result = log_kernel_tight(black_box(&data));
            black_box(result);
        });
    });

    group.bench_function("dlog_fused_1M", |b| {
        b.iter(|| {
            let result = dlog_fused_kernel_tight(black_box(&data), black_box(1));
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_log,
    bench_shift,
    bench_sub,
    bench_dlog_fused_vs_unfused,
    bench_throughput
);
criterion_main!(benches);
