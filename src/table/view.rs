//! Table view with orientation
//!
//! Provides O(1) orientation changes without copying data.
//! Physical storage remains columnar, orientation just changes interpretation.

use super::orientation::{Ori, OriClass, ReduceMode, VecAxis, ORI_H};
use super::Table;
use std::sync::Arc;

/// A view of a Table with an orientation
///
/// This is the key abstraction for O(1) orientation changes:
/// - Physical data never moves
/// - Orientation flag changes how operators interpret the data
/// - Cloning a view is cheap (Arc + Copy)
#[derive(Clone, Debug)]
pub struct TableView {
    /// The underlying table (shared via Arc for cheap cloning)
    pub table: Arc<Table>,

    /// Current orientation for this view
    pub ori: Ori,
}

impl TableView {
    /// Create a new view with default orientation (H)
    pub fn new(table: Table) -> Self {
        TableView {
            table: Arc::new(table),
            ori: ORI_H,
        }
    }

    /// Create a view from an Arc<Table> with default orientation
    pub fn from_arc(table: Arc<Table>) -> Self {
        TableView { table, ori: ORI_H }
    }

    /// Create a view with explicit orientation
    pub fn with_ori(table: Table, ori: Ori) -> Self {
        TableView {
            table: Arc::new(table),
            ori,
        }
    }

    /// Change orientation (O(1) operation!)
    ///
    /// Returns a new view with the same underlying table but different orientation.
    /// No data is copied or moved.
    #[inline]
    pub fn with_orientation(&self, new_ori: Ori) -> Self {
        TableView {
            table: Arc::clone(&self.table),
            ori: new_ori,
        }
    }

    /// Get logical shape under current orientation
    ///
    /// May be transposed compared to physical shape if orientation swaps dimensions.
    #[inline]
    pub fn logical_shape(&self) -> (usize, usize) {
        let nr = self.table.row_count();
        let nc = self.table.columns.len();
        self.ori.logical_shape(nr, nc)
    }

    /// Get physical shape (always same regardless of orientation)
    #[inline]
    pub fn physical_shape(&self) -> (usize, usize) {
        (self.table.row_count(), self.table.columns.len())
    }

    /// Get orientation class for dispatch
    #[inline]
    pub fn ori_class(&self) -> OriClass {
        self.ori.class()
    }

    /// Get reduce mode for aggregations
    #[inline]
    pub fn reduce_mode(&self) -> ReduceMode {
        self.ori.reduce_mode()
    }

    /// Get vector axis for window operations
    #[inline]
    pub fn vec_axis(&self) -> Option<VecAxis> {
        self.ori.vec_axis()
    }

    /// Access element at logical indices (i, j)
    ///
    /// Maps through orientation to physical storage.
    ///
    /// # Panics
    /// Panics if indices are out of bounds or column type mismatch.
    pub fn get_f64(&self, i: usize, j: usize) -> f64 {
        let (nr, nc) = self.physical_shape();
        let (phys_r, phys_c) = self.ori.map_ij(nr, nc, i, j);

        // Physical storage is columns[phys_c][phys_r]
        match &self.table.columns[phys_c] {
            super::Column::F64(data) => data[phys_r],
            _ => panic!("Column {} is not F64", phys_c),
        }
    }

    /// Check if this view shares the same underlying table with another view
    pub fn shares_table_with(&self, other: &TableView) -> bool {
        Arc::ptr_eq(&self.table, &other.table)
    }

