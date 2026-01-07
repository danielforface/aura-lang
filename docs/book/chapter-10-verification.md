# Chapter 10: Verification & Proof-Driven Development

## Introduction

Verification is a first-class feature in Aura. Unlike optional lint warnings or runtime assertions, Aura's verification system proves program correctness at compile time using automated SMT solvers (Z3). This chapter teaches you how to write provably-correct programs.

### Why Verification?

Traditional testing finds bugs *after* they happen. Verification prevents them:

- **No undefined behavior** — Array bounds, integer overflow, null pointers are impossible
- **Proof of correctness** — Loops terminate, functions return the right type, concurrent code has no races
- **Confident refactoring** — Change code freely; the verifier ensures you didn't break invariants
- **Documentation** — `requires` and `ensures` clauses are machine-checked contracts

### The Aura Verification Philosophy

Aura's verifier follows three principles:

1. **Interactive & Explainable** — Proofs stream incrementally; failures show counterexamples with variable traces
2. **Zero-Cost** — Verification happens at compile time; proven code runs at full speed
3. **Predictable** — Sub-200ms feedback for typical functions; latency is tuned and cached

---

## Part 1: Contracts (Basic Assertions)

### Simple Assertions

The simplest proof is an assertion:

```aura
fn increment(x: i32) -> i32 {
    let result = x + 1;
    assert result > x;  // The verifier proves this is always true
    result
}
```

When you save this file in Sentinel, the verifier runs and **proves** the assertion. If it can't, it shows a counterexample.

### Preconditions with `requires`

Use `requires` to document expectations:

```aura
fn divide(numerator: i32, divisor: i32) -> i32
    requires divisor != 0
{
    numerator / divisor
}
```

The `requires` clause tells the verifier: "assume the caller passes a non-zero divisor." The function body is then verified against this assumption.

**In the caller:**

```aura
fn main() {
    let x = divide(10, 2);  // ✅ Passes: 2 != 0
    let y = divide(10, 0);  // ❌ Error: doesn't satisfy requires divisor != 0
}
```

### Postconditions with `ensures`

Use `ensures` to promise output properties:

```aura
fn absolute(x: i32) -> i32
    ensures return >= 0
{
    if x >= 0 { x } else { -x }
}
```

The verifier checks that the function always returns a non-negative value.

### Combined Example

```aura
fn safe_divide(numerator: i32, divisor: i32) -> Option[i32]
    requires numerator >= 0
    requires divisor > 0
    ensures match return {
        case Some(q) => q >= 0,
        case None => false,
    }
{
    if divisor > 0 {
        Some(numerator / divisor)
    } else {
        None
    }
}
```

---

## Part 2: Loop Invariants

Loops are hard to prove because the verifier must handle unbounded iteration. **Loop invariants** help.

### Anatomy of a Loop Invariant

```aura
fn sum_to_n(n: i32) -> i32
    requires n >= 0
    ensures return == (n * (n + 1)) / 2
{
    let mut total = 0;
    let mut i = 0;
    
    while i < n
        invariant 0 <= i && i <= n              // Loop invariant (1)
        invariant total == (i * (i + 1)) / 2   // Loop invariant (2)
        decreases n - i                          // Termination hint
    {
        total = total + i;
        i = i + 1;
    }
    
    total
}
```

**How it works:**

1. **Before loop:** Invariants are assumed true (initial state)
2. **Loop body:** Verifier proves that if invariants hold at loop entry, they still hold after one iteration
3. **After loop:** Invariants are guaranteed to be true; the verifier also knows the loop condition is false

In `sum_to_n`:
- `invariant total == (i * (i + 1)) / 2` tracks the mathematical invariant
- When loop exits (`i == n`), we know `total == (n * (n + 1)) / 2`, which is the `ensures` clause ✓

### Termination with `decreases`

The `decreases` clause helps prove loops terminate:

```aura
while i < n
    decreases n - i   // This value strictly decreases each iteration
{
    // ...
    i = i + 1;
}
```

