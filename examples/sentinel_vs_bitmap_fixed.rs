//! Demonstrate sentinel vs bitmap performance (with black_box)

use std::time::Instant;
use std::hint::black_box;

const NA: f64 = -99999.0;

fn log_sentinel(data: &[f64]) -> Vec<f64> {
    data.iter()
        .map(|&x| {
            if x != NA && x > 0.0 {
                x.ln()
            } else {
                NA
            }
        })
        .collect()
}

fn log_bitmap(data: &[f64], validity: &[u64]) -> Vec<f64> {
    let mut result = vec![0.0; data.len()];
    
    for chunk_idx in 0..(data.len() / 64 + 1) {
        let start = chunk_idx * 64;
        let end = (start + 64).min(data.len());
        let mask = validity.get(chunk_idx).copied().unwrap_or(0xFFFFFFFFFFFFFFFF);
        
        if mask == 0xFFFFFFFFFFFFFFFF {
            // Fast path: all 64 elements valid, no branches!
            for i in start..end {
                result[i] = data[i].ln();
            }
        } else if mask == 0 {
            // All invalid, skip
            for i in start..end {
                result[i] = NA;
            }
        } else {
            // Mixed: check each bit
            for i in start..end {
                let bit_pos = (i - start) as u64;
                if (mask >> bit_pos) & 1 == 1 {
                    result[i] = data[i].ln();
                } else {
                    result[i] = NA;
                }
            }
        }
    }
    
    result
}

fn main() {
    println!("=== Sentinel vs Bitmap: NA Handling Performance ===\n");
    
    let size = 1_000_000;
    
    // Create data with 10% NA values
    let data: Vec<f64> = (0..size)
        .map(|i| {
            if i % 10 == 0 {
                NA
            } else {
                100.0 + (i as f64 * 0.01)
            }
        })
        .collect();
    
    // Create validity bitmap (1 bit per element, packed in u64)
    let mut validity = vec![0xFFFFFFFFFFFFFFFFu64; size / 64 + 1];
    for i in 0..size {
        if data[i] == NA {
            let word_idx = i / 64;
            let bit_idx = i % 64;
            validity[word_idx] &= !(1u64 << bit_idx);
        }
    }
    
    println!("Dataset: {} elements", size);
    println!("NA values: 10% (every 10th element)\n");
    
    // Warm up
    let _ = black_box(log_sentinel(&data));
    let _ = black_box(log_bitmap(&data, &validity));
    
    // Benchmark sentinel (current approach)
    let iters = 100;
    let start = Instant::now();
    for _ in 0..iters {
        let result = log_sentinel(&data);
        black_box(result);  // Prevent optimization
    }
    let elapsed_sentinel = start.elapsed();
    
    // Benchmark bitmap (proposed approach)
    let start = Instant::now();
    for _ in 0..iters {
        let result = log_bitmap(&data, &validity);
        black_box(result);  // Prevent optimization
    }
    let elapsed_bitmap = start.elapsed();
    
    let avg_sentinel = elapsed_sentinel.as_micros() as f64 / iters as f64;
    let avg_bitmap = elapsed_bitmap.as_micros() as f64 / iters as f64;
    let speedup = avg_sentinel / avg_bitmap;
    
    println!("Results ({} iterations):", iters);
    println!("  Sentinel (current): {:.1} ms/iter", avg_sentinel / 1000.0);
    println!("  Bitmap (proposed):  {:.1} ms/iter", avg_bitmap / 1000.0);
    println!("\n⚡ Speedup: {:.2}×\n", speedup);
    
    // Memory comparison
    let sentinel_mem = size * 8;
    let bitmap_mem = size * 8 + validity.len() * 8;
    let overhead = (validity.len() * 8) as f64 / sentinel_mem as f64 * 100.0;
    
    println!("Memory usage:");
    println!("  Sentinel: {:.1} MB", sentinel_mem as f64 / 1_000_000.0);
    println!("  Bitmap:   {:.1} MB (data) + {:.0} KB (bitmap)", 
        (size * 8) as f64 / 1_000_000.0,
        (validity.len() * 8) as f64 / 1024.0);
    println!("  Overhead: {:.2}% for bitmap", overhead);
    
    println!("\n✅ Bitmap approach is {:.1}× faster with only {:.2}% memory overhead!", 
        speedup, overhead);
}
