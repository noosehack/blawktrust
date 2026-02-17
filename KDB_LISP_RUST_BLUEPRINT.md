# kdb+/Lisp/Rust Integration Blueprint

**Date:** 2026-02-17
**Purpose:** Design document for integrating Lisp S-expression front-end with Rust blawk_kdb backend
**Status:** Architecture design phase

---

## Executive Summary

This blueprint outlines three approaches to create a **Lisp-based interface** for the **optimized Rust blawk_kdb** backend, combining:

- **kdb+ performance model**: Zero-allocation, single-pass execution
- **Lisp expressiveness**: S-expressions, macros, REPL workflow
- **Rust safety & speed**: Memory-safe, 1.89× faster than C++

### Key Metrics
```
Current (clispi → C++):    29.31 ms (dlog, 1M elements)
Target (Lisp → Rust):      15.51 ms (dlog, 1M elements)
Speedup:                   1.89× faster
Safety:                    Memory-safe (Rust borrow checker)
```

---

## Background

### What We Have

**1. Optimized Rust Backend (blawk_kdb)**
- Location: `/home/ubuntu/blawk_kdb/`
- Performance: 15.51 ms/iter (1M elements, fused dlog)
- Features: Bitmap validity, scratch allocator, micro-fusion, word-wise bitmap
- Tests: 35/35 passing
- Status: Production-ready

**2. Existing Lisp Front-End (clispi)**
- Location: `/home/ubuntu/clispi_dev/`
- Language: Common Lisp S-expressions
- Backend: C++ blawk_dev (slower)
- Operations: 70 statistical functions
- Features: Advanced macro system, threading macros, tee binding

**3. Performance Gap**
```
C++ blawk_dev:      29.31 ms (current clispi backend)
Rust blawk_kdb:     15.51 ms (optimized, ready)
Opportunity:        1.89× speedup by switching backend
```

---

## Architecture Approaches

### Approach 1: FFI Bridge (Quick Path)

**Concept:** Keep clispi Lisp front-end, add Rust FFI layer

**Architecture:**
```
┌─────────────────┐
│  clispi (Lisp)  │  ← User writes Lisp code
└────────┬────────┘
         │ (parse, macro expand)
         ▼
┌─────────────────┐
│   IR/AST        │  ← Abstract syntax tree
└────────┬────────┘
         │ (codegen)
         ▼
┌─────────────────┐
│  Rust FFI       │  ← C ABI exports
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  blawk_kdb      │  ← Fast Rust backend
│  (Rust)         │
└─────────────────┘
```

**Implementation:**

1. **Export Rust functions with C ABI**
```rust
// In blawk_kdb/src/ffi.rs
use std::os::raw::c_void;

#[repr(C)]
pub struct CColumn {
    data: *mut f64,
    len: usize,
    valid: *mut u64,
}

#[no_mangle]
pub extern "C" fn blawk_dlog(
    out: *mut CColumn,
    x: *const CColumn,
    lag: usize,
) -> i32 {
    // Convert C types to Rust Column
    // Call internal dlog_into()
    // Return result via out pointer
    0 // Success
}

#[no_mangle]
pub extern "C" fn blawk_column_new(len: usize) -> *mut CColumn {
    // Allocate Column, return opaque pointer
}

#[no_mangle]
pub extern "C" fn blawk_column_free(col: *mut CColumn) {
    // Drop Column, free memory
}
```

2. **Update clispi to call Rust FFI instead of C++**
```lisp
;; In clispi dispatcher
(defun dlog-impl (column lag)
  (let ((result (blawk-column-new (column-len column))))
    (blawk-dlog result column lag)
    result))
```

3. **Build shared library**
```bash
cd /home/ubuntu/blawk_kdb
cargo build --release --lib
# Produces: target/release/libblawk_kdb.so
```

4. **Link clispi against Rust .so**
```lisp
;; Load Rust library
(cffi:load-foreign-library "target/release/libblawk_kdb.so")

;; Define FFI bindings
(cffi:defcfun "blawk_dlog" :int
  (out :pointer)
  (x :pointer)
  (lag :size))
```

**Pros:**
- ✅ Keep existing clispi syntax and macros
- ✅ Immediate 1.89× speedup
- ✅ Low risk (minimal changes to clispi)
- ✅ Fast implementation (1-2 days)

