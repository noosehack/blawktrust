//! Fused kernels for common multi-op patterns (Step 3)
//!
//! These kernels eliminate intermediate passes by computing entire
//! pipelines in a single memory pass.
//!
//! Philosophy: "kdb primitive set" - small number of heavily-used
//! fused ops, optimized hard.

use crate::table::Bitmap;

// ===========================================================================
// DLOG_SCALE_ADD: a * dlog(x, lag) + b
// ===========================================================================
// Pattern: out = a * dlog(x, lag) + b
// Use case: Returns scaling, zscore prep, signal transforms
// Eliminates: materialized dlog vector, separate scale/add pass

/// dlog_scale_add fast path: No nulls
///
/// Computes: out[i] = a * (ln(x[i]) - ln(x[i-lag])) + b
///
/// Single pass through memory, no intermediate allocations.
pub fn dlog_scale_add_no_nulls(out: &mut [f64], x: &[f64], lag: usize, a: f64, b: f64) {
    let n = x.len();
    assert_eq!(out.len(), n);

    if lag == 0 || lag >= n {
        out.fill(f64::NAN);
        return;
    }

    // Prefix is invalid (no prior data)
    for out_val in &mut out[..lag] {
        *out_val = f64::NAN;
    }

    unsafe {
        let xp = x.as_ptr();
        let op = out.as_mut_ptr();

        // 🔥 FUSED LOOP: ln() -> diff -> scale -> add in ONE PASS
        for i in lag..n {
            let curr_ln = (*xp.add(i)).ln();
            let prev_ln = (*xp.add(i - lag)).ln();
            *op.add(i) = a * (curr_ln - prev_ln) + b;
        }
    }
}

/// dlog_scale_add masked path: Check validity bitmap
///
/// Validity: out.valid[i] = x.valid[i] & x.valid[i-lag]
/// Only writes data when valid (Step 1 contract)
pub fn dlog_scale_add_masked(
    out: &mut [f64],
    out_valid: &mut Bitmap,
    x: &[f64],
    x_valid: &Bitmap,
    lag: usize,
    a: f64,
    b: f64,
) {
    let n = x.len();
    assert_eq!(out.len(), n);
    assert_eq!(x_valid.len(), n);
    assert_eq!(out_valid.len(), n);

    if lag == 0 || lag >= n {
        // Mark all as invalid
        for w in 0..out_valid.words_len() {
            out_valid.bits_mut()[w] = 0;
        }
        return;
    }

    // Prefix invalid (just set validity bits, don't touch data)
    for i in 0..lag {
        out_valid.set(i, false);
    }

    unsafe {
        let xp = x.as_ptr();
        let op = out.as_mut_ptr();

        for i in lag..n {
            let v_curr = x_valid.get(i);
            let v_prev = x_valid.get(i - lag);

            if v_curr && v_prev {
                // Both valid: fused compute
                let curr_ln = (*xp.add(i)).ln();
                let prev_ln = (*xp.add(i - lag)).ln();
                *op.add(i) = a * (curr_ln - prev_ln) + b;
                out_valid.set(i, true);
            } else {
                // Invalid: just set bit, don't write data (Step 1)
                out_valid.set(i, false);
            }
        }
    }
}

// ===========================================================================
// LN_SCALE_ADD: a * ln(x) + b
// ===========================================================================
// Pattern: out = a * ln(x) + b
// Use case: Log transform with scaling/offset

/// ln_scale_add fast path: No nulls
pub fn ln_scale_add_no_nulls(out: &mut [f64], x: &[f64], a: f64, b: f64) {
    assert_eq!(out.len(), x.len());

    unsafe {
        let xp = x.as_ptr();
        let op = out.as_mut_ptr();

        for i in 0..x.len() {
            *op.add(i) = a * (*xp.add(i)).ln() + b;
        }
    }
}

/// ln_scale_add masked path
pub fn ln_scale_add_masked(
    out: &mut [f64],
    out_valid: &mut Bitmap,
    x: &[f64],
    x_valid: &Bitmap,
    a: f64,
    b: f64,
) {
    let n = x.len();
    assert_eq!(out.len(), n);
    assert_eq!(x_valid.len(), n);
    assert_eq!(out_valid.len(), n);

    unsafe {
        let xp = x.as_ptr();
        let op = out.as_mut_ptr();

        for i in 0..n {
            if x_valid.get(i) {
                *op.add(i) = a * (*xp.add(i)).ln() + b;
                out_valid.set(i, true);
            } else {
                out_valid.set(i, false);
            }
        }
    }
}

// ===========================================================================
// SUB_MUL_ADD: (x - y) * a + b
// ===========================================================================
// Pattern: out = (x - y) * a + b
// Use case: Spread construction, normalized residuals, linear transforms

