//! Sentinel-to-bitmap conversion for backward compatibility

use crate::table::{Column, Bitmap};

/// Convert sentinel NA values (-99999) to validity bitmap
///
/// This is a one-time conversion to migrate from sentinel-based
/// to bitmap-based null representation.
///
/// Only creates bitmap if at least one NA is found.
pub fn sentinel_to_bitmap_inplace(col: &mut Column, na: f64) {
    let Column::F64 { data, valid } = col else { return };

    // Only build bitmap if we see at least one NA
    let mut bm: Option<Bitmap> = None;

    for (i, &x) in data.iter().enumerate() {
        if x == na {
            if bm.is_none() {
                // First NA found - create bitmap with all valid
                bm = Some(Bitmap::new_all_valid(data.len()));
            }
            bm.as_mut().unwrap().set(i, false);
        }
    }

    // Only set validity if we found NAs
    if let Some(bitmap) = bm {
        *valid = Some(bitmap);
    }
}

/// Create column from data with sentinel NA conversion
pub fn from_sentinel_data(data: Vec<f64>, na: f64) -> Column {
    let mut col = Column::new_f64(data);
    sentinel_to_bitmap_inplace(&mut col, na);
    col
}

/// Materialize sentinel values at invalid positions (for legacy compatibility)
///
/// This is the INVERSE of sentinel_to_bitmap: it writes sentinel values
/// to data where validity bitmap indicates invalid.
///
/// Use case: Exporting to legacy systems that expect sentinel-based NAs.
///
/// IMPORTANT: This is only for compatibility layers. Kernels should NEVER
/// call this - they work with validity bitmaps directly.
pub fn materialize_sentinel(col: &mut Column, na: f64) {
    let Column::F64 { data, valid } = col else { return };

    if let Some(bitmap) = valid {
        // Write sentinel to invalid positions
        for i in 0..data.len() {
            if !bitmap.get(i) {
                data[i] = na;
            }
        }
    }
    // If valid=None, all data is valid, nothing to do
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_nas() {
        let mut col = Column::new_f64(vec![1.0, 2.0, 3.0]);
        sentinel_to_bitmap_inplace(&mut col, -99999.0);
        
        // No NAs found, so validity should still be None
        assert!(col.validity().is_none());
    }

    #[test]
    fn test_with_nas() {
        let mut col = Column::new_f64(vec![1.0, -99999.0, 3.0, -99999.0]);
        sentinel_to_bitmap_inplace(&mut col, -99999.0);
        
        // NAs found, bitmap created
        let valid = col.validity().expect("bitmap should exist");
        assert!(valid.get(0));   // Valid
        assert!(!valid.get(1));  // NA
        assert!(valid.get(2));   // Valid
        assert!(!valid.get(3));  // NA
    }

    #[test]
    fn test_from_sentinel_data() {
        let col = from_sentinel_data(vec![1.0, -99999.0, 3.0], -99999.0);

        let valid = col.validity().expect("bitmap should exist");
        assert!(valid.get(0));
        assert!(!valid.get(1));
        assert!(valid.get(2));
    }

    #[test]
    fn test_materialize_sentinel_no_bitmap() {
        let mut col = Column::new_f64(vec![1.0, 2.0, 3.0]);
        materialize_sentinel(&mut col, -99999.0);

        // No bitmap, so data unchanged
        let Column::F64 { data, .. } = &col;
        assert_eq!(data[0], 1.0);
        assert_eq!(data[1], 2.0);
        assert_eq!(data[2], 3.0);
    }

    #[test]
    fn test_materialize_sentinel_with_bitmap() {
        // Start with clean data + bitmap
        let data = vec![100.0, 200.0, 300.0, 400.0];
        let mut bitmap = Bitmap::new_all_valid(4);
        bitmap.set(1, false);  // Mark index 1 as invalid
        bitmap.set(3, false);  // Mark index 3 as invalid

        let mut col = Column::F64 {
            data,
            valid: Some(bitmap),
        };

        // Materialize sentinels
        materialize_sentinel(&mut col, -99999.0);

        // Check data
        let Column::F64 { data, .. } = &col;
        assert_eq!(data[0], 100.0);
        assert_eq!(data[1], -99999.0);  // Materialized!
        assert_eq!(data[2], 300.0);
        assert_eq!(data[3], -99999.0);  // Materialized!
    }

    #[test]
    fn test_roundtrip_sentinel_bitmap_sentinel() {
        // Start with sentinel-based data
        let original = vec![1.0, -99999.0, 3.0, -99999.0, 5.0];
        let mut col = from_sentinel_data(original.clone(), -99999.0);

        // Bitmap should be created
        assert!(col.validity().is_some());

        // Materialize back to sentinels
        materialize_sentinel(&mut col, -99999.0);

        // Data should match original (where valid)
        let Column::F64 { data, valid } = &col;
        let bitmap = valid.as_ref().unwrap();

        for i in 0..data.len() {
            if bitmap.get(i) {
                assert_eq!(data[i], original[i]);
            } else {
                assert_eq!(data[i], -99999.0);
            }
        }
    }
}