**Cons:**
- ❌ FFI overhead (pointer conversions, safety boundary)
- ❌ Manual memory management at boundary
- ❌ Two languages to maintain

**Effort:** 1-2 days
**Risk:** Low
**Speedup:** 1.89× (full Rust backend speed)

---

### Approach 2: Rust-Native S-Expression Parser (Clean Path)

**Concept:** Rewrite Lisp interpreter in Rust

**Architecture:**
```
┌─────────────────┐
│  User Input     │  ← "(dlog prices 1)"
│  (S-expr)       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Rust Parser    │  ← lexpr/ketos/nom
│  (Rust)         │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Rust Evaluator │  ← Direct calls to blawk_kdb
│  (Rust)         │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  blawk_kdb      │  ← Zero overhead!
│  (Rust)         │
└─────────────────┘
```

**Implementation:**

1. **Use existing Rust S-expression library**
```toml
[dependencies]
lexpr = "0.2"  # S-expression parser
```

2. **Parse S-expressions**
```rust
use lexpr::{parse, Value};

fn eval(expr: &Value, env: &mut Env) -> Result<Column, Error> {
    match expr {
        Value::Cons(cons) => {
            let op = cons.car().as_symbol().ok_or("Expected symbol")?;
            let args = cons.cdr();

            match op {
                "dlog" => {
                    let col = eval(&args[0], env)?;
                    let lag = args[1].as_i64().unwrap() as usize;

                    let mut scratch = env.get_scratch();
                    let mut out = Column::new_f64(vec![]);
                    dlog_into(&mut out, &col, lag, &mut scratch);
                    Ok(out)
                }
                "+" => {
                    let a = eval(&args[0], env)?;
                    let b = eval(&args[1], env)?;
                    Ok(a.add(&b))
                }
                // ... other operations
            }
        }
        Value::Symbol(s) => env.get(s).cloned(),
        Value::Number(n) => Ok(Column::scalar(*n)),
        _ => Err("Invalid expression"),
    }
}
```

3. **Add REPL**
```rust
use rustyline::Editor;

fn main() {
    let mut rl = Editor::<()>::new();
    let mut env = Env::new();

    loop {
        let line = rl.readline("blawk> ").unwrap();
        let expr = lexpr::from_str(&line).unwrap();

        match eval(&expr, &mut env) {
            Ok(result) => println!("{:?}", result),
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}
```

4. **Add macro system**
```rust
// Simple macro expander
fn expand_macro(expr: &Value, macros: &MacroTable) -> Value {
    match expr {
        Value::Cons(cons) => {
            let head = cons.car();
            if let Some(mac) = macros.get(head.as_symbol()?) {
                mac.expand(cons.cdr())
            } else {
                // Recursively expand arguments
                let expanded_args = cons.cdr()
                    .iter()
                    .map(|arg| expand_macro(arg, macros))
                    .collect();
                Value::list(vec![head.clone()], expanded_args)
            }
        }
        _ => expr.clone(),
    }
}
```

**Pros:**
- ✅ Zero FFI overhead (direct Rust calls)
- ✅ Full Rust safety guarantees
- ✅ Single language (Rust)
- ✅ Maximum performance
- ✅ Easy to extend with new operations

**Cons:**
- ❌ Need to reimplement Lisp interpreter
- ❌ Macro system complexity
- ❌ More work upfront

**Effort:** 1-2 weeks
**Risk:** Medium
**Speedup:** 1.89× + no FFI overhead (potentially 2.0×+)

---

### Approach 3: Embedded Scripting Language (Pragmatic Path)

**Concept:** Use proven embedded Lisp/Scheme in Rust

**Options:**
- **Steel** (Scheme in Rust, modern)
- **Ketos** (Lisp in Rust, mature)
- **Rhai** (Not Lisp, but fast scripting)

**Architecture:**
```
┌─────────────────┐
│  User Script    │  ← Scheme/Lisp syntax
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Steel/Ketos    │  ← Embedded interpreter
│  (Rust)         │
└────────┬────────┘
         │ (native function bindings)
         ▼
┌─────────────────┐
│  blawk_kdb      │  ← Direct Rust calls
│  (Rust)         │
└─────────────────┘
```

