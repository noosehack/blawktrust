//! Mathematical operations on columns (DEPRECATED - use ops.rs)
//!
//! This file kept for backward compatibility.
//! New code should use ops.rs with bitmap support.

use crate::Column;

const NA: f64 = -99999.0;

impl Column {
    /// Natural logarithm (element-wise) - OLD API (DEPRECATED)
    ///
    /// Use ops::log_column for production code.
    /// This old API kept for backward compatibility tests only.
    pub fn log(&self) -> Result<Self, &'static str> {
        let x = self.as_f64_slice()?;
        Ok(Column::from_f64_vec(log_kernel_old(x)))
    }

    /// Shift/lag operation - OLD API (DEPRECATED)
    pub fn shift(&self, lag: usize) -> Result<Self, &'static str> {
        let x = self.as_f64_slice()?;
        Ok(Column::from_f64_vec(shift_kernel_old(x, lag)))
    }

    /// Subtract two columns element-wise - OLD API (DEPRECATED)
    pub fn sub(&self, other: &Self) -> Result<Self, &'static str> {
        let a = self.as_f64_slice()?;
        let b = other.as_f64_slice()?;
        if a.len() != b.len() {
            return Err("Column length mismatch");
        }
        Ok(Column::from_f64_vec(sub_kernel_old(a, b)))
    }

    /// Log returns (NON-FUSED) - OLD API (DEPRECATED)
    pub fn dlog_non_fused(&self, lag: usize) -> Result<Self, &'static str> {
        let log_x = self.log()?;
        let log_x_lag = log_x.shift(lag)?;
        log_x.sub(&log_x_lag)
    }

    /// Log returns (FUSED) - OLD API (DEPRECATED)
    pub fn dlog_fused(&self, lag: usize) -> Result<Self, &'static str> {
        let x = self.as_f64_slice()?;
        Ok(Column::from_f64_vec(dlog_fused_kernel_old(x, lag)))
    }
}

// Monomorphic kernel functions - kdb-style tight loops
// No enum dispatch, no bounds checks, no iterator overhead

/// Log kernel: tight loop with prealloc + unchecked
#[inline(always)]
fn log_kernel_old(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![0.0; n];

    for i in 0..n {
        unsafe {
            let v = *x.get_unchecked(i);
            *out.get_unchecked_mut(i) = if v != NA && v > 0.0 { v.ln() } else { NA };
        }
    }
    out
}

/// Shift kernel: tight loop with prealloc + unchecked
#[inline(always)]
fn shift_kernel_old(x: &[f64], lag: usize) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![0.0; n];

    // First lag elements are NA
    for i in 0..lag.min(n) {
        unsafe {
            *out.get_unchecked_mut(i) = NA;
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

/// Subtract kernel: tight loop with prealloc + unchecked
#[inline(always)]
fn sub_kernel_old(a: &[f64], b: &[f64]) -> Vec<f64> {
    let n = a.len();
    debug_assert_eq!(n, b.len());

    let mut out = vec![0.0; n];

    for i in 0..n {
        unsafe {
            let x = *a.get_unchecked(i);
            let y = *b.get_unchecked(i);
            *out.get_unchecked_mut(i) = if x != NA && y != NA { x - y } else { NA };
        }
    }
    out
}

/// Dlog fused kernel: ONE loop, ONE allocation, NO intermediate vectors
///
/// Computes ln(x[i]) - ln(x[i-lag]) in a single pass.
/// This is the kdb-style fused operation.
#[inline(always)]
fn dlog_fused_kernel_old(x: &[f64], lag: usize) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![0.0; n];

    // First lag elements are NA (no prior value)
    for i in 0..lag.min(n) {
        unsafe {
            *out.get_unchecked_mut(i) = NA;
        }
    }

    // Fused: ln(curr) - ln(prev) in one pass
    for i in lag..n {
        unsafe {
            let curr = *x.get_unchecked(i);
            let prev = *x.get_unchecked(i - lag);

            *out.get_unchecked_mut(i) = if curr != NA && curr > 0.0 && prev != NA && prev > 0.0 {
                curr.ln() - prev.ln()
            } else {
                NA
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fused_matches_non_fused() {
        let data = vec![100.0, 102.0, 101.0, 103.0, 105.0];
        let col = Column::new_f64(data);

        let result_non_fused = col.dlog_non_fused(1).unwrap();
        let result_fused = col.dlog_fused(1).unwrap();

        let a = result_non_fused.f64_data();
        let b = result_fused.f64_data();
        for (i, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
            if x != NA {
                let diff: f64 = (x - y).abs();
                assert!(diff < 1e-10, "Mismatch at index {}: {} vs {}", i, x, y);
            }
        }
    }
}
