//! Intermediate Representation for pipeline operations
//!
//! This IR represents a sequence of operations in a pipeline (-> x op1 op2 op3)
//! before planning and optimization.

use crate::table::Ori;

/// Operation identifier
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OpId {
    /// Delta log: dlog(period)
    Dlog,
    /// Rolling window mean: w5 (window=5)
    W5,
    /// Cumulative sum: cs1
    Cs1,
    /// Add constant: x+ c
    AddConst,
    /// Subtract constant: x- c
    SubConst,
    /// Multiply constant: x* c
    MulConst,
    /// Divide constant: x/ c
    DivConst,
    /// Sum aggregation
    Sum,
    /// Mean aggregation
    Mean,
    /// Generic operation (fallback)
    Generic(String),
}

/// A single step in the pipeline IR
#[derive(Clone, Debug)]
pub enum Step {
    /// Set absolute orientation: (o A)
    OriSet(Ori),

    /// Compose relative orientation: (ro A)
    OriRel(Ori),

    /// Apply operation with arguments
    Op {
        name: OpId,
        /// Scalar arguments (constants for arithmetic, lag for dlog, etc.)
        args: Vec<f64>,
    },
}

/// Pipeline intermediate representation
///
/// Represents a linear sequence of operations: (-> x step1 step2 ... stepN)
#[derive(Clone, Debug)]
pub struct PipeIR {
    pub steps: Vec<Step>,
}

impl PipeIR {
    /// Create empty pipeline
    pub fn new() -> Self {
        PipeIR { steps: Vec::new() }
    }

    /// Add a step to the pipeline
    pub fn push(&mut self, step: Step) {
        self.steps.push(step);
    }

    /// Number of steps in pipeline
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if pipeline is empty
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl Default for PipeIR {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::ORI_H;

    #[test]
    fn test_pipe_ir_creation() {
        let mut ir = PipeIR::new();
        assert!(ir.is_empty());

        ir.push(Step::OriSet(ORI_H));
        ir.push(Step::Op { name: OpId::Dlog, args: vec![1.0] });
        ir.push(Step::Op { name: OpId::AddConst, args: vec![10.0] });

        assert_eq!(ir.len(), 3);
    }

    #[test]
    fn test_step_types() {
        let step1 = Step::OriSet(ORI_H);
        let step2 = Step::Op { name: OpId::W5, args: vec![] };
        let step3 = Step::Op { name: OpId::MulConst, args: vec![2.5] };

        let mut ir = PipeIR::new();
        ir.push(step1);
        ir.push(step2);
        ir.push(step3);

        assert_eq!(ir.len(), 3);
    }
}