/// sub_mul_add fast path: No nulls
pub fn sub_mul_add_no_nulls(out: &mut [f64], x: &[f64], y: &[f64], a: f64, b: f64) {
    assert_eq!(out.len(), x.len());
    assert_eq!(out.len(), y.len());

    unsafe {
        let xp = x.as_ptr();
        let yp = y.as_ptr();
        let op = out.as_mut_ptr();

        for i in 0..x.len() {
            *op.add(i) = (*xp.add(i) - *yp.add(i)) * a + b;
        }
    }
}

/// sub_mul_add masked path
#[allow(clippy::too_many_arguments)]
pub fn sub_mul_add_masked(
    out: &mut [f64],
    out_valid: &mut Bitmap,
    x: &[f64],
    x_valid: &Bitmap,
    y: &[f64],
    y_valid: &Bitmap,
    a: f64,
    b: f64,
) {
    let n = x.len();
    assert_eq!(out.len(), n);
    assert_eq!(y.len(), n);
    assert_eq!(x_valid.len(), n);
    assert_eq!(y_valid.len(), n);
    assert_eq!(out_valid.len(), n);

    unsafe {
        let xp = x.as_ptr();
        let yp = y.as_ptr();
        let op = out.as_mut_ptr();

        for i in 0..n {
            let vx = x_valid.get(i);
            let vy = y_valid.get(i);

            if vx && vy {
                *op.add(i) = (*xp.add(i) - *yp.add(i)) * a + b;
                out_valid.set(i, true);
            } else {
                out_valid.set(i, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlog_scale_add_no_nulls() {
        let x = vec![100.0, 101.0, 102.0, 103.0];
        let mut out = vec![0.0; 4];

        // Compute: 2.0 * dlog(x, 1) + 1.0
        dlog_scale_add_no_nulls(&mut out, &x, 1, 2.0, 1.0);

        assert!(out[0].is_nan()); // Prefix

        // Expected: 2.0 * (ln(101) - ln(100)) + 1.0
        let expected = 2.0 * (101.0_f64.ln() - 100.0_f64.ln()) + 1.0;
        assert!((out[1] - expected).abs() < 1e-10);
    }

    #[test]
    fn test_dlog_scale_add_masked() {
        let x = vec![100.0, 101.0, 102.0, 103.0];
        let mut x_valid = Bitmap::new_all_valid(4);
        x_valid.set(2, false); // Mark index 2 as null

        let mut out = vec![0.0; 4];
        let mut out_valid = Bitmap::new_all_null(4);

        dlog_scale_add_masked(&mut out, &mut out_valid, &x, &x_valid, 1, 2.0, 1.0);

        assert!(!out_valid.get(0)); // Prefix invalid
        assert!(out_valid.get(1)); // Valid
        assert!(!out_valid.get(2)); // x[2] is null
        assert!(!out_valid.get(3)); // Depends on x[2]
    }

    #[test]
    fn test_ln_scale_add_no_nulls() {
        let x = vec![1.0, std::f64::consts::E, 10.0];
        let mut out = vec![0.0; 3];

        // Compute: 2.0 * ln(x) + 1.0
        ln_scale_add_no_nulls(&mut out, &x, 2.0, 1.0);

        assert!((out[0] - 1.0).abs() < 1e-10); // 2*ln(1) + 1 = 1
        assert!((out[1] - 3.0).abs() < 1e-10); // 2*ln(e) + 1 = 3
        let expected = 2.0 * 10.0_f64.ln() + 1.0;
        assert!((out[2] - expected).abs() < 1e-10);
    }

    #[test]
    fn test_sub_mul_add_no_nulls() {
        let x = vec![10.0, 20.0, 30.0];
        let y = vec![1.0, 2.0, 3.0];
        let mut out = vec![0.0; 3];

        // Compute: (x - y) * 2.0 + 1.0
        sub_mul_add_no_nulls(&mut out, &x, &y, 2.0, 1.0);

        assert_eq!(out[0], (10.0 - 1.0) * 2.0 + 1.0); // 19.0
        assert_eq!(out[1], (20.0 - 2.0) * 2.0 + 1.0); // 37.0
        assert_eq!(out[2], (30.0 - 3.0) * 2.0 + 1.0); // 55.0
    }

    #[test]
    fn test_sub_mul_add_masked() {
        let x = vec![10.0, 20.0, 30.0];
        let y = vec![1.0, 2.0, 3.0];

        let x_valid = Bitmap::new_all_valid(3);
        let mut y_valid = Bitmap::new_all_valid(3);
        y_valid.set(1, false); // y[1] is null

        let mut out = vec![0.0; 3];
        let mut out_valid = Bitmap::new_all_null(3);

        sub_mul_add_masked(
            &mut out,
            &mut out_valid,
            &x,
            &x_valid,
            &y,
            &y_valid,
            2.0,
            1.0,
        );

        assert!(out_valid.get(0)); // Both valid
        assert!(!out_valid.get(1)); // y[1] null
        assert!(out_valid.get(2)); // Both valid
    }
}
