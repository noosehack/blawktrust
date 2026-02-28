//! Core table and column types

pub mod bitmap;
pub mod column;
pub mod d4_compose;
pub mod orientation;
pub mod view;

pub use bitmap::Bitmap;
pub use column::{Column, NULL_DATE, NULL_TIMESTAMP, NULL_TS};
pub use d4_compose::compose;

/// A table is a collection of named, typed columns
#[derive(Debug, Clone)]
pub struct Table {
    pub names: Vec<String>,
    pub columns: Vec<Column>,
}

impl Table {
    pub fn new(names: Vec<String>, columns: Vec<Column>) -> Self {
        assert_eq!(names.len(), columns.len());
        Self { names, columns }
    }

    pub fn row_count(&self) -> usize {
        self.columns.first().map(|c| c.len()).unwrap_or(0)
    }

    pub fn col_count(&self) -> usize {
        self.columns.len()
    }
}
pub use orientation::{
    lookup_ori, Ori, OriClass, OriSpec, ReduceMode, VecAxis, ORI_H, ORI_N, ORI_R, ORI_S, ORI_SPECS,
    ORI_X, ORI_Z, ORI__H, ORI__N, ORI__S, ORI__Z,
};
// Table is now defined directly in this module
pub use view::TableView;