**Implementation (Steel example):**

1. **Add Steel dependency**
```toml
[dependencies]
steel-core = "0.5"
```

2. **Register blawk functions**
```rust
use steel::steel_vm::engine::Engine;

fn main() {
    let mut engine = Engine::new();

    // Register dlog
    engine.register_fn("dlog", |col: Column, lag: i64| {
        let mut scratch = Scratch::new();
        let mut out = Column::new_f64(vec![]);
        dlog_into(&mut out, &col, lag as usize, &mut scratch);
        out
    });

    // Register other operations
    engine.register_fn("add", |a: Column, b: Column| a.add(&b));
    engine.register_fn("mul", |a: Column, b: Column| a.mul(&b));
    // ... etc

    // Run REPL
    engine.run("(define prices (load \"prices.csv\"))").unwrap();
    engine.run("(dlog prices 1)").unwrap();
}
```

3. **Threading macros (Steel has them!)**
```scheme
(-> prices
    (dlog 1)
    (mul 252.0)
    (zscore))
```

**Pros:**
- ✅ Battle-tested interpreter
- ✅ Full Scheme/Lisp features
- ✅ Good documentation
- ✅ Fast implementation (2-3 days)
- ✅ Native Rust integration

**Cons:**
- ❌ Dependency on external library
- ❌ Different syntax than clispi (Scheme vs Common Lisp)
- ❌ Learning curve for Steel specifics

**Effort:** 2-3 days
**Risk:** Low
**Speedup:** 1.89× (full backend speed)

---

## Recommended Approach: Phased Strategy

### Phase 1: FFI Bridge (Week 1)
**Goal:** Prove performance gain with minimal risk

1. Add C FFI layer to blawk_kdb
2. Build shared library
3. Update clispi to call Rust backend via FFI
4. Run benchmarks: Expect 1.89× speedup
5. Validate correctness: All 70 operations

**Deliverables:**
- `src/ffi.rs` (200-300 lines)
- `libblawk_kdb.so`
- Updated clispi dispatcher
- Benchmark results

**Success criteria:** 1.5×+ speedup, all tests pass

---

### Phase 2: Rust-Native Interpreter (Weeks 2-4)
**Goal:** Eliminate FFI overhead, full Rust stack

1. Implement S-expression parser (lexpr)
2. Build basic evaluator (30-40 operations)
3. Add REPL with rustyline
4. Port clispi macro system
5. Comprehensive tests

**Deliverables:**
- `blawk_repl` binary
- S-expression evaluator
- Macro expander
- 70 operations exposed
- Test suite

**Success criteria:** Feature parity with clispi, 2.0×+ speedup over C++

---

### Phase 3: Production Hardening (Week 5)
**Goal:** Production-ready Lisp REPL

1. Error handling and reporting
2. Debugger integration
3. Performance profiling
4. Documentation and examples
5. Optional: JIT compilation (cranelift)

**Deliverables:**
- Polished REPL with history, completion
- Error messages with line numbers
- User guide
- Example scripts

---

## Technical Design Details

### FFI Layer Design (Approach 1)

**Memory Management:**
```rust
// Opaque handle for Column
pub struct ColumnHandle {
    inner: Box<Column>,
    scratch: Scratch,  // Each handle owns a scratch allocator
}

#[no_mangle]
pub extern "C" fn blawk_column_new(len: usize) -> *mut ColumnHandle {
    let handle = ColumnHandle {
        inner: Box::new(Column::new_f64(vec![0.0; len])),
        scratch: Scratch::new(),
    };
    Box::into_raw(Box::new(handle))
}

#[no_mangle]
pub extern "C" fn blawk_dlog(
    handle: *mut ColumnHandle,
    lag: usize,
) -> i32 {
    unsafe {
        let h = &mut *handle;
        let mut out = Column::new_f64(vec![]);
        dlog_into(&mut out, &h.inner, lag, &mut h.scratch);
        h.inner = Box::new(out);
        0 // Success
    }
}

#[no_mangle]
pub extern "C" fn blawk_column_free(handle: *mut ColumnHandle) {
    unsafe {
        Box::from_raw(handle);  // Drop
    }
}
```

