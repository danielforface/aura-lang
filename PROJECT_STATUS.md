# Aura Project Status

This file is the public status reconciliation layer for Aura. It exists because the repository contains multiple version namespaces and historical milestone documents that should not be collapsed into a single marketing version.

**Audit date:** 2026-08-24  
**Audit basis:** `main@74e0708ac33e622ffbb512c7baba68296c38be0b`  
**PR #5 validation head:** `3d1eed1e33b24de34638615ba3d78e8abefbbbf4`

## Executive status

Aura is an **active, pre-stable programming-language platform** with a substantial implementation across compiler frontend, semantic analysis, IR, verification, development runtime, native-oriented backends, package tooling, LSP/IDE integration, UI plugins, SDK/release tooling, examples, and Android support.

The repository is not accurately described by any one of these phrases alone:

- “v0.2 compiler” — too small; many subsystems are beyond a minimal compiler.
- “v1.0 stable language” — too strong; the workspace and primary CLI remain `0.2.0` and documented semantics are still evolving.
- “formally verified compiler” — not established by the repository.
- “prototype only” — too weak; multiple subsystems have meaningful implementation and tests.

The public label used by the wrapper is therefore:

> **Aura 2026 Edition — active, pre-stable language platform**

## Version reconciliation

| Surface | Version/state observed | Interpretation |
|---|---:|---|
| Workspace package version | `0.2.0` | primary Rust workspace version |
| `aura` CLI crate | `0.2.0` | compiler/orchestration executable version |
| Aura Sentinel app | `0.2.0` | desktop IDE package version |
| `aura-pkg` crate | `1.0.0` | package-manager component version, not whole-language GA |
| Language edition | `2026` concept | syntax/feature compatibility namespace |
| Historical status doc | `v0.3 complete` | milestone language, not workspace package version |
| Historical v1.0 docs | phase/reliability/completion language | roadmap or subsystem milestone evidence; not treated as unified stable release |

A future repository-wide release should explicitly reconcile these namespaces before advertising `Aura v1.0` as a stable language release.

## Implementation matrix

### Language frontend

| Capability | Status | Evidence surface |
|---|---|---|
| indentation-aware lexing | CODE-BACKED | `aura-lex`, SDK reference |
| parsing | CODE-BACKED | `aura-parse` |
| AST | CODE-BACKED | `aura-ast` |
| semantic/lowering core | CODE-BACKED | `aura-core` |
| Aura IR | CODE-BACKED | `aura-ir` |
| edition / feature gate plumbing | CODE-BACKED | CLI + parse config |
| advanced compatibility guarantees | PRE-STABLE | edition docs exist; stable language contract not yet cut |

### Current language/reference surface

| Capability | Status | Notes |
|---|---|---|
| `val`, `val mut` | REFERENCE-BACKED | current SDK reference |
| `cell`, `extern cell`, `trusted extern cell` | REFERENCE-BACKED | current SDK reference |
| `if`, `match`, `while` | REFERENCE-BACKED | current SDK reference |
| `requires`, `ensures`, `assert`, `assume` | REFERENCE-BACKED | verifier guide/reference |
| loop invariants | REFERENCE-BACKED | verifier guide |
| `u32` range refinements | REFERENCE-BACKED | `u32[lo..hi]` |
| `u32`, `bool`, `String`, `Unit` | REFERENCE-BACKED | current built-ins |
| Tensor/Model/Style resource-like types | REFERENCE-BACKED | current reference |
| sync/async flow operators `->`, `~>` | REFERENCE-BACKED | current reference |
| UI `layout:` / `render:` blocks | REFERENCE-BACKED | current reference |
| broad Rust-equivalent type system | NOT CLAIMED | current reference is narrower |

### Verification and safety

| Capability | Status | Notes |
|---|---|---|
| Z3-backed verify path | CODE-BACKED / FEATURE-GATED | `aura-verify`; `aura --features z3 verify` |
| SMT profiles fast/ci/thorough | CODE-BACKED | CLI |
| optional incremental solver mode | CODE-BACKED | `AURA_Z3_INCREMENTAL=1` path documented |
| structured counterexample mapping | CODE-BACKED / PROTOCOL-BACKED | verifier + LSP protocol |
| proof streaming to IDE | CODE-BACKED / PROTOCOL-BACKED | `aura-lsp`, Z3 Gate |
| proof summaries | CODE-BACKED | `aura-verify` |
| ownership/move analysis modules | CODE-BACKED | `aura-core`, `aura-verify` |
| capability enforcement modules | CODE-BACKED | `aura-core` |
| race detector module | CODE-BACKED | `aura-core` |
| current MVP resource-move rule | REFERENCE-BACKED | SDK reference |
| “zero segfaults” guarantee | TARGET / NOT CLAIMED | historical v1.0 target, not a current universal guarantee |
| formally verified compiler | NOT ESTABLISHED | do not claim |

### Execution and backends

| Capability | Status | Notes |
|---|---|---|
| AVM / Dev-VM | CODE-BACKED | `aura-interpret` |
| C-oriented backend | CODE-BACKED | `aura-backend-c`; default CLI backend |
| LLVM IR backend | CODE-BACKED / EVOLVING | feature-gated; source calls it a skeleton/evolving phase |
| WASI-oriented build path | CLI-BACKED | exposed as `wasm` backend path through toolchain/clang |
| native runtime support | CODE-BACKED | `aura-rt`, `aura-rt-native` |
| hybrid mode selector | CODE-BACKED | CLI exposes it |
| automatic AVM → LLVM promotion | NOT IMPLEMENTED | explicit source comment |
| production JIT | NOT CLAIMED | hybrid must not be marketed as JIT today |

