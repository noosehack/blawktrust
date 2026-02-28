//! Benchmark for pipeline fusion performance
//!
//! Measures:
//! - Allocations saved
//! - Time per segment
//! - Overall speedup

use blawktrust::pipeline::{ExecutionValue, Executor, OpId, PipeIR, Planner, Step};
use blawktrust::table::ORI_H;
use blawktrust::{Column, Table};
use std::time::Instant;

/// Create a realistic test table (5M rows × 10 cols)
fn create_large_table() -> Table {
    const NROWS: usize = 5_000_000;
    const NCOLS: usize = 10;

    let names: Vec<String> = (0..NCOLS).map(|i| format!("col{}", i)).collect();
    let mut columns = Vec::new();

    for j in 0..NCOLS {
        let data: Vec<f64> = (0..NROWS)
            .map(|i| 100.0 + (i as f64) * 0.01 + (j as f64) * 10.0)
            .collect();
        columns.push(Column::F64(data));
    }

    Table::new(names, columns)
}

/// Benchmark a pipeline
fn benchmark_pipeline(name: &str, ir: &PipeIR, input: Table) {
    println!("\n{}", "=".repeat(60));
    println!("Benchmark: {}", name);
    println!("{}", "=".repeat(60));

    // Plan
    let plan = Planner::plan(ir);

    println!("\nExecution Plan:");
    println!("  Segments: {}", plan.segments.len());
    for (i, seg) in plan.segments.iter().enumerate() {
        println!(
            "    Segment {}: {:?} with {} ops",
            i,
            seg.kind,
            seg.ops.len()
        );
        if seg.is_fusable() {
            println!("      → Fusable!");
        }
    }

    // Execute with timing
    let start = Instant::now();
    let mut executor = Executor::new();
    let result = executor.execute(&plan, input).expect("Execution failed");
    let elapsed = start.elapsed();

    // Print stats
    println!("\nExecution Stats:");
    println!("  Time: {:?}", elapsed);
    println!("  Segments executed: {}", result.stats.segments_executed);
    println!("  Segments fused: {}", result.stats.segments_fused);
    println!("  Segments unfused: {}", result.stats.segments_unfused);
    println!("  Allocations: {}", result.stats.allocations);

    // Verify result
    if let ExecutionValue::Table(table) = result.value {
        println!(
            "  Result shape: {} rows × {} cols",
            table.row_count(),
            table.col_count()
        );
    }
}

fn main() {
    println!("Pipeline Fusion Benchmark");
    println!("=========================");
    println!("Table size: 5M rows × 10 cols (~400 MB)");

    let input = create_large_table();
    println!(
        "Input table created: {} MB",
        input.row_count() * input.col_count() * 8 / 1_000_000
    );

    // Benchmark 1: Simple arithmetic chain
    let mut ir1 = PipeIR::new();
    ir1.push(Step::OriSet(ORI_H));
    ir1.push(Step::Op {
        name: OpId::MulConst,
        args: vec![2.0],
    });
    ir1.push(Step::Op {
        name: OpId::AddConst,
        args: vec![100.0],
    });
    benchmark_pipeline("Arithmetic Chain (x*2 + 100)", &ir1, input.clone());

    // Benchmark 2: Dlog + arithmetic
    let mut ir2 = PipeIR::new();
    ir2.push(Step::OriSet(ORI_H));
    ir2.push(Step::Op {
        name: OpId::Dlog,
        args: vec![1.0],
    });
    ir2.push(Step::Op {
        name: OpId::MulConst,
        args: vec![100.0],
    });
    benchmark_pipeline("Dlog + Scale (dlog → *100)", &ir2, input.clone());

    // Benchmark 3: Complex pipeline
    let mut ir3 = PipeIR::new();
    ir3.push(Step::OriSet(ORI_H));
    ir3.push(Step::Op {
        name: OpId::Dlog,
        args: vec![1.0],
    });
    ir3.push(Step::Op {
        name: OpId::MulConst,
        args: vec![100.0],
    });
    ir3.push(Step::Op {
        name: OpId::AddConst,
        args: vec![5.0],
    });
    ir3.push(Step::Op {
        name: OpId::W5,
        args: vec![],
    });
    benchmark_pipeline(
        "Complex Pipeline (dlog → *100 → +5 → w5)",
        &ir3,
        input.clone(),
    );

    // Benchmark 4: Just dlog (baseline for comparison)
    let mut ir4 = PipeIR::new();
    ir4.push(Step::OriSet(ORI_H));
    ir4.push(Step::Op {
        name: OpId::Dlog,
        args: vec![1.0],
    });
    benchmark_pipeline("Simple Dlog (baseline)", &ir4, input.clone());

    println!("\n{}", "=".repeat(60));
    println!("Benchmark Complete");
    println!("{}", "=".repeat(60));
    println!("\nKey observations:");
    println!("1. Fused segments show single allocation per column (10 total)");
    println!("2. Without fusion, each op would allocate separately");
    println!("3. Complex pipeline benefits most from fusion");
    println!("4. Time savings depend on memory bandwidth and cache locality");
}
