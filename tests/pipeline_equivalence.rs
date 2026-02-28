//! Equivalence tests for pipeline fusion
//!
//! Verifies that fused execution produces identical results to unfused execution.

use blawktrust::builtins::ops::{dlog_column, wmean0};
use blawktrust::pipeline::{ExecutionValue, Executor, OpId, PipeIR, Planner, Step};
use blawktrust::table::ORI_H;
use blawktrust::{Column, Table};

/// Helper to execute a pipeline using the fused executor
fn execute_fused(ir: &PipeIR, input: Table) -> Table {
    let plan = Planner::plan(ir);
    let mut executor = Executor::new();
    let result = executor.execute(&plan, input).expect("Execution failed");

    match result.value {
        ExecutionValue::Table(t) => t,
        _ => panic!("Expected table result"),
    }
}

/// Helper to execute operations manually (baseline)
fn execute_baseline_add(input: Table, c: f64) -> Table {
    let mut new_columns = Vec::new();
    for col in &input.columns {
        let new_col = match col {
            Column::F64(data) => {
                let result: Vec<f64> = data
                    .iter()
                    .map(|&x| if x.is_nan() { f64::NAN } else { x + c })
                    .collect();
                Column::F64(result)
            }
            other => other.clone(),
        };
        new_columns.push(new_col);
    }
    Table::new(input.names.clone(), new_columns)
}

fn execute_baseline_mul(input: Table, c: f64) -> Table {
    let mut new_columns = Vec::new();
    for col in &input.columns {
        let new_col = match col {
            Column::F64(data) => {
                let result: Vec<f64> = data
                    .iter()
                    .map(|&x| if x.is_nan() { f64::NAN } else { x * c })
                    .collect();
                Column::F64(result)
            }
            other => other.clone(),
        };
        new_columns.push(new_col);
    }
    Table::new(input.names.clone(), new_columns)
}

fn execute_baseline_dlog(input: Table, period: usize) -> Table {
    let mut new_columns = Vec::new();
    for col in &input.columns {
        let new_col = match col {
            Column::F64(_) => dlog_column(col, period),
            other => other.clone(),
        };
        new_columns.push(new_col);
    }
    Table::new(input.names.clone(), new_columns)
}

fn execute_baseline_wmean5(input: Table) -> Table {
    let mut new_columns = Vec::new();
    for col in &input.columns {
        let new_col = match col {
            Column::F64(_) => wmean0(col, 5),
            other => other.clone(),
        };
        new_columns.push(new_col);
    }
    Table::new(input.names.clone(), new_columns)
}

/// Helper to compare two tables for equality (allowing NaN == NaN)
fn tables_equal(a: &Table, b: &Table) -> bool {
    if a.columns.len() != b.columns.len() {
        return false;
    }

    for (col_a, col_b) in a.columns.iter().zip(b.columns.iter()) {
        if !columns_equal(col_a, col_b) {
            return false;
        }
    }

    true
}

fn columns_equal(a: &Column, b: &Column) -> bool {
    match (a, b) {
        (Column::F64(data_a), Column::F64(data_b)) => {
            if data_a.len() != data_b.len() {
                return false;
            }
            for (x, y) in data_a.iter().zip(data_b.iter()) {
                if x.is_nan() && y.is_nan() {
                    continue; // Both NaN - OK
                }
                if (x - y).abs() > 1e-10 {
                    return false;
                }
            }
            true
        }
        (Column::Date(a), Column::Date(b)) => a == b,
        (Column::Timestamp(a), Column::Timestamp(b)) => a == b,
        _ => false,
    }
}

#[test]
fn test_equivalence_simple_add() {
    let input = Table::new(
        vec!["a".to_string()],
        vec![Column::F64(vec![1.0, 2.0, 3.0, 4.0, 5.0])],
    );

    // Fused
    let mut ir = PipeIR::new();
    ir.push(Step::OriSet(ORI_H));
    ir.push(Step::Op {
        name: OpId::AddConst,
        args: vec![10.0],
    });
    let fused_result = execute_fused(&ir, input.clone());

    // Baseline
    let baseline_result = execute_baseline_add(input, 10.0);

    assert!(
        tables_equal(&fused_result, &baseline_result),
        "Add const: fused != baseline"
    );
}

#[test]
fn test_equivalence_chain_arithmetic() {
    let input = Table::new(
        vec!["x".to_string()],
        vec![Column::F64(vec![1.0, 2.0, 3.0, 4.0, 5.0])],
    );

    // Fused: x * 2 + 10
    let mut ir = PipeIR::new();
    ir.push(Step::OriSet(ORI_H));
    ir.push(Step::Op {
        name: OpId::MulConst,
        args: vec![2.0],
    });
    ir.push(Step::Op {
        name: OpId::AddConst,
        args: vec![10.0],
    });
    let fused_result = execute_fused(&ir, input.clone());

    // Baseline: x * 2 + 10
    let baseline_result = execute_baseline_add(execute_baseline_mul(input, 2.0), 10.0);

    assert!(
        tables_equal(&fused_result, &baseline_result),
        "Chain arithmetic: fused != baseline"
    );
}

#[test]
fn test_equivalence_dlog() {
    let input = Table::new(
        vec!["prices".to_string()],
        vec![Column::F64(vec![100.0, 110.0, 121.0, 133.1, 146.41])],
    );

    // Fused
    let mut ir = PipeIR::new();
    ir.push(Step::OriSet(ORI_H));
    ir.push(Step::Op {
        name: OpId::Dlog,
        args: vec![1.0],
    });
    let fused_result = execute_fused(&ir, input.clone());

    // Baseline
    let baseline_result = execute_baseline_dlog(input, 1);

    assert!(
        tables_equal(&fused_result, &baseline_result),
        "Dlog: fused != baseline"
    );
}