    /// Compose current orientation with another D4 orientation (relative orientation change)
    ///
    /// Returns a new view with orientation = other ∘ current.
    /// This is different from `with_orientation()` which sets an absolute orientation.
    ///
    /// # Example:
    /// ```
    /// use blawktrust::{Table, TableView, Column, ORI_H, ORI_Z};
    ///
    /// let table = Table::new(vec!["a".to_string()], vec![Column::F64(vec![1.0])]);
    /// let view = TableView::new(table);  // Starts with H
    ///
    /// // Absolute: replace orientation
    /// let view_z = view.with_orientation(ORI_Z);  // Now Z
    ///
    /// // Relative: compose orientations
    /// let view_rel = view.compose_orientation(ORI_Z).unwrap();  // H ∘ Z = Z
    /// let view_rel2 = view_rel.compose_orientation(ORI_Z).unwrap();  // Z ∘ Z = H
    /// ```
    ///
    /// # Errors
    /// Returns `None` if the other orientation is not D4 (i.e., if it's X or R).
    pub fn compose_orientation(&self, other: Ori) -> Option<Self> {
        let new_ori = super::d4_compose::compose(self.ori, other)?;
        Some(TableView {
            table: Arc::clone(&self.table),
            ori: new_ori,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{Column, ORI_H, ORI_Z};

    fn make_test_table() -> Table {
        // 3x4 table with values a[i][j] = 10*i + j
        let names = vec![
            "col0".to_string(),
            "col1".to_string(),
            "col2".to_string(),
            "col3".to_string(),
        ];

        let columns = (0..4)
            .map(|j| {
                Column::F64(vec![
                    (10 * 0 + j) as f64,
                    (10 * 1 + j) as f64,
                    (10 * 2 + j) as f64,
                ])
            })
            .collect();

        Table::new(names, columns)
    }

    #[test]
    fn test_view_creation() {
        let table = make_test_table();
        let view = TableView::new(table);

        assert_eq!(view.ori, ORI_H);
        assert_eq!(view.physical_shape(), (3, 4));
        assert_eq!(view.logical_shape(), (3, 4));
    }

    #[test]
    fn test_orientation_change_is_o1() {
        let table = make_test_table();
        let view1 = TableView::new(table);

        // Change orientation - should be O(1), no allocation
        let view2 = view1.with_orientation(ORI_Z);

        // Both views share the same underlying table
        assert!(view1.shares_table_with(&view2));

        // But have different orientations
        assert_eq!(view1.ori, ORI_H);
        assert_eq!(view2.ori, ORI_Z);

        // And different logical shapes
        assert_eq!(view1.logical_shape(), (3, 4));
        assert_eq!(view2.logical_shape(), (4, 3)); // Transposed!
    }

    #[test]
    fn test_element_access_h_orientation() {
        let table = make_test_table();
        let view = TableView::new(table);

        // H orientation: identity mapping
        // Logical (i,j) → Physical (i,j)
        // Value at logical (1, 2) should be 10*1 + 2 = 12
        assert_eq!(view.get_f64(1, 2), 12.0);

        // Corner cases
        assert_eq!(view.get_f64(0, 0), 0.0); // 10*0 + 0
        assert_eq!(view.get_f64(2, 3), 23.0); // 10*2 + 3
    }

    #[test]
    fn test_element_access_z_orientation() {
        let table = make_test_table();
        let view = TableView::with_ori(table, ORI_Z);

        // Z orientation: transposed
        // Logical (i,j) → Physical (j,i)
        // Logical shape is (4, 3) now

        // Logical (0, 0) → Physical (0, 0) → value 0
        assert_eq!(view.get_f64(0, 0), 0.0);

        // Logical (1, 2) → Physical (2, 1) → value 10*2 + 1 = 21
        assert_eq!(view.get_f64(1, 2), 21.0);

        // Logical (3, 2) → Physical (2, 3) → value 10*2 + 3 = 23
        assert_eq!(view.get_f64(3, 2), 23.0);
    }

    #[test]
    fn test_multiple_views_same_table() {
        let table = Arc::new(make_test_table());

        let view_h = TableView::from_arc(Arc::clone(&table));
        let view_z = view_h.with_orientation(ORI_Z);
        let view_x = view_h.with_orientation(super::super::ORI_X);

        // All views share the same table
        assert!(view_h.shares_table_with(&view_z));
        assert!(view_h.shares_table_with(&view_x));
        assert!(view_z.shares_table_with(&view_x));

        // But have different orientations
        assert_eq!(view_h.ori_class(), OriClass::ColwiseLike);
        assert_eq!(view_z.ori_class(), OriClass::RowwiseLike);
        assert_eq!(view_x.ori_class(), OriClass::Each);
    }

    #[test]
    fn test_reduce_mode_by_orientation() {
        let table = make_test_table();

        let view_h = TableView::new(table.clone());
        assert_eq!(view_h.reduce_mode(), ReduceMode::ByCols);

        let view_z = TableView::with_ori(table.clone(), ORI_Z);
        assert_eq!(view_z.reduce_mode(), ReduceMode::ByRows);

        let view_r = TableView::with_ori(table, super::super::ORI_R);
        assert_eq!(view_r.reduce_mode(), ReduceMode::Scalar);
    }

    #[test]
    fn test_compose_orientation() {
        let table = make_test_table();

        // Start with H
        let view_h = TableView::new(table);
        assert_eq!(view_h.ori, ORI_H);

        // Compose with Z: H ∘ Z = Z
        let view_z = view_h.compose_orientation(ORI_Z).unwrap();
        assert_eq!(view_z.ori, ORI_Z);
        assert!(view_h.shares_table_with(&view_z));

        // Compose Z with Z: Z ∘ Z = H (involution)
        let view_h2 = view_z.compose_orientation(ORI_Z).unwrap();
        assert_eq!(view_h2.ori, ORI_H);

        // Compose with X/R should return None
        assert!(view_h.compose_orientation(super::super::ORI_X).is_none());
        assert!(view_h.compose_orientation(super::super::ORI_R).is_none());
    }

    #[test]
    fn test_absolute_vs_relative_orientation() {
        let table = make_test_table();
        let view = TableView::new(table);

        // Absolute: always replaces
        let view_abs1 = view.with_orientation(ORI_Z);
        let view_abs2 = view_abs1.with_orientation(ORI_Z);
        assert_eq!(view_abs2.ori, ORI_Z); // Still Z, not H

        // Relative: composes
        let view_rel1 = view.compose_orientation(ORI_Z).unwrap();
        let view_rel2 = view_rel1.compose_orientation(ORI_Z).unwrap();
        assert_eq!(view_rel2.ori, ORI_H); // Z ∘ Z = H (back to identity)
    }

    #[test]
    fn test_ro_rejects_current_x() {
        let table = make_test_table();
        let view_x = TableView::with_ori(table, super::super::ORI_X);

        // ro should reject when current orientation is X
        assert!(view_x.compose_orientation(ORI_Z).is_none());
        assert!(view_x.compose_orientation(ORI_H).is_none());
    }

    #[test]
    fn test_ro_rejects_current_r() {
        let table = make_test_table();
        let view_r = TableView::with_ori(table, super::super::ORI_R);

        // ro should reject when current orientation is R
        assert!(view_r.compose_orientation(ORI_Z).is_none());
        assert!(view_r.compose_orientation(ORI_H).is_none());
    }
}
