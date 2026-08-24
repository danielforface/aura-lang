# Aura 1.0 (2026 Edition) Engineering Qualification Program â€” Baseline v1
*Normative Architecture, Capability Inventory, Safety Contract, Backend Qualification, and Release Gating*

---

## 1. Executive Mission & Target Position

### 1.1 The 1.0 Target Position ("The Third Way")
Aura 1.0 is engineered as an **aspirational target position** in modern systems programming:
1. **C/Zig-level runtime performance, predictability, and mechanical sympathy** (zero garbage collection pauses, deterministic memory layout, lean binary footprint, portable native C emission).
2. **Rust-level compile-time safety and concurrency bounds** (elimination of use-after-free, dangling references, and data races via scoped lexical regions and linear/affine ownership capabilities).
3. **Dafny/SPARK-level formal verification certainty** made ergonomic as a **default, real-time developer experience** (streaming SMT solver feedback, structured counterexamples, typed variable traces, and fully exposed trust boundaries).

> [!NOTE]
> Aura focuses on automated, SMT-driven program verification for systems code, rather than general interactive theorem proving (e.g. Lean or Coq). Aura preferentially targets tractable/decidable theory fragments (such as quantifier-free bitvectors `QF_BV` and linear integer arithmetic `QF_LIA`) for fast interactive feedback, while handling nonlinear arithmetic and quantifiers via bounded profiles and solver heuristics.

### 1.2 Monorepo Foundation vs. The 1.0 Qualification Mandate
Following the completion of PR #5 and PR #6:
- **Existing Baseline**: A unified 22-crate monorepo, Z3 SMT solver integration via static linking, an AVM development interpreter, C and LLVM code generators, a Tauri 2 desktop IDE (Sentinel), and 7 validated GitHub Actions CI workflows.
- **The 1.0 Mandate**: Transition from an "assemblage of capable subsystems" to a **qualified, specified, sound, and release-gated systems programming language**.

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚                                    AURA 1.0 QUALIFICATION STACK                                         â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚        SOURCE LANGUAGE        â”‚             PROOF GUARANTEE              â”‚       EXECUTION & RT          â”‚
â”‚ â€¢ Frozen 1.0 Grammar & AST    â”‚ â€¢ Z3 SMT Engine (Static Link)            â”‚ â€¢ Qualified C11/C23 Backend   â”‚
â”‚ â€¢ Refined Primitives & Structsâ”‚ â€¢ requires / ensures / assert / assume   â”‚ â€¢ AVM Development VM (REPL)   â”‚
â”‚ â€¢ Algebraic Enums & Match     â”‚ â€¢ Invariants, Decreases, Quantifiers     â”‚ â€¢ Experimental LLVM Backend   â”‚
â”‚ â€¢ Option B+ Scoped Regions    â”‚ â€¢ Structured Counterexample v2 (Ghost)   â”‚ â€¢ Zero-Overhead C Native RT   â”‚
â”‚ â€¢ Linear/Affine Capabilities  â”‚ â€¢ Multi-Dimensional Merkle Proof Cache   â”‚ â€¢ stdlib (Vec, Map, Net, IO)  â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚                                     DEVELOPER PLATFORM & TOOLING                                         â”‚
â”‚ â€¢ aura CLI (build, run, verify, test, fmt, lint)  â€¢ aura-pkg (Package Manager, Registry, Lockfiles)     â”‚
â”‚ â€¢ aura-lsp (LSP + Proof Streaming Protocol v1)    â€¢ Aura Sentinel IDE (Obligation Cockpit, Timelines)   â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

---

## 2. Phase 0: Ground-Truth Capability Inventory

Before freezing syntax or altering compiler passes, the engineering program establishes an **empirical, code-backed inventory** mapping every language feature across all 13 qualification dimensions.