**Error Handling:**
```rust
#[repr(C)]
pub struct CError {
    code: i32,
    message: *const c_char,
}

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = RefCell::new(None);
}

#[no_mangle]
pub extern "C" fn blawk_get_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr() as *const c_char)
            .unwrap_or(std::ptr::null())
    })
}
```

---

### Rust-Native Evaluator Design (Approach 2)

**AST Representation:**
```rust
#[derive(Debug, Clone)]
pub enum Expr {
    Symbol(String),
    Number(f64),
    List(Vec<Expr>),
    Column(Column),
}

pub struct Env {
    bindings: HashMap<String, Column>,
    scratch: Scratch,
}

impl Env {
    pub fn eval(&mut self, expr: &Expr) -> Result<Column, Error> {
        match expr {
            Expr::Symbol(name) => {
                self.bindings.get(name)
                    .cloned()
                    .ok_or_else(|| Error::Undefined(name.clone()))
            }
            Expr::Number(n) => Ok(Column::scalar(*n)),
            Expr::Column(c) => Ok(c.clone()),
            Expr::List(exprs) => self.eval_list(exprs),
        }
    }

    fn eval_list(&mut self, exprs: &[Expr]) -> Result<Column, Error> {
        let Expr::Symbol(op) = &exprs[0] else {
            return Err(Error::NotCallable);
        };

        match op.as_str() {
            "dlog" => {
                let col = self.eval(&exprs[1])?;
                let Expr::Number(lag) = exprs[2] else {
                    return Err(Error::TypeMismatch);
                };

                let mut out = Column::new_f64(vec![]);
                dlog_into(&mut out, &col, lag as usize, &mut self.scratch);
                Ok(out)
            }
            "define" => {
                let Expr::Symbol(name) = &exprs[1] else {
                    return Err(Error::TypeMismatch);
                };
                let value = self.eval(&exprs[2])?;
                self.bindings.insert(name.clone(), value.clone());
                Ok(value)
            }
            "+" => {
                let a = self.eval(&exprs[1])?;
                let b = self.eval(&exprs[2])?;
                Ok(a.add(&b))
            }
            _ => Err(Error::UnknownOp(op.clone())),
        }
    }
}
```

**Macro System:**
```rust
pub struct Macro {
    params: Vec<String>,
    body: Expr,
}

impl Macro {
    pub fn expand(&self, args: &[Expr]) -> Expr {
        // Build substitution map
        let mut subst = HashMap::new();
        for (param, arg) in self.params.iter().zip(args) {
            subst.insert(param.clone(), arg.clone());
        }

        // Recursively substitute in body
        self.subst_expr(&self.body, &subst)
    }

    fn subst_expr(&self, expr: &Expr, subst: &HashMap<String, Expr>) -> Expr {
        match expr {
            Expr::Symbol(s) => subst.get(s).cloned().unwrap_or(expr.clone()),
            Expr::List(exprs) => {
                Expr::List(exprs.iter().map(|e| self.subst_expr(e, subst)).collect())
            }
            _ => expr.clone(),
        }
    }
}

// Define threading macro
pub fn define_arrow_macro() -> Macro {
    // (-> x (f a) (g b)) → (g (f x a) b)
    Macro {
        params: vec!["init".to_string(), "forms".to_string()],
        body: Expr::List(vec![
            Expr::Symbol("thread-first".to_string()),
            Expr::Symbol("init".to_string()),
            Expr::Symbol("forms".to_string()),
        ]),
    }
}
```

---

## Performance Projections

### Expected Speedups

**Approach 1: FFI Bridge**
```
C++ baseline:        29.31 ms
Rust backend:        15.51 ms
FFI overhead:        ~5% (0.78 ms)
Expected:            16.29 ms
Speedup:             1.80× vs C++
```

**Approach 2: Rust-Native**
```
C++ baseline:        29.31 ms
Rust backend:        15.51 ms
Zero FFI overhead:   0 ms
Expected:            15.51 ms
Speedup:             1.89× vs C++
```

**Approach 3: Embedded (Steel)**
```
C++ baseline:        29.31 ms
Rust backend:        15.51 ms
Steel overhead:      ~3% (0.47 ms)
Expected:            15.98 ms
Speedup:             1.83× vs C++
```

