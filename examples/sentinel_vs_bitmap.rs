//! Demonstrate sentinel vs bitmap performance

use std::time::Instant;

const NA: f64 = -99999.0;

// Current approach: Sentinel values
fn log_sentinel(data: &[f64]) -> Vec<f64> {
    data.iter()
        .map(|&x| {
            if x != NA && x > 0.0 {  // Branch per element
                x.ln()
            } else {
                NA
            }
        })
        .collect()
}

// Bitmap approach: Validity mask
fn log_bitmap(data: &[f64], validity: &[u8]) -> Vec<f64> {
    let mut result = vec![0.0; data.len()];
    
    for chunk_idx in 0..(data.len() / 64 + 1) {
        let start = chunk_idx * 64;
        let end = (start + 64).min(data.len());
        let validity_byte = validity.get(chunk_idx).copied().unwrap_or(0xFF);
        
        if validity_byte == 0xFF {
            // Fast path: all valid, no branches!
            for i in start..end {
                result[i] = data[i].ln();
            }
        } else {
            // Slow path: check bits
            for i in start..end {
                let bit_idx = i % 64;
                if (validity_byte >> bit_idx) & 1 == 1 {
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
    println!("=== Sentinel vs Bitmap Performance ===\n");
    
    // Test with 1M elements, 10% NA values
    let size = 1_000_000;
    let data: Vec<f64> = (0..size)
        .map(|i| {
            if i % 10 == 0 {
                NA  // 10% NA values
            } else {
                100.0 + (i as f64 * 0.01)
            }
        })
        .collect();
    
    // Create validity bitmap (1 bit per element)
    let mut validity = vec![0xFFu8; size / 8 + 1];
    for i in 0..size {
        if data[i] == NA {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            validity[byte_idx] &= !(1 << bit_idx);
        }
    }
    
    println!("Dataset: {} elements, 10% NA values\n", size);
    
    // Benchmark sentinel
    let start = Instant::now();
    for _ in 0..100 {
        let _ = log_sentinel(&data);
    }
    let elapsed_sentinel = start.elapsed();
    
    // Benchmark bitmap
    let start = Instant::now();
    for _ in 0..100 {
        let _ = log_bitmap(&data, &validity);
    }
    let elapsed_bitmap = start.elapsed();
    
    println!("Sentinel approach: {:?}", elapsed_sentinel);
    println!("Bitmap approach:   {:?}", elapsed_bitmap);
    println!("\nSpeedup: {:.2}×\n", 
        elapsed_sentinel.as_secs_f64() / elapsed_bitmap.as_secs_f64());
    
    // Memory usage
    let sentinel_mem = size * 8;  // 8 bytes per f64
    let bitmap_mem = size * 8 + validity.len();  // data + bitmap
    
    println!("Memory usage:");
    println!("  Sentinel: {} MB", sentinel_mem / 1_000_000);
    println!("  Bitmap:   {} MB (+ {} KB for bitmap)", 
        sentinel_mem / 1_000_000, 
        validity.len() / 1024);
    println!("  Overhead: {:.1}% for bitmap", 
        (validity.len() as f64 / sentinel_mem as f64) * 100.0);
}
