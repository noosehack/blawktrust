//! Typed column with embedded null sentinels (kdb-style)

/// Null sentinel for Ts columns (kdb-style)
///
/// Using i64::MIN as the null date sentinel, similar to kdb's type-specific nulls.
/// This avoids bitmap overhead and keeps null embedded in the data vector.
pub const NULL_TS: i64 = i64::MIN;

/// A typed column of data with type-specific null representation (kdb-style)
///
/// All nulls are embedded as sentinel values in the data vector:
/// - F64: f64::NAN
/// - Ts: NULL_TS (i64::MIN)
///
/// No validity bitmaps - keeps compute engine pure and vectorizable.
#[derive(Clone, Debug)]
pub enum Column {
    /// F64 column: data with embedded NaN for missing values
    ///
    /// Missing values represented as f64::NAN (IEEE 754).
    /// Pure kdb-style: null is a value, no bitmap overhead.
    F64(Vec<f64>),

    /// Timestamp column: days since epoch (1970-01-01)
    ///
    /// Missing values represented as NULL_TS (i64::MIN).
    /// Pure kdb-style: null is a value, no bitmap overhead.
    Ts(Vec<i64>),

    // TODO: I64, Sym, Bool
}

impl Column {
    /// Create F64 column with embedded NaN for missing values (kdb-style)
    pub fn new_f64(data: Vec<f64>) -> Self {
        Column::F64(data)
    }

    /// Create Ts (timestamp) column with embedded NULL_TS for missing dates (kdb-style)
    pub fn new_ts(data: Vec<i64>) -> Self {
        Column::Ts(data)
    }

    pub fn len(&self) -> usize {
        match self {
            Column::F64(data) => data.len(),
            Column::Ts(data) => data.len(),
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get data slice (F64) - kdb-style direct access
    pub fn f64_data(&self) -> &[f64] {
        match self {
            Column::F64(data) => data,
            _ => panic!("Not an F64 column"),
        }
    }

    /// Get mutable data slice (F64)
    pub fn f64_data_mut(&mut self) -> &mut [f64] {
        match self {
            Column::F64(data) => data,
            _ => panic!("Not an F64 column"),
        }
    }

    /// Get data slice (Ts) - kdb-style direct access
    pub fn ts_data(&self) -> &[i64] {
        match self {
            Column::Ts(data) => data,
            _ => panic!("Not a Ts column"),
        }
    }

    /// Get mutable data slice (Ts)
    pub fn ts_data_mut(&mut self) -> &mut [i64] {
        match self {
            Column::Ts(data) => data,
            _ => panic!("Not a Ts column"),
        }
    }

    /// Get raw F64 slice for monomorphic kernels (zero-cost)
    ///
    /// Returns error instead of panic for better error handling.
    /// Inlined to avoid any accessor overhead in hot paths.
    #[inline(always)]
    pub fn as_f64_slice(&self) -> Result<&[f64], &'static str> {
        match self {
            Column::F64(data) => Ok(data),
            _ => Err("Expected F64 column"),
        }
    }

    /// Get raw Ts slice for monomorphic kernels (zero-cost)
    #[inline(always)]
    pub fn as_ts_slice(&self) -> Result<&[i64], &'static str> {
        match self {
            Column::Ts(data) => Ok(data),
            _ => Err("Expected Ts column"),
        }
    }

    /// Create F64 column from raw vector (for kernel output) - kdb-style
    #[inline(always)]
    pub fn from_f64_vec(data: Vec<f64>) -> Self {
        Column::F64(data)
    }

    /// Create Ts column from raw vector (for kernel output) - kdb-style
    #[inline(always)]
    pub fn from_ts_vec(data: Vec<i64>) -> Self {
        Column::Ts(data)
    }

    /// Check if column contains any NaN values (F64 only)
    ///
    /// For Ts columns, check for NULL_TS instead.
    pub fn has_nulls(&self) -> bool {
        match self {
            Column::F64(data) => data.iter().any(|x| x.is_nan()),
            Column::Ts(data) => data.iter().any(|x| *x == NULL_TS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_f64() {
        let col = Column::new_f64(vec![1.0, 2.0, 3.0]);
        assert_eq!(col.len(), 3);
        assert!(!col.has_nulls());
    }

    #[test]
    fn test_has_nulls() {
        // Column without nulls
        let col = Column::new_f64(vec![1.0, 2.0, 3.0]);
        assert!(!col.has_nulls());

        // Column with NaN
        let col_with_nan = Column::new_f64(vec![1.0, f64::NAN, 3.0]);
        assert!(col_with_nan.has_nulls());

        // Ts column with NULL_TS
        let col_ts = Column::Ts(vec![100, NULL_TS, 300]);
        assert!(col_ts.has_nulls());
    }
}
