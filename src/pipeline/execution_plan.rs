//! Execution plan with segments
//!
//! Splits a pipeline into segments based on orientation class stability.
//! Each segment has a constant orientation class and can be optimized independently.

use crate::table::{Ori, OriClass};
use super::ir::{OpId, Step};

/// Kind of execution segment
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentKind {
    /// Colwise operations (ColwiseLike orientation)
    Colwise,

    /// Rowwise operations (RowwiseLike orientation)
    Rowwise,

    /// Elementwise operations (Each orientation)
    Each,

    /// Scalar reduction (Real orientation)
    Real,

    /// Single scalar operation (non-table input/output)
    Scalar,

    /// Single vector operation (non-table input/output)
    Vector,
}

/// A single operation within a segment
#[derive(Clone, Debug)]
pub struct OpStep {
    pub name: OpId,
    pub args: Vec<f64>,
}

/// A segment of operations with stable orientation class
#[derive(Clone, Debug)]
pub struct Segment {
    /// Kind of segment (determines dispatch strategy)
    pub kind: SegmentKind,

    /// Orientation at start of segment
    pub start_ori: Ori,

    /// Operations in this segment
    pub ops: Vec<OpStep>,
}

/// Complete execution plan for a pipeline
#[derive(Clone, Debug)]
pub struct ExecutionPlan {
    /// Segments to execute in order
    pub segments: Vec<Segment>,
}

impl ExecutionPlan {
    /// Create empty execution plan
    pub fn new() -> Self {
        ExecutionPlan {
            segments: Vec::new(),
        }
    }

    /// Add a segment to the plan
    pub fn push(&mut self, segment: Segment) {
        self.segments.push(segment);
    }

    /// Number of segments in plan
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Check if plan is empty
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl Default for ExecutionPlan {
    fn default() -> Self {
        Self::new()
    }
}

impl Segment {
    /// Create a new segment
    pub fn new(kind: SegmentKind, start_ori: Ori) -> Self {
        Segment {
            kind,
            start_ori,
            ops: Vec::new(),
        }
    }

    /// Add an operation to this segment
    pub fn push(&mut self, op: OpStep) {
        self.ops.push(op);
    }

    /// Check if this segment can fuse operations
    pub fn is_fusable(&self) -> bool {
        // Only Colwise segments can currently fuse
        if self.kind != SegmentKind::Colwise {
            return false;
        }

        // Check if all ops are in the fusable subset
        self.ops.iter().all(|op| is_fusable_op(&op.name))
    }
}

/// Check if an operation is in the fusable subset
fn is_fusable_op(op: &OpId) -> bool {
    matches!(
        op,
        OpId::Dlog
            | OpId::AddConst
            | OpId::SubConst
            | OpId::MulConst
            | OpId::DivConst
            | OpId::W5
            | OpId::Cs1
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::ORI_H;

    #[test]
    fn test_segment_creation() {
        let mut seg = Segment::new(SegmentKind::Colwise, ORI_H);
        assert_eq!(seg.kind, SegmentKind::Colwise);
        assert!(seg.ops.is_empty());

        seg.push(OpStep {
            name: OpId::Dlog,
            args: vec![1.0],
        });
        seg.push(OpStep {
            name: OpId::AddConst,
            args: vec![10.0],
        });

        assert_eq!(seg.ops.len(), 2);
    }

    #[test]
    fn test_fusable_ops() {
        assert!(is_fusable_op(&OpId::Dlog));
        assert!(is_fusable_op(&OpId::AddConst));
        assert!(is_fusable_op(&OpId::W5));
        assert!(!is_fusable_op(&OpId::Sum));
        assert!(!is_fusable_op(&OpId::Generic("custom".to_string())));
    }

    #[test]
    fn test_segment_fusability() {
        let mut seg = Segment::new(SegmentKind::Colwise, ORI_H);
        seg.push(OpStep {
            name: OpId::Dlog,
            args: vec![1.0],
        });
        seg.push(OpStep {
            name: OpId::MulConst,
            args: vec![2.0],
        });

        assert!(seg.is_fusable());

        // Add a non-fusable op
        seg.push(OpStep {
            name: OpId::Sum,
            args: vec![],
        });

        assert!(!seg.is_fusable());
    }

    #[test]
    fn test_execution_plan() {
        let mut plan = ExecutionPlan::new();
        assert!(plan.is_empty());

        let seg1 = Segment::new(SegmentKind::Colwise, ORI_H);
        plan.push(seg1);

        assert_eq!(plan.len(), 1);
    }
}
