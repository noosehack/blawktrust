//! blawktrust: High-performance columnar analytical engine
//!
//! Fast, memory-safe columnar operations with zero-allocation execution.

pub mod table;
pub mod io;
pub mod expr;
pub mod exec;
pub mod builtins;

pub use table::{Table, Column, NULL_TS};
pub use builtins::{dlog_column, ln_column, abs_column, sum, sum0, mean, mean0};
