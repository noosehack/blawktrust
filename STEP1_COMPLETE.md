# Step 1 Complete: Stop Writing Sentinel NA in Kernels

**Status:** ✅ Complete
**Date:** 2026-02-17

## What Changed

### Before (Old Contract)
```rust
if invalid {
    out[i] = f64::NAN;      // ← EXTRA WORK!
    out_valid.set(i, false);
}
```

### After (New Contract - kdb/Arrow-ish)
```rust
if invalid {
    // Just set validity bit, don't touch data
    out_valid.set(i, false);
    // out[i] is "don't care" - can be uninitialized
}
```

## Files Modified

1. **src/builtins/kernels_masked.rs**
   - `dlog_masked()`: Removed `*op.add(i) = f64::NAN;` in invalid path
   - `unary_masked()`: Removed `out[i] = f64::NAN;` in invalid path
   - `binary_masked()`: Removed `out[i] = f64::NAN;` in invalid path
   - Also removed `out[..lag].fill(f64::NAN)` in prefix handling

2. **src/builtins/nulls.rs**
   - Added `materialize_sentinel()` for legacy compatibility
   - Converts bitmap → sentinel (inverse of `sentinel_to_bitmap`)
   - Use case: Exporting to legacy systems

3. **src/lib.rs**
   - Exported null handling functions at crate root

## Tests

- **All 21 tests passing** ✅
- Added 4 new tests for `materialize_sentinel()`
- Verified correctness of "don't care" data contract

## Performance Impact

**Benchmark:** 1M elements, 10% nulls, 50 iterations
- Time: ~17.3 ms/iter (masked path)
- Benefit: Fewer memory writes in invalid path
- Expected: Small improvement in masked operations

## Why This Matters

### 1. Fewer Memory Writes
Invalid path now only sets 1 bit instead of writing 8 bytes (f64)

### 2. Cleaner Separation
Validity is **only** in bitmap, not encoded in data

### 3. Enables Downstream Optimizations
- Pipelines check validity masks only (not data)
- Can use `MaybeUninit` buffers (future)
- Word-wise validity checks (Step 5)
- Scratch allocator optimizations (Step 2)

### 4. kdb/Arrow Philosophy
```
Invalidness = validity bitmap only
Data at invalid indices = unspecified (implementation detail)
Only materialize sentinels at boundaries (I/O, legacy APIs)
```

## API

### For Kernels (Internal)
```rust
// Fast path: No nulls
pub fn dlog_no_nulls(out: &mut [f64], x: &[f64], lag: usize)

// Masked path: Check validity, don't write on invalid
pub fn dlog_masked(
    out: &mut [f64],        // DON'T CARE at invalid indices!
    out_valid: &mut Bitmap, // Truth lives here
    x: &[f64],
    x_valid: &Bitmap,
    lag: usize,
)
```

### For Compatibility (Boundary)
```rust
// Convert bitmap → sentinel (for legacy export)
pub fn materialize_sentinel(col: &mut Column, na: f64)

// Convert sentinel → bitmap (for legacy import)
pub fn sentinel_to_bitmap_inplace(col: &mut Column, na: f64)
```

## Example Usage

```rust
use blawk_kdb::{Column, Bitmap, materialize_sentinel};

// Create column with validity bitmap
let data = vec![100.0, 200.0, 300.0];
let mut bitmap = Bitmap::new_all_valid(3);
bitmap.set(1, false);  // Mark index 1 as invalid

let mut col = Column::F64 {
    data,
    valid: Some(bitmap),
};

// Kernels don't write sentinel to data[1]
// (data[1] is "don't care" - could be anything!)

// Only materialize sentinels when exporting to legacy
materialize_sentinel(&mut col, -99999.0);
// Now data[1] = -99999.0 (for legacy compatibility)
```

## Verification

Run the demo:
```bash
cargo run --example step1_no_sentinel_writes --release
```

Expected output:
- Time: ~17 ms/iter (1M elements, 10% nulls)
- All 21 tests passing
- Verification of materialize_sentinel() roundtrip

## Next Steps

**Ready for Step 2:** Scratch allocator + "into" kernels

Step 2 will:
- Eliminate allocation churn in pipelines
- Rewrite ops to: `fn dlog_into(out: &mut Column, x: &Column, lag: usize, scratch: &mut Scratch)`
- Goal: ~0 allocations after warmup in multi-op pipelines

This is where we get the "3× in pipelines" reliably.

---

**Step 1 Complete:** Kernel contract standardized on validity-only null representation ✅
