//! blawktrust: High-performance columnar analytical engine
//!
//! Fast, memory-safe columnar operations with zero-allocation execution.

pub mod table;
pub mod io;
pub mod expr;
pub mod exec;
pub mod builtins;
// pub mod pipeline;  // WIP: untracked

pub use table::{
    Table, Column, NULL_DATE, NULL_TIMESTAMP, NULL_TS,
    TableView, Ori, OriClass,
    ORI_H, ORI_N, ORI__N, ORI__H,
    ORI_Z, ORI_S, ORI__Z, ORI__S,
    ORI_X, ORI_R,
    ReduceMode, VecAxis, lookup_ori, compose,
};
pub use builtins::{dlog_column, ln_column, abs_column, sum, sum0, mean, mean0};
