//! Fused colwise kernel executor
//!
//! Executes a sequence of colwise operations in a single pass per column,
//! minimizing intermediate allocations.

use crate::table::{Table, Column};
use super::execution_plan::Segment;
use super::ir::OpId;

/// Fused operation types (safe subset for fusion)
#[derive(Clone, Debug)]
pub enum FusedOp {
    /// Delta log: log(x[i]) - log(x[i-period])
    Dlog { period: usize },

    /// Add constant: x[i] + c
    AddConst(f64),

    /// Subtract constant: x[i] - c
    SubConst(f64),

    /// Multiply constant: x[i] * c
    MulConst(f64),

    /// Divide constant: x[i] / c
    DivConst(f64),

    /// Rolling window mean (window=5)
    WMean5,

    /// Cumulative sum
    Cumsum,
}

/// Fused colwise kernel
#[derive(Clone, Debug)]
pub struct ColwiseKernel {
    pub ops: Vec<FusedOp>,
}

impl ColwiseKernel {
    /// Try to build a fused kernel from a segment
    ///
    /// Returns None if segment contains non-fusable operations.
    pub fn from_segment(segment: &Segment) -> Option<Self> {
        if !segment.is_fusable() {
            return None;
        }

        let mut ops = Vec::new();

        for op_step in &segment.ops {
            let fused_op = match &op_step.name {
                OpId::Dlog => {
                    let period = op_step.args.get(0).copied().unwrap_or(1.0) as usize;
                    FusedOp::Dlog { period }
                }
                OpId::AddConst => {
                    let c = op_step.args.get(0).copied().unwrap_or(0.0);
                    FusedOp::AddConst(c)
                }
                OpId::SubConst => {
                    let c = op_step.args.get(0).copied().unwrap_or(0.0);
                    FusedOp::SubConst(c)
                }
                OpId::MulConst => {
                    let c = op_step.args.get(0).copied().unwrap_or(1.0);
                    FusedOp::MulConst(c)
                }
                OpId::DivConst => {
                    let c = op_step.args.get(0).copied().unwrap_or(1.0);
                    FusedOp::DivConst(c)
                }
                OpId::W5 => FusedOp::WMean5,
                OpId::Cs1 => FusedOp::Cumsum,
                _ => return None, // Non-fusable op
            };

            ops.push(fused_op);
        }

        Some(ColwiseKernel { ops })
    }

    /// Execute kernel on a table
    ///
    /// Processes each F64 column in a single pass through the kernel.
    /// Preserves Date/Timestamp columns unchanged.
    pub fn execute(&self, input: &Table) -> Table {
        let mut new_columns = Vec::with_capacity(input.columns.len());

        for col in &input.columns {
            let new_col = match col {
                Column::F64(data) => {
                    // Execute fused kernel on this column
                    let result = self.execute_column(data);
                    Column::F64(result)
                }
                Column::Date(_) | Column::Timestamp(_) => {
                    // Preserve temporal columns unchanged
                    col.clone()
                }
            };

            new_columns.push(new_col);
        }

        Table::new(input.names.clone(), new_columns)
    }

    /// Execute kernel on a single F64 column
    fn execute_column(&self, data: &[f64]) -> Vec<f64> {
        let n = data.len();
        if n == 0 {
            return Vec::new();
        }

        // Start with input data
        let mut result = data.to_vec();

        // Apply each operation in sequence
        for op in &self.ops {
            result = self.apply_op(op, &result);
        }

        result
    }