The verifier checks:
- `n - i > 0` before loop body (i.e., it's a positive integer)
- `n - i` strictly decreases after each iteration
- When `n - i <= 0`, loop must exit

---

## Part 3: Data Structure Invariants

### Region-Based Collections

Aura's region-based memory model provides **verified collections** with proven invariants:

```aura
use std::collections_region;

fn process_items() {
    let mut vec = collections_region::Vec::new();
    
    vec.push(1);
    vec.push(2);
    vec.push(3);
    
    // The verifier knows:
    // - vec.len() == 3
    // - vec.capacity() >= 3
    // - vec[0] == 1, vec[1] == 2, vec[2] == 3
    // - No out-of-bounds access is possible
    
    for i in 0..vec.len() {
        let item = vec.get(i);  // ✅ Always safe (verified bounds)
        println!("{}", item);
    }
}
```

### Custom Invariants

Define your own data structure with proven invariants:

```aura
struct SortedList {
    data: collections_region::Vec[i32],
    // Implicit invariant: data[i] <= data[i+1] for all i
}

impl SortedList {
    fn new() -> SortedList
        ensures return.data.len() == 0
    {
        SortedList { data: collections_region::Vec::new() }
    }
    
    fn insert(mut self, value: i32)
        ensures self.is_sorted()
    {
        let mut pos = 0;
        while pos < self.data.len() && self.data.get(pos) < value
            invariant pos <= self.data.len()
            invariant is_sorted_prefix(self.data, pos)
        {
            pos = pos + 1;
        }
        self.data.insert(pos, value);
    }
    
    fn is_sorted(self) -> bool {
        let mut i = 0;
        while i + 1 < self.data.len() {
            if self.data.get(i) > self.data.get(i + 1) {
                return false;
            }
            i = i + 1;
        }
        true
    }
}
```

---

## Part 4: Understanding Counterexamples

When a proof fails, the verifier shows a **counterexample** — a concrete input that violates your assertion.

### Example: Failing Proof

```aura
fn process(x: i32) -> i32
    ensures return > 0
{
    if x > 0 { x } else { 1 }  // Oops: doesn't handle negative x
}
```

**Counterexample shown in Sentinel:**

```
Assertion Failed: return > 0

Variable Trace:
  x: i32 = -5                    // Input value
  (x > 0): bool = false
  Result Branch: 1 (else clause)
  return: i32 = 1                // Returned 1

Repair Suggestions:
  [1] Strengthen precondition: Add `requires x > 0`
  [2] Weaken postcondition: Change `ensures return > 0` to `ensures return >= 1`
  [3] Fix logic: Handle negative case (e.g., return abs(x))
```

### Reading Variable Traces

The **variable trace** shows:
- **Definition** — Where the variable was first assigned
- **Assignments** — All modifications along the execution path
- **Final value** — The value when the assertion failed
- **Repair hints** — Suggested fixes (prove, relax, or refactor)

Click any variable to jump to its definition in source code.

---

## Part 5: SMT Solver Tuning

Aura uses Z3 for automated proving. You can tune solver behavior with directives:

### Fast vs. Thorough Proving

```aura
// Fast mode: quick feedback, may miss some proofs
#[verify_profile = "fast"]
fn quick_check(x: i32) -> bool
    ensures return == (x > 0)
{
    x > 0
}

// Thorough mode: slower, more powerful solver
#[verify_profile = "thorough"]
fn deep_check(x: i32, y: i32) -> bool
    ensures return == (x * y == 0)
{
    x == 0 || y == 0
}
```

### Helping the Solver with Triggers

For quantified formulas, provide **triggers** to guide instantiation:

```aura
fn all_positive(arr: &[i32]) -> bool
    ensures return => forall i. 0 <= i && i < arr.len() => arr[i] > 0
{
    let mut i = 0;
    while i < arr.len()
        invariant forall j. 0 <= j && j < i => arr[j] > 0  // {arr[j]}
        decreases arr.len() - i
    {
        if arr[i] <= 0 { return false; }
        i = i + 1;
    }
    true
}
```

The `{arr[j]}` syntax is a **trigger** — a hint to instantiate the quantifier when Z3 encounters a matching term.

---

## Part 6: Debugging Failed Proofs

When a proof fails, follow this workflow:

### Step 1: Read the Counterexample

Click the **Explain** button in the Proofs panel. Read the variable trace:
- What input triggers the failure?
- Which branch was taken?
- What's the final value of variables?

### Step 2: Add Intermediate Assertions

Break large proofs into smaller steps:

```aura
fn compute(x: i32) -> i32 {
    let y = x + 1;
    assert y == x + 1;      // Break point (1)
    
    let z = y * 2;
    assert z == (x + 1) * 2; // Break point (2)
    
    let result = z - 1;
    assert result == 2 * x + 1;  // Final goal
    
    result
}
```

If the second assertion fails, you know the problem is in the `z = y * 2` computation, not elsewhere.

### Step 3: Strengthen Preconditions

If the counterexample suggests an input shouldn't be allowed:

```aura
// Before
fn divide(a: i32, b: i32) -> i32 {
    a / b  // Fails on b == 0
}

// After
fn divide(a: i32, b: i32) -> i32
    requires b != 0
{
    a / b  // ✅ Now proven
}
```

### Step 4: Weaken Postconditions

If a postcondition is too strict:

```aura
// Before
fn approx_sqrt(x: i32) -> i32
    requires x >= 0
    ensures return * return == x   // Too strict (only exact for perfect squares)
{
    // ...
}

// After
fn approx_sqrt(x: i32) -> i32
    requires x >= 0
    ensures return * return <= x && (return + 1) * (return + 1) > x
{
    // ...
}
```

---

## Part 7: Incremental Proof Streaming

Aura streams proofs incrementally. As you type, the verifier shows progress:

### Proof Phases

1. **Parse** — Syntax check (instant, errors only)
2. **Semantic** — Type check, name resolution (<10ms)
3. **Normalize** — Simplify logic for solver (<50ms)
4. **Z3 Gate** — SMT solving (50–200ms typical)

Each phase appears in the Sentinel Proofs panel. You can:
- **View phase timeline** — Hover over phase bars to see durations
- **Cancel** — Click ✕ to stop a long-running proof
- **Rerun** — Click ↻ to re-verify after edits

### Proof Caching

Aura caches proofs based on:
- **File content hash** — Cache invalidates when file changes
- **Solver config** — Different configs = different cache
- **Dependencies** — If an import changes, dependent proofs re-run

Status in Sentinel:
- 🔵 **Cached** — Result came from cache (instant)
- 🟡 **Running** — Currently verifying
- 🟢 **Verified** — Proof completed successfully
- 🔴 **Failed** — Counterexample found

---

## Part 8: Advanced Topics

### Quantifiers with Guardrails

```aura
fn no_duplicates(arr: &[i32]) -> bool {
    forall i: i32, j: i32.
        (0 <= i && i < arr.len() &&
         0 <= j && j < arr.len() &&
         i != j)
        => arr[i] != arr[j]
}
```

**Timeout protection:** The verifier limits quantifier instantiation. If a proof times out, simplify or add stronger triggers.

### Existential Proofs

```aura
fn exists_max(arr: &[i32]) -> bool
    requires arr.len() > 0
    ensures exists i: i32.
        (0 <= i && i < arr.len() &&
         forall j: i32.
            (0 <= j && j < arr.len()) => arr[j] <= arr[i])
{
    true  // ✅ Z3 synthesizes a witness for i
}
```

### Reasoning About Concurrency

(Detailed in Chapter 12: Concurrency & Race-Free Proofs)

```aura
fn atomic_counter() {
    let counter = Atomic::new(0);
    
    // Proof: Two threads incrementing leads to count == 2
    spawn {
        counter.fetch_add(1);
    }
    spawn {
        counter.fetch_add(1);
    }
    
    join_all();
    assert counter.load() == 2;  // ✅ Proven race-free
}
```

---

## Summary: Verification Workflow

1. **Write assertions** — Document expected behavior with `assert`, `requires`, `ensures`
2. **Add loop invariants** — Help the verifier reason about unbounded loops
3. **Read counterexamples** — Use variable traces to understand failures
4. **Debug incrementally** — Add intermediate assertions to isolate issues
5. **Tune performance** — Use profiles and triggers for complex proofs
6. **Leverage caching** — Edit quickly; proofs stay fast via incremental cache

---

## Recommended Reading

- **Next chapter:** Chapter 11 — Debugging with Sentinel (stepping, breakpoints, profiling)
- **Type system:** Chapter 5 — Types & Ownership (refinement types, linear capabilities)
- **Standard library:** Chapters 14–16 — `std.collections_region`, `std.concurrent`

---

## Examples Repository

Full examples with verified code:
- `examples/01_assertions.aura` — Basic assertions and contracts
- `examples/02_loops.aura` — Loop invariants and termination
- `examples/03_data_structures.aura` — Verified lists and maps
- `examples/04_concurrency.aura` — Race-free concurrent code
- `examples/05_refinements.aura` — Refinement types in practice

Browse and run: `aura verify examples/*.aura`
