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
