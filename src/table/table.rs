//! Table structure (collection of typed columns)

use super::Column;

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
