//! Orientation-aware operations
//!
//! Operations that dispatch based on TableView orientation.
//! Demonstrates the O(1) orientation system in action.

use crate::table::{Column, Table, TableView, OriClass};
use crate::builtins::{dlog_column, wmean0};

/// Sum operation with orientation-aware dispatch
///
/// # Behavior by orientation:
/// - ColwiseLike (H, N, _N, _H): Sum down each column → output has ncols values
/// - RowwiseLike (Z, S, _Z, _S): Sum across each row → output has nrows values
/// - Real (R): Sum all values → output is single scalar
/// - Each (X): Not defined (broadcast mode, no vector structure for aggregation)
///
/// # Example:
/// ```
/// use blawktrust::{Table, TableView, Column, ORI_H, ORI_Z};
/// use blawktrust::builtins::ori_ops::sum;
///
/// let table = Table::new(
///     vec!["a".to_string(), "b".to_string()],
///     vec![
///         Column::F64(vec![1.0, 2.0, 3.0]),
///         Column::F64(vec![4.0, 5.0, 6.0]),
///     ]
/// );
///
/// // H orientation: sum columns
/// let view_h = TableView::with_ori(table.clone(), ORI_H);
/// let result = sum(&view_h);
/// // result = [6.0, 15.0] (sum of each column)
///
/// // Z orientation: sum rows
/// let view_z = TableView::with_ori(table, ORI_Z);
/// let result = sum(&view_z);
/// // result = [5.0, 7.0, 9.0] (sum of each row)
/// ```
pub fn sum(view: &TableView) -> Column {
    match view.ori_class() {
        OriClass::ColwiseLike => sum_colwise(&view.table),
        OriClass::RowwiseLike => sum_rowwise_tiled(&view.table),
        OriClass::Real => sum_scalar(&view.table),
        OriClass::Each => panic!("sum not defined for Each (X) orientation - use for broadcast context only"),
    }
}

/// Sum each column (ColwiseLike mode)
///
/// Fast path: columns are contiguous in memory.
/// Output has one value per column.
fn sum_colwise(table: &Table) -> Column {
    let ncols = table.col_count();
    let mut result = Vec::with_capacity(ncols);

    for col in &table.columns {
        match col {
            Column::F64(data) => {
                // Sum this column, skipping NaN values
                let mut sum = 0.0;
                let mut has_valid = false;
                for &val in data {
                    if !val.is_nan() {
                        sum += val;
                        has_valid = true;
                    }
                }
                result.push(if has_valid { sum } else { f64::NAN });
            }
            Column::Date(_) | Column::Timestamp(_) => {
                // Non-numeric columns: output NA
                result.push(f64::NAN);
            }
        }
    }

    Column::F64(result)
}

/// Sum each row (RowwiseLike mode) with tiling
///
/// Cache-friendly tiled implementation:
/// - Process 128 rows at a time
/// - Accumulate within each tile
/// - Reduces cache misses on wide tables
///
/// Output has one value per row.
fn sum_rowwise_tiled(table: &Table) -> Column {
    const TILE_SIZE: usize = 128;

    let nrows = table.row_count();
    let ncols = table.col_count();
    let mut result = vec![0.0; nrows];

    if nrows == 0 || ncols == 0 {
        return Column::F64(result);
    }

    // Extract F64 columns (skip temporal columns)
    let f64_cols: Vec<&[f64]> = table.columns.iter()
        .filter_map(|col| match col {
            Column::F64(data) => Some(data.as_slice()),
            _ => None,
        })
        .collect();

    if f64_cols.is_empty() {
        // No numeric columns: all NaN
        result.iter_mut().for_each(|x| *x = f64::NAN);
        return Column::F64(result);
    }

    // Process in tiles for cache efficiency
    for tile_start in (0..nrows).step_by(TILE_SIZE) {
        let tile_end = (tile_start + TILE_SIZE).min(nrows);

        for row in tile_start..tile_end {
            let mut sum = 0.0;
            let mut has_valid = false;

            for col_data in &f64_cols {
                let val = col_data[row];
                if !val.is_nan() {
                    sum += val;
                    has_valid = true;
                }
            }

            result[row] = if has_valid { sum } else { f64::NAN };
        }
    }

    Column::F64(result)
}