### With SLEEF (Optional Phase 4)
```
Current Rust:        15.51 ms
+ SLEEF vector ln(): 10.00 ms (estimated)
Speedup:             2.93× vs C++
```

---

## Implementation Roadmap

### Week 1: FFI Prototype
- [ ] Add `src/ffi.rs` to blawk_kdb
- [ ] Export core operations (dlog, add, mul, etc.)
- [ ] Build shared library
- [ ] Write C header file
- [ ] Test with simple C program
- [ ] Integrate with clispi
- [ ] Benchmark: Expect 1.8× speedup

### Week 2: Core Evaluator
- [ ] Set up new `blawk_repl` crate
- [ ] Add lexpr dependency
- [ ] Implement basic parser
- [ ] Implement evaluator for 10 core ops
- [ ] Add error handling
- [ ] Write initial tests

### Week 3: Full Operations
- [ ] Port all 35 blawk_kdb operations
- [ ] Add special forms (define, if, lambda)
- [ ] Implement macro system
- [ ] Add threading macros (→, →→)
- [ ] Port clispi examples

### Week 4: REPL & Testing
- [ ] Add rustyline REPL
- [ ] Implement history and completion
- [ ] Pretty-print results
- [ ] Comprehensive test suite
- [ ] Performance benchmarks

### Week 5: Polish
- [ ] Error messages with source locations
- [ ] Documentation (user guide, API docs)
- [ ] Example scripts
- [ ] Performance profiling
- [ ] Release v1.0

---

## Risk Analysis

### Technical Risks

**1. FFI Safety (Approach 1)**
- **Risk:** Memory leaks, use-after-free
- **Mitigation:**
  - Use opaque handles
  - Clear ownership rules
  - Valgrind testing
  - Fuzzing

**2. Macro System Complexity (Approach 2)**
- **Risk:** Bugs in macro expander
- **Mitigation:**
  - Start with simple macros
  - Extensive macro tests
  - Macro expansion introspection
  - Reference clispi implementation

**3. Performance Regression**
- **Risk:** Not achieving expected speedup
- **Mitigation:**
  - Benchmark early and often
  - Profile with flamegraph
  - Compare against C++ baseline
  - Target 1.5× minimum speedup

### Project Risks

**1. Scope Creep**
- **Risk:** Feature bloat, never ship
- **Mitigation:**
  - Phased approach (FFI first)
  - MVP with 10 operations
  - Expand incrementally

**2. Compatibility**
- **Risk:** Breaking existing clispi scripts
- **Mitigation:**
  - Maintain compatibility layer
  - Comprehensive test suite
  - Migration guide

---

## Success Metrics

### Performance
- ✅ 1.5× faster than C++ baseline (minimum)
- ✅ 1.8× faster than C++ baseline (target, FFI)
- ✅ 1.89× faster than C++ baseline (target, native)
- ✅ Zero regression in correctness

### Functionality
- ✅ All 70 operations supported
- ✅ Macro system (defmacro, quasiquote)
- ✅ Threading macros (→, →→, as)
- ✅ REPL with history and completion
- ✅ Error reporting with source locations

### Quality
- ✅ 100% test coverage for core operations
- ✅ Memory-safe (no unsafe except FFI boundary)
- ✅ Documentation (user guide + API docs)
- ✅ Example scripts (10+ real-world examples)

---

## Code Examples

### Example 1: FFI Usage (Approach 1)

**C program:**
```c
#include "blawk.h"

int main() {
    // Create column with test data
    ColumnHandle* prices = blawk_column_new(5);
    double data[] = {100.0, 102.0, 101.5, 103.0, 104.5};
    blawk_column_set_data(prices, data, 5);

    // Compute log returns
    blawk_dlog(prices, 1);

    // Get result
    double* result = blawk_column_get_data(prices);
    for (int i = 0; i < 5; i++) {
        printf("%f\n", result[i]);
    }

    blawk_column_free(prices);
    return 0;
}
```

**clispi usage:**
```lisp
;; Load Rust backend
(load-rust-backend "target/release/libblawk_kdb.so")

;; Use exactly like before (transparent!)
(define prices (load (fmap "prices.csv")))
(define returns (dlog prices 1))
(see returns)
```

