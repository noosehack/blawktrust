//! Pipeline planning and optimization
//!
//! This module provides infrastructure for optimizing table operation pipelines:
//!
//! 1. **IR (Intermediate Representation)**: Converts Blisp AST into a linear sequence of steps
//! 2. **Planner**: Splits IR into segments based on orientation class stability
//! 3. **Fused Execution**: Executes colwise segments in a single pass per column
//!
//! ## Architecture
//!
//! ```text
//! Blisp AST: (-> x (o H) (dlog) (x+ 1) (w5))
//!     ↓
//! PipeIR: [OriSet(H), Op(Dlog), Op(AddConst, 1), Op(W5)]
//!     ↓
//! ExecutionPlan: [Segment(Colwise, [Dlog, AddConst, W5])]
//!     ↓
//! ColwiseKernel: Single-pass execution per column
//! ```
//!
//! ## Benefits
//!
//! - **Fewer allocations**: One output buffer per column instead of N intermediate tables
//! - **Better cache locality**: Single pass through data
//! - **Reduced dispatch overhead**: Plan once, execute optimally
//!
//! ## Limitations (Phase 3.1)
//!
//! - Only ColwiseLike segments fuse
//! - Limited op set: dlog, arithmetic, w5, cumsum
//! - No cross-segment optimization
//! - Single-threaded execution

pub mod ir;
pub mod execution_plan;
pub mod planner;
pub mod colwise_fused;
pub mod executor;

pub use ir::{OpId, Step, PipeIR};
pub use execution_plan::{ExecutionPlan, Segment, SegmentKind, OpStep};
pub use planner::Planner;
pub use colwise_fused::{ColwiseKernel, FusedOp};
pub use executor::{Executor, ExecutionValue, ExecutionResult, ExecutionStats};