/// Sum all values (Real mode)
///
/// Reduces entire table to single scalar.
fn sum_scalar(table: &Table) -> Column {
    let mut total = 0.0;
    let mut has_valid = false;

    for col in &table.columns {
        match col {
            Column::F64(data) => {
                for &val in data {
                    if !val.is_nan() {
                        total += val;
                        has_valid = true;
                    }
                }
            }
            Column::Date(_) | Column::Timestamp(_) => {
                // Skip non-numeric columns
            }
        }
    }

    let result = if has_valid { total } else { f64::NAN };
    Column::F64(vec![result])
}

/// Daily log returns (dlog) with orientation-aware dispatch
///
/// Computes: dlog(x[i]) = log(x[i] / x[i-1])
///
/// # Behavior by orientation:
/// - ColwiseLike (H, N, _N, _H): Apply dlog down each column (vector is along i)
/// - RowwiseLike (Z, S, _Z, _S): Apply dlog across each row (vector is along j)
/// - Real (R): Not defined (panic) - dlog requires sequence
/// - Each (X): Not defined (panic) - dlog requires sequence
///
/// # Example:
/// ```
/// use blawktrust::{Table, TableView, Column, ORI_H, ORI_Z};
/// use blawktrust::builtins::ori_ops::dlog;
///
/// let table = Table::new(
///     vec!["a".to_string(), "b".to_string()],
///     vec![
///         Column::F64(vec![100.0, 110.0, 105.0]),
///         Column::F64(vec![50.0, 52.0, 51.0]),
///     ]
/// );
///
/// // H orientation: dlog down each column
/// let view_h = TableView::with_ori(table.clone(), ORI_H);
/// let result = dlog(&view_h);
/// // Each column transformed independently
///
/// // Z orientation: dlog across each row
/// let view_z = TableView::with_ori(table, ORI_Z);
/// let result = dlog(&view_z);
/// // Each row transformed independently
/// ```
pub fn dlog(view: &TableView) -> Table {
    match view.ori_class() {
        OriClass::ColwiseLike => dlog_colwise(&view.table),
        OriClass::RowwiseLike => dlog_rowwise(&view.table),
        OriClass::Real => panic!("dlog not defined for Real (R) orientation - requires sequence"),
        OriClass::Each => panic!("dlog not defined for Each (X) orientation - requires sequence"),
    }
}

/// Apply dlog down each column (ColwiseLike mode)
///
/// Each column is a time series; compute dlog within each column.
fn dlog_colwise(table: &Table) -> Table {
    let mut new_columns = Vec::with_capacity(table.columns.len());

    for col in &table.columns {
        let new_col = match col {
            Column::F64(_) => dlog_column(col, 1), // lag=1 for daily returns
            Column::Date(_) | Column::Timestamp(_) => col.clone(),
        };
        new_columns.push(new_col);
    }

    Table::new(table.names.clone(), new_columns)
}

/// Apply dlog across each row (RowwiseLike mode)
///
/// Each row is a sequence; compute dlog within each row.
/// Output has same shape as input.
fn dlog_rowwise(table: &Table) -> Table {
    let nrows = table.row_count();
    let ncols = table.col_count();

    if nrows == 0 || ncols == 0 {
        return Table::new(table.names.clone(), table.columns.clone());
    }

    // Extract F64 columns (preserve temporal as-is)
    let f64_indices: Vec<usize> = table.columns.iter()
        .enumerate()
        .filter_map(|(i, col)| match col {
            Column::F64(_) => Some(i),
            _ => None,
        })
        .collect();

    // Build result columns
    let mut new_columns = vec![Column::F64(vec![f64::NAN; nrows]); ncols];

    // Copy temporal columns as-is
    for (i, col) in table.columns.iter().enumerate() {
        if matches!(col, Column::Date(_) | Column::Timestamp(_)) {
            new_columns[i] = col.clone();
        }
    }

    // Process each row
    for row in 0..nrows {
        // Collect this row's values from F64 columns
        let mut row_values: Vec<f64> = Vec::with_capacity(f64_indices.len());
        for &col_idx in &f64_indices {
            if let Column::F64(data) = &table.columns[col_idx] {
                row_values.push(data[row]);
            }
        }

        // Compute dlog for this row
        let dlog_values = compute_dlog_sequence(&row_values);

        // Write back to result
        for (result_idx, &col_idx) in f64_indices.iter().enumerate() {
            if let Column::F64(data) = &mut new_columns[col_idx] {
                data[row] = dlog_values[result_idx];
            }
        }
    }

    Table::new(table.names.clone(), new_columns)
}