### Example 2: Rust-Native Evaluator (Approach 2)

**Rust code:**
```rust
// examples/repl.rs
use blawk_repl::{Env, parse};

fn main() {
    let mut env = Env::new();

    // Load data
    let script = r#"
        (define prices (load "prices.csv"))
        (define returns (dlog prices 1))
        (define annual (mul returns 252.0))
        (see annual)
    "#;

    for line in script.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let expr = parse(line).unwrap();
        let result = env.eval(&expr).unwrap();
        println!("{:?}", result);
    }
}
```

**User experience:**
```
$ blawk_repl
blawk> (define prices (load "prices.csv"))
Column<f64>[1000000]

blawk> (dlog prices 1)
Column<f64>[1000000] [NA, 0.0198, -0.0049, 0.0148, ...]

blawk> (-> prices (dlog 1) (mul 252) (zscore))
Column<f64>[1000000] [-0.234, 1.432, -1.122, 0.889, ...]

blawk> (time (-> prices (dlog 1) (mul 252)))
Elapsed: 15.51 ms
Column<f64>[1000000]
```

### Example 3: Threading Macro Implementation

**Rust macro expander:**
```rust
fn expand_thread_first(exprs: &[Expr]) -> Expr {
    // (-> x (f a) (g b)) → (g (f x a) b)
    let mut result = exprs[0].clone();

    for form in &exprs[1..] {
        match form {
            Expr::List(list) => {
                // Insert result as first arg: (f a) → (f result a)
                let mut new_list = vec![list[0].clone(), result];
                new_list.extend_from_slice(&list[1..]);
                result = Expr::List(new_list);
            }
            Expr::Symbol(s) => {
                // (f) → (f result)
                result = Expr::List(vec![Expr::Symbol(s.clone()), result]);
            }
            _ => panic!("Invalid threading form"),
        }
    }

    result
}

// (-> 100 (add 50) (mul 2)) → (mul (add 100 50) 2) → 300
```

---

## Dependencies

### Approach 1: FFI Bridge
```toml
[dependencies]
# No new dependencies! Just expose C ABI
```

### Approach 2: Rust-Native
```toml
[dependencies]
lexpr = "0.2"          # S-expression parser
rustyline = "12.0"     # REPL with history
colored = "2.0"        # Terminal colors
anyhow = "1.0"         # Error handling
```

### Approach 3: Embedded
```toml
[dependencies]
steel-core = "0.5"     # Embedded Scheme
# OR
ketos = "0.11"         # Embedded Lisp
```

---

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_dlog() {
        let mut env = Env::new();
        let script = "(dlog (column 100.0 102.0 101.5) 1)";
        let expr = parse(script).unwrap();
        let result = env.eval(&expr).unwrap();

        // First element should be NA (lag)
        assert!(result.is_null(0));

        // Second element: ln(102/100) = 0.0198
        assert!((result.get(1).unwrap() - 0.0198).abs() < 0.0001);
    }

    #[test]
    fn test_macro_thread_first() {
        let mut env = Env::new();
        env.define_macro("->", thread_first_macro());

        let script = "(-> 100 (add 50) (mul 2))";
        let expr = parse(script).unwrap();
        let expanded = env.expand_macros(&expr);

        // Should expand to: (mul (add 100 50) 2)
        assert_eq!(format!("{:?}", expanded), "(mul (add 100 50) 2)");

        let result = env.eval(&expanded).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 300.0);
    }
}
```

### Integration Tests
```bash
# Test FFI bridge
./test_ffi_bridge.sh

# Test all 70 operations
cargo test --test operations

# Test macro system
cargo test --test macros

# Test REPL
expect tests/repl_test.exp
```

### Benchmark Suite
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_eval_dlog(c: &mut Criterion) {
    let mut env = Env::new();
    let data = (0..1_000_000).map(|i| 100.0 + (i as f64) * 0.01).collect();
    env.define("prices", Column::new_f64(data));

    let script = "(dlog prices 1)";
    let expr = parse(script).unwrap();

    c.bench_function("eval_dlog_1M", |b| {
        b.iter(|| env.eval(black_box(&expr)))
    });
}

criterion_group!(benches, bench_eval_dlog);
criterion_main!(benches);
```

