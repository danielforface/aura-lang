# UB boundaries (defined behavior policy)

This document defines what Aura considers **undefined behavior (UB)** vs **defined runtime failure**.

## Goal

Backends must not rely on C/LLVM UB for user-visible semantics.

If a program hits a UB boundary at runtime, the implementation must:

- **Trap** (abort/fail-fast), or
- **Return an error** (where the language surface defines an error channel).

The current native pipeline uses **fail-fast traps** for these cases.

## U32 arithmetic

- `u32` add/sub/mul overflow is **defined** (wraparound) in the current implementation model.
- `u32` division by zero is **not** permitted to become C/LLVM UB.
  - Backends must emit a runtime check that traps when the divisor is `0`.

## Bounds checks

- Array/tensor indexing outside valid bounds must not become UB.
- The runtime provides `aura_range_check_u32(v, lo, hi)` which traps on failure.

## Notes

- The AVM/interpreter may trap via host errors/panics; this is still considered a defined failure mode.
- Future editions may refine these rules, but changes must be editioned or feature-gated.
