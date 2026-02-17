//! Column-level operations (automatic fast-path dispatch)
//!
//! Two API styles:
//! - `*_column()`: Allocating (backward compat)
//! - `*_into()`: Non-allocating using Scratch (zero-alloc pipelines)

use crate::table::{Column, Bitmap};
use crate::builtins::kernels_masked::{dlog_no_nulls, dlog_masked, unary_no_nulls, unary_masked};
use crate::builtins::kernels_fused::{
    dlog_scale_add_no_nulls, dlog_scale_add_masked,
    ln_scale_add_no_nulls, ln_scale_add_masked,
    sub_mul_add_no_nulls, sub_mul_add_masked,
};
use crate::builtins::kernels_wordwise::{dlog_wordwise, dlog_scale_add_wordwise};
use crate::builtins::Scratch;

/// dlog: Log returns with automatic fast-path dispatch
pub fn dlog_column(x: &Column, lag: usize) -> Column {
    let Column::F64 { data, valid } = x else { 
        panic!("dlog_column: expected F64 column"); 
    };
    
    let n = data.len();
    let mut out_data = vec![0.0; n];

    match valid {
        // 🔥 FAST PATH: No nulls!
        None => {
            dlog_no_nulls(&mut out_data, data, lag);
            Column::F64 { 
                data: out_data, 
                valid: None 
            }
        }
        
        // Masked path: Has nulls
        Some(xv) => {
            let mut out_valid = Bitmap::new_all_null(n);
            dlog_masked(&mut out_data, &mut out_valid, data, xv, lag);
            Column::F64 { 
                data: out_data, 
                valid: Some(out_valid) 
            }
        }
    }
}

/// ln: Natural logarithm with automatic dispatch
pub fn ln_column(x: &Column) -> Column {
    let Column::F64 { data, valid } = x else { 
        panic!("ln_column: expected F64 column"); 
    };
    
    let n = data.len();
    let mut out_data = vec![0.0; n];

    match valid {
        None => {
            unary_no_nulls(&mut out_data, data, |x| x.ln());
            Column::F64 { data: out_data, valid: None }
        }
        Some(xv) => {
            let mut out_valid = Bitmap::new_all_null(n);
            unary_masked(&mut out_data, &mut out_valid, data, xv, |x| x.ln());
            Column::F64 { data: out_data, valid: Some(out_valid) }
        }
    }
}

/// abs: Absolute value with automatic dispatch
pub fn abs_column(x: &Column) -> Column {
    let Column::F64 { data, valid } = x else { 
        panic!("abs_column: expected F64 column"); 
    };
    
    let n = data.len();
    let mut out_data = vec![0.0; n];

    match valid {
        None => {
            unary_no_nulls(&mut out_data, data, |x| x.abs());
            Column::F64 { data: out_data, valid: None }
        }
        Some(xv) => {
            // Validity passes through (abs doesn't create new nulls)
            unary_no_nulls(&mut out_data, data, |x| x.abs());
            Column::F64 { data: out_data, valid: Some(xv.clone()) }
        }
    }
}

// ===========================================================================
// NON-ALLOCATING "INTO" API (Step 2)
// ===========================================================================

/// dlog_into: Log returns (non-allocating)
///
/// Writes result into `out` column, reusing buffers from `scratch`.
/// After warmup, allocates ~0.
pub fn dlog_into(out: &mut Column, x: &Column, lag: usize, scratch: &mut Scratch) {
    let Column::F64 { data: x_data, valid: x_valid } = x else {
        panic!("dlog_into: expected F64 column");
    };

    let n = x_data.len();

    match x_valid {
        // 🔥 FAST PATH: No nulls
        None => {
            let mut out_data = scratch.get_f64(n);
            dlog_no_nulls(&mut out_data, x_data, lag);
            *out = Column::F64 {
                data: out_data,
                valid: None,
            };
        }

        // Masked path: word-wise + uninit (Step 2 optimization)
        Some(xv) => {
            let mut out_data = scratch.get_f64_uninit(n);
            unsafe { out_data.set_len(n); }
            let mut out_valid = scratch.get_bitmap(n);
            dlog_wordwise(&mut out_data, &mut out_valid, x_data, xv, lag);
            *out = Column::F64 {
                data: out_data,
                valid: Some(out_valid),
            };
        }
    }
}

