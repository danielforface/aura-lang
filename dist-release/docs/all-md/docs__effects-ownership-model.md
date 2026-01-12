# Effects / Ownership / Lifetime Model (Draft v0.3)

This document defines Aura’s **safety model** at a language level: how values are owned, when aliasing is allowed, how mutation is controlled, what it means to be concurrency-safe, and where unsafe/trusted boundaries live.

This is intended to satisfy the roadmap items under **“Effects / ownership / lifetime model”**.

## Chosen model (coherent default)

Aura uses an **ownership + borrowing inspired model** (Option A) with an explicit notion of **effects** for operations that can mutate or perform I/O.

- **Ownership** controls who is responsible for a value’s lifetime.
- **Borrowing** controls aliasing (who can access a value and how).
- **Effects** make “this code can mutate / do I/O / call FFI” explicit in signatures and at trust boundaries.

This hybrid keeps the mental model familiar while remaining compatible with a verifier: aliasing constraints are structural and effects are explicit.

## Core rules

### 1) Ownership

- Every value has a single *owner*.
- When an owning binding goes out of scope, the value is dropped.
- **Move semantics**: assigning/passing an owned value transfers ownership (the source can’t be used after the move).
- “Copy” types may be implicitly copied (small scalar-like types).

### 2) Borrowing (aliasing control)

Aura has two borrow forms:

- **Shared borrow** `&T` (read-only)
- **Mutable borrow** `&mut T` (read-write)

Borrowing rules (the invariant):

- Any number of shared borrows may coexist, **or** exactly one mutable borrow may exist.
- You cannot mutate through a shared borrow.
- While a mutable borrow is live, the owner cannot be accessed except through that mutable borrow.

This guarantees the classic aliasing property:

- No `&mut` aliases with any other reference.

### 3) Lifetimes / regions (implicit, checked)

- Borrows have **lifetimes** bounded by scopes.
- The compiler infers lifetimes for most code.
- A borrow cannot outlive the owned value it refers to.

Implementation note: this can be represented as region constraints during typechecking.

## Sound aliasing rules for mutation

Mutation is allowed only when **uniqueness** is ensured.

A program is accepted only if for every mutation through `&mut T`:

- The underlying location is not reachable by any other live reference.
- No shared borrows overlap the mutable borrow’s lifetime.

If Aura supports interior mutability (future):

- It must be explicitly wrapped (e.g. `Cell<T>`, `RefCell<T>`, `Mutex<T>`), and the wrapper defines the synchronization/aliasing contract.

## Effects system (explicit capabilities)

Aura tracks a small set of effects as part of function types. Minimal starting set:

- `pure`: no mutation, no I/O, no FFI
- `mut`: may mutate through `&mut`
- `io`: may perform I/O
- `ffi`: may call foreign code
- `unsafe`: may perform unchecked operations / assume invariants

Rules:

- Calling a function requires the caller to permit its effects.
- `pure` code can call only `pure` code.
- `unsafe` is an explicit boundary.

This is a spec-level contract; exact syntax is intentionally deferred.

## Safe concurrency story (no data races)

Aura prevents data races by construction:

- Shared data across threads must be **immutable** (`&T` where `T` is thread-safe), **or** protected by explicit synchronization.
- Mutable shared state must be behind a concurrency primitive (e.g. `Mutex<T>`, channels), which enforces exclusive access.

Type-level constraints (direction):

- Types that are safe to send/share across threads satisfy traits like `Send` / `Sync` (names TBD).
- `&mut T` is never simultaneously accessible from multiple threads.

If the runtime provides green threads / async, the same data-race rule applies at the task boundary.

## Explicit FFI boundaries (trusted modules / unsafe blocks)

FFI is treated as a **trust boundary**.

- Foreign declarations must live in an `extern` block.
- Calling foreign functions requires `unsafe`/`ffi` effect permission.
- Types crossing FFI boundaries must be explicitly marked as ABI-safe (e.g. fixed layout, no managed references).

Trusted modules (direction):

- A module can be declared `trusted` (or similar), meaning its internal invariants are assumed by the verifier.
- The IDE can surface “trusted vs proven” regions using this metadata.

## What this enables

- A crisp aliasing story for the verifier: mutable access is unique.
- Clean “no data races” guarantee for concurrency.
- Explicit boundaries for unverified code (FFI / unsafe).

## Open questions (intentionally deferred)

- Exact surface syntax for effects in function signatures.
- Whether lifetimes are always inferred or can be named.
- Whether/which interior mutability primitives are built-in.
- The formal trait system for concurrency (`Send`/`Sync` analogs).
