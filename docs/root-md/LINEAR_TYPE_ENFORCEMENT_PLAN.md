# Linear Type Enforcement Implementation Plan

## Critical Bottleneck #2: Type System Ownership Enforcement

The roadmap correctly identifies that without **linear type enforcement** in the type-checker, Aura cannot formally declare "Memory Safe by construction".

### Current State
- ✅ Parser supports linear type syntax  (`use x; x;` patterns)
- ✅ AST represents ownership information
- ❌ **Type-checker doesn't enforce** linear ownership rules

### What Must Be Implemented

#### 1. Ownership Tracking in Type Context
Every variable binding must track:
- **Consumed**: Has the value been moved?
- **Borrowed**: Is it currently borrowed (immutable or mutable)?
- **Returned**: Was it returned from the function?

#### 2. Enforcement Rules to Implement

**Rule 1: Each Linear Value Can Only Be Used Once**
```aura
fn test(x: Linear<i32>) {
    print(x);    // OK: consuming x
    print(x);    // ERROR: x was already consumed
}
```

**Rule 2: Cannot Return + Use After**
```aura
fn test(x: Linear<i32>) -> Linear<i32> {
    y := x;      // OK: moving x into y
    return y;    // ERROR: y was consumed but we return it?
}
```

**Rule 3: Borrowing is Exclusive**
```aura
fn test(x: Linear<i32>) {
    let ref1 = &x;     // OK: borrow
    let ref2 = &x;     // ERROR: can't have two borrows of same linear value
}
```

**Rule 4: After Borrow, Can't Move**
```aura
fn test(x: Linear<i32>) {
    let ref = &x;
    y := x;           // ERROR: x is borrowed, can't move
}
```

#### 3. Control Flow Analysis

Must track ownership through branches:
```aura
if condition {
    y := x;   // Moves x in true branch
} else {
    z := x;   // Moves x in false branch
}
// x is consumed in both branches - OK to continue
```

#### 4. Function Signature Validation

```aura
fn consume(x: Linear<i32>) {
    // x must be consumed before function ends
    print(x);  // Consumes x - good
}

fn borrow(ref: &Linear<i32>) {
    // Borrowed parameter - OK to not consume
    print(*ref);
}
```

### Implementation Strategy

#### Phase 1: Core Tracking (This Session)
1. Extend `TypeContext` to track ownership state
2. Implement `OwnershipState` enum
3. Add `consume_value()`, `borrow_value()` methods
4. Modify variable binding to track state

#### Phase 2: Enforcement (Next Session)
1. Add enforcement in expression type-checker
2. Block illegal operations with clear errors
3. Implement control flow analysis
4. Add function signature validation

#### Phase 3: Advanced (Future)
1. Borrow checker sophistication (Rust-style regions)
2. Generic ownership parameters
3. Lifetime inference

### Bottleneck Impact

**Without this:**
- Can't prove memory safety
- Enterprise won't adopt
- Doesn't fulfill v1.0 requirements

**With this:**
- "Memory Safe by construction" is REAL
- Can prove no double-free, use-after-free
- Differentiates from Python/JavaScript
- Enables safe concurrent programming

---

## Implementation Start