/// ln_into: Natural logarithm (non-allocating)
pub fn ln_into(out: &mut Column, x: &Column, scratch: &mut Scratch) {
    let Column::F64 { data: x_data, valid: x_valid } = x else {
        panic!("ln_into: expected F64 column");
    };

    let n = x_data.len();
    let mut out_data = scratch.get_f64(n);

    match x_valid {
        None => {
            unary_no_nulls(&mut out_data, x_data, |x| x.ln());
            *out = Column::F64 {
                data: out_data,
                valid: None,
            };
        }
        Some(xv) => {
            let mut out_valid = scratch.get_bitmap(n);
            unary_masked(&mut out_data, &mut out_valid, x_data, xv, |x| x.ln());
            *out = Column::F64 {
                data: out_data,
                valid: Some(out_valid),
            };
        }
    }
}

/// abs_into: Absolute value (non-allocating)
pub fn abs_into(out: &mut Column, x: &Column, scratch: &mut Scratch) {
    let Column::F64 { data: x_data, valid: x_valid } = x else {
        panic!("abs_into: expected F64 column");
    };

    let n = x_data.len();
    let mut out_data = scratch.get_f64(n);

    match x_valid {
        None => {
            unary_no_nulls(&mut out_data, x_data, |x| x.abs());
            *out = Column::F64 {
                data: out_data,
                valid: None,
            };
        }
        Some(xv) => {
            unary_no_nulls(&mut out_data, x_data, |x| x.abs());
            *out = Column::F64 {
                data: out_data,
                valid: Some(xv.clone()),
            };
        }
    }
}

// ===========================================================================
// FUSED OPERATIONS (Step 3)
// ===========================================================================

/// dlog_scale_add_into: Fused a * dlog(x, lag) + b (non-allocating)
///
/// Computes entire pipeline in single pass:
/// - out = a * dlog(x, lag) + b
///
/// Eliminates:
/// - Materialized dlog intermediate
/// - Separate scale/add pass
/// - Extra validity operations
///
/// Use case: Returns scaling, zscore prep, signal transforms
pub fn dlog_scale_add_into(
    out: &mut Column,
    x: &Column,
    lag: usize,
    a: f64,
    b: f64,
    scratch: &mut Scratch,
) {
    let Column::F64 { data: x_data, valid: x_valid } = x else {
        panic!("dlog_scale_add_into: expected F64 column");
    };

    let n = x_data.len();

    match x_valid {
        // 🔥 FAST PATH: No nulls, fused computation
        None => {
            let mut out_data = scratch.get_f64(n);
            dlog_scale_add_no_nulls(&mut out_data, x_data, lag, a, b);
            *out = Column::F64 {
                data: out_data,
                valid: None,
            };
        }

        // Masked path: word-wise + uninit (Step 2 optimization)
        Some(xv) => {
            let mut out_data = scratch.get_f64_uninit(n);
            unsafe { out_data.set_len(n); }
            let mut out_valid = scratch.get_bitmap(n);
            dlog_scale_add_wordwise(&mut out_data, &mut out_valid, x_data, xv, lag, a, b);
            *out = Column::F64 {
                data: out_data,
                valid: Some(out_valid),
            };
        }
    }
}

/// ln_scale_add_into: Fused a * ln(x) + b (non-allocating)
pub fn ln_scale_add_into(
    out: &mut Column,
    x: &Column,
    a: f64,
    b: f64,
    scratch: &mut Scratch,
) {
    let Column::F64 { data: x_data, valid: x_valid } = x else {
        panic!("ln_scale_add_into: expected F64 column");
    };

    let n = x_data.len();
    let mut out_data = scratch.get_f64(n);

    match x_valid {
        None => {
            ln_scale_add_no_nulls(&mut out_data, x_data, a, b);
            *out = Column::F64 {
                data: out_data,
                valid: None,
            };
        }
        Some(xv) => {
            let mut out_valid = scratch.get_bitmap(n);
            ln_scale_add_masked(&mut out_data, &mut out_valid, x_data, xv, a, b);
            *out = Column::F64 {
                data: out_data,
                valid: Some(out_valid),
            };
        }
    }
}

