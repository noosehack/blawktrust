# Orientation System Contract

**Version**: 1.0
**Status**: FROZEN — These invariants are immutable

## Physical Invariants (Guaranteed)

### 1. Columnar Storage is Immutable
```rust
// Physical storage is ALWAYS: Vec<Column>
// where Column = F64(Vec<f64>) | Date(Vec<i32>) | Timestamp(Vec<i64>)
pub struct Table {
    pub names: Vec<String>,
    pub columns: Vec<Column>,
}
```

**Law**: No operation ever changes physical layout. Storage is columnar forever.

### 2. Orientation Never Reallocates
```rust
pub fn with_orientation(&self, new_ori: Ori) -> Self {
    TableView {
        table: Arc::clone(&self.table),  // O(1): only clone Arc pointer
        ori: new_ori,                     // O(1): Copy a small enum
    }
}
```

**Law**: Orientation changes are pure functions that create new views. No data is ever copied, moved, or reallocated.

### 3. O(1) Orientation Changes
```rust
let view_h = TableView::new(table);
let view_z = view_h.with_orientation(ORI_Z);  // O(1)
let view_r = view_z.with_orientation(ORI_R);  // O(1)

assert!(view_h.shares_table_with(&view_z));
assert!(view_z.shares_table_with(&view_r));
```

**Law**: All views share the same `Arc<Table>`. Memory cost is O(1) per view.

## Semantic Invariants (The Law)

### Orientation Dispatch Table

This table defines **ALL** operator behavior. If an operator is not listed here, it must follow these rules or panic.

| Orientation Class | Map Operations<br>(dlog, diff, shift, w5...) | Reduce Operations<br>(sum, mean, max...) | Output Shape |
|-------------------|-----------------------------------------------|------------------------------------------|--------------|
| **ColwiseLike**<br>(H, N, _N, _H) | Apply down each **column**<br>Vector = column (contiguous) | Reduce down each **column**<br>One result per column | **ncols** |
| **RowwiseLike**<br>(Z, S, _Z, _S) | Apply across each **row**<br>Vector = row (tiled) | Reduce across each **row**<br>One result per row | **nrows** |
| **Real**<br>(R) | **Invalid** — PANIC<br>No sequence structure | Reduce **all** elements<br>Single scalar output | **1** (scalar) |
| **Each**<br>(X) | Elementwise **only**<br>No sequence dependency | **Invalid** — PANIC<br>No aggregation structure | **nrows × ncols** |

### Detailed Rules

#### Map Operations (Transforms)
Operations that produce same-shape output: `dlog`, `diff`, `shift`, `w5`, `wzscore`, `locf`, etc.

- **ColwiseLike**: Vector = column → apply kernel down each column independently
  - Memory access: **contiguous** (fast)
  - Shape: `(nrows, ncols)` → `(nrows, ncols)`

- **RowwiseLike**: Vector = row → apply kernel across each row independently
  - Memory access: **strided** (use tiling)
  - Shape: `(nrows, ncols)` → `(nrows, ncols)`

- **Real**: PANIC — no sequence structure

- **Each**: Only for elementwise ops (no window/lag dependencies)

#### Reduce Operations (Aggregations)
Operations that collapse a dimension: `sum`, `mean`, `max`, `min`, `std`, etc.

- **ColwiseLike**: Reduce down each column → `ncols` outputs
  - Result: `Column::F64(vec![v1, v2, ..., v_ncols])`

- **RowwiseLike**: Reduce across each row → `nrows` outputs
  - Result: `Column::F64(vec![r1, r2, ..., r_nrows])`

- **Real**: Reduce all → single scalar
  - Result: `Column::F64(vec![scalar])`

- **Each**: PANIC — no aggregation structure

#### Elementwise Operations (Arithmetic)
Operations on two tables: `+`, `-`, `*`, `/`, etc.

- **All orientations**: Apply element-by-element
  - Shape must match after accounting for orientation
  - Broadcast rules apply

### The 10 Canonical Orientations

#### ColwiseLike (4 orientations)
| Name | Compass | swap | flip_i | flip_j | Description |
|------|---------|------|--------|--------|-------------|
| **H** | NSWE | false | false | false | Normal (identity) |
| **N** | SNWE | false | true | false | Rows reversed (South→North) |
| **_N** | NSEW | false | false | true | Columns reversed |
| **_H** | SNEW | false | true | true | Both reversed |

#### RowwiseLike (4 orientations)
| Name | Compass | swap | flip_i | flip_j | Description |
|------|---------|------|--------|--------|-------------|
| **Z** | WENS | true | false | false | Transposed (row-major) |
| **S** | EWNS | true | false | false | Synonym for Z |
| **_Z** | EWSN | true | true | false | Transposed + rows reversed |
| **_S** | WESN | true | false | true | Transposed + columns reversed |

#### Special (2 orientations)
| Name | Compass | Mode | Description |
|------|---------|------|-------------|
| **X** | X | Each | Elementwise (no vector structure) |
| **R** | R | Real | Scalar reduction (collapse all) |

### Index Mapping Contract

```rust
pub fn map_ij(self, nr: usize, nc: usize, i: usize, j: usize) -> (usize, usize)
```

