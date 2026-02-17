//! Built-in operations

pub mod math;
pub mod fast_kernels;
pub mod kernels_masked;
pub mod kernels_fused;
pub mod kernels_wordwise;
// pub mod nulls;  // Obsolete: kdb-style uses embedded sentinels, not bitmap conversion
pub mod ops;
pub mod scratch;

pub use math::*;
// pub use nulls::*;  // Removed: bitmap-based null handling obsolete
pub use ops::{dlog_column, ln_column, abs_column, sum, sum0, mean, mean0};
pub use scratch::Scratch;
