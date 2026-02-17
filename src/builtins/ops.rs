//! Column-level operations (kdb-style: embedded NaN, no bitmaps)
//!
//! All operations work directly on data vectors.
//! NaN propagation handled by IEEE 754 automatically.

use crate::table::Column;
use crate::builtins::kernels_masked::{dlog_no_nulls, unary_no_nulls};

/// dlog: Log returns (kdb-style)
///
/// NaN values propagate automatically via IEEE 754.
pub fn dlog_column(x: &Column, lag: usize) -> Column {
    let Column::F64(data) = x else {
        panic!("dlog_column: expected F64 column");
    };

    let n = data.len();
    let mut out_data = vec![0.0; n];
    dlog_no_nulls(&mut out_data, data, lag);
    Column::F64(out_data)
}

/// ln: Natural logarithm (kdb-style)
pub fn ln_column(x: &Column) -> Column {
    let Column::F64(data) = x else {
        panic!("ln_column: expected F64 column");
    };

    let n = data.len();
    let mut out_data = vec![0.0; n];
    unary_no_nulls(&mut out_data, data, |x| x.ln());
    Column::F64(out_data)
}

/// abs: Absolute value (kdb-style)
pub fn abs_column(x: &Column) -> Column {
    let Column::F64(data) = x else {
        panic!("abs_column: expected F64 column");
    };

    let n = data.len();
    let mut out_data = vec![0.0; n];
    unary_no_nulls(&mut out_data, data, |x| x.abs());
    Column::F64(out_data)
}

// ============================================================================
// Aggregations (kdb-style)
// ============================================================================

/// sum: Sum column (propagates NaN) — fast path
///
/// If any value is NaN, result is NaN. Uses tight loop with no branching.
#[inline]
pub fn sum(x: &Column) -> f64 {
    let Column::F64(data) = x else {
        panic!("sum: expected F64 column");
    };

    let mut result = 0.0;
    for &val in data {
        result += val;  // NaN propagates automatically
    }
    result
}

/// sum0: Sum column (ignores NaN) — explicit slower path
///
/// Skips NaN values. Only use when you explicitly want to ignore nulls.
#[inline]
pub fn sum0(x: &Column) -> f64 {
    let Column::F64(data) = x else {
        panic!("sum0: expected F64 column");
    };

    let mut result = 0.0;
    for &val in data {
        if !val.is_nan() {
            result += val;
        }
    }
    result
}

/// mean: Mean (propagates NaN) — fast path
///
/// If any value is NaN, result is NaN.
#[inline]
pub fn mean(x: &Column) -> f64 {
    let Column::F64(data) = x else {
        panic!("mean: expected F64 column");
    };

    if data.is_empty() {
        return f64::NAN;
    }

    let s = sum(x);
    s / (data.len() as f64)
}

/// mean0: Mean (ignores NaN) — explicit slower path
///
/// Skips NaN values. Returns NaN if all values are NaN.
#[inline]
pub fn mean0(x: &Column) -> f64 {
    let Column::F64(data) = x else {
        panic!("mean0: expected F64 column");
    };

    if data.is_empty() {
        return f64::NAN;
    }

    let mut s = 0.0;
    let mut count = 0;
    for &val in data {
        if !val.is_nan() {
            s += val;
            count += 1;
        }
    }

    if count == 0 {
        f64::NAN
    } else {
        s / (count as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sum_no_nulls() {
        let col = Column::new_f64(vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(sum(&col), 10.0);
    }

    #[test]
    fn test_sum_with_nan() {
        let col = Column::new_f64(vec![1.0, f64::NAN, 3.0]);
        assert!(sum(&col).is_nan());
    }

    #[test]
    fn test_sum0_with_nan() {
        let col = Column::new_f64(vec![1.0, f64::NAN, 3.0, 4.0]);
        assert_eq!(sum0(&col), 8.0);
    }

    #[test]
    fn test_sum0_all_nan() {
        let col = Column::new_f64(vec![f64::NAN, f64::NAN]);
        assert_eq!(sum0(&col), 0.0);
    }

    #[test]
    fn test_mean_no_nulls() {
        let col = Column::new_f64(vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(mean(&col), 2.5);
    }

    #[test]
    fn test_mean_with_nan() {
        let col = Column::new_f64(vec![1.0, f64::NAN, 3.0]);
        assert!(mean(&col).is_nan());
    }

    #[test]
    fn test_mean0_with_nan() {
        let col = Column::new_f64(vec![2.0, f64::NAN, 4.0, 6.0]);
        assert_eq!(mean0(&col), 4.0);
    }

    #[test]
    fn test_mean0_all_nan() {
        let col = Column::new_f64(vec![f64::NAN, f64::NAN]);
        assert!(mean0(&col).is_nan());
    }

    #[test]
    fn test_mean_empty() {
        let col = Column::new_f64(vec![]);
        assert!(mean(&col).is_nan());
    }

    #[test]
    fn test_mean0_empty() {
        let col = Column::new_f64(vec![]);
        assert!(mean0(&col).is_nan());
    }
}