/// sub_mul_add_into: Fused (x - y) * a + b (non-allocating)
pub fn sub_mul_add_into(
    out: &mut Column,
    x: &Column,
    y: &Column,
    a: f64,
    b: f64,
    scratch: &mut Scratch,
) {
    let Column::F64 { data: x_data, valid: x_valid } = x else {
        panic!("sub_mul_add_into: expected F64 column");
    };
    let Column::F64 { data: y_data, valid: y_valid } = y else {
        panic!("sub_mul_add_into: expected F64 column");
    };

    let n = x_data.len();
    assert_eq!(y_data.len(), n);

    let mut out_data = scratch.get_f64(n);

    match (x_valid, y_valid) {
        // Both no nulls
        (None, None) => {
            sub_mul_add_no_nulls(&mut out_data, x_data, y_data, a, b);
            *out = Column::F64 {
                data: out_data,
                valid: None,
            };
        }

        // At least one has nulls
        _ => {
            // Create temporary all-valid bitmaps for columns without nulls
            let x_bm;
            let y_bm;
            let x_valid_ref = match x_valid {
                Some(bm) => bm,
                None => {
                    x_bm = Bitmap::new_all_valid(n);
                    &x_bm
                }
            };
            let y_valid_ref = match y_valid {
                Some(bm) => bm,
                None => {
                    y_bm = Bitmap::new_all_valid(n);
                    &y_bm
                }
            };

            let mut out_valid = scratch.get_bitmap(n);
            sub_mul_add_masked(
                &mut out_data,
                &mut out_valid,
                x_data,
                x_valid_ref,
                y_data,
                y_valid_ref,
                a,
                b,
            );
            *out = Column::F64 {
                data: out_data,
                valid: Some(out_valid),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlog_column_no_nulls() {
        let x = Column::new_f64(vec![100.0, 101.0, 102.0, 103.0]);
        let result = dlog_column(&x, 1);
        
        assert!(result.is_all_valid());
        let data = result.f64_data();
        assert!(data[0].is_nan());
        assert!((data[1] - (101.0_f64.ln() - 100.0_f64.ln())).abs() < 1e-10);
    }

    #[test]
    fn test_dlog_column_with_nulls() {
        let mut bm = Bitmap::new_all_valid(4);
        bm.set(2, false);
        let x = Column::new_f64_masked(vec![100.0, 101.0, 102.0, 103.0], bm);
        
        let result = dlog_column(&x, 1);
        
        let valid = result.validity().unwrap();
        assert!(!valid.get(0));  // Prefix
        assert!(valid.get(1));   // Valid
        assert!(!valid.get(2));  // x[2] null
        assert!(!valid.get(3));  // Depends on x[2]
    }

    #[test]
    fn test_ln_column() {
        let x = Column::new_f64(vec![1.0, std::f64::consts::E, 10.0]);
        let result = ln_column(&x);

        let data = result.f64_data();
        assert!((data[0] - 0.0).abs() < 1e-10);
        assert!((data[1] - 1.0).abs() < 1e-10);
        assert!((data[2] - 10.0_f64.ln()).abs() < 1e-10);
    }

    #[test]
    fn test_dlog_into_reuses_buffers() {
        let mut scratch = Scratch::new();
        let x = Column::new_f64(vec![100.0, 101.0, 102.0, 103.0]);
        let mut out = Column::new_f64(vec![]);

        // First call allocates
        dlog_into(&mut out, &x, 1, &mut scratch);
        assert!(out.is_all_valid());

        // Extract and return buffers to scratch
        if let Column::F64 { data, valid } = out {
            scratch.return_f64(data);
            if let Some(bm) = valid {
                scratch.return_bitmap(bm);
            }
            out = Column::new_f64(vec![]);
        }

        // Second call reuses (zero allocation!)
        let stats_before = scratch.stats();
        assert_eq!(stats_before.f64_bufs, 1);  // Buffer in pool

        dlog_into(&mut out, &x, 1, &mut scratch);

        // Verify correctness
        assert!(out.is_all_valid());
        let data = out.f64_data();
        assert!(data[0].is_nan());
        assert!((data[1] - (101.0_f64.ln() - 100.0_f64.ln())).abs() < 1e-10);
    }

    #[test]
    fn test_ln_into_zero_alloc_after_warmup() {
        let mut scratch = Scratch::new();
        let x = Column::new_f64(vec![1.0, 2.0, 3.0]);
        let mut out = Column::new_f64(vec![]);

        // Warmup: First call allocates
        ln_into(&mut out, &x, &mut scratch);

        // Return to scratch
        if let Column::F64 { data, .. } = out {
            scratch.return_f64(data);
            out = Column::new_f64(vec![]);
        }

        // Second call reuses
        ln_into(&mut out, &x, &mut scratch);

        // Verify
        let data = out.f64_data();
        assert!((data[0] - 1.0_f64.ln()).abs() < 1e-10);
        assert!((data[1] - 2.0_f64.ln()).abs() < 1e-10);
    }
}
