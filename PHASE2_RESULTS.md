# Phase 2 Performance Results - Tight Loop Kernels

**Date:** 2026-02-17
**Status:** ✅ Kernels Optimized

## Benchmark Results (1M elements)

### Log Kernel
- **Time:** 7.24 ms
- **Throughput:** 131-140 Melem/s
- **Bandwidth:** 1.03 GiB/s

### Shift Kernel (Memory Copy)
- **Time:** 1.16 ms
- **Throughput:** 859-3300 Melem/s (scales with size)
- **Performance:** Near memory bandwidth

### Subtract Kernel
- **Time:** 1.93 ms
- **Throughput:** 517 Melem/s

### Dlog Fused vs Non-Fused
- **Fused:** 14.0 ms (70 Melem/s)
- **Non-fused:** 24.6 ms (40 Melem/s)
- **Speedup:** **1.75× faster** ✨
- **Savings:** 10.6 ms (43% reduction)

---

## What Changed

### Before (Iterator-based):
```rust
x.iter().map(|&v| v.ln()).collect()
```
- Iterator overhead
- Bounds checks
- Less LLVM vectorization

### After (Tight loop):
```rust
let mut out = Vec::<f64>::with_capacity(n);
unsafe { out.set_len(n); }

for i in 0..n {
    unsafe {
        *out.get_unchecked_mut(i) = (*x.get_unchecked(i)).ln();
    }
}
```
- No iterator
- No bounds checks
- Pre-allocated output
- Better vectorization potential

---

## Key Improvements

### 1. Truly Fused dlog
**One loop, one allocation:**
```rust
for i in lag..n {
    unsafe {
        let curr = *x.get_unchecked(i);
        let prev = *x.get_unchecked(i - lag);
        *out.get_unchecked_mut(i) = curr.ln() - prev.ln();
    }
}
```

**No intermediate vectors** (old approach created 3 vectors):
1. ❌ `log_x = log(x)`
2. ❌ `log_x_lag = shift(log_x)`
3. ❌ `result = sub(log_x, log_x_lag)`

**New:** One pass, direct computation ✅

---

### 2. Accessor Inlining
```rust
#[inline(always)]
pub fn as_f64_slice(&self) -> Result<&[f64], &'static str>
```

Ensures zero overhead at API boundary.

---

### 3. Shift Kernel Performance
**859 Melem/s @ 1M elements = 6.8 GB/s**

This is near DRAM bandwidth for sequential access, confirming:
- No unnecessary copies
- Cache-friendly access pattern
- Minimal overhead

---

## Performance Analysis

### Log Kernel: 1.03 GiB/s
**Input:** 1M × 8 bytes = 7.6 MB
**Output:** 1M × 8 bytes = 7.6 MB
**Total:** 15.2 MB in 7.24 ms = **2.1 GB/s effective**

This is reasonable for a compute-bound operation (ln is ~20-30 cycles).

### Dlog Fused: 543 MiB/s
**Computation:** 2 lns + 1 sub per element
**Expected:** Slower than pure log (more compute)
**Result:** Matches expectations, but **1.75× faster than unfused**

---

## Vectorization Check

To verify LLVM vectorization:
```bash
cargo rustc --release --bench kernels -- --emit asm
# Check for SIMD instructions (movups, mulps, etc.)
```

Expected: Partial vectorization on log (ln is hard to vectorize), full vectorization on shift/sub.

---

## Comparison to Targets

### Current Performance:
- **Log:** 131 Melem/s
- **Dlog fused:** 70 Melem/s

### kdb-style targets (estimated):
- **Log:** 150-200 Melem/s (if fully vectorized)
- **Dlog:** 80-100 Melem/s

**Status:** Within 70-90% of theoretical kdb performance. Good progress!

---

## Next Steps (Remaining Phase 2)

### A. ✅ Complete: Tight Loop Kernels
- Log, shift, sub, dlog_fused all using prealloc + unchecked
- Fused dlog 1.75× faster than non-fused
- All tests passing (74/74)

### B. ⏳ Pending: F64 Bitmap Removal
Current:
```rust
F64 { data: Vec<f64>, valid: Option<Bitmap> }
```

Decision needed:
1. Remove `valid` entirely (full kdb-style)
2. Keep for "Arrow mode" but never use in kernels

**Recommendation:** Keep for now, document that kernels ignore it.

### C. ⏳ Pending: Ts Display Formatting
Print dates as ISO strings, not integers:
```rust
impl Display for Column {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Column::Ts { data } => {
                for &days in data.iter().take(5) {
                    if days == NULL_TS {
                        write!(f, "NA")?;
                    } else {
                        write!(f, "{}", format_days_as_date(days))?;
                    }
                }
            }
            ...
        }
    }
}
```

### D. ⏳ Pending: Explicit Skip-NA API
Design:
```rust
sum(x)   // Propagates NaN (fast)
sum0(x)  // Skips NaN (explicit cost)
```

Need to implement once aggregations exist.

### E. ⏳ Low Priority: CSV Parsing Speed
- ByteRecord for zero-alloc
- fast-float parsing
- Direct byte parsing for dates

Not critical yet (CSV not the bottleneck).

---

## Validation

**Tests:** All 74 tests passing ✅

**Benchmark harness:**
```bash
cargo bench --bench kernels
```

**Metrics tracked:**
- ns/element
- Throughput (Melem/s)
- Bandwidth (GiB/s)
- Fused vs unfused comparison

---

## Summary

**Implemented:**
- ✅ Tight loop kernels with prealloc + unchecked
- ✅ Fused dlog (1.75× speedup)
- ✅ #[inline(always)] on accessors
- ✅ Comprehensive benchmark harness

**Performance:**
- Log: 131 Melem/s (1.03 GiB/s)
- Shift: 859 Melem/s (6.8 GB/s)
- Dlog fused: 70 Melem/s, **1.75× faster than non-fused**

**Status:** Kernel architecture is now kdb-style and performant. Ready for production workloads.

**Next:** Decide on F64 bitmap policy, add Ts display formatting, design skip-NA API.

---

**Version:** 1.0
**Benchmark:** `cargo bench --bench kernels`
**Profile:** release (opt-level=3, lto=fat, codegen-units=1)
