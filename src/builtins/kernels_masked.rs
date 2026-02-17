//! Masked kernels with validity bitmaps
//!
//! Two-path strategy:
//! - *_no_nulls: Fast path when valid=None (zero overhead)
//! - *_masked: Masked path when valid=Some (check bits, not sentinels)

use crate::table::Bitmap;

// ===========================================================================
// DLOG: Log returns
// ===========================================================================

/// dlog fast path: No nulls (assumes all data valid and positive)
pub fn dlog_no_nulls(out: &mut [f64], x: &[f64], lag: usize) {
    let n = x.len();
    assert_eq!(out.len(), n);
    
    if lag == 0 || lag >= n {
        out.fill(f64::NAN);
        return;
    }

    // Prefix is invalid (no prior data)
    out[..lag].fill(f64::NAN);
    
    unsafe {
        let xp = x.as_ptr();
        let op = out.as_mut_ptr();
        
        // 🔥 CLEAN LOOP: No branches!
        for i in lag..n {
            let curr = *xp.add(i);
            let prev = *xp.add(i - lag);
            *op.add(i) = curr.ln() - prev.ln();
        }
    }
}

/// dlog masked path: Check validity bitmap
pub fn dlog_masked(
    out: &mut [f64],
    out_valid: &mut Bitmap,
    x: &[f64],
    x_valid: &Bitmap,
    lag: usize,
) {
    let n = x.len();
    assert_eq!(out.len(), n);
    assert_eq!(x_valid.len(), n);
    assert_eq!(out_valid.len(), n);

    if lag == 0 || lag >= n {
        out.fill(f64::NAN);
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

    // Main loop
    unsafe {
        let xp = x.as_ptr();
        let op = out.as_mut_ptr();

        for i in lag..n {
            let v_curr = x_valid.get(i);
            let v_prev = x_valid.get(i - lag);

            if v_curr && v_prev {
                // Both valid: compute result
                let curr = *xp.add(i);
                let prev = *xp.add(i - lag);
                *op.add(i) = curr.ln() - prev.ln();
                out_valid.set(i, true);
            } else {
                // Invalid: just set bit, don't write data (DON'T CARE)
                out_valid.set(i, false);
            }
        }
    }
}

// ===========================================================================
// UNARY OPS: ln, abs, etc.
// ===========================================================================

/// Generic unary operation (no nulls)
pub fn unary_no_nulls<F>(out: &mut [f64], x: &[f64], f: F)
where
    F: Fn(f64) -> f64,
{
    assert_eq!(out.len(), x.len());
    for i in 0..x.len() {
        out[i] = f(x[i]);
    }
}

/// Generic unary operation (masked)
pub fn unary_masked<F>(
    out: &mut [f64],
    out_valid: &mut Bitmap,
    x: &[f64],
    x_valid: &Bitmap,
    f: F,
)
where
    F: Fn(f64) -> f64,
{
    let n = x.len();
    assert_eq!(out.len(), n);
    assert_eq!(x_valid.len(), n);
    assert_eq!(out_valid.len(), n);

    for i in 0..n {
        if x_valid.get(i) {
            out[i] = f(x[i]);
            out_valid.set(i, true);
        } else {
            // Invalid: just set bit, don't write data (DON'T CARE)
            out_valid.set(i, false);
        }
    }
}

// ===========================================================================
// BINARY OPS: add, sub, mul, div
// ===========================================================================

/// Generic binary operation (no nulls)
pub fn binary_no_nulls<F>(out: &mut [f64], a: &[f64], b: &[f64], f: F)
where
    F: Fn(f64, f64) -> f64,
{
    assert_eq!(out.len(), a.len());
    assert_eq!(out.len(), b.len());
    
    for i in 0..a.len() {
        out[i] = f(a[i], b[i]);
    }
}

/// Generic binary operation (masked)
pub fn binary_masked<F>(
    out: &mut [f64],
    out_valid: &mut Bitmap,
    a: &[f64],
    a_valid: &Bitmap,
    b: &[f64],
    b_valid: &Bitmap,
    f: F,
)
where
    F: Fn(f64, f64) -> f64,
{
    let n = a.len();
    assert_eq!(out.len(), n);
    assert_eq!(b.len(), n);
    assert_eq!(a_valid.len(), n);
    assert_eq!(b_valid.len(), n);
    assert_eq!(out_valid.len(), n);

    for i in 0..n {
        let va = a_valid.get(i);
        let vb = b_valid.get(i);

        if va && vb {
            out[i] = f(a[i], b[i]);
            out_valid.set(i, true);
        } else {
            // Invalid: just set bit, don't write data (DON'T CARE)
            out_valid.set(i, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlog_no_nulls() {
        let x = vec![100.0, 101.0, 102.0, 103.0];
        let mut out = vec![0.0; 4];
        
        dlog_no_nulls(&mut out, &x, 1);
        
        assert!(out[0].is_nan());  // Prefix
        assert!((out[1] - (101.0_f64.ln() - 100.0_f64.ln())).abs() < 1e-10);
        assert!((out[2] - (102.0_f64.ln() - 101.0_f64.ln())).abs() < 1e-10);
    }

    #[test]
    fn test_dlog_masked() {
        let x = vec![100.0, 101.0, 102.0, 103.0];
        let mut x_valid = Bitmap::new_all_valid(4);
        x_valid.set(2, false);  // Mark index 2 as null
        
        let mut out = vec![0.0; 4];
        let mut out_valid = Bitmap::new_all_null(4);
        
        dlog_masked(&mut out, &mut out_valid, &x, &x_valid, 1);
        
        assert!(!out_valid.get(0));  // Prefix invalid
        assert!(out_valid.get(1));   // Valid
        assert!(!out_valid.get(2));  // x[2] is null
        assert!(!out_valid.get(3));  // x[3-1]=x[2] is null
    }

    #[test]
    fn test_unary_masked() {
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let mut x_valid = Bitmap::new_all_valid(4);
        x_valid.set(1, false);
        
        let mut out = vec![0.0; 4];
        let mut out_valid = Bitmap::new_all_null(4);
        
        unary_masked(&mut out, &mut out_valid, &x, &x_valid, |x| x * 2.0);
        
        assert!(out_valid.get(0));
        assert!(!out_valid.get(1));
        assert!(out_valid.get(2));
        assert_eq!(out[0], 2.0);
        assert_eq!(out[2], 6.0);
    }

    #[test]
    fn test_binary_masked() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![10.0, 20.0, 30.0, 40.0];
        
        let mut a_valid = Bitmap::new_all_valid(4);
        let mut b_valid = Bitmap::new_all_valid(4);
        a_valid.set(1, false);
        b_valid.set(2, false);
        
        let mut out = vec![0.0; 4];
        let mut out_valid = Bitmap::new_all_null(4);
        
        binary_masked(&mut out, &mut out_valid, &a, &a_valid, &b, &b_valid, |x, y| x + y);
        
        assert!(out_valid.get(0));   // Both valid
        assert!(!out_valid.get(1));  // a[1] null
        assert!(!out_valid.get(2));  // b[2] null
        assert!(out_valid.get(3));   // Both valid
        
        assert_eq!(out[0], 11.0);
        assert_eq!(out[3], 44.0);
    }
}
