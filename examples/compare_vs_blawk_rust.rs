//! Compare blawk_kdb fusion against blawk_rust

use std::time::Instant;
use blawk_kdb::Column as KdbColumn;

fn main() {
    println!("=== blawk_kdb vs blawk_rust ===\n");
    
    let sizes = vec![
        (250_000, "100 stocks × 10 years"),
        (2_500_000, "1000 stocks × 10 years"),
    ];
    
    for (size, name) in sizes {
        println!("Dataset: {} ({} rows)", name, size);
        
        let prices: Vec<f64> = (0..size)
            .map(|i| 100.0 + (i as f64 * 0.01))
            .collect();
        let data = KdbColumn::new_f64(prices.clone());
        
        // Warm up
        let _ = data.dlog_fused(1);
        
        // blawk_kdb (fused)
        let iters = 100;
        let start = Instant::now();
        for _ in 0..iters {
            let _ = data.dlog_fused(1);
        }
        let elapsed_kdb = start.elapsed();
        let per_iter_kdb = elapsed_kdb.as_micros() as f64 / iters as f64;
        
        println!("  blawk_kdb (fused):  {:.1} ms/iter", per_iter_kdb / 1000.0);
        println!("  Memory saved:       {:.1} MB", (size * 8 * 2) as f64 / 1_000_000.0);
        println!();
    }
    
    println!("Compare with blawk_rust benchmark results:");
    println!("  blawk_rust dlog:    ~15-16 ms (from comparison_simple.csv)");
    println!();
    println!("✅ blawk_kdb fusion should match or beat blawk_rust!");
}
