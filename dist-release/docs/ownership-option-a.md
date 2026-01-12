# Ownership and Borrowing System: Option A Analysis

## Executive Summary

This document explores **Option A: Rust-like Borrow Checker** as an alternative to Aura's current Option B (manual resource management + linear types). Option A would provide compile-time memory safety guarantees at the cost of increased language complexity and learning curve.

## Option A: Borrow Checker (Rust-Inspired)

### Core Concept

Implement a region-based type system with borrow tracking:

```aura
fn example(s: &String) -> Unit {
  // s is borrowed, cannot move
  print(s);
  // s still valid at function end
}

fn move_example(s: String) -> Unit {
  // s is owned, can move
  let s2 = s;  // s moved to s2
  // print(s);  // ERROR: s no longer owned
  print(s2);
}
```

**Key Rules:**
- Default: `let x = value` creates **owned** binding
- Borrow syntax: `&x` creates immutable **borrowed** reference
- Mutable borrow: `&mut x` creates mutable reference (exclusive)
- **NLL (Non-Lexical Lifetimes):** Borrow scope = actual usage, not declaration scope
- **Implicit return:** Value moved if not consumed

### Advantages of Option A

1. **Memory Safety at Compile Time**
   - No garbage collection overhead
   - No runtime undefined behavior
   - Catches use-after-free, double-free, data races at compile time

2. **Predictable Performance**
   - Zero-copy abstractions via borrowing
   - No GC pauses
   - Stack allocation wherever possible

3. **Explicit Ownership**
   - Clear who owns what
   - Easier reasoning about resource cleanup
   - Excellent for systems programming

4. **Maturity**
   - Proven in Rust (since 2015)
   - Well-documented, large community
   - Rich ecosystem of patterns

### Disadvantages of Option A

1. **Steep Learning Curve**
   - Borrow checker frustration ("fighting the borrow checker")
   - Error messages can be cryptic
   - Many developers report 2-4 weeks adjustment period

2. **Development Friction**
   - Refactoring can trigger cascading borrow errors
   - Workarounds needed for complex graph structures
   - Ergonomics often require helper types (`Rc`, `RefCell`)

3. **Language Complexity**
   - Lifetime parameters add syntactic overhead
   - Complex inference rules (variance, outlives bounds)
   - More error states to handle

4. **Code Style Constraints**
   - Callback patterns become awkward
   - Some designs require `Arc<Mutex<T>>` (runtime overhead)
   - Mutable global state nearly impossible (forces interior mutability)

### Feasibility Assessment

#### Type System Changes Required

1. **Ownership Model**
   ```
   Type := Owned<T> | &T | &mut T
   
   where:
   - Owned<T> can be moved
   - &T can be copied (shared borrow)
   - &mut T exclusive, cannot be copied
   ```

2. **Lifetime Parameters**
   ```
   fn example<'a>(s: &'a String) -> &'a String { s }
   
   'a tracks the borrowed region
   ```

3. **Borrowing Rules (Checked at Compile Time)**
   ```
   • At any time: Either 1 &mut T OR N &T borrows, not both
   • Borrow cannot outlive value
   • Moved values cannot be used
   ```

#### Implementation Estimate

- **Borrow checker inference engine:** 2-3 weeks
- **Lifetime inference:** 3-4 weeks
- **Error messages + recovery:** 2 weeks
- **Standard library adaptation:** 3-4 weeks
- **Testing + refinement:** 3-4 weeks

**Total: 13-19 weeks** (3-4.5 months)

### Comparison with Current Approaches

| Feature | Option A (Borrow) | Option B (Linear + Manual) | GC (Module 7) |
|---------|-------------------|---------------------------|---------------|
| Memory Safety | ✅ Compile-time | ⚠️ Runtime + discipline | ✅ Runtime |
| Performance | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ (GC pauses) |
| Learning Curve | ⭐⭐ (steep) | ⭐⭐⭐⭐ (moderate) | ⭐⭐⭐⭐⭐ (easiest) |
| Ergonomics | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| Real-time Safe | ✅ Yes | ✅ Yes | ❌ GC pauses |
| Systems Prog | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐ (not suitable) |

## Recommended Path Forward

### Short Term (v0.3 - v0.4)
Maintain **Option B** (manual + linear types):
- Proven to work in Aura today
- No compiler rewrite needed
- Allows users to opt into GC via Module 7 (allocator)

### Medium Term (v0.5+)
If memory safety issues surface:
1. Implement **borrow checker** (Option A) as an opt-in compile mode
2. Allow code to target `--memory-model=borrow` or `--memory-model=linear`
3. Provide migration guide for users

### Not Recommended
- Forced migration to Option A without opt-in period
- Mixing borrowing + linear types in same program (confusing)
- Attempting concurrent GC alongside Option A (complexity explosion)

## Prototype Implementation

To validate feasibility, a minimal borrow checker could be implemented:

```rust
// aura-verify/src/borrow_checker.rs (pseudocode)

pub struct BorrowState {
    borrowed: BTreeSet<Var>,
    mutably_borrowed: BTreeSet<Var>,
    moved: BTreeSet<Var>,
}

impl BorrowState {
    pub fn check_borrow(&self, var: &Var) -> Result<(), BorrowError> {
        if self.mutably_borrowed.contains(var) {
            return Err(BorrowError::AlreadyMutablyBorrowed);
        }
        // ... check consistency
        Ok(())
    }
}
```

## Conclusion

**Option A (Rust-like Borrow Checker)** is **feasible but not urgent**:

✅ **Pros:** Memory safety, performance, systems-friendly  
❌ **Cons:** 3-4 month rewrite, steep learning curve  

**Recommendation:** Implement when Option B + manual testing reveals critical safety gaps (estimated 1-2 years of v0.3+ usage).

---

## References

- "Ownership and Moves" - Rust Book (https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)
- "How to Infer Rust Lifetimes" - Aaron Turon (2015)
- Aura Design Doc: Memory Models (internal)
