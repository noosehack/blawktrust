//! Pipeline planner
//!
//! Converts PipeIR into ExecutionPlan by:
//! 1. Tracking orientation class symbolically
//! 2. Splitting into segments at boundaries (orientation changes, reducers, etc.)

use crate::table::{Ori, OriClass, ORI_H};
use super::ir::{OpId, PipeIR, Step};
use super::execution_plan::{ExecutionPlan, Segment, SegmentKind, OpStep};

/// Pipeline planner
pub struct Planner {
    /// Current orientation (tracked symbolically)
    current_ori: Ori,

    /// Current segment being built
    current_segment: Option<Segment>,

    /// Completed segments
    segments: Vec<Segment>,
}

impl Planner {
    /// Create a new planner with default H orientation
    pub fn new() -> Self {
        Planner {
            current_ori: ORI_H,
            current_segment: None,
            segments: Vec::new(),
        }
    }

    /// Plan a pipeline IR into an execution plan
    pub fn plan(ir: &PipeIR) -> ExecutionPlan {
        let mut planner = Planner::new();

        for step in &ir.steps {
            planner.process_step(step);
        }

        // Flush any remaining segment
        planner.flush_segment();

        ExecutionPlan {
            segments: planner.segments,
        }
    }

    /// Process a single step
    fn process_step(&mut self, step: &Step) {
        match step {
            Step::OriSet(new_ori) => {
                // Absolute orientation change - always flush
                self.flush_segment();
                self.current_ori = *new_ori;
            }

            Step::OriRel(rel_ori) => {
                // Relative orientation change - compose and check if class changed
                if let Some(new_ori) = super::super::table::d4_compose::compose(self.current_ori, *rel_ori) {
                    let old_class = self.current_ori.class();
                    let new_class = new_ori.class();

                    if old_class != new_class {
                        // Class changed - flush segment
                        self.flush_segment();
                    }

                    self.current_ori = new_ori;
                } else {
                    // Composition failed (X or R) - flush and set absolute
                    self.flush_segment();
                    self.current_ori = *rel_ori;
                }
            }

            Step::Op { name, args } => {
                // Check if this op requires a boundary
                if self.requires_boundary(name) {
                    self.flush_segment();
                }

                // Ensure we have a segment
                if self.current_segment.is_none() {
                    self.start_segment();
                }

                // Add op to current segment
                if let Some(seg) = &mut self.current_segment {
                    seg.push(OpStep {
                        name: name.clone(),
                        args: args.clone(),
                    });
                }

                // If this was a reducer, flush immediately
                if self.is_reducer(name) {
                    self.flush_segment();
                }
            }
        }
    }

    /// Check if an operation requires a segment boundary before it
    fn requires_boundary(&self, op: &OpId) -> bool {
        // Reducers change shape/type - always boundary
        self.is_reducer(op)
    }

    /// Check if an operation is a reducer (changes shape/type)
    fn is_reducer(&self, op: &OpId) -> bool {
        matches!(op, OpId::Sum | OpId::Mean)
    }

    /// Start a new segment with current orientation
    fn start_segment(&mut self) {
        let kind = self.orientation_to_segment_kind(self.current_ori);
        self.current_segment = Some(Segment::new(kind, self.current_ori));
    }

    /// Flush current segment to completed list
    fn flush_segment(&mut self) {
        if let Some(seg) = self.current_segment.take() {
            if !seg.ops.is_empty() {
                self.segments.push(seg);
            }
        }
    }

    /// Convert orientation to segment kind
    fn orientation_to_segment_kind(&self, ori: Ori) -> SegmentKind {
        match ori.class() {
            OriClass::ColwiseLike => SegmentKind::Colwise,
            OriClass::RowwiseLike => SegmentKind::Rowwise,
            OriClass::Each => SegmentKind::Each,
            OriClass::Real => SegmentKind::Real,
        }
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{ORI_H, ORI_Z};

    #[test]
    fn test_simple_colwise_segment() {
        let mut ir = PipeIR::new();
        ir.push(Step::OriSet(ORI_H));
        ir.push(Step::Op { name: OpId::Dlog, args: vec![1.0] });
        ir.push(Step::Op { name: OpId::AddConst, args: vec![10.0] });

        let plan = Planner::plan(&ir);

        assert_eq!(plan.segments.len(), 1);
        assert_eq!(plan.segments[0].kind, SegmentKind::Colwise);
        assert_eq!(plan.segments[0].ops.len(), 2);
    }

    #[test]
    fn test_orientation_change_splits_segment() {
        let mut ir = PipeIR::new();
        ir.push(Step::OriSet(ORI_H));
        ir.push(Step::Op { name: OpId::Dlog, args: vec![1.0] });
        ir.push(Step::OriSet(ORI_Z)); // Orientation change
        ir.push(Step::Op { name: OpId::W5, args: vec![] });

        let plan = Planner::plan(&ir);

        assert_eq!(plan.segments.len(), 2);
        assert_eq!(plan.segments[0].kind, SegmentKind::Colwise);
        assert_eq!(plan.segments[0].ops.len(), 1); // Just dlog
        assert_eq!(plan.segments[1].kind, SegmentKind::Rowwise);
        assert_eq!(plan.segments[1].ops.len(), 1); // Just w5
    }

    #[test]
    fn test_reducer_splits_segment() {
        let mut ir = PipeIR::new();
        ir.push(Step::OriSet(ORI_H));
        ir.push(Step::Op { name: OpId::Dlog, args: vec![1.0] });
        ir.push(Step::Op { name: OpId::Sum, args: vec![] }); // Reducer
        ir.push(Step::Op { name: OpId::AddConst, args: vec![1.0] });

        let plan = Planner::plan(&ir);

        // Should have 3 segments: [dlog], [sum], [add]
        assert_eq!(plan.segments.len(), 3);
        assert_eq!(plan.segments[0].ops.len(), 1); // dlog
        assert_eq!(plan.segments[1].ops.len(), 1); // sum
        assert_eq!(plan.segments[2].ops.len(), 1); // add
    }

    #[test]
    fn test_empty_ir() {
        let ir = PipeIR::new();
        let plan = Planner::plan(&ir);

        assert!(plan.segments.is_empty());
    }

    #[test]
    fn test_multiple_ops_in_segment() {
        let mut ir = PipeIR::new();
        ir.push(Step::OriSet(ORI_H));
        ir.push(Step::Op { name: OpId::Dlog, args: vec![1.0] });
        ir.push(Step::Op { name: OpId::AddConst, args: vec![5.0] });
        ir.push(Step::Op { name: OpId::MulConst, args: vec![2.0] });
        ir.push(Step::Op { name: OpId::W5, args: vec![] });

        let plan = Planner::plan(&ir);

        assert_eq!(plan.segments.len(), 1);
        assert_eq!(plan.segments[0].kind, SegmentKind::Colwise);
        assert_eq!(plan.segments[0].ops.len(), 4);
    }
}