### 2.1 Qualification Dimensions (The 13 Pipeline Stages)
1. **LEX**: Tokenized in [`aura-lex`](file:///c:/code/aura-lang/aura-lex)
2. **PARSE**: Parsed in [`aura-parse`](file:///c:/code/aura-lang/aura-parse)
3. **AST**: Represented in [`aura-ast`](file:///c:/code/aura-lang/aura-ast)
4. **SEMA**: Type-checked & validated in [`aura-core`](file:///c:/code/aura-lang/aura-core)
5. **IR**: Lowered to SSA Intermediate Representation in [`aura-ir`](file:///c:/code/aura-lang/aura-ir)
6. **VERIFY**: SMT-encoded & proved in [`aura-verify`](file:///c:/code/aura-lang/aura-verify)
7. **AVM**: Executed in development VM in [`aura-interpret`](file:///c:/code/aura-lang/aura-interpret)
8. **C**: Emitted to native C in [`aura-backend-c`](file:///c:/code/aura-lang/aura-backend-c)
9. **LLVM**: Emitted to LLVM IR in [`aura-backend-llvm`](file:///c:/code/aura-lang/aura-backend-llvm)
10. **LSP**: Exposed via language server in [`aura-lsp`](file:///c:/code/aura-lang/aura-lsp)
11. **DIAGNOSTICS**: Structured error code, span, explanation & actionable suggestion
12. **SPEC**: Normatively defined in language specification
13. **TESTS**: Covered by unit, integration, or differential test suite

### 2.2 Provisional Capability Matrix
*(To be replaced by the Phase 0 code-backed evidence artifact: `AURA_1_0_CAPABILITY_INVENTORY.md`)*

**Legend**:
- `âœ“` : confirmed code-backed
- `~` : partial / incomplete
- `âœ—` : confirmed absent
- `?` : not yet audited (pending Phase 0 audit)
- `â€”` : not applicable

| Feature Construct | LEX | PARSE | AST | SEMA | IR | VERIFY | AVM | C | LLVM | LSP | DIAG | SPEC | TESTS | 1.0 Target Disposition |
|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|---|
| **Numeric Primitives (`u8..u128`, `i8..i128`, `f32`, `f64`)** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Include & Freeze** |
| **Range Refinements (`T[lo..hi]`)** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Include & Freeze** |
| **Records / Structs (Named fields)** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Include & Monomorphize** |
| **Enums (Algebraic Sum Types)** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Qualify C Tagged Unions** |
| **Pattern Matching (`match`)** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Qualify Exhaustiveness** |
| **Generics & Type Parameters** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Monomorphize in C** |
| **Traits / Typeclasses** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Monomorphized Traits 1.0** |
| **Option B+ Scoped Regions** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Formalize & Qualify** |
| **Linear Ownership (`Owned`/`Consumed`)** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Formalize & Qualify** |
| **Flow Operators (`->` sync, `~>` async)** | âœ“ | âœ“ | âœ“ | ? | ? | ? | ? | ? | ? | ? | ? | âœ— | ? | **Standardize on Event Loop** |
| **Parser Macros (Substitution + Gensym)** | âœ“ | âœ“ | âœ“ | â€” | â€” | â€” | â€” | â€” | â€” | â€” | â€” | âœ— | ? | **DEFER from 1.0 Contract** |
| **Ad-hoc `prop:` statements** | âœ“ | âœ“ | âœ“ | ? | â€” | â€” | ? | â€” | â€” | ? | ? | âœ— | ? | **DEFER from 1.0 Core** |
| **Automatic AVM â†’ LLVM JIT Promotion** | â€” | â€” | â€” | â€” | â€” | â€” | âœ— | â€” | âœ— | â€” | â€” | âœ— | âœ— | **DEFER / Non-Goal for 1.0** |

---

## 3. Pillar 1: Freezing the Language Core (Phase 1)

### 3.1 1.0 Frozen Core Inclusions
1. **Exact-width Numeric Primitives**:
   - Unsigned: `u8`, `u16`, `u32`, `u64`, `u128`, `usize`
   - Signed: `i8`, `i16`, `i32`, `i64`, `i128`, `isize`
   - Floating-point: `f32`, `f64` (IEEE 754)
   - Boolean & Text: `bool`, `char` (Unicode scalar), `str` (UTF-8 slice), `String` (Owned UTF-8)
2. **Refinement Types**:
   - `T[lo..hi]` generating automatic SMT boundary obligations.
3. **Records & Algebraic Sum Types**:
   - Named record fields with default values.
   - Tagged enum variants carrying typed algebraic payloads.
4. **Monomorphized Generics & Bounded Traits**:
   - Parameterized types `Record<T>` and traits `trait Hash<T>` monomorphized into specialized C symbols (zero runtime vtable overhead).
5. **Deterministic Error Handling**:
   - Algebraic `Result<T, E>` and `Option<T>` with `?` early-return propagation operator.
   - No untracked exceptions.

### 3.2 Normative Panic & Non-Unwinding Termination Semantics
- **Aura 1.0 Panic Policy**: **Non-unwinding termination**.
- For recoverable language-level panic points (such as assertion failures or explicit `panic("msg")`), Aura may perform explicitly specified region-local cleanup before terminating the process.
- **No cross-frame exception-style stack unwinding** is part of Aura 1.0.
- Hard runtime failures where cleanup cannot be guaranteed (e.g. allocator corruption, OS faults) terminate immediately via `abort()`.

### 3.3 Explicit Deferral from the 1.0 Contract
- **Parser Macros**: The current parser-time macro expansion system (with lexical substitution and gensym renaming) is deferred and excluded from the frozen 1.0 language contract. Metaprogramming will be redesigned post-1.0 via hygienic procedural macro RFCs.
- **Ad-hoc `prop:` Statements**: Excluded from core language grammar; subsumed under dedicated Lumina UI plugin declarations.

---

## 4. Pillar 2: Normative Safety & Memory Model (Option B+)

Aura adopts **Safety Option B+: Scoped Memory Regions + Linear/Affine Capabilities**.

```
                                  AURA MEMORY & SAFETY STATE MACHINE
   â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
   â”‚                                                                                              â”‚
   â”‚                                â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”                                 â”‚
   â”‚                                â”‚           Owned           â”‚                                 â”‚
   â”‚                                â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜                                 â”‚
   â”‚                                              â”‚                                               â”‚
   â”‚                     â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”                      â”‚
   â”‚                     â”‚ Move / Transfer        â”‚ & (Borrow Imm)         â”‚ &mut (Borrow Mut)    â”‚
   â”‚                     â–¼                        â–¼                        â–¼                      â”‚
   â”‚              â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”          â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”          â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”               â”‚
   â”‚              â”‚  Consumed   â”‚          â”‚BorrowedImmutâ”‚          â”‚ BorrowedMut â”‚               â”‚
   â”‚              â”‚   (Moved)   â”‚          â””â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”˜          â””â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”˜               â”‚
   â”‚              â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜                 â”‚ Scope End              â”‚ Scope End            â”‚
   â”‚                                              â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜                      â”‚
   â”‚                                                          â–¼                                   â”‚
   â”‚                                               [Restored to Owned]                            â”‚
   â”‚                                                          â”‚                                   â”‚
   â”‚                                                          â–¼ Region Exit                       â”‚
   â”‚                                                   â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”                            â”‚
   â”‚                                                   â”‚   Dropped   â”‚                            â”‚
   â”‚                                                   â”‚ (Destructed)â”‚                            â”‚
   â”‚                                                   â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜                            â”‚
   â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

### 4.1 Normative Specification Checklist (The 16 Safety Invariants)
Prior to declaring safety qualification, the language specification must normatively define:
1. **Reborrowing Rules**: Allowing temporary sub-borrows from `&mut T` that suspend the parent borrow until the child scope closes.
2. **Aliasing Invariant**: At any program point, a memory location may have *either* multiple shared `&T` references *or* exactly one exclusive `&mut T` reference, but never both.
3. **Nested Regions (`region 'outer { region 'inner { ... } }`)**: References allocated in `'inner` cannot be assigned, returned, or stored into `'outer`.
4. **Reference Escape Prevention**: Function return types cannot return references to stack-local variables or expired regions.
5. **Interior Mutability**: Restricted exclusively to atomic primitives (`Atomic<T>`) and mutex abstractions (`Mutex<T>`).
6. **Raw Pointer Semantics**: `*const T` and `*mut T` are valid only inside explicit `unsafe:` blocks.
7. **Conservative FFI Aliasing Default**: Foreign raw pointers are **conservatively assumed to potentially alias**. No unique/exclusive Aura borrow may be derived from them unless an explicit FFI contract establishes the required aliasing guarantee.
8. **Deterministic Destructor Ordering**: Destruction occurs in strict reverse lexical declaration order at region exit.
9. **Partial Moves**: Moving fields out of composite records is forbidden unless the entire record is consumed.
10. **Moves Out of Enums**: Prohibited without replacing the enum variant (e.g. `std::mem::replace`).
11. **Closure Capture**: Distinguishing by-value `move` capture from by-reference borrow capture.
12. **Async (`~>`) Capture**: Asynchronous tasks must own their environment (`'static` or region-bounded lifetime).
13. **Task Ownership Transfer**: Only types satisfying the `Send` capability can cross thread/task boundaries.
14. **Panic During Drop**: Triggers an immediate abort (`SIGABRT` / `abort()`); nested drops during panic are forbidden.
15. **Cycles & Self-Referential Data**: Heap cycles must use weak references or explicit arena indices.
16. **Subtyping & Variance**: Covariant over immutable region references `&'a T`, invariant over `&mut T`.

### 4.2 Concurrency Safety Bounds (Scoped Concurrency)
- Aura 1.0 guarantees **compile-time data-race freedom over the structured concurrency and Send/Sync subset**.
- Rather than claiming general mathematical deadlock freedom over arbitrary dynamic locking topologies, Aura enforces:
  - Static detection of cyclic lock ordering across statically declared lock graphs.
  - Compile-time prevention of data races via linear capability isolation and `Send`/`Sync` trait bounds.

### 4.3 Explicit TCB & Trusted Core Auditing
- **`unsafe:` Blocks**: Required for raw pointer arithmetic, unverified casts, and untrusted foreign calls.
- **`extern cell` (Untrusted)**: Foreign declarations callable only within `unsafe:`.
- **`trusted extern cell` (Audited TCB)**: Verified/audited foreign implementations admitted into safe code, automatically producing an audit entry in the **Trusted Core Report**.

---

## 5. Pillar 3: Verification as a Normative Language Guarantee

```
                             AURA VERIFICATION & PROOF PIPELINE
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”       â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”       â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”       â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚   Aura Source   â”‚       â”‚   Verification  â”‚       â”‚  SMT Encoding   â”‚       â”‚   Z3 Solver     â”‚
â”‚   â€¢ requires    â”‚ â”€â”€â”€â”€> â”‚   Conditions    â”‚ â”€â”€â”€â”€> â”‚   â€¢ Formatted   â”‚ â”€â”€â”€â”€> â”‚   â€¢ Incremental â”‚
â”‚   â€¢ ensures     â”‚       â”‚   â€¢ Weakest     â”‚       â”‚     Theories    â”‚       â”‚   â€¢ Scoped Push â”‚
â”‚   â€¢ assert      â”‚       â”‚     Precond.    â”‚       â”‚   â€¢ Unsat Cores â”‚       â”‚   â€¢ Warm Cache  â”‚
â”‚   â€¢ invariant   â”‚       â”‚   â€¢ Frame Axiomsâ”‚       â”‚   â€¢ Bitvectors  â”‚       â”‚                 â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜       â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜       â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜       â””â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                                                                                       â”‚
                               â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”
                               â–¼                                                              â–¼
                     â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”                                           â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
                     â”‚   UNSAT (PROVED) â”‚                                           â”‚   SAT / UNKNOWN   â”‚
                     â”‚ â€¢ Cache Merkle   â”‚                                           â”‚ â€¢ Extract Model   â”‚
                     â”‚   Key in Cache   â”‚                                           â”‚ â€¢ Map AST Symbols â”‚
                     â”‚ â€¢ Emit Pass Note â”‚                                           â”‚ â€¢ Emit Counterex  â”‚
                     â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜                                           â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

### 5.1 Normative Semantics of `verify`
1. **The Meaning of UNSAT**:
   `UNSAT` from the solver establishes that the verification condition is mathematically valid under all possible variable valuations conforming to declared preconditions, **under Aura's formally specified SMT encoding and the trusted assumptions of the selected solver and theories (TCB)**.
2. **Incompleteness & Timeout Rule**:
   A solver `TIMEOUT` or `UNKNOWN` is **NEVER a passing result**. It is classified as an **Unproved Obligation (`V0102`)**, failing CI and release builds.
3. **Assumptions (`assume:`)**:
   Explicit postulates are highlighted in compiler output and recorded in the module verification manifest.
4. **Framing & Separation**:
   Functions state their modification scope via `modifies [...]`. State locations outside the `modifies` clause are proved invariant across function calls.
5. **Ghost State**:
   `ghost` variables, fields, and lemmas exist solely for inductive proofs and are completely erased during IR lowering and C emission.

### 5.2 Cryptographic Multi-Dimensional Proof Cache Key
To eliminate false cache hits, the Merkle proof cache key is computed as:

$$\text{ProofKey} = \text{SHA256}\Big(\text{Edition} \parallel \text{CompilerVersion} \parallel \text{NormalizedAST} \parallel \text{ContractHashes} \parallel \text{TypeLayouts} \parallel \text{StdlibHash} \parallel \text{TCBManifest} \parallel \text{TargetTriple} \parallel \text{FeatureFlags} \parallel \text{SMTEncodingVersion} \parallel \text{SolverVersion} \parallel \text{SolverParams} \parallel \text{VerifyProfile}\Big)$$

---

## 6. Pillar 4: Production-Grade Qualification of Backend C

### 6.1 Backend Status Hierarchy
- **Production Baseline (1.0)**: `aura-backend-c` (Emitting portable ISO C11 with optional C17/C23 compiler features).
- **Experimental Path**: `aura-backend-llvm` (Feature-gated `--backend llvm`).
- **Reference / Development VM**: `aura-interpret` (AVM for REPL and differential oracle).

```
             Aura Source Semantics
                       â”‚
               â”Œâ”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”
               â”‚                â”‚
              AVM              Aura IR
               â”‚                â”‚
               â””â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”˜
                       â”‚ Differential Parity Gate
                       â–¼
                  C Backend
                       â”‚
               GCC / Clang / MSVC
```

### 6.2 The Comprehensive Qualification Matrix
The 1.0 backend qualification requires passing a multi-dimensional test corpus:
1. **Deterministic Functional Fixtures**: 5,000+ targeted language feature programs.
2. **Differential Parity Suite**: Bit-for-bit equivalence of `stdout`, `stderr`, and exit code across $\text{AVM} \equiv \text{Aura IR} \equiv \text{Compiled C Native}$.
3. **Continuous Fuzzing**: Grammar-based fuzzing (AFL++ / libFuzzer) exercising parser, semantic analyzer, and IR optimizations.
4. **Memory & Concurrency Sanitizers**: AddressSanitizer (ASan), UndefinedBehaviorSanitizer (UBSan), and ThreadSanitizer (TSan) executed on all C backend binaries.
5. **ABI & Struct Layout Tests**: Validating byte alignment, padding, and foreign struct interop with standard C compilers.
6. **Unicode & Numerical Edge Corpus**: UTF-8 boundary slices, integer overflow limits, and floating-point subnormals.

---

## 7. Pillar 5: Standard Library (`aura-stdlib`) & Runtime Stabilization

```
                                AURA 1.0 STANDARD LIBRARY MAP
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚                                           aura-stdlib                                           â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚    std::core      â”‚    std::coll      â”‚     std::io       â”‚     std::net      â”‚    std::sync    â”‚
â”‚ â€¢ Primitives      â”‚ â€¢ Vec<T>          â”‚ â€¢ File, Path      â”‚ â€¢ TcpStream       â”‚ â€¢ Mutex<T>      â”‚
â”‚ â€¢ Option, Result  â”‚ â€¢ HashMap<K, V>   â”‚ â€¢ BufReader       â”‚ â€¢ TcpListener     â”‚ â€¢ RwLock<T>     â”‚
â”‚ â€¢ Range, Iterator â”‚ â€¢ RingBuffer<T>   â”‚ â€¢ BufWriter       â”‚ â€¢ UdpSocket       â”‚ â€¢ Channel<T>    â”‚
â”‚ â€¢ Drop, Clone     â”‚ â€¢ BTreeMap<K, V>  â”‚ â€¢ Stdin, Stdout   â”‚ â€¢ DnsResolver     â”‚ â€¢ Atomic<T>     â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚    std::string    â”‚    std::time      â”‚    std::process   â”‚    std::serde     â”‚    std::crypto  â”‚
â”‚ â€¢ String (Owned)  â”‚ â€¢ Instant         â”‚ â€¢ Command         â”‚ â€¢ JsonValue       â”‚ â€¢ Sha256        â”‚
â”‚ â€¢ Str (Zero-Copy) â”‚ â€¢ Duration        â”‚ â€¢ Child, Output   â”‚ â€¢ Serialize       â”‚ â€¢ Ed25519       â”‚
â”‚ â€¢ Utf8Validator   â”‚ â€¢ SystemClock     â”‚ â€¢ Env, Args       â”‚ â€¢ Deserialize     â”‚ â€¢ Random        â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

### 7.1 Core Modules & Invariants
1. **`std::string` & `std::str`**:
   - Zero-copy `&str` slices and owned `String` buffers with verified UTF-8 validation contracts.
2. **`std::collections`**:
   - `Vec<T>`, `HashMap<K, V>`, `RingBuffer<T>`, and `BTreeMap<K, V>` with region-aware allocation and linear access.
3. **`std::io` & `std::fs`**:
   - Buffered reading/writing, path resolution, and deterministic flush contracts.
4. **`std::net`**:
   - Non-blocking TCP/UDP sockets integrated with `~>` async flow.
5. **`std::sync` & `std::time`**:
   - `Mutex<T>`, `RwLock<T>`, `Channel<T>`, `Atomic<T>`, and monotonic high-resolution clocks (`Instant`).
6. **`std::serde`**:
   - Safe, verified JSON, TOML, and binary serialization.

---

## 8. Pillar 6: Unified Ecosystem & Release Tooling

### 8.1 Unified Package Manager (`aura-pkg`)
- **Manifest (`Package.toml`)**: Standardized declarative package manifest.
- **Lockfile (`aura.lock`)**: Cryptographically pinned dependency graph with package checksums.
- **Registry Infrastructure**: Secure HTTPS/Git registry index with Ed25519 package signing. Host configuration decoupled until official domain provisioning.

### 8.2 Canonical IDE Integrations
- **Official VS Code Extension (`aura-sentinel-vscode`)**: Syntax highlighting, diagnostics, ghost-text counterexamples.
- **Sentinel Desktop IDE (`aura-sentinel-app`)**: Tauri 2 + CodeMirror 6 dedicated desktop verification environment.

### 8.3 Release Channels
- **Release Cadence**: Gate-driven qualification progression (Nightly $\to$ Beta $\to$ Stable).

---

## 9. Pillar 7: Empirical Performance & Verification Benchmarking

### 9.1 Benchmark Methodology & Performance Hypotheses
All performance claims must derive from transparent, reproducible measurements rather than pre-mature marketing claims.

| Dimension | Initial Performance Hypothesis (Target) | Evaluation Methodology | Baseline Comparison |
|---|---|---|---|
| **Compilation Speed** | $> 40,000$ lines/sec | C backend, release build, warm disk cache | Compare vs `gcc`, `clang`, `rustc` |
| **Interactive Proof Latency** | P95 $< 150\text{ms}$ | Warm Merkle cache, `fast` profile, editing session | SMT cache hit measurement |
| **Cold Proof Latency** | Sub-second for standard modules | Cold cache, `ci` profile, 500 LOC module | Compare vs Dafny on identical obligations |
| **Runtime Execution** | Within $0.95\times - 1.05\times$ of native C | Computer Language Benchmarks Game suite | Idiomatic C (GCC -O3) & Rust |
| **Binary Footprint** | $< 30\text{KB}$ | Minimal hello-world binary on C backend | Compare vs Zig / C11 |
| **Compiler Peak RSS** | $< 150\text{MB}$ | Compiling 50,000 LOC project | Resource monitoring |

---

## 10. Pillar 8: Sentinel as the Competitive Moat (Productizing Trust)

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚                                        AURA SENTINEL TRUST COCKPIT                                      â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚                 EDITOR & GHOST-TEXT               â”‚                   TRUST ACCOUNTING                  â”‚
â”‚ 1 cell safe_divide(x: u32, y: u32) -> u32:        â”‚ â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â” â”‚
â”‚ 2     requires y != 0                             â”‚ â”‚ PROOF OBLIGATIONS:                              â”‚ â”‚
â”‚ 3     ensures result * y <= x                     â”‚ â”‚ â€¢ Proved:          1,846 obligations            â”‚ â”‚
â”‚ 4                                                 â”‚ â”‚ â€¢ Unproved:           12 obligations            â”‚ â”‚
â”‚ 5     x / y                                       â”‚ â”‚ â€¢ Timeout:             0 obligations            â”‚ â”‚
â”‚                                                   â”‚ â”‚ â€¢ Assumed:             2 obligations            â”‚ â”‚
â”‚ [PASS: Proved in 18ms via Z3 QF_BV (Cache: HIT)]  â”‚ â”‚ TRUST SURFACE:                                  â”‚ â”‚
â”‚                                                   â”‚ â”‚ â€¢ unsafe blocks:       0 blocks                 â”‚ â”‚
â”‚                                                   â”‚ â”‚ â€¢ trusted extern:      2 functions              â”‚ â”‚
â”‚                                                   â”‚ â”‚ â€¢ backend parity:      PASS (AVM == C)          â”‚ â”‚
â”‚                                                   â”‚ â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜ â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚                               COUNTEREXAMPLE TIMELINE & DIAGNOSTIC INSPECTOR                            â”‚
â”‚ â”€â”€â—â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â—â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â—â”€â”€â”€â”€ â”‚
â”‚ Step 1: Input Bindings                            Step 2: Loop Invariant                          Step 3â”‚
â”‚ [x = 100, y = 0]                                  [i = 0, sum = 0]                                [Viol]â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

### 10.1 Trust Accounting by Obligations
Sentinel displays precise obligation telemetry:
- **Proof Obligations**: Count of `Proved`, `Unproved`, `Timeout`, and `Assumed`.
- **Trust Surface**: Count of `unsafe:` blocks, `trusted extern` declarations, and raw FFI boundaries.
- **Backend Parity**: Live status of the AVM $\leftrightarrow$ C backend equivalence tests.
- **Interactive Counterexample Timeline**: Step-by-step debugger stepping through SMT-synthesized counterexample states.

---

## 11. Pillar 9: Strategic Interoperability Roadmap

1. **Phase 1 (1.0 Baseline): High-Performance C/C++ FFI**:
   - `aura bindgen` parsing C headers via libclang with inferred refinement bounds (`const char*` $\to$ `&str`).
   - Native C ABI calling convention (`extern "C"`).
2. **Phase 2 (1.1 Fast Follow): Native Python Ecosystem Bridge**:
   - Direct CPython C-API bindings enabling zero-copy tensor sharing (NumPy / PyTorch) with Aura's `Tensor` types for the AI/ML ecosystem.
3. **Phase 3 (Post-1.0 Future): Heterogeneous Compute & GPU Kernels**:
   - MLIR / SPIR-V integration for compiled GPU compute shaders.

---

## 12. Pillar 10: 1.0 Governance & Compatibility Contract

### 12.1 The Normative Stability Promise
> **Valid Aura 2026 source code remains source-compatible throughout the 1.x series, except where a documented correctness or security defect requires a narrowly scoped compatibility exception. Specified language semantics remain stable within the Edition. Unspecified behavior is not covered by the compatibility guarantee.**

### 12.2 Edition System & Evolution
- Epoch-based editions (`edition = "2026"`, `edition = "2029"`).
- Syntax evolution occurs across editions while maintaining seamless inter-crate dependency compatibility.
- Public RFC process (`rfcs/`) for community proposals and Core Team consensus.

### 12.3 Target Platform Tier Matrix

| Tier | Guarantee Level | Target Architectures / Platforms |
|---|---|---|
| **Tier 1** | Fully Automated CI Gated (All PRs must pass) | `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-apple-darwin` |
| **Tier 2** | Automated Release Builds & Smoke Tested | `aarch64-linux-android`, `armv7-linux-androideabi`, `wasm32-wasip1`, `x86_64-unknown-linux-musl` |
| **Tier 3** | Best-Effort Community Maintained | `riscv64gc-unknown-linux-gnu`, `wasm32-unknown-unknown`, `arm-none-eabi` |

---

## 13. Phased Implementation Roadmap (Gate-Driven)

Progress between phases is strictly **gate-driven** based on acceptance exit criteria rather than calendar deadlines.

```
PHASE 0: Capability Inventory & Baseline Extraction
  â”‚ (Audit 22 crates, build ground-truth pipeline matrix: AURA_1_0_CAPABILITY_INVENTORY.md)
  â–¼
PHASE 1: Core Language Freeze & Normative Specification
  â”‚ (Freeze primitives, structs, ADTs, match, Monomorphized generics; defer macros)
  â–¼
PHASE 2: Safety & Memory Contract (Option B+)
  â”‚ (16 safety invariants, regions, linear capabilities, scoped concurrency)
  â–¼
PHASE 3: Verification Semantics & Proof Cache
  â”‚ (UNSAT/TIMEOUT rules, framing, ghost state, cryptographic Merkle cache key)
  â–¼
PHASE 4: Production C Backend Qualification
  â”‚ (C11/C23 emitter, 5,000+ differential corpus, fuzzing, ASan/UBSan)
  â–¼
PHASE 5: Standard Library & Native Runtime
  â”‚ (std::core, collections, io, net, sync, serde, zero-overhead C runtime)
  â–¼
PHASE 6: Package Manager & Official Ecosystem
  â”‚ (aura-pkg, lockfiles, signed registry, VS Code extension, desktop Sentinel)
  â–¼
PHASE 7: Sentinel Trust Platform & Cockpit
  â”‚ (Obligation accounting, proof DAG, counterexample debugger timeline)
  â–¼
PHASE 8: Empirical Benchmarks & Platform Tier Matrix
  â”‚ (Compiler speed, proof latency, runtime shootout benchmarks, Tier 1/2 gates)
  â–¼
RELEASE CANDIDATE 1 (RC1)
  â”‚ (Bug bounty, security audit, conformance suite freeze)
  â–¼
AURA 1.0 (2026 EDITION) GENERAL AVAILABILITY (GA)
```

---

## 14. Comprehensive Multi-Dimensional Verification Matrix

Prior to declaring Aura 1.0 GA, the qualification program enforces this full multi-dimensional test matrix across continuous integration:

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚                                   1.0 QUALIFICATION VERIFICATION MATRIX                                 â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ Configuration Modes            â”‚ â€¢ Core / --no-default-features                                         â”‚
â”‚                                â”‚ â€¢ Core / default features                                              â”‚
â”‚                                â”‚ â€¢ Z3 static-link feature path                                          â”‚
â”‚                                â”‚ â€¢ C Backend (C11/C23) native compilation                               â”‚
â”‚                                â”‚ â€¢ LLVM experimental feature path                                       â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ Subsystem Test Suites          â”‚ â€¢ Rust workspace tests (22 crates)                                     â”‚
â”‚                                â”‚ â€¢ SMT verification suite & Merkle cache hit verification               â”‚
â”‚                                â”‚ â€¢ LSP & Debugger protocol integration suite                            â”‚
â”‚                                â”‚ â€¢ Sentinel Tauri 2 desktop integration & UI test suite                 â”‚
â”‚                                â”‚ â€¢ Android NDK cross-compilation & APK build suite                      â”‚
â”‚                                â”‚ â€¢ Website production static export & encoding hygiene                  â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ Differential & Robustness      â”‚ â€¢ 5,000+ differential tests (AVM == IR == C)                           â”‚
â”‚                                â”‚ â€¢ Continuous libFuzzer/AFL++ (Parser, Sema, IR)                        â”‚
â”‚                                â”‚ â€¢ Memory Sanitizers: ASan, UBSan, TSan                                 â”‚
â”‚                                â”‚ â€¢ ABI & Struct Layout conformance corpus                               â”‚
â”‚                                â”‚ â€¢ Unicode UTF-8 & Numerical overflow boundary tests                    â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ Platform Target Matrix         â”‚ â€¢ Windows (x86_64-pc-windows-msvc)                                     â”‚
â”‚                                â”‚ â€¢ Linux (x86_64-unknown-linux-gnu, musl, aarch64)                      â”‚
â”‚                                â”‚ â€¢ macOS (x86_64-apple-darwin, aarch64 Apple Silicon)                   â”‚
â”œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¤
â”‚ Release & Distribution Quality â”‚ â€¢ Portable SDK fresh-install verification                              â”‚
â”‚                                â”‚ â€¢ Clean-clone developer onboarding verification                        â”‚
â”‚                                â”‚ â€¢ Cryptographic attestation and SHA-256 manifest audit                 â”‚
â”‚                                â”‚ â€¢ Trusted Core Report zero-unaccounted-FFI enforcement                 â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```
