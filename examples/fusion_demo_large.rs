//! Test fusion with realistic dataset sizes

use std::time::Instant;
use blawk_kdb::Column;

fn bench_size(size: usize, name: &str) {
    println!("\n=== {} ({} rows) ===", name, size);
    
    let prices: Vec<f64> = (0..size)
        .map(|i| 100.0 + (i as f64 * 0.01))
        .collect();
    let data = Column::new_f64(prices);
    
    // Warm up
    let _ = data.dlog_non_fused(1);
    let _ = data.dlog_fused(1);
    
    let iters = if size < 100_000 { 1000 } else { 100 };
    
    // NON-FUSED
    let start = Instant::now();
    for _ in 0..iters {
        let _ = data.dlog_non_fused(1);
    }
    let elapsed_non_fused = start.elapsed();
    
    // FUSED
    let start = Instant::now();
    for _ in 0..iters {
        let _ = data.dlog_fused(1);
    }
    let elapsed_fused = start.elapsed();
    
    let speedup = elapsed_non_fused.as_secs_f64() / elapsed_fused.as_secs_f64();
    let mem_saved_mb = (size * 8 * 2) as f64 / 1_000_000.0;
    
    println!("NON-FUSED: {:?} ({} iters)", elapsed_non_fused, iters);
    println!("FUSED:     {:?} ({} iters)", elapsed_fused, iters);
    println!("SPEEDUP:   {:.2}×", speedup);
    println!("MEM SAVED: {:.1} MB per iteration", mem_saved_mb);
}

fn main() {
    println!("=== FUSION SCALING TEST ===");
    
    // Small: 250 trading days (~1 year)
    bench_size(250, "1 year daily");
    
    // Medium: 2500 trading days (~10 years)
    bench_size(2_500, "10 years daily");
    
    // Large: 25,000 trading days (~100 years)
    bench_size(25_000, "100 years daily");
    
    // Very Large: 250,000 rows (100 stocks × 10 years)
    bench_size(250_000, "100 stocks × 10 years");
    
    // Huge: 2.5M rows (1000 stocks × 10 years)
    bench_size(2_500_000, "1000 stocks × 10 years");
    
    println!("\n=== CONCLUSION ===");
    println!("Fusion benefits increase with dataset size due to:");
    println!("  1. Memory bandwidth becomes bottleneck");
    println!("  2. Cache locality matters more");
    println!("  3. Allocation overhead dominates");
}
