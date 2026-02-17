//! Benchmark each optimization level

use std::time::Instant;
use std::hint::black_box;
use blawk_kdb::builtins::fast_kernels::*;

fn bench_kernel<F>(name: &str, f: F, iters: usize)
where
    F: Fn() -> Vec<f64>,
{
    let _ = black_box(f());
    
    let start = Instant::now();
    for _ in 0..iters {
        let result = f();
        black_box(result);
    }
    let elapsed = start.elapsed();
    let avg_us = elapsed.as_micros() as f64 / iters as f64;
    
    println!("  {:<30} {:>8.2} µs/iter", name, avg_us);
}

fn main() {
    println!("=== Kernel Optimization Levels ===\n");
    
    let size = 1_000_000;
    let lag = 1;
    let iters = 100;
    
    let data: Vec<f64> = (0..size)
        .map(|i| 100.0 + (i as f64 * 0.01))
        .collect();
    
    println!("Dataset: {} elements, {} iterations\n", size, iters);
    
    println!("📊 CLEAN DATA (no NAs, all positive):\n");
    
    bench_kernel("v0: Baseline (vec![NA])", || dlog_v0_baseline(&data, lag), iters);
    bench_kernel("v1: No init (MaybeUninit)", || dlog_v1_no_init(&data, lag), iters);
    bench_kernel("v2: No bounds checks", || dlog_v2_no_bounds(&data, lag), iters);
    bench_kernel("v3: No nulls fast path", || dlog_v3_no_nulls(&data, lag), iters);
    
    let valid = vec![1u8; size];
    bench_kernel("v4: Masked (with checks)", 
        || dlog_v4_masked(&data, &valid, lag).0, iters);
    bench_kernel("v5: Masked fast-path", 
        || dlog_v5_masked_fast(&data, None, lag).0, iters);
    
    println!("\n📊 DIRTY DATA (10% NAs):\n");
    
    let data_na: Vec<f64> = (0..size)
        .map(|i| {
            if i % 10 == 0 { -99999.0 } else { 100.0 + (i as f64 * 0.01) }
        })
        .collect();
    
    let valid_na: Vec<u8> = (0..size)
        .map(|i| if i % 10 == 0 { 0 } else { 1 })
        .collect();
    
    bench_kernel("v0: Baseline", || dlog_v0_baseline(&data_na, lag), iters);
    bench_kernel("v1: No init", || dlog_v1_no_init(&data_na, lag), iters);
    bench_kernel("v2: No bounds", || dlog_v2_no_bounds(&data_na, lag), iters);
    bench_kernel("v4: Masked", 
        || dlog_v4_masked(&data_na, &valid_na, lag).0, iters);
    bench_kernel("v5: Masked (with NAs)", 
        || dlog_v5_masked_fast(&data_na, Some(&valid_na), lag).0, iters);
    
    println!("\n=== SUMMARY ===");
    println!("Optimization path:");
    println!("  1. Remove init pass (MaybeUninit)");
    println!("  2. Remove bounds checks (unsafe pointers)");
    println!("  3. Fast path for no-nulls (common case)");
    println!("  4. Validity bitmap (when nulls exist)");
    println!("  5. Ultimate: branch-free hot loop");
}