    /// Apply a single fused operation
    fn apply_op(&self, op: &FusedOp, data: &[f64]) -> Vec<f64> {
        let n = data.len();

        match op {
            FusedOp::Dlog { period } => {
                let mut out = vec![f64::NAN; n];
                for i in *period..n {
                    let curr = data[i];
                    let prev = data[i - period];
                    if curr.is_nan() || prev.is_nan() || curr <= 0.0 || prev <= 0.0 {
                        out[i] = f64::NAN;
                    } else {
                        out[i] = curr.ln() - prev.ln();
                    }
                }
                out
            }

            FusedOp::AddConst(c) => {
                let mut out = Vec::with_capacity(n);
                for &x in data {
                    out.push(if x.is_nan() { f64::NAN } else { x + c });
                }
                out
            }

            FusedOp::SubConst(c) => {
                let mut out = Vec::with_capacity(n);
                for &x in data {
                    out.push(if x.is_nan() { f64::NAN } else { x - c });
                }
                out
            }

            FusedOp::MulConst(c) => {
                let mut out = Vec::with_capacity(n);
                for &x in data {
                    out.push(if x.is_nan() { f64::NAN } else { x * c });
                }
                out
            }

            FusedOp::DivConst(c) => {
                let mut out = Vec::with_capacity(n);
                for &x in data {
                    out.push(if x.is_nan() { f64::NAN } else { x / c });
                }
                out
            }

            FusedOp::WMean5 => {
                const WINDOW: usize = 5;
                let mut out = vec![f64::NAN; n];

                for i in WINDOW - 1..n {
                    let mut sum = 0.0;
                    let mut count = 0;

                    for j in 0..WINDOW {
                        let val = data[i - j];
                        if !val.is_nan() {
                            sum += val;
                            count += 1;
                        }
                    }

                    out[i] = if count > 0 {
                        sum / (count as f64)
                    } else {
                        f64::NAN
                    };
                }

                out
            }

            FusedOp::Cumsum => {
                let mut out = Vec::with_capacity(n);
                let mut cumsum = 0.0;

                for &x in data {
                    if x.is_nan() {
                        out.push(f64::NAN);
                    } else {
                        cumsum += x;
                        out.push(cumsum);
                    }
                }

                out
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fused_add_const() {
        let kernel = ColwiseKernel {
            ops: vec![FusedOp::AddConst(10.0)],
        };

        let data = vec![1.0, 2.0, 3.0];
        let result = kernel.execute_column(&data);

        assert_eq!(result, vec![11.0, 12.0, 13.0]);
    }

    #[test]
    fn test_fused_mul_const() {
        let kernel = ColwiseKernel {
            ops: vec![FusedOp::MulConst(2.0)],
        };

        let data = vec![1.0, 2.0, 3.0];
        let result = kernel.execute_column(&data);

        assert_eq!(result, vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_fused_chain() {
        // x * 2 + 10
        let kernel = ColwiseKernel {
            ops: vec![FusedOp::MulConst(2.0), FusedOp::AddConst(10.0)],
        };

        let data = vec![1.0, 2.0, 3.0];
        let result = kernel.execute_column(&data);

        assert_eq!(result, vec![12.0, 14.0, 16.0]);
    }

    #[test]
    fn test_fused_dlog() {
        let kernel = ColwiseKernel {
            ops: vec![FusedOp::Dlog { period: 1 }],
        };

        let data = vec![100.0, 110.0, 121.0];
        let result = kernel.execute_column(&data);

        assert!(result[0].is_nan());
        assert!((result[1] - (110.0_f64 / 100.0).ln()).abs() < 1e-10);
        assert!((result[2] - (121.0_f64 / 110.0).ln()).abs() < 1e-10);
    }

    #[test]
    fn test_fused_cumsum() {
        let kernel = ColwiseKernel {
            ops: vec![FusedOp::Cumsum],
        };

        let data = vec![1.0, 2.0, 3.0, 4.0];
        let result = kernel.execute_column(&data);

        assert_eq!(result, vec![1.0, 3.0, 6.0, 10.0]);
    }

    #[test]
    fn test_fused_wmean5() {
        let kernel = ColwiseKernel {
            ops: vec![FusedOp::WMean5],
        };

        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let result = kernel.execute_column(&data);

        // First 4 should be NaN (not enough data)
        assert!(result[0].is_nan());
        assert!(result[1].is_nan());
        assert!(result[2].is_nan());
        assert!(result[3].is_nan());

        // Position 4: mean of [1,2,3,4,5] = 3.0
        assert!((result[4] - 3.0).abs() < 1e-10);

        // Position 5: mean of [2,3,4,5,6] = 4.0
        assert!((result[5] - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_nan_handling() {
        let kernel = ColwiseKernel {
            ops: vec![FusedOp::AddConst(10.0)],
        };

        let data = vec![1.0, f64::NAN, 3.0];
        let result = kernel.execute_column(&data);

        assert_eq!(result[0], 11.0);
        assert!(result[1].is_nan());
        assert_eq!(result[2], 13.0);
    }

    #[test]
    fn test_execute_table() {
        let kernel = ColwiseKernel {
            ops: vec![FusedOp::MulConst(2.0)],
        };

        let table = Table::new(
            vec!["a".to_string(), "b".to_string()],
            vec![
                Column::F64(vec![1.0, 2.0, 3.0]),
                Column::F64(vec![4.0, 5.0, 6.0]),
            ],
        );

        let result = kernel.execute(&table);

        if let Column::F64(data) = &result.columns[0] {
            assert_eq!(data, &vec![2.0, 4.0, 6.0]);
        } else {
            panic!("Expected F64 column");
        }

        if let Column::F64(data) = &result.columns[1] {
            assert_eq!(data, &vec![8.0, 10.0, 12.0]);
        } else {
            panic!("Expected F64 column");
        }
    }
}