#[test]
fn test_equivalence_dlog_then_add() {
    let input = Table::new(
        vec!["prices".to_string()],
        vec![Column::F64(vec![100.0, 110.0, 121.0, 133.1, 146.41])],
    );

    // Fused: dlog + 0.5
    let mut ir = PipeIR::new();
    ir.push(Step::OriSet(ORI_H));
    ir.push(Step::Op {
        name: OpId::Dlog,
        args: vec![1.0],
    });
    ir.push(Step::Op {
        name: OpId::AddConst,
        args: vec![0.5],
    });
    let fused_result = execute_fused(&ir, input.clone());

    // Baseline: dlog + 0.5
    let baseline_result = execute_baseline_add(execute_baseline_dlog(input, 1), 0.5);

    assert!(
        tables_equal(&fused_result, &baseline_result),
        "Dlog+add: fused != baseline"
    );
}

#[test]
fn test_equivalence_wmean5() {
    let input = Table::new(
        vec!["data".to_string()],
        vec![Column::F64(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0])],
    );

    // Fused
    let mut ir = PipeIR::new();
    ir.push(Step::OriSet(ORI_H));
    ir.push(Step::Op {
        name: OpId::W5,
        args: vec![],
    });
    let fused_result = execute_fused(&ir, input.clone());

    // Baseline
    let baseline_result = execute_baseline_wmean5(input);

    assert!(
        tables_equal(&fused_result, &baseline_result),
        "WMean5: fused != baseline"
    );
}

#[test]
fn test_equivalence_complex_pipeline() {
    // Complex pipeline: dlog → *2 → +1 → w5
    let input = Table::new(
        vec!["prices".to_string()],
        vec![Column::F64(vec![
            100.0, 105.0, 110.0, 115.0, 120.0, 125.0, 130.0, 135.0,
        ])],
    );

    // Fused
    let mut ir = PipeIR::new();
    ir.push(Step::OriSet(ORI_H));
    ir.push(Step::Op {
        name: OpId::Dlog,
        args: vec![1.0],
    });
    ir.push(Step::Op {
        name: OpId::MulConst,
        args: vec![2.0],
    });
    ir.push(Step::Op {
        name: OpId::AddConst,
        args: vec![1.0],
    });
    ir.push(Step::Op {
        name: OpId::W5,
        args: vec![],
    });
    let fused_result = execute_fused(&ir, input.clone());

    // Baseline
    let baseline_result = execute_baseline_wmean5(execute_baseline_add(
        execute_baseline_mul(execute_baseline_dlog(input, 1), 2.0),
        1.0,
    ));

    assert!(
        tables_equal(&fused_result, &baseline_result),
        "Complex pipeline: fused != baseline"
    );
}

#[test]
fn test_equivalence_multi_column() {
    let input = Table::new(
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
        vec![
            Column::F64(vec![1.0, 2.0, 3.0]),
            Column::F64(vec![10.0, 20.0, 30.0]),
            Column::F64(vec![100.0, 200.0, 300.0]),
        ],
    );

    // Fused: x * 2 + 5
    let mut ir = PipeIR::new();
    ir.push(Step::OriSet(ORI_H));
    ir.push(Step::Op {
        name: OpId::MulConst,
        args: vec![2.0],
    });
    ir.push(Step::Op {
        name: OpId::AddConst,
        args: vec![5.0],
    });
    let fused_result = execute_fused(&ir, input.clone());

    // Baseline
    let baseline_result = execute_baseline_add(execute_baseline_mul(input, 2.0), 5.0);

    assert!(
        tables_equal(&fused_result, &baseline_result),
        "Multi-column: fused != baseline"
    );
}

#[test]
fn test_equivalence_with_nan() {
    let input = Table::new(
        vec!["data".to_string()],
        vec![Column::F64(vec![1.0, f64::NAN, 3.0, 4.0, f64::NAN, 6.0])],
    );

    // Fused: x * 3 + 10
    let mut ir = PipeIR::new();
    ir.push(Step::OriSet(ORI_H));
    ir.push(Step::Op {
        name: OpId::MulConst,
        args: vec![3.0],
    });
    ir.push(Step::Op {
        name: OpId::AddConst,
        args: vec![10.0],
    });
    let fused_result = execute_fused(&ir, input.clone());

    // Baseline
    let baseline_result = execute_baseline_add(execute_baseline_mul(input, 3.0), 10.0);

    assert!(
        tables_equal(&fused_result, &baseline_result),
        "With NaN: fused != baseline"
    );
}

#[test]
fn test_equivalence_large_table() {
    // Test with larger data
    let data: Vec<f64> = (0..1000).map(|i| (i as f64) * 0.1).collect();
    let input = Table::new(vec!["seq".to_string()], vec![Column::F64(data)]);

    // Fused: dlog → *100
    let mut ir = PipeIR::new();
    ir.push(Step::OriSet(ORI_H));
    ir.push(Step::Op {
        name: OpId::Dlog,
        args: vec![1.0],
    });
    ir.push(Step::Op {
        name: OpId::MulConst,
        args: vec![100.0],
    });
    let fused_result = execute_fused(&ir, input.clone());

    // Baseline
    let baseline_result = execute_baseline_mul(execute_baseline_dlog(input, 1), 100.0);

    assert!(
        tables_equal(&fused_result, &baseline_result),
        "Large table: fused != baseline"
    );
}
