//! Pipeline executor
//!
//! Executes an ExecutionPlan by dispatching segments to appropriate executors.

use crate::table::{Table, Column, TableView};
use crate::builtins::ori_ops;
use super::execution_plan::{ExecutionPlan, Segment, SegmentKind};
use super::colwise_fused::ColwiseKernel;
use std::sync::Arc;

/// Execution statistics for performance measurement
#[derive(Clone, Debug, Default)]
pub struct ExecutionStats {
    /// Number of segments executed
    pub segments_executed: usize,

    /// Number of segments that used fusion
    pub segments_fused: usize,

    /// Number of segments that fell back to unfused execution
    pub segments_unfused: usize,

    /// Number of column allocations
    pub allocations: usize,
}

/// Pipeline executor
pub struct Executor {
    stats: ExecutionStats,
}

impl Executor {
    /// Create a new executor
    pub fn new() -> Self {
        Executor {
            stats: ExecutionStats::default(),
        }
    }

    /// Execute a plan on input table
    pub fn execute(&mut self, plan: &ExecutionPlan, input: Table) -> Result<ExecutionResult, String> {
        let mut current_value = ExecutionValue::Table(input);

        for segment in &plan.segments {
            current_value = self.execute_segment(segment, current_value)?;
            self.stats.segments_executed += 1;
        }

        Ok(ExecutionResult {
            value: current_value,
            stats: self.stats.clone(),
        })
    }

    /// Execute a single segment
    fn execute_segment(&mut self, segment: &Segment, input: ExecutionValue) -> Result<ExecutionValue, String> {
        match segment.kind {
            SegmentKind::Colwise => self.execute_colwise_segment(segment, input),
            SegmentKind::Rowwise => self.execute_rowwise_segment(segment, input),
            SegmentKind::Each | SegmentKind::Real => self.execute_other_segment(segment, input),
            SegmentKind::Scalar | SegmentKind::Vector => {
                // These should not appear in table pipelines
                Err("Scalar/Vector segments not supported in table pipelines".to_string())
            }
        }
    }

    /// Execute a colwise segment (try fusion, fallback to unfused)
    fn execute_colwise_segment(&mut self, segment: &Segment, input: ExecutionValue) -> Result<ExecutionValue, String> {
        let table = input.as_table()?;

        // Try to build a fused kernel
        if let Some(kernel) = ColwiseKernel::from_segment(segment) {
            // Execute fused
            self.stats.segments_fused += 1;
            self.stats.allocations += table.columns.len(); // One allocation per column
            let result = kernel.execute(&table);
            Ok(ExecutionValue::Table(result))
        } else {
            // Fallback to unfused execution
            self.stats.segments_unfused += 1;
            self.execute_unfused_colwise(segment, table)
        }
    }

    /// Execute colwise segment without fusion (fallback)
    fn execute_unfused_colwise(&mut self, _segment: &Segment, table: Table) -> Result<ExecutionValue, String> {
        // Execute each op in sequence using existing kernels
        // For now, return error - we haven't implemented unfused dispatch yet
        Err("Unfused colwise execution not yet implemented".to_string())
    }

    /// Execute a rowwise segment
    fn execute_rowwise_segment(&mut self, _segment: &Segment, input: ExecutionValue) -> Result<ExecutionValue, String> {
        let table = input.as_table()?;
        self.stats.segments_unfused += 1;

        // For now, just return the table unchanged
        // TODO: Implement rowwise dispatch
        Ok(ExecutionValue::Table(table))
    }

    /// Execute other segment types (Each, Real)
    fn execute_other_segment(&mut self, _segment: &Segment, input: ExecutionValue) -> Result<ExecutionValue, String> {
        let table = input.as_table()?;
        self.stats.segments_unfused += 1;

        // For now, just return the table unchanged
        // TODO: Implement Each/Real dispatch
        Ok(ExecutionValue::Table(table))
    }