/// Compute dlog for a sequence: dlog[i] = log(x[i] / x[i-1])
///
/// First element is NaN (no previous value).
fn compute_dlog_sequence(values: &[f64]) -> Vec<f64> {
    let mut result = vec![f64::NAN; values.len()];

    if values.is_empty() {
        return result;
    }

    for i in 1..values.len() {
        let curr = values[i];
        let prev = values[i - 1];

        if !curr.is_nan() && !prev.is_nan() && prev > 0.0 {
            result[i] = (curr / prev).ln();
        }
    }

    result
}

/// Rolling 5-period window mean (w5) with orientation-aware dispatch
///
/// Computes: w5(x[i]) = mean(x[i-4], x[i-3], x[i-2], x[i-1], x[i])
///
/// # Behavior by orientation:
/// - ColwiseLike (H, N, _N, _H): Apply w5 down each column (vector is along i)
/// - RowwiseLike (Z, S, _Z, _S): Apply w5 across each row (vector is along j)
/// - Real (R): Not defined (panic) - w5 requires sequence
/// - Each (X): Not defined (panic) - w5 requires sequence
///
/// # Window Semantics:
/// - First 4 values are NaN (not enough history)
/// - NaN values in window are skipped (0-fill semantics)
/// - If entire window is NaN, output is NaN
///
/// # Example:
/// ```
/// use blawktrust::{Table, TableView, Column, ORI_H, ORI_Z};
/// use blawktrust::builtins::ori_ops::w5;
///
/// let table = Table::new(
///     vec!["prices".to_string()],
///     vec![Column::F64(vec![100.0, 102.0, 101.0, 103.0, 105.0, 104.0])]
/// );
///
/// // H orientation: w5 down the column
/// let view_h = TableView::with_ori(table, ORI_H);
/// let result = w5(&view_h);
/// // result column: [NaN, NaN, NaN, NaN, mean(100..105), mean(102..104)]
/// ```
pub fn w5(view: &TableView) -> Table {
    const WINDOW: usize = 5;

    match view.ori_class() {
        OriClass::ColwiseLike => w5_colwise(&view.table, WINDOW),
        OriClass::RowwiseLike => w5_rowwise(&view.table, WINDOW),
        OriClass::Real => panic!("w5 not defined for Real (R) orientation - requires sequence"),
        OriClass::Each => panic!("w5 not defined for Each (X) orientation - requires sequence"),
    }
}

/// Apply w5 down each column (ColwiseLike mode)
///
/// Each column is a time series; compute rolling window within each column.
fn w5_colwise(table: &Table, window: usize) -> Table {
    let mut new_columns = Vec::with_capacity(table.columns.len());

    for col in &table.columns {
        let new_col = match col {
            Column::F64(_) => wmean0(col, window),
            Column::Date(_) | Column::Timestamp(_) => col.clone(),
        };
        new_columns.push(new_col);
    }

    Table::new(table.names.clone(), new_columns)
}