**For D4 orientations**:
1. If `swap == true`: transpose first → `(j, i)`
2. If `flip_i == true`: reverse row index → `(nr - 1 - i, j)`
3. If `flip_j == true`: reverse col index → `(i, nc - 1 - j)`

**For Each/Real**: Mapping is identity (not used by operators)

**Guarantee**: This mapping is used **only** at operator dispatch boundaries, never inside tight loops.

### Performance Contract

#### Fast Paths (Must Remain Fast)
- **ColwiseLike map operations**: Pure contiguous memory loops
  - NO `map_ij()` calls inside inner loops
  - Direct access: `column[i]`

- **ColwiseLike reduce operations**: SIMD-friendly
  - Vector reduction with no index translation

#### Tiled Paths (Optimized for Cache)
- **RowwiseLike operations**: 128-row tiles
  - Minimizes cache misses on wide tables
  - Still no `map_ij()` in inner loops

#### Forbidden Slow Paths
- ❌ Calling `map_ij()` per element in a tight loop
- ❌ Materializing intermediate reordered data
- ❌ Copying data on orientation change

## Operator Implementation Rules

### Rule 1: Check Orientation Class, Not Individual Orientations
```rust
// ✅ CORRECT
pub fn sum(view: &TableView) -> Column {
    match view.ori_class() {
        OriClass::ColwiseLike => sum_colwise(&view.table),
        OriClass::RowwiseLike => sum_rowwise_tiled(&view.table),
        OriClass::Real => sum_scalar(&view.table),
        OriClass::Each => panic!("sum not defined for Each"),
    }
}

// ❌ WRONG — Don't check individual orientations
match view.ori {
    ORI_H => ...,
    ORI_Z => ...,
    ORI_N => ...,
    // ... 10 branches!
}
```

### Rule 2: Map Operations Preserve Shape
```rust
pub fn dlog(view: &TableView) -> Table {
    let result = match view.ori_class() {
        OriClass::ColwiseLike => dlog_colwise(&view.table),
        OriClass::RowwiseLike => dlog_rowwise(&view.table),
        _ => panic!("dlog requires sequence structure"),
    };

    // Output Table has SAME physical shape as input
    assert_eq!(result.row_count(), view.table.row_count());
    assert_eq!(result.col_count(), view.table.col_count());
    result
}
```

### Rule 3: Reduce Operations Change Shape
```rust
pub fn sum(view: &TableView) -> Column {
    match view.ori_class() {
        OriClass::ColwiseLike => {
            // Output: ncols values
            Column::F64(vec![...]) // len = ncols
        }
        OriClass::RowwiseLike => {
            // Output: nrows values
            Column::F64(vec![...]) // len = nrows
        }
        OriClass::Real => {
            // Output: 1 scalar
            Column::F64(vec![scalar])
        }
        _ => panic!(),
    }
}
```

### Rule 4: Temporal Columns Are Non-Numeric
- **Map operations**: Preserve Date/Timestamp columns unchanged
- **Reduce operations**: Produce NaN for Date/Timestamp columns
- **Arithmetic**: Panic on Date/Timestamp operands

## Testing Contract

Every new operator must pass these tests:

1. **ColwiseLike correctness** (test with H orientation)
2. **RowwiseLike correctness** (test with Z orientation)
3. **Real correctness** (test with R orientation if applicable)
4. **Each correctness or panic** (test with X orientation)
5. **NaN handling** (verify NaN propagation rules)
6. **Empty table** (verify edge case handling)
7. **Temporal columns** (verify preservation or filtering)
8. **Large table** (test >128 rows for tiling)
9. **Share table verification** (ensure no data copying)

Minimum: **9 tests per operator** (some may panic, that's valid)

## Future Extension Points (Not Yet Implemented)

### Window Direction
Some operators may need to know scan direction:
- `w5` (rolling window)
- `cs1` (cumulative sum)
- `ur` (unique rows)

**Potential extension**:
```rust
pub struct OriSpec {
    pub name: &'static str,
    pub compass: &'static str,
    pub ori: Ori,
    pub class: OriClass,
    pub scan_reverse: bool,  // ← NEW: does orientation flip direction?
}
```

**Rule**: Do NOT implement reversal by copying data. Iterate in reverse if needed.

### Physical Materialization
```rust
pub fn pack(view: &TableView, target_ori: Ori) -> Table {
    // Physically re-layout data to target orientation
    // Returns new Table (not a view)
    // Only use if profiling shows need
}
```

**Rule**: Keep `(o H)` as view creation. Add separate `(pack H)` for materialization.

## Drift Prevention

This document is **THE LAW**. Any changes require:
1. Updated version number
2. Clear migration path
3. Verification that all existing operators still conform

**No exceptions.**

## Summary

The orientation system guarantees:
- ✅ Physical storage never changes
- ✅ Orientation changes are O(1)
- ✅ Operators dispatch by class, not by individual orientation
- ✅ Semantic behavior is deterministic and documented
- ✅ Fast paths remain fast (no hidden overhead)

If all operators follow this contract, the system is **kdb-level stable**.
