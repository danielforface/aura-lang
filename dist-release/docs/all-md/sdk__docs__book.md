# The Aura Language (Book)

This is the living “book” for Aura.

Status: prototype (matches the current compiler in this repo).

## 1. Getting started

- Install the SDK (see `sdk/install.ps1` / `sdk/install.sh`).
- Run a file:
  - `aura run main.aura --mode avm`

## 2. Language basics

### 2.1. Files and imports

- Import a module name into scope:
  - `import std::io`
  - `import aura::tensor` (legacy path also accepted by SDK injection)

Imports are used by the SDK to auto-inject `sdk/std/<name>.aura` into the source bundle.

### 2.2. Bindings

- Immutable binding:
  - `val x: u32 = 1`
- Mutable binding:
  - `val mut i: u32 = 0`

### 2.3. Control flow

- `if` / `else`
- `while` with optional `invariant` and `decreases`
- `match` (requires a final `_` arm; wildcard must be last)

## 3. Safety model (current MVP)

Aura’s roadmap safety model is implemented in a conservative MVP form:

### 3.1. Mutation aliasing

Resource-like values (e.g. `Tensor`, `Model`, `Style`) are treated as non-copy.

- Binding from an identifier moves the resource.
- Assigning from an identifier moves the resource.

This prevents accidental aliasing of mutable resources.

### 3.2. Safe concurrency

- Async lambdas (`~> { ... }`) cannot capture mutable bindings from outer scopes.

This blocks a common class of data races by construction.

### 3.3. Unsafe/FFI boundaries

- Use `unsafe:` blocks to call untrusted externs.
- Use `trusted extern cell ...` to declare a trusted boundary.

## 4. Verification (Z3)

Aura supports verification annotations (when built with Z3 features):

- `requires` / `ensures`
- `assert` / `assume`
- `while ... invariant ... decreases ...`
- Quantifiers (`forall` / `exists`) with guardrails

See `sdk/docs/z3-gate.md` and `sdk/docs/verifier-guide.md`.