/// Apply w5 across each row (RowwiseLike mode)
///
/// Each row is a sequence; compute rolling window within each row.
/// Output has same shape as input.
fn w5_rowwise(table: &Table, window: usize) -> Table {
    let nrows = table.row_count();
    let ncols = table.col_count();

    if nrows == 0 || ncols == 0 {
        return Table::new(table.names.clone(), table.columns.clone());
    }

    // Extract F64 columns (preserve temporal as-is)
    let f64_indices: Vec<usize> = table.columns.iter()
        .enumerate()
        .filter_map(|(i, col)| match col {
            Column::F64(_) => Some(i),
            _ => None,
        })
        .collect();

    // Build result columns
    let mut new_columns = vec![Column::F64(vec![f64::NAN; nrows]); ncols];

    // Copy temporal columns as-is
    for (i, col) in table.columns.iter().enumerate() {
        if matches!(col, Column::Date(_) | Column::Timestamp(_)) {
            new_columns[i] = col.clone();
        }
    }

    // Process each row
    for row in 0..nrows {
        // Collect this row's values from F64 columns
        let mut row_values: Vec<f64> = Vec::with_capacity(f64_indices.len());
        for &col_idx in &f64_indices {
            if let Column::F64(data) = &table.columns[col_idx] {
                row_values.push(data[row]);
            }
        }

        // Compute w5 for this row sequence
        let w5_values = compute_wmean_sequence(&row_values, window);

        // Write back to result
        for (result_idx, &col_idx) in f64_indices.iter().enumerate() {
            if let Column::F64(data) = &mut new_columns[col_idx] {
                data[row] = w5_values[result_idx];
            }
        }
    }

    Table::new(table.names.clone(), new_columns)
}