### Developer tooling

| Capability | Status | Notes |
|---|---|---|
| `build`, `run`, `verify` | CODE-BACKED | primary CLI |
| `test`, `lint`, `fmt`, `init` | CODE-BACKED | primary CLI |
| C/C++ bindgen bootstrap | CODE-BACKED | primary CLI + bridge |
| LSP | CODE-BACKED | `aura-lsp` |
| Aura protocol v1 | PROTOCOL-BACKED | LSP stability doc |
| Aura Sentinel desktop app | CODE-BACKED | Tauri 2 + CodeMirror |
| VS Code integrations | CODE-BACKED | `editors/` |
| debugger protocol layers | CODE-BACKED | Dev-VM + LSP debugger modules |
| deterministic SDK packaging | CODE-BACKED | `tools/release/release.py` |

### Package ecosystem

| Capability | Status | Notes |
|---|---|---|
| package manager crate | CODE-BACKED | substantial `aura-pkg` implementation |
| init/add/remove/list/publish/verify | CODE-BACKED | standalone `aura-pkg` CLI |
| integrated add/publish/deprecate | CODE-BACKED | main `aura` CLI package surface |
| cache / resolver / lockfile | CODE-BACKED | package modules |
| SHA-256 integrity | CODE-BACKED | dependencies and package code |
| optional Ed25519 signing | CODE-BACKED | signing/security modules |
| every command shown in package guide | NOT ESTABLISHED | docs currently exceed executable command surface |
| public registry SLA / availability | NOT CLAIMED | requires separate operational evidence |

### Lumina / applications

| Capability | Status | Notes |
|---|---|---|
| Lumina plugin | CODE-BACKED | `aura-plugin-lumina` |
| Raylib-backed feature | CODE-BACKED | default Aura feature routes to Lumina/Raylib |
| UI examples | CODE-BACKED | examples + docs |
| TextInput / Box / grid/media work | CODE-BACKED / EVOLVING | recent commits and plugin source |
| stable cross-platform UI framework contract | NOT CLAIMED | evolving plugin |

### Android

| Capability | Status | Notes |
|---|---|---|
| SDK/NDK setup tooling | CODE-BACKED | `sdk/android`, scripts |
| sample APK build workflow | WORKFLOW-BACKED | Android GitHub workflow |
| `aura-rt` aarch64 cross-build path | WORKFLOW-BACKED | Android workflow |
| `aura-rt` armv7 cross-build path | WORKFLOW-BACKED | Android workflow |
| APK emulator/build helper tooling | CODE-BACKED | current tree/latest main commit |
| universal production Android support | NOT CLAIMED | needs broader end-to-end release evidence |

## Performance claims

Historical planning and completion docs use a `<200ms` proof-latency objective. The repository also contains caching, profiling and performance-tuning implementations.

For public communication:

- `<200ms` should be presented as a **latency target/SLO** unless accompanied by a reproducible benchmark artifact for the exact workload and commit.
- no README badge or headline should imply a universal P95 result without current benchmark evidence.
- performance dashboards and tuning code are evidence that the project engineers for latency; they are not themselves measurements.

## CI status and evidence integrity

Following the CI architecture refresh (PR #5), the repository enforces a 7-workflow automated validation matrix on GitHub Actions:

| Workflow | Scope / Gate | Result (PR #5 / Audit Basis) |
|---|---|---|
| **Core CI** | Workspace tests, portable Z3 feature path, compatibility suite, trusted-core audit | PASS |
| **Backend Matrix** | IR oracle, C backend, LLVM backend, AVM interpreter, CLI build profiles | PASS |
| **Android** | Rust Android cross-compilation (aarch64/armv7) & sample APK build | PASS |
| **LSP / Debugger** | LSP smoke/trace test suite, debugger fixture smoke & variable trace | PASS |
| **Website Static Export** | Next.js production static export and doc verification | PASS |
| **Release Tooling Validation**| Cross-platform package tooling & artifact generation | PASS |
| **Dependency Review** | Supply-chain dependency vulnerability & license screening | PASS |

Note: Passing these automated gates establishes that the implemented features, parser rules, and differential test suites pass in the automated environment. It is evidence of engineering hygiene and active gating, not a claim of complete formal compiler correctness or 100% backend equivalence across all unexercised programs.

## What “ready” means today

Aura is ready to be presented publicly as:

- a serious language implementation,
- a proof-driven compiler/tooling research project,
- an integrated compiler + verifier + IDE platform,
- an active pre-stable systems-language ecosystem.

Aura should **not yet** be presented as:

- a stable v1.0 language with frozen semantics,
- a formally verified compiler,
- a language with universally proven memory safety,
- a universally production-ready Android toolchain,
- a system with guaranteed sub-200ms proofs across workloads,
- a JIT/hybrid compiler that automatically promotes AVM execution to LLVM.

That framing is stronger, not weaker: it gives technical readers a precise surface they can trust.
