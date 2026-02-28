//! Quick demonstration of fusion speedup

use blawk_kdb::Column;
use std::time::Instant;

fn main() {
    println!("=== FUSION DEMO: blawk_kdb ===\n");

    // Test with realistic financial data size
    let size = 10_000; // ~10 years of daily data
    println!("Dataset: {} rows (typical stock price history)\n", size);

    // Generate test data (stock prices)
    let prices: Vec<f64> = (0..size)
        .map(|i| 100.0 + (i as f64 * 0.01) + ((i as f64 / 10.0).sin() * 5.0))
        .collect();
    let data = Column::new_f64(prices);

    // Warm up
    let _ = data.dlog_non_fused(1);
    let _ = data.dlog_fused(1);

    // Benchmark NON-FUSED (current blawk_rust approach)
    println!("🔴 NON-FUSED (3 passes, 3 allocations):");
    let start = Instant::now();
    for _ in 0..100 {
        let _ = data.dlog_non_fused(1);
    }
    let elapsed_non_fused = start.elapsed();
    println!(
        "   Time: {:?} (avg: {:?}/iter)",
        elapsed_non_fused,
        elapsed_non_fused / 100
    );
    println!(
        "   Memory: {} KB per iteration (2 intermediate allocations)",
        (size * 8 * 2) / 1024
    );

    // Benchmark FUSED (blawk_kdb approach)
    println!("\n🟢 FUSED (1 pass, 1 allocation):");
    let start = Instant::now();
    for _ in 0..100 {
        let _ = data.dlog_fused(1);
    }
    let elapsed_fused = start.elapsed();
    println!(
        "   Time: {:?} (avg: {:?}/iter)",
        elapsed_fused,
        elapsed_fused / 100
    );
    println!(
        "   Memory: {} KB per iteration (0 intermediate allocations)",
        0
    );

    // Calculate speedup
    let speedup = elapsed_non_fused.as_secs_f64() / elapsed_fused.as_secs_f64();
    println!("\n⚡ SPEEDUP: {:.2}× faster", speedup);
    println!(
        "   Memory saved: {} KB per iteration",
        (size * 8 * 2) / 1024
    );

    // Verify correctness
    println!("\n✅ CORRECTNESS CHECK:");
    let result_non_fused = data.dlog_non_fused(1);
    let result_fused = data.dlog_fused(1);

    match (result_non_fused, result_fused) {
        (Column::F64 { data: a, .. }, Column::F64 { data: b, .. }) => {
            let mut max_diff: f64 = 0.0;
            for (&x, &y) in a.iter().zip(b.iter()) {
                if x != -99999.0 {
                    let diff = (x - y).abs();
                    max_diff = max_diff.max(diff);
                }
            }
            println!("   Max difference: {:.2e} (should be ~0)", max_diff);
            if max_diff < 1e-10 {
                println!("   ✓ Results are IDENTICAL");
            } else {
                println!("   ✗ Results differ!");
            }
        }
    }

    println!("\n=== CONCLUSION ===");
    if speedup > 1.5 {
        println!(
            "✅ Fusion provides {:.2}× speedup - BLUEPRINT VALIDATED!",
            speedup
        );
        println!("   Next: Implement IR and auto-fusion for all operations");
    } else {
        println!("⚠️  Speedup is only {:.2}× - investigate overhead", speedup);
    }
}
