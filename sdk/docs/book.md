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

## 5. Apps & UI (Lumina)

Aura’s current UI story is **Lumina** (Raylib backend in this repo).

### 5.1. Core idea

- The UI is rebuilt every frame from Aura code (`layout:` / `render:` blocks).
- Callbacks (`on_click: ~> { ... }`) run on the UI loop.

Rule: **Never block inside UI callbacks** (no long I/O, no waiting for stdin, no sleep).
If you block, the window will stop responding (mouse/keyboard appear “broken”).

### 5.2. Styling & colors

Style fields like `bg`, `fg`, and `color` accept:

- Named colors: `"Black"`, `"White"`, `"Gold"`, plus a small set like `"Gray"`, `"DarkGray"`, `"SkyBlue"`, etc.
- Hex colors: `"#RRGGBB"` or `"#RRGGBBAA"`
- CSS-like forms: `"rgb(r,g,b)"`, `"rgba(r,g,b,a)"`

Apps can also set an app background color:

- `App(title: "My App", bg: "#0B0F14") { ... }`

### 5.3. Input (current + planned)

Current prototype approach (AVM): staged, non-blocking console input for demos.

Planned (Lumina): real UI input widgets (TextInput), focus management, and event APIs
so apps can be fully mouse+keyboard driven without the terminal.
