//! blawktrust: High-performance columnar analytical engine
//!
//! Fast, memory-safe columnar operations with zero-allocation execution.

pub mod builtins;
pub mod exec;
pub mod expr;
pub mod io;
pub mod table;
// pub mod pipeline;  // WIP: untracked

pub use builtins::{abs_column, dlog_column, ln_column, mean, mean0, sum, sum0};
pub use table::{
    compose, lookup_ori, Column, Ori, OriClass, ReduceMode, Table, TableView, VecAxis, NULL_DATE,
    NULL_TIMESTAMP, NULL_TS, ORI_H, ORI_N, ORI_R, ORI_S, ORI_X, ORI_Z, ORI__H, ORI__N, ORI__S,
    ORI__Z,
};

/// API Contract Self-Test
///
/// This test is a local failsafe that catches API removal even without CI.
/// If BLISP's integration test is removed, this ensures blawktrust itself
/// fails to compile if critical types are removed.
///
/// **DO NOT REMOVE** - This is part of the public API stability contract.
#[cfg(test)]
mod api_contract_self_test {
    use super::*;

    /// Ensures Column types that downstream crates depend on exist
    #[test]
    fn column_types_api_contract() {
        // These must compile - if removed, local tests fail
        let _f64 = Column::F64(vec![1.0]);
        let _date = Column::Date(vec![18628]);
        let _timestamp = Column::Timestamp(vec![0]);
        let _ts = Column::Ts(vec![100]);

        // Constructors must exist
        let _f64_ctor = Column::new_f64(vec![1.0]);
        let _date_ctor = Column::new_date(vec![18628]);
        let _timestamp_ctor = Column::new_timestamp(vec![0]);
        let _ts_ctor = Column::new_ts(vec![100]);
    }

    /// Ensures NULL sentinels that downstream crates depend on are exported
    #[test]
    fn null_sentinels_api_contract() {
        // These must be public exports
        let _date_null: i32 = NULL_DATE;
        let _timestamp_null: i64 = NULL_TIMESTAMP;
        let _ts_null: i64 = NULL_TS;

        // Values must be sentinels
        assert_eq!(NULL_DATE, i32::MIN);
        assert_eq!(NULL_TIMESTAMP, i64::MIN);
        assert_eq!(NULL_TS, i64::MIN);
    }

    /// Ensures TableView and orientation types exist
    #[test]
    fn tableview_api_contract() {
        let table = Table::new(vec!["a".to_string()], vec![Column::new_f64(vec![1.0])]);

        // TableView construction must work
        let _view_h = TableView::with_ori(table.clone(), ORI_H);
        let _view_z = TableView::with_ori(table.clone(), ORI_Z);
        let _view_r = TableView::with_ori(table, ORI_R);

        // Orientation constants must exist
        let _h = ORI_H;
        let _z = ORI_Z;
        let _r = ORI_R;
        let _x = ORI_X;
    }
}