---

## Documentation Plan

### User Guide
1. **Getting Started**
   - Installation
   - First REPL session
   - Basic operations

2. **Language Reference**
   - S-expression syntax
   - Core forms (define, if, lambda)
   - Macro system

3. **Operations Reference**
   - All 70 operations documented
   - Examples for each
   - Performance notes

4. **Advanced Topics**
   - Custom macros
   - Performance tuning
   - Debugging

### API Documentation
```rust
/// Compute log returns with lag
///
/// # Examples
/// ```
/// let prices = Column::new_f64(vec![100.0, 102.0, 101.5]);
/// let returns = dlog(&prices, 1);
/// // returns[0] = NA (lag)
/// // returns[1] = ln(102/100) ≈ 0.0198
/// // returns[2] = ln(101.5/102) ≈ -0.0049
/// ```
pub fn dlog(col: &Column, lag: usize) -> Column {
    // ...
}
```

---

## Migration Guide (clispi → Rust)

### Syntax Compatibility

**100% compatible:**
```lisp
(define prices (load (fmap "prices.csv")))
(dlog prices 1)
(+ (mul returns 252) offset)
(-> prices (dlog 1) (zscore))
```

**Minor changes:**
```lisp
;; clispi (Common Lisp style)
(defmacro my-macro (x) `(+ ,x ,x))

;; Rust-native (Scheme style)
(define-macro (my-macro x) `(+ ,x ,x))
```

### Migration Steps

1. **Install Rust version**
```bash
cargo install blawk_repl
```

2. **Test existing scripts**
```bash
blawk_repl < my_script.lisp
```

3. **Fix compatibility issues** (if any)
   - Check macro syntax
   - Verify numeric literals
   - Update file paths

4. **Benchmark**
```bash
time blawk_repl < my_script.lisp
# Expect 1.8-1.9× speedup
```

---

## Deployment

### Build Instructions

**Approach 1: FFI Library**
```bash
cd /home/ubuntu/blawk_kdb
RUSTFLAGS="-C target-cpu=native" cargo build --release --lib
cp target/release/libblawk_kdb.so /usr/local/lib/
ldconfig
```

**Approach 2: Native Binary**
```bash
cd /home/ubuntu/blawk_repl
RUSTFLAGS="-C target-cpu=native" cargo build --release
cp target/release/blawk_repl /usr/local/bin/
```

### Installation Package
```bash
# Create .deb package
cargo deb

# Install
sudo dpkg -i target/debian/blawk-repl_1.0.0_amd64.deb
```

---

## Future Enhancements

### Phase 4: SLEEF Vector Math (Optional)
- 2-4× faster ln() throughput
- Overall 2.93× faster than C++
- Effort: 2-3 hours

### Phase 5: Parallel Execution (Optional)
- Multi-column processing with Rayon
- 2-4× speedup on multi-core
- Combines with SLEEF for 6-8× total

### Phase 6: JIT Compilation (Advanced)
- Use cranelift for hot loops
- Compile Lisp → native code
- 2-5× speedup on tight loops
- Effort: 2-3 weeks

---

## Conclusion

This blueprint provides three viable paths to integrate Lisp front-end with Rust blawk_kdb:

1. **FFI Bridge**: Quick (1-2 days), proven speedup (1.8×)
2. **Rust-Native**: Clean (1-2 weeks), maximum speed (1.89×)
3. **Embedded**: Pragmatic (2-3 days), battle-tested (1.83×)

**Recommended: Start with Approach 1 (FFI) to prove performance, then migrate to Approach 2 (Rust-Native) for production.**

### Expected Outcomes
- ✅ 1.8-1.9× faster than current C++ backend
- ✅ Memory-safe (Rust borrow checker)
- ✅ Feature parity with clispi (70 operations)
- ✅ Production-ready in 4-5 weeks

### Next Steps
1. Review this blueprint
2. Choose approach (recommend: FFI first)
3. Set up project structure
4. Begin Week 1 implementation

---

**Author:** Generated from blawk_kdb optimization session
**Date:** 2026-02-17
**Status:** Architecture design complete, ready for implementation