    /// Get execution statistics
    pub fn stats(&self) -> &ExecutionStats {
        &self.stats
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

/// Value types during execution
#[derive(Clone, Debug)]
pub enum ExecutionValue {
    Table(Table),
    Column(Column),
    Scalar(f64),
}

impl ExecutionValue {
    fn as_table(&self) -> Result<Table, String> {
        match self {
            ExecutionValue::Table(t) => Ok(t.clone()),
            _ => Err("Expected Table value".to_string()),
        }
    }
}

/// Result of pipeline execution
pub struct ExecutionResult {
    pub value: ExecutionValue,
    pub stats: ExecutionStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{PipeIR, Step, OpId, Planner};
    use crate::table::ORI_H;

    #[test]
    fn test_execute_simple_pipeline() {
        // Create IR: (o H) (x+ 10) (x* 2)
        let mut ir = PipeIR::new();
        ir.push(Step::OriSet(ORI_H));
        ir.push(Step::Op { name: OpId::AddConst, args: vec![10.0] });
        ir.push(Step::Op { name: OpId::MulConst, args: vec![2.0] });

        // Plan
        let plan = Planner::plan(&ir);

        // Create input table
        let input = Table::new(
            vec!["a".to_string()],
            vec![Column::F64(vec![1.0, 2.0, 3.0])],
        );

        // Execute
        let mut executor = Executor::new();
        let result = executor.execute(&plan, input).unwrap();

        // Check result
        if let ExecutionValue::Table(table) = result.value {
            if let Column::F64(data) = &table.columns[0] {
                // (x + 10) * 2 = (1+10)*2=22, (2+10)*2=24, (3+10)*2=26
                assert_eq!(data, &vec![22.0, 24.0, 26.0]);
            } else {
                panic!("Expected F64 column");
            }
        } else {
            panic!("Expected Table result");
        }

        // Check stats
        assert_eq!(result.stats.segments_executed, 1);
        assert_eq!(result.stats.segments_fused, 1);
    }

    #[test]
    fn test_execute_with_dlog() {
        // Create IR: (o H) (dlog)
        let mut ir = PipeIR::new();
        ir.push(Step::OriSet(ORI_H));
        ir.push(Step::Op { name: OpId::Dlog, args: vec![1.0] });

        // Plan
        let plan = Planner::plan(&ir);

        // Create input table
        let input = Table::new(
            vec!["prices".to_string()],
            vec![Column::F64(vec![100.0, 110.0, 121.0])],
        );

        // Execute
        let mut executor = Executor::new();
        let result = executor.execute(&plan, input).unwrap();

        // Check result
        if let ExecutionValue::Table(table) = result.value {
            if let Column::F64(data) = &table.columns[0] {
                assert!(data[0].is_nan());
                assert!((data[1] - (110.0_f64 / 100.0_f64).ln()).abs() < 1e-10);
                assert!((data[2] - (121.0_f64 / 110.0_f64).ln()).abs() < 1e-10);
            } else {
                panic!("Expected F64 column");
            }
        } else {
            panic!("Expected Table result");
        }
    }

    #[test]
    fn test_multi_column_execution() {
        // Create IR: (o H) (x* 3)
        let mut ir = PipeIR::new();
        ir.push(Step::OriSet(ORI_H));
        ir.push(Step::Op { name: OpId::MulConst, args: vec![3.0] });

        // Plan
        let plan = Planner::plan(&ir);

        // Create input table with 2 columns
        let input = Table::new(
            vec!["a".to_string(), "b".to_string()],
            vec![
                Column::F64(vec![1.0, 2.0]),
                Column::F64(vec![3.0, 4.0]),
            ],
        );

        // Execute
        let mut executor = Executor::new();
        let result = executor.execute(&plan, input).unwrap();

        // Check result
        if let ExecutionValue::Table(table) = result.value {
            if let Column::F64(data) = &table.columns[0] {
                assert_eq!(data, &vec![3.0, 6.0]);
            }
            if let Column::F64(data) = &table.columns[1] {
                assert_eq!(data, &vec![9.0, 12.0]);
            }
        }

        // Check that we allocated 2 columns (one per input column)
        assert_eq!(result.stats.allocations, 2);
    }
}
