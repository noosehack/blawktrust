//! Built-in operations

pub mod fast_kernels;
pub mod kernels_fused;
pub mod kernels_masked;
pub mod kernels_wordwise;
pub mod math;
// pub mod nulls;  // Obsolete: kdb-style uses embedded sentinels, not bitmap conversion
pub mod ops;
pub mod ori_ops;
pub mod rolling_moments;
pub mod scratch;

// Re-exports from math are unused at module level
// pub use nulls::*;  // Removed: bitmap-based null handling obsolete
pub use ops::{abs_column, dlog_column, ln_column, mean, mean0, sum, sum0};
pub use rolling_moments::{rolling_moments_past_only_f64, MomentsMask};
pub use scratch::Scratch;
