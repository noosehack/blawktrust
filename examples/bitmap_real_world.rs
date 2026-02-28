//! Real-world scenario: Bitmaps win with mostly-valid data

use std::hint::black_box;
use std::time::Instant;

const NA: f64 = -99999.0;

fn pipeline_sentinel(data: &[f64]) -> Vec<f64> {
    // Simulated pipeline: log -> abs -> sqrt (3 operations)
    let step1: Vec<f64> = data
        .iter()
        .map(|&x| if x != NA && x > 0.0 { x.ln() } else { NA })
        .collect();

    let step2: Vec<f64> = step1
        .iter()
        .map(|&x| if x != NA { x.abs() } else { NA })
        .collect();

    step2
        .iter()
        .map(|&x| if x != NA && x > 0.0 { x.sqrt() } else { NA })
        .collect()
}

fn pipeline_bitmap(data: &[f64], validity: &[u64]) -> Vec<f64> {
    let mut result = vec![0.0; data.len()];

    for chunk_idx in 0..(data.len() / 64 + 1) {
        let start = chunk_idx * 64;
        let end = (start + 64).min(data.len());
        let mask = validity
            .get(chunk_idx)
            .copied()
            .unwrap_or(0xFFFFFFFFFFFFFFFF);

        if mask == 0xFFFFFFFFFFFFFFFF {
            // ✅ Fast path: ALL 64 elements valid
            // Do 3 operations with ZERO branches!
            for i in start..end {
                let x = data[i].ln();
                let x = x.abs();
                result[i] = x.sqrt();
            }
        } else if mask == 0 {
            // All invalid
            for i in start..end {
                result[i] = NA;
            }
        } else {
            // Mixed: check bits
            for i in start..end {
                let bit_pos = (i - start) as u64;
                if (mask >> bit_pos) & 1 == 1 {
                    let x = data[i].ln();
                    let x = x.abs();
                    result[i] = x.sqrt();
                } else {
                    result[i] = NA;
                }
            }
        }
    }

    result
}

fn main() {
    println!("=== Real-World Scenario: Pipeline with Mostly-Valid Data ===\n");

    let size = 1_000_000;

    // Scenario 1: 99% valid (typical clean financial data)
    println!("📊 SCENARIO 1: 99% valid data (clean financial data)\n");

    let data1: Vec<f64> = (0..size)
        .map(|i| {
            if i % 100 == 0 {
                NA
            } else {
                100.0 + (i as f64 * 0.01)
            }
        })
        .collect();

    let mut validity1 = vec![0xFFFFFFFFFFFFFFFFu64; size / 64 + 1];
    for i in 0..size {
        if data1[i] == NA {
            validity1[i / 64] &= !(1u64 << (i % 64));
        }
    }

    let iters = 50;

    let start = Instant::now();
    for _ in 0..iters {
        black_box(pipeline_sentinel(&data1));
    }
    let elapsed_sentinel1 = start.elapsed();

    let start = Instant::now();
    for _ in 0..iters {
        black_box(pipeline_bitmap(&data1, &validity1));
    }
    let elapsed_bitmap1 = start.elapsed();

    let speedup1 = elapsed_sentinel1.as_secs_f64() / elapsed_bitmap1.as_secs_f64();
    println!(
        "  Sentinel (3 passes, check NA each time): {:.1} ms/iter",
        elapsed_sentinel1.as_millis() as f64 / iters as f64
    );
    println!(
        "  Bitmap (1 pass, check once per 64):      {:.1} ms/iter",
        elapsed_bitmap1.as_millis() as f64 / iters as f64
    );
    println!("  ⚡ Speedup: {:.2}×\n", speedup1);

    // Scenario 2: 100% valid (perfect data, no NAs)
    println!("📊 SCENARIO 2: 100% valid data (no NAs at all)\n");

    let data2: Vec<f64> = (0..size).map(|i| 100.0 + (i as f64 * 0.01)).collect();

    let validity2 = vec![0xFFFFFFFFFFFFFFFFu64; size / 64 + 1];

    let start = Instant::now();
    for _ in 0..iters {
        black_box(pipeline_sentinel(&data2));
    }
    let elapsed_sentinel2 = start.elapsed();

    let start = Instant::now();
    for _ in 0..iters {
        black_box(pipeline_bitmap(&data2, &validity2));
    }
    let elapsed_bitmap2 = start.elapsed();

    let speedup2 = elapsed_sentinel2.as_secs_f64() / elapsed_bitmap2.as_secs_f64();
    println!(
        "  Sentinel (still checks every element): {:.1} ms/iter",
        elapsed_sentinel2.as_millis() as f64 / iters as f64
    );
    println!(
        "  Bitmap (fast path, no branches):       {:.1} ms/iter",
        elapsed_bitmap2.as_millis() as f64 / iters as f64
    );
    println!("  ⚡ Speedup: {:.2}×\n", speedup2);

    println!("=== CONCLUSION ===");
    println!("Bitmap approach wins when:");
    println!("  ✅ Data is mostly valid (typical for clean financial data)");
    println!("  ✅ Multiple operations in pipeline (checks once, not 3 times)");
    println!("  ✅ Large datasets (memory bandwidth matters)");
    println!("\nSentinel approach acceptable when:");
    println!("  • Small datasets (<10K rows)");
    println!("  • Single operations (no pipeline)");
    println!("  • Backward compatibility needed");
}