/// Compute rolling window mean for a sequence
///
/// For each position i, compute mean of window [i-w+1, i]
fn compute_wmean_sequence(values: &[f64], window: usize) -> Vec<f64> {
    let n = values.len();
    let mut result = vec![f64::NAN; n];

    if window == 0 {
        return result;
    }

    for i in 0..n {
        if i < window - 1 {
            // Not enough history for full window
            continue;
        }

        let start = i + 1 - window;
        let mut sum = 0.0;
        let mut count = 0;

        for j in start..=i {
            let val = values[j];
            if !val.is_nan() {
                sum += val;
                count += 1;
            }
        }

        result[i] = if count == 0 {
            f64::NAN
        } else {
            sum / (count as f64)
        };
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{ORI_H, ORI_Z, ORI_R, ORI_X};

    fn make_test_table() -> Table {
        // 3x2 table:
        // col_a: [1, 2, 3]
        // col_b: [4, 5, 6]
        Table::new(
            vec!["a".to_string(), "b".to_string()],
            vec![
                Column::F64(vec![1.0, 2.0, 3.0]),
                Column::F64(vec![4.0, 5.0, 6.0]),
            ]
        )
    }

    #[test]
    fn test_sum_colwise() {
        let table = make_test_table();
        let view = TableView::with_ori(table, ORI_H);

        let result = sum(&view);

        match result {
            Column::F64(data) => {
                assert_eq!(data.len(), 2);
                assert_eq!(data[0], 6.0);  // 1 + 2 + 3
                assert_eq!(data[1], 15.0); // 4 + 5 + 6
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    fn test_sum_rowwise() {
        let table = make_test_table();
        let view = TableView::with_ori(table, ORI_Z);

        let result = sum(&view);

        match result {
            Column::F64(data) => {
                assert_eq!(data.len(), 3);
                assert_eq!(data[0], 5.0); // 1 + 4
                assert_eq!(data[1], 7.0); // 2 + 5
                assert_eq!(data[2], 9.0); // 3 + 6
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    fn test_sum_scalar() {
        let table = make_test_table();
        let view = TableView::with_ori(table, ORI_R);

        let result = sum(&view);

        match result {
            Column::F64(data) => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0], 21.0); // 1+2+3+4+5+6
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    #[should_panic(expected = "sum not defined for Each")]
    fn test_sum_each_panics() {
        let table = make_test_table();
        let view = TableView::with_ori(table, ORI_X);
        sum(&view); // Should panic
    }

    #[test]
    fn test_sum_with_nan() {
        let table = Table::new(
            vec!["a".to_string()],
            vec![Column::F64(vec![1.0, f64::NAN, 3.0])],
        );

        // Colwise
        let view = TableView::with_ori(table.clone(), ORI_H);
        let result = sum(&view);
        match result {
            Column::F64(data) => assert_eq!(data[0], 4.0), // 1 + 3 (skip NaN)
            _ => panic!(),
        }

        // Scalar
        let view = TableView::with_ori(table, ORI_R);
        let result = sum(&view);
        match result {
            Column::F64(data) => assert_eq!(data[0], 4.0),
            _ => panic!(),
        }
    }

    #[test]
    fn test_sum_rowwise_with_nan() {
        let table = Table::new(
            vec!["a".to_string(), "b".to_string()],
            vec![
                Column::F64(vec![1.0, f64::NAN, 3.0]),
                Column::F64(vec![4.0, 5.0, f64::NAN]),
            ]
        );

        let view = TableView::with_ori(table, ORI_Z);
        let result = sum(&view);

        match result {
            Column::F64(data) => {
                assert_eq!(data[0], 5.0);  // 1 + 4
                assert_eq!(data[1], 5.0);  // NaN + 5 = 5
                assert_eq!(data[2], 3.0);  // 3 + NaN = 3
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_sum_all_nan() {
        let table = Table::new(
            vec!["a".to_string()],
            vec![Column::F64(vec![f64::NAN, f64::NAN])],
        );

        // Colwise
        let view = TableView::with_ori(table.clone(), ORI_H);
        let result = sum(&view);
        match result {
            Column::F64(data) => assert!(data[0].is_nan()),
            _ => panic!(),
        }

        // Scalar
        let view = TableView::with_ori(table, ORI_R);
        let result = sum(&view);
        match result {
            Column::F64(data) => assert!(data[0].is_nan()),
            _ => panic!(),
        }
    }

    #[test]
    fn test_sum_empty_table() {
        let table = Table::new(vec![], vec![]);

        let view = TableView::with_ori(table.clone(), ORI_H);
        let result = sum(&view);
        match result {
            Column::F64(data) => assert_eq!(data.len(), 0),
            _ => panic!(),
        }

        let view = TableView::with_ori(table, ORI_R);
        let result = sum(&view);
        match result {
            Column::F64(data) => {
                assert_eq!(data.len(), 1);
                assert!(data[0].is_nan());
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_sum_temporal_columns() {
        use crate::table::{NULL_DATE, NULL_TIMESTAMP};

        let table = Table::new(
            vec!["date".to_string(), "value".to_string(), "ts".to_string()],
            vec![
                Column::Date(vec![18628, 18629, NULL_DATE]),
                Column::F64(vec![1.0, 2.0, 3.0]),
                Column::Timestamp(vec![0, 1_000_000_000, NULL_TIMESTAMP]),
            ]
        );

        // Colwise: date and timestamp columns should produce NaN
        let view = TableView::with_ori(table.clone(), ORI_H);
        let result = sum(&view);
        match result {
            Column::F64(data) => {
                assert_eq!(data.len(), 3);
                assert!(data[0].is_nan());  // date → NaN
                assert_eq!(data[1], 6.0);   // value → 6.0
                assert!(data[2].is_nan());  // timestamp → NaN
            }
            _ => panic!(),
        }

        // Scalar: only sum numeric column
        let view = TableView::with_ori(table, ORI_R);
        let result = sum(&view);
        match result {
            Column::F64(data) => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0], 6.0); // Only value column contributes
            }
            _ => panic!(),
        }
    }

    #[test]
    fn test_sum_rowwise_large() {
        // Test tiling with > 128 rows
        let nrows = 300;
        let data_a: Vec<f64> = (0..nrows).map(|i| i as f64).collect();
        let data_b: Vec<f64> = (0..nrows).map(|i| (i * 10) as f64).collect();

        let table = Table::new(
            vec!["a".to_string(), "b".to_string()],
            vec![
                Column::F64(data_a.clone()),
                Column::F64(data_b.clone()),
            ]
        );

        let view = TableView::with_ori(table, ORI_Z);
        let result = sum(&view);

        match result {
            Column::F64(data) => {
                assert_eq!(data.len(), nrows);
                for i in 0..nrows {
                    let expected = (i + i * 10) as f64;
                    assert_eq!(data[i], expected);
                }
            }
            _ => panic!(),
        }
    }

    // ============ dlog tests ============

    #[test]
    fn test_dlog_colwise() {
        // 3 rows x 2 cols:
        // col_a: [100, 110, 121]
        // col_b: [50, 55, 50]
        let table = Table::new(
            vec!["a".to_string(), "b".to_string()],
            vec![
                Column::F64(vec![100.0, 110.0, 121.0]),
                Column::F64(vec![50.0, 55.0, 50.0]),
            ]
        );

        let view = TableView::with_ori(table, ORI_H);
        let result = dlog(&view);

        assert_eq!(result.names, vec!["a", "b"]);
        assert_eq!(result.col_count(), 2);
        assert_eq!(result.row_count(), 3);

        // col_a: dlog[0] = NaN, dlog[1] = ln(110/100), dlog[2] = ln(121/110)
        if let Column::F64(data) = &result.columns[0] {
            assert!(data[0].is_nan());
            assert!((data[1] - (110.0f64 / 100.0f64).ln()).abs() < 1e-10);
            assert!((data[2] - (121.0f64 / 110.0f64).ln()).abs() < 1e-10);
        } else {
            panic!("Expected F64 column");
        }

        // col_b: dlog[0] = NaN, dlog[1] = ln(55/50), dlog[2] = ln(50/55)
        if let Column::F64(data) = &result.columns[1] {
            assert!(data[0].is_nan());
            assert!((data[1] - (55.0f64 / 50.0f64).ln()).abs() < 1e-10);
            assert!((data[2] - (50.0f64 / 55.0f64).ln()).abs() < 1e-10);
        } else {
            panic!("Expected F64 column");
        }
    }

    #[test]
    fn test_dlog_rowwise() {
        // 3 rows x 2 cols (but in Z orientation, we think of it as 2 rows x 3 cols):
        // row[0]: [100, 110, 121]
        // row[1]: [50, 55, 50]
        // row[2]: [200, 210, 220]
        let table = Table::new(
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec![
                Column::F64(vec![100.0, 50.0, 200.0]),
                Column::F64(vec![110.0, 55.0, 210.0]),
                Column::F64(vec![121.0, 50.0, 220.0]),
            ]
        );

        let view = TableView::with_ori(table, ORI_Z);
        let result = dlog(&view);

        assert_eq!(result.col_count(), 3);
        assert_eq!(result.row_count(), 3);

        // Physical storage unchanged, but dlog applied across rows
        // row[0]: [NaN, ln(110/100), ln(121/110)]
        if let (Column::F64(a), Column::F64(b), Column::F64(c)) = (&result.columns[0], &result.columns[1], &result.columns[2]) {
            assert!(a[0].is_nan());
            assert!((b[0] - (110.0f64 / 100.0f64).ln()).abs() < 1e-10);
            assert!((c[0] - (121.0f64 / 110.0f64).ln()).abs() < 1e-10);

            // row[1]: [NaN, ln(55/50), ln(50/55)]
            assert!(a[1].is_nan());
            assert!((b[1] - (55.0f64 / 50.0f64).ln()).abs() < 1e-10);
            assert!((c[1] - (50.0f64 / 55.0f64).ln()).abs() < 1e-10);

            // row[2]: [NaN, ln(210/200), ln(220/210)]
            assert!(a[2].is_nan());
            assert!((b[2] - (210.0f64 / 200.0f64).ln()).abs() < 1e-10);
            assert!((c[2] - (220.0f64 / 210.0f64).ln()).abs() < 1e-10);
        } else {
            panic!("Expected F64 columns");
        }
    }

    #[test]
    #[should_panic(expected = "dlog not defined for Real")]
    fn test_dlog_real_panics() {
        let table = make_test_table();
        let view = TableView::with_ori(table, ORI_R);
        dlog(&view);
    }

    #[test]
    #[should_panic(expected = "dlog not defined for Each")]
    fn test_dlog_each_panics() {
        let table = make_test_table();
        let view = TableView::with_ori(table, ORI_X);
        dlog(&view);
    }

    #[test]
    fn test_dlog_with_nan() {
        let table = Table::new(
            vec!["a".to_string()],
            vec![Column::F64(vec![100.0, f64::NAN, 120.0, 130.0])],
        );

        let view = TableView::with_ori(table, ORI_H);
        let result = dlog(&view);

        if let Column::F64(data) = &result.columns[0] {
            assert!(data[0].is_nan()); // First always NaN
            assert!(data[1].is_nan()); // NaN input
            assert!(data[2].is_nan()); // 120/NaN
            assert!((data[3] - (130.0f64 / 120.0f64).ln()).abs() < 1e-10);
        } else {
            panic!("Expected F64 column");
        }
    }

    #[test]
    fn test_dlog_preserves_temporal() {
        use crate::table::NULL_DATE;

        let table = Table::new(
            vec!["date".to_string(), "value".to_string()],
            vec![
                Column::Date(vec![18628, 18629, NULL_DATE]),
                Column::F64(vec![100.0, 110.0, 121.0]),
            ]
        );

        // Colwise
        let view = TableView::with_ori(table.clone(), ORI_H);
        let result = dlog(&view);

        assert_eq!(result.col_count(), 2);
        assert!(matches!(result.columns[0], Column::Date(_)));
        assert!(matches!(result.columns[1], Column::F64(_)));

        // Date column unchanged
        if let Column::Date(data) = &result.columns[0] {
            assert_eq!(data, &vec![18628, 18629, NULL_DATE]);
        }

        // Value column has dlog applied
        if let Column::F64(data) = &result.columns[1] {
            assert!(data[0].is_nan());
            assert!((data[1] - (110.0f64 / 100.0f64).ln()).abs() < 1e-10);
        }
    }

    // ============ w5 tests ============

    #[test]
    fn test_w5_colwise() {
        // Column with 7 values: [10, 20, 30, 40, 50, 60, 70]
        let table = Table::new(
            vec!["prices".to_string()],
            vec![Column::F64(vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0])],
        );

        let view = TableView::with_ori(table, ORI_H);
        let result = w5(&view);

        if let Column::F64(data) = &result.columns[0] {
            // First 4 values should be NaN (not enough history)
            assert!(data[0].is_nan());
            assert!(data[1].is_nan());
            assert!(data[2].is_nan());
            assert!(data[3].is_nan());

            // w5[4] = mean(10, 20, 30, 40, 50) = 30
            assert_eq!(data[4], 30.0);

            // w5[5] = mean(20, 30, 40, 50, 60) = 40
            assert_eq!(data[5], 40.0);

            // w5[6] = mean(30, 40, 50, 60, 70) = 50
            assert_eq!(data[6], 50.0);
        } else {
            panic!("Expected F64 column");
        }
    }

    #[test]
    fn test_w5_rowwise() {
        // 3 rows x 7 cols (each row is a sequence)
        // row[0]: [10, 20, 30, 40, 50, 60, 70]
        // row[1]: [100, 200, 300, 400, 500, 600, 700]
        // row[2]: [1, 2, 3, 4, 5, 6, 7]
        let table = Table::new(
            vec![
                "c0".to_string(), "c1".to_string(), "c2".to_string(),
                "c3".to_string(), "c4".to_string(), "c5".to_string(),
                "c6".to_string(),
            ],
            vec![
                Column::F64(vec![10.0, 100.0, 1.0]),
                Column::F64(vec![20.0, 200.0, 2.0]),
                Column::F64(vec![30.0, 300.0, 3.0]),
                Column::F64(vec![40.0, 400.0, 4.0]),
                Column::F64(vec![50.0, 500.0, 5.0]),
                Column::F64(vec![60.0, 600.0, 6.0]),
                Column::F64(vec![70.0, 700.0, 7.0]),
            ],
        );

        let view = TableView::with_ori(table, ORI_Z);
        let result = w5(&view);

        // row[0]: first 4 NaN, then [30, 40, 50]
        assert!(matches!(&result.columns[0], Column::F64(_)));
        if let Column::F64(c0) = &result.columns[0] {
            assert!(c0[0].is_nan());
            assert!(c0[1].is_nan());
            assert!(c0[2].is_nan());
        }

        if let Column::F64(c4) = &result.columns[4] {
            assert_eq!(c4[0], 30.0); // mean(10,20,30,40,50)
            assert_eq!(c4[1], 300.0); // mean(100,200,300,400,500)
            assert_eq!(c4[2], 3.0); // mean(1,2,3,4,5)
        }

        if let Column::F64(c5) = &result.columns[5] {
            assert_eq!(c5[0], 40.0); // mean(20,30,40,50,60)
            assert_eq!(c5[1], 400.0);
            assert_eq!(c5[2], 4.0);
        }

        if let Column::F64(c6) = &result.columns[6] {
            assert_eq!(c6[0], 50.0); // mean(30,40,50,60,70)
            assert_eq!(c6[1], 500.0);
            assert_eq!(c6[2], 5.0);
        }
    }

    #[test]
    fn test_w5_with_nan() {
        // Test NaN handling: [10, NaN, 30, 40, 50, 60]
        let table = Table::new(
            vec!["a".to_string()],
            vec![Column::F64(vec![10.0, f64::NAN, 30.0, 40.0, 50.0, 60.0])],
        );

        let view = TableView::with_ori(table, ORI_H);
        let result = w5(&view);

        if let Column::F64(data) = &result.columns[0] {
            // First 4 are NaN (not enough history)
            assert!(data[0].is_nan());
            assert!(data[1].is_nan());
            assert!(data[2].is_nan());
            assert!(data[3].is_nan());

            // w5[4] = mean(10, NaN, 30, 40, 50) = mean(10, 30, 40, 50) = 32.5
            assert_eq!(data[4], 32.5);

            // w5[5] = mean(NaN, 30, 40, 50, 60) = mean(30, 40, 50, 60) = 45
            assert_eq!(data[5], 45.0);
        } else {
            panic!("Expected F64 column");
        }
    }

    #[test]
    fn test_w5_all_nan_window() {
        // Window with all NaN should produce NaN
        let table = Table::new(
            vec!["a".to_string()],
            vec![Column::F64(vec![f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN, 100.0])],
        );

        let view = TableView::with_ori(table, ORI_H);
        let result = w5(&view);

        if let Column::F64(data) = &result.columns[0] {
            // First 5 should be NaN (either not enough history or all NaN in window)
            assert!(data[0].is_nan());
            assert!(data[1].is_nan());
            assert!(data[2].is_nan());
            assert!(data[3].is_nan());
            assert!(data[4].is_nan()); // Window is all NaN

            // w5[5] = mean(NaN, NaN, NaN, NaN, 100) = 100
            assert_eq!(data[5], 100.0);
        } else {
            panic!("Expected F64 column");
        }
    }

    #[test]
    #[should_panic(expected = "w5 not defined for Real")]
    fn test_w5_real_panics() {
        let table = make_test_table();
        let view = TableView::with_ori(table, ORI_R);
        w5(&view);
    }

    #[test]
    #[should_panic(expected = "w5 not defined for Each")]
    fn test_w5_each_panics() {
        let table = make_test_table();
        let view = TableView::with_ori(table, ORI_X);
        w5(&view);
    }

    #[test]
    fn test_w5_preserves_temporal() {
        use crate::table::NULL_DATE;

        let table = Table::new(
            vec!["date".to_string(), "value".to_string()],
            vec![
                Column::Date(vec![18628, 18629, 18630, 18631, 18632, 18633]),
                Column::F64(vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0]),
            ],
        );

        let view = TableView::with_ori(table.clone(), ORI_H);
        let result = w5(&view);

        // Date column unchanged
        if let Column::Date(data) = &result.columns[0] {
            assert_eq!(data, &vec![18628, 18629, 18630, 18631, 18632, 18633]);
        }

        // Value column has w5 applied
        if let Column::F64(data) = &result.columns[1] {
            assert!(data[0].is_nan());
            assert!(data[3].is_nan());
            assert_eq!(data[4], 30.0); // mean(10,20,30,40,50)
            assert_eq!(data[5], 40.0); // mean(20,30,40,50,60)
        }
    }

    #[test]
    fn test_w5_short_sequence() {
        // Sequence with only 3 elements (< window size)
        let table = Table::new(
            vec!["a".to_string()],
            vec![Column::F64(vec![10.0, 20.0, 30.0])],
        );

        let view = TableView::with_ori(table, ORI_H);
        let result = w5(&view);

        if let Column::F64(data) = &result.columns[0] {
            // All should be NaN (never enough history for window of 5)
            assert!(data[0].is_nan());
            assert!(data[1].is_nan());
            assert!(data[2].is_nan());
        } else {
            panic!("Expected F64 column");
        }
    }

    #[test]
    fn test_w5_empty_table() {
        let table = Table::new(vec![], vec![]);

        let view = TableView::with_ori(table.clone(), ORI_H);
        let result = w5(&view);

        assert_eq!(result.col_count(), 0);
        assert_eq!(result.row_count(), 0);
    }
}
