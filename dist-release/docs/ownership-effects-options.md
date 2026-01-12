# Ownership / Effects Model — Options A/B/C

This document describes three viable safety models for Aura’s mutation + concurrency story.

Design goals (shared)
- Prevent data races by construction.
- Make mutation explicit and locally checkable.
- Keep UX simple for most users (good error messages, minimal annotations).
- Make trust boundaries explicit (FFI/bridge/plugin code).

---

## Option A — Ownership + Borrowing (Rust-inspired)

Core idea
- Every value has a single *owner*.
- You may temporarily *borrow* (shared or mutable) under strict aliasing rules:
  - Many `&T` OR one `&mut T` at a time.

What Aura would need
- A notion of move vs copy types.
- Borrow checking over blocks/loops/matches.
- Lifetimes (mostly inferred) for references escaping scopes.

Pros
- Extremely strong aliasing guarantees; great for safe concurrency.
- Well-understood model with proven ergonomics.

Cons
- Borrow checking can be hard to explain in edge cases.
- Higher implementation complexity (especially with closures + UI callbacks).

Minimal surface sketch (illustrative)
- `val x = ...` (move by default for non-copy types)
- `&x` shared borrow
- `&mut x` mutable borrow
- `move` closures for UI callbacks

---

## Option B — Region-Based Memory + Linear Capabilities

Core idea
- Memory is organized into *regions* (lifetimes are region-scoped).
- Mutation and aliasing are controlled by *capabilities* that are linear (cannot be duplicated).

What Aura would need
- Region inference (or explicit `region` blocks for advanced users).
- Capability tokens for mutation, e.g. `Cap<write T>`.
- Rules for passing capabilities across calls/threads.

Pros
- Can make “who may mutate this?” very explicit.
- Often composes nicely with effectful UI/runtime systems.
- Potentially simpler error messages than full borrow checking.

Cons
- Requires introducing the concept of regions/capabilities to users.
- Needs careful design to avoid capability boilerplate.

Minimal surface sketch (illustrative)
- `region r { ... }`
- `val cap = write_cap(x)` then `mutate(x, cap)`

---

## Option C — Effect System + Explicit Alias Rules

Core idea
- Functions and blocks carry *effects* like `Read(x)`, `Write(x)`, `IO`, etc.
- The typechecker enforces that effects are permitted at each callsite.
- Aliasing is controlled by explicit rules (e.g., “no `Write` if multiple aliases exist”).

What Aura would need
- An effect lattice (at least `Pure`, `Read`, `Write`, `IO`).
- Effect inference for most code; explicit annotations for public APIs.
- Alias-tracking sufficient to connect effects to names/paths.

Pros
- Great for API clarity: you can see what a function may do.
- Plays well with verification: effects can become assumptions/obligations.

Cons
- Needs careful design to prevent “effect annotation fatigue”.
- Alias-tracking is still non-trivial (but can be less strict than Option A).

Minimal surface sketch (illustrative)
- `cell f(x: T) effects(write x): ...`
- `pure cell g(...): ...`

---

## How this interacts with verification

Regardless of A/B/C:
- `requires/ensures` can specify pre/postconditions about ownership, regions, or effects.
- Loop `invariant` and `decreases` stay orthogonal.
- Effects/capabilities can be modeled in Z3 as uninterpreted predicates for lightweight checking.

## Recommendation framing (not a decision)

- If the goal is “maximum safety + familiar model”: Option A.
- If the goal is “explicit mutation authority + UI-friendly ergonomics”: Option B.
- If the goal is “API clarity + verification synergy”: Option C.
