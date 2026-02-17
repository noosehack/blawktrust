# blawk_kdb Optimization Roadmap

**Project:** kdb+ style optimization for blawk
**Goal:** Achieve 3-5× speedup in pipelines

---

## ✅ Step 1: Stop Writing Sentinel NA in Kernels (COMPLETE)

**Status:** Done ✅
**Date:** 2026-02-17

### What Changed
- Masked kernels no longer write NA to `out[i]` when invalid
- Only set `validity bit = 0` (fewer memory writes)
- Added `materialize_sentinel()` for legacy compatibility

### Results
- Cleaner kernel contract (validity-only)
- Enables downstream optimizations
- Foundation for word-wise validity checks

### Details
See: [STEP1_COMPLETE.md](STEP1_COMPLETE.md)

---

## ✅ Step 2: Scratch Allocator + "Into" Kernels (COMPLETE)

**Status:** Done ✅
**Date:** 2026-02-17

### What Changed
- Implemented `Scratch` buffer pool (reusable f64 + bitmap buffers)
- Added `*_into()` non-allocating kernels (e.g., `dlog_into`)
- Kept old `*_column()` APIs for backward compatibility

### Results
**Single operation:**
- OLD: 781 KB/iter
- NEW: 0 KB/iter (800000× less!)

**3-op pipeline:**
- Time: 2.68 ms/iter
- Allocated: 0 KB/iter (after warmup)

### Key Insight
Zero allocation churn = better cache locality + predictable performance

### Details
See: [STEP2_COMPLETE.md](STEP2_COMPLETE.md)

---

## ✅ Step 3: Micro-Fusion at Kernel Layer (COMPLETE)

**Status:** Done ✅
**Date:** 2026-02-17
**Actual Gain:** 1.04-1.13× on ln-dominated pattern

### What Changed
Implemented 3 fused kernels (fast + masked paths):
- `dlog_scale_add`: `out = a * dlog(x, lag) + b`
- `ln_scale_add`: `out = a * ln(x) + b`
- `sub_mul_add`: `out = (x - y) * a + b`

### Results
**Pattern:** `dlog(x, lag) * a + b`

| Size | Pipeline | Fused | Speedup |
|------|----------|-------|---------|
| 10K | 0.19 ms | 0.18 ms | 1.04× |
| 100K | 1.61 ms | 1.54 ms | 1.04× |
| 1M | 19.19 ms | 17.04 ms | **1.13×** |

### Key Insights
- Modest speedup (4-13%) because ln() dominates (95% of time)
- 50% memory bandwidth reduction (2 passes → 1 pass)
- Larger gains expected on lighter ops or longer chains
- Foundation for compound fusion benefits

### Details
See: [STEP3_COMPLETE.md](STEP3_COMPLETE.md)

---

## 🔮 Step 4: Attack ln() Throughput (LATER)

**Status:** Not started
**Priority:** Medium (only after Steps 1-3)
**Expected Gain:** Depends on approach

### Options (Ranked)

**A) Ensure fastest native path:**
- Build with `-C target-cpu=native`
- Verify using `perf`/`flamegraph`

**B) Vector math backend (SLEEF/SVML):**
- Batched ln() operations
- SIMD vector instructions
- Potential 2-4× speedup on ln()

**C) Approximate ln (if acceptable):**
- Fast approximations for trading signals
- Not for accounting/compliance

### Why Later
Current benchmarks show ln() is 95% of cost, but:
- Steps 1-3 reduce number of ln() calls (fusion)
- Need to measure again after fusion

---

## 🔮 Step 5: Word-Wise Masked Execution (LATER)

**Status:** Not started
**Priority:** Low
**Expected Gain:** 10-20% in masked operations

### Plan
Process validity bitmap 64 bits at a time:
- If word is all-valid (0xFFFFFFFFFFFFFFFF), run tight loop (no checks)
- If word is all-null (0x0000000000000000), skip compute entirely
- Otherwise, fall back to per-bit checks

### Why Later
- Benefit only shows when nulls are clustered
- More complex code
- Optimize common case first (Steps 1-3)

---

## 📊 Progress Summary

| Step | Status | Key Metric | Gain |
|------|--------|------------|------|
| Step 1: Stop sentinel writes | ✅ Complete | Cleaner contract | Foundation |
| Step 2: Scratch allocator | ✅ Complete | 0 KB/iter | 800000× less alloc |
| Step 3: Micro-fusion | ✅ Complete | 17.04 ms/iter (1M) | 1.04-1.13× |
| Step 4: ln() throughput | 🔮 Next | TBD | Expected 2-4× |
| Step 5: Word-wise validity | 🔮 Later | TBD | Expected 1.1-1.2× |

---

## 🎯 Current State

**Completed:**
- ✅ Bitmap validity (Phase 1)
- ✅ Stop sentinel writes (Step 1)
- ✅ Zero-alloc pipelines (Step 2)
- ✅ Micro-fusion (Step 3)

**Tests:** 32/32 passing ✅

**Ready for:** Step 4 (ln() throughput) or Step 5 (word-wise validity)

---

## 🚀 Next Action

**Two options:**

### Option A: Step 4 - Attack ln() Throughput (RECOMMENDED)
Since ln() dominates 95% of cost:
1. Build with `-C target-cpu=native`
2. Profile with `cargo flamegraph`
3. Investigate SLEEF/SVML vector math
4. Measure single-op speedup

Expected: 2-4× speedup on ln()-heavy operations

### Option B: Step 5 - Word-Wise Validity
Process 64 bits at once:
1. Check if word is all-valid (0xFFFF...)
2. Tight loop with no checks
3. Skip all-null words

Expected: 10-20% speedup in masked operations

**Recommendation:** Profile first, then attack ln() if it's the clear bottleneck.

---

**Last Updated:** 2026-02-17
**Tests Passing:** 27/27 ✅
**Current Focus:** Step 3 (micro-fusion)
