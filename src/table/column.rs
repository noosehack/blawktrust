//! Typed column with embedded null sentinels (kdb-style)

/// Null sentinel for Date columns (i32 days since epoch)
///
/// Using i32::MIN as the null date sentinel, similar to kdb's type-specific nulls.
/// This avoids bitmap overhead and keeps null embedded in the data vector.
pub const NULL_DATE: i32 = i32::MIN;

/// Null sentinel for Timestamp columns (i64 nanoseconds since epoch)
///
/// Using i64::MIN as the null timestamp sentinel, similar to kdb's type-specific nulls.
/// This avoids bitmap overhead and keeps null embedded in the data vector.
pub const NULL_TIMESTAMP: i64 = i64::MIN;

/// Null sentinel for Ts columns (deprecated, use NULL_TIMESTAMP)
///
/// Using i64::MIN as the null date sentinel, similar to kdb's type-specific nulls.
/// This avoids bitmap overhead and keeps null embedded in the data vector.
pub const NULL_TS: i64 = i64::MIN;

/// A typed column of data with type-specific null representation (kdb-style)
///
/// All nulls are embedded as sentinel values in the data vector:
/// - F64: f64::NAN
/// - Date: NULL_DATE (i32::MIN)
/// - Timestamp: NULL_TIMESTAMP (i64::MIN)
/// - Ts: NULL_TS (i64::MIN, deprecated)
///
/// No validity bitmaps - keeps compute engine pure and vectorizable.
#[derive(Clone, Debug)]
pub enum Column {
    /// F64 column: data with embedded NaN for missing values
    ///
    /// Missing values represented as f64::NAN (IEEE 754).
    /// Pure kdb-style: null is a value, no bitmap overhead.
    F64(Vec<f64>),

    /// Date column: days since epoch (1970-01-01) as i32
    ///
    /// Missing values represented as NULL_DATE (i32::MIN).
    /// Range: ±5.8M years (sufficient for all financial data).
    /// Uses i32 for 50% memory savings vs i64.
    Date(Vec<i32>),

    /// Timestamp column: nanoseconds since epoch (1970-01-01 00:00:00) as i64
    ///
    /// Missing values represented as NULL_TIMESTAMP (i64::MIN).
    /// Nanosecond precision for high-frequency trading data.
    Timestamp(Vec<i64>),

    /// Ts column (deprecated, use Date or Timestamp)
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

    /// Create Date column with embedded NULL_DATE for missing dates (kdb-style)
    pub fn new_date(data: Vec<i32>) -> Self {
        Column::Date(data)
    }

    /// Create Timestamp column with embedded NULL_TIMESTAMP for missing timestamps (kdb-style)
    pub fn new_timestamp(data: Vec<i64>) -> Self {
        Column::Timestamp(data)
    }

    /// Create Ts (timestamp) column with embedded NULL_TS for missing dates (kdb-style)
    pub fn new_ts(data: Vec<i64>) -> Self {
        Column::Ts(data)
    }

    pub fn len(&self) -> usize {
        match self {
            Column::F64(data) => data.len(),
            Column::Date(data) => data.len(),
            Column::Timestamp(data) => data.len(),
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

    /// Get data slice (Date) - kdb-style direct access
    pub fn date_data(&self) -> &[i32] {
        match self {
            Column::Date(data) => data,
            _ => panic!("Not a Date column"),
        }
    }

    /// Get mutable data slice (Date)
    pub fn date_data_mut(&mut self) -> &mut [i32] {
        match self {
            Column::Date(data) => data,
            _ => panic!("Not a Date column"),
        }
    }

    /// Get data slice (Timestamp) - kdb-style direct access
    pub fn timestamp_data(&self) -> &[i64] {
        match self {
            Column::Timestamp(data) => data,
            _ => panic!("Not a Timestamp column"),
        }
    }

    /// Get mutable data slice (Timestamp)
    pub fn timestamp_data_mut(&mut self) -> &mut [i64] {
        match self {
            Column::Timestamp(data) => data,
            _ => panic!("Not a Timestamp column"),
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

    /// Get raw Date slice for monomorphic kernels (zero-cost)
    #[inline(always)]
    pub fn as_date_slice(&self) -> Result<&[i32], &'static str> {
        match self {
            Column::Date(data) => Ok(data),
            _ => Err("Expected Date column"),
        }
    }

    /// Get raw Timestamp slice for monomorphic kernels (zero-cost)
    #[inline(always)]
    pub fn as_timestamp_slice(&self) -> Result<&[i64], &'static str> {
        match self {
            Column::Timestamp(data) => Ok(data),
            _ => Err("Expected Timestamp column"),
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

    /// Create Date column from raw vector (for kernel output) - kdb-style
    #[inline(always)]
    pub fn from_date_vec(data: Vec<i32>) -> Self {
        Column::Date(data)
    }

    /// Create Timestamp column from raw vector (for kernel output) - kdb-style
    #[inline(always)]
    pub fn from_timestamp_vec(data: Vec<i64>) -> Self {
        Column::Timestamp(data)
    }

    /// Create Ts column from raw vector (for kernel output) - kdb-style
    #[inline(always)]
    pub fn from_ts_vec(data: Vec<i64>) -> Self {
        Column::Ts(data)
    }

    /// Check if column contains any null values
    ///
    /// Checks for type-specific null sentinels.
    pub fn has_nulls(&self) -> bool {
        match self {
            Column::F64(data) => data.iter().any(|x| x.is_nan()),
            Column::Date(data) => data.iter().any(|x| *x == NULL_DATE),
            Column::Timestamp(data) => data.iter().any(|x| *x == NULL_TIMESTAMP),
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

        // Date column with NULL_DATE
        let col_date = Column::Date(vec![100, NULL_DATE, 300]);
        assert!(col_date.has_nulls());

        // Timestamp column with NULL_TIMESTAMP
        let col_ts = Column::Timestamp(vec![100, NULL_TIMESTAMP, 300]);
        assert!(col_ts.has_nulls());

        // Ts column with NULL_TS
        let col_ts_old = Column::Ts(vec![100, NULL_TS, 300]);
        assert!(col_ts_old.has_nulls());
    }
}
