# Aura Roadmap Checklist (Future + Status)

This is the roadmap converted into a **future checklist**.

Legend:
- [x] Done in this repo (implemented and/or verified in our build pipeline)
- [ ] Not done yet / planned
- [~] In progress / partially done (we have pieces, not end-to-end)

---

## North Star (direction, not a task list)

Aura’s north star is a language where:
1. Correctness is a first-class UX (proofs stream, counterexamples are explainable, “trust boundaries” are explicit).
2. Performance is not an afterthought (zero-cost abstractions where possible, predictable runtime, good interop).
3. The toolchain is integrated (formatter, linter, verifier, package manager, debugger, profiler, and IDE work together).
4. The ecosystem is coherent (standard library conventions, strong versioning, reproducible builds).

Current focus (Jan 2026): ship Aura as a production-ready daily driver (v1.0) by removing the top bottlenecks:
- Explainable verification UX (make failures actionable in-editor)
- High-speed incremental proof streaming (keep proofs under interactive latency)
- Region-based memory model (Option B) codified across type system + stdlib
- Differential backend testing (keep Dev-VM + native backends in lockstep)

---

## 0.1 — Current baseline (what exists now)

### Language
- [x] Core syntax and parsing pipeline
- [x] Module structure (basic)
- [x] Diagnostics with source mapping

### Tooling
- [x] `aura` CLI
- [x] `aura-lsp` for editor integration
- [x] Aura Sentinel desktop IDE
- [x] Z3 Gate proof streaming contract (non-blocking + UI cues)
  - [x] Non-blocking runs (async, cancellable)
  - [x] Phase cues (parse / sema / z3)
  - [x] Structured `Diagnostic.data` payloads for proof notes and verify errors

### Distribution / builds
- [x] SDK zip build
- [x] VSIX packaging
- [x] Sentinel installer packaging (MSI + NSIS)
- [x] Website download staging via release script
- [x] Repo hygiene: `.gitignore` excludes build outputs and large generated artifacts

---

## 0.2 — Stability + UX (first “solid daily driver”)

### Language: clarify and lock down the basics
- [x] Syntax stabilization (reduce ambiguity, formalize precedence)
- [x] Canonical formatter (deterministic output, minimal flags)
- [x] Parser error recovery improvements (“keep going” diagnostics)
- [x] Module & import rules fully specified (shadowing, resolution order)
- [x] Numeric literal rules fully specified (suffixes, overflow, coercions)
- [x] String model fully specified (UTF-8 rules, escapes, raw strings)
- [x] Pattern matching exhaustiveness checking (first pass)

### Diagnostics & language server
- [x] Stable diagnostic codes (versioned, documented)
- [x] Go-to-definition / find references correctness pass
- [x] Rename (safe, project-wide) with preview
- [x] Inlay hints (types/params) + semantic tokens polish

### Verifier (Z3 Gate UX)
- [x] Proofs never block the UI (streaming protocol)
- [x] Proof stream phases expanded (parse → sema → normalize → z3)
- [x] Incremental proof runs (re-check affected regions)
- [x] Proof caching (file hash + deps + solver config)
- [x] Counterexample rendering improvements
  - [x] Minimal model slice
  - [x] Highlight bindings relevant to failing assertion
- [~] One-click “explain” for models/counterexamples
  - [x] Explain panel shows model slice + relevant bindings
  - [x] Counterexample mapping schema `aura.counterexample.v2` (mapped bindings + inline injections)
  - [ ] Unsat-core-driven “Explain” engine (minimal binding set / variable trace)

### Sentinel (IDE)
- [x] Proofs panel UX (streaming + cancellation is done; polish still planned)
  - [x] Streaming progress + cancellation
  - [x] Robust counterexample/trace rendering (structured bindings/model + related navigation)
- [x] Problems panel upgrades (de-duplication, severity mapping, quick actions)

#### Sentinel — User-facing actions + settings (inventory)
- [x] Command palette actions
  - [x] Proofs: Run
  - [x] Proofs: Rerun Affected
  - [x] Proofs: Toggle Profile (Fast/Thorough)
  - [x] Proofs: Clear Cache
  - [x] LSP: Reindex (Restart)
- [x] Proofs view controls
  - [x] Export proofs (JSON / HTML)
  - [x] Quick Fix entry point (applies WorkspaceEdit + refresh)
  - [x] Cache indicator (when telemetry is available)
- [x] Settings surfaced in UI
  - [x] Theme
  - [x] Format-on-save
  - [x] Proof mode + proof debounce
  - [x] Proof profile (fast vs thorough)

### Build & packaging
- [x] Reproducible builds tightened (policies + CI enforcement)
- [x] SDK contents documented enough to use (includes android scripts + sample)
- [x] “First run” checks (toolchain sanity, sidecar availability)

---

## 0.3 — Safety + Power (language grows teeth)

### Type system growth
- [x] Generics
  - [x] Parametric polymorphism (types + values)
  - [x] Constraints/traits (minimal viable)
- [x] Sum/product types (tagged unions, records with defaults)
- [x] Type inference improvements (UX + error messages)
- [x] Refinement types (initial) integrated with verifier
  - [x] `x: Int where x >= 0`
  - [x] Propagation through simple arithmetic

### Effects / ownership / lifetime model
- [x] Pick and document a coherent model
  - [ ] Option A: ownership/borrowing inspired
  - [x] Option B: region-based + linear capabilities
  - [ ] Option C: effect system + explicit alias rules
- [x] Sound aliasing rules for mutation
- [x] Safe concurrency story (no data races by construction)
- [x] Explicit FFI boundaries (unsafe blocks / trusted modules)

### Verification: beyond “check an assertion”
- [x] Contracts (`requires` / `ensures`)
- [x] Loop invariants (first pass)
- [x] Termination hints (variant/decreases)
- [x] Quantifiers with guardrails (timeouts + trigger guidance)
- [x] SMT configuration profiles (fast / thorough / CI-locked)

### Runtime and performance
- [x] Faster interpreter / VM
- [x] Baseline optimization pipeline (const fold, DCE)
- [x] Better debug info mapping for stepping

### Developer experience (instant feedback loop)
- [x] High-speed Dev-VM / JIT interpreter path
  - [x] `aura run` in dev mode can execute without native toolchain
  - [x] Parity tests between Dev-VM and native backends for core semantics
  - [x] Hot-reload loop optimized for sub-second iteration

---

## 0.4 — Interop (FFI automation at scale)

### FFI Automation & ecosystem interop
- [x] Aura-Bindgen (automated binding generator)
  - [x] Parse C headers (`.h`) and generate Aura externs + shim module
  - [x] Type mapping with best-effort safety (pointers → opaque handles/capabilities where possible)
  - [x] Optional refined-type mapping via annotations (e.g., ranges, nullability)
  - [x] Emits an explicit “trusted boundary” report (what must be trusted vs proven)
  - [x] Deterministic output + caching (stable diffs in CI)

## 0.5 — Ecosystem (packages, libraries, real projects)

### Package manager and registry
- [x] `aura pkg` (or equivalent)
  - [x] Semantic versioning rules
  - [x] Lockfiles
  - [x] Integrity hashes
  - [x] Offline cache
- [x] Registry
  - [x] Namespacing
  - [x] Signed releases (optional)
  - [x] Deprecation policy

### Build system
- [x] `aura build`
- [x] `aura test`
- [x] `aura fmt`
- [x] `aura lint`
- [x] `aura verify`
- [x] Workspaces and multi-package repos
- [x] Build profiles (dev / release / verify)
- [x] Artifact caching

### Standard library (first “complete feeling” set)
- [x] `std.core` (prelude, option/result, iterators)
- [x] `std.math`
- [x] `std.collections`
- [x] `std.io`
- [x] `std.net`
- [x] `std.time`
- [x] `std.concurrent` (aligned with the safety model)
- [x] `std.crypto` (only if committing to audits; otherwise wrappers)
- [x] `std.json`
- [x] `std.yaml`
- [x] `std.toml`

### Docs and learnability
- [x] Book: The Aura Language
- [x] Cookbook: Aura by example
- [x] Reference spec (normative)
- [x] Verifier guide: Z3 Gate patterns

---

## 1.0 — Production (compatibility guarantees)

This section is tracked by strategic pillars (v1.0 daily-driver focus).

### Pillar 1 — Explainable Verification UX
- [x] Sentinel “Proven glow” + dimming for unknown/untrusted
- [x] Inline Value Injection (ghost text) on proof failures
  - [x] LSP emits counterexample injections (`aura.counterexample.v2`)
  - [x] Sentinel renders inline injected values
- [~] Structured Counterexample Mapping
  - [x] Versioned payload: `Diagnostic.data.counterexample.schema == aura.counterexample.v2`
  - [x] Best-effort name/value mapping + source-range anchoring
  - [~] Typed mapping from Z3 model → Aura AST types (records/enums/collections) + pretty-printer
- [~] “Explain” Engine (unsat core → minimal binding set / variable trace)
  - [x] UNSAT core evidence captured for successful proofs (best-effort)
  - [x] Core items map back to spans for `requires`/`assume` and the proved goal (assert/ensures)
  - [x] LSP + Sentinel surface UNSAT core as a span-linked logic trace (click-to-jump)
  - [ ] Variable trace + invariant repair suggestions (end-to-end)

### Pillar 2 — High-Speed Incremental Proof Streaming
- [x] Dependency-aware proof caching (file hash + solver config)
- [x] Incremental proof runs (re-check affected regions)
- [x] Statement-level caching (per-stmt hash) + UI-subsystem caching
- [x] Persistent proof cache on disk (warm start across IDE restarts)
- [x] Stable cache identity (workspace-relative file IDs)
- [x] Dependency invalidation (import-content hash)
- [x] Merkle-style statement hash with intra-file deps (transitive call graph + import hash)
- [x] Atomic persistent cache writes (temp + rename)
- [x] Persist only the updated workspace entry (no full snapshot clone/write)
- [x] Cache controls
  - [x] LSP request: `aura/proofCacheClear`
  - [x] Sentinel command: “Proofs: Clear Cache”
- [x] Cached-proof indicator (telemetry surfaced in Sentinel)
- [ ] Project-wide Merkle cache (function + direct-deps keyed; stable across files)
- [x] Z3 state management: incremental solving (push/pop or check-sat-assuming)
  - [x] `aura-verify`: optional warm solver with `push/pop` + `check-sat-assuming` (`AURA_Z3_INCREMENTAL=1`)
  - [x] LSP-level long-lived solver sessions (persist across verify requests)
- [ ] Module-level decomposition: proof summaries at module boundary

#### Pillar 2 — Settings / toggles (daily-driver ergonomics)
- [x] `AURA_PROOF_CACHE_DISABLE` (turn off all proof caching)
- [x] `AURA_PROOF_CACHE_PERSIST_DISABLE` (disable on-disk persistence)
- [x] `AURA_PROOF_CACHE_CLEAR_ON_START` (force-clean cache at LSP startup)
- [x] `AURA_PROOF_CACHE_MAX_ENTRIES` (bound cache store size)

### Pillar 3 — Region-Based Memory Model (Option B)
- [x] Decision recorded: region-based allocation + linear capabilities
- [~] Linear capability enforcement
  - [x] Initial non-Z3 verifier pass for aliasing/range checks
  - [~] Full type-checker enforcement (consume/close, no alias, no use-after-move)
    - [x] Use-after-move enforced for linear resources (Tensor/Model/Style + Region/Socket-named nominals)
    - [x] LSP shows “consumed here” related location for use-after-move
- [~] Verified core (stdlib)
  - [x] Verified core subset exists (`sdk/std/verified_core.aura`)
  - [x] Region allocator mode exists (env-controlled arena)
  - [ ] Refactor `std.collections` to region allocation + Z3-gated contracts per operation
- [~] Explicit trust boundaries
  - [x] Trusted boundary reports (FFI/bindgen surfaced in tooling)
  - [ ] CI “Trusted Core Report” audit policy (fail builds on unreviewed trusted expansions)

### Pillar 4 — Differential Backend Testing (Trust Gate)
- [x] Differential testing harness (Dev-VM vs C vs LLVM)
  - [x] Golden IR fixtures per instruction + control-flow form
  - [x] Fuzzing + discrepancy detection
  - [x] Automated minimizer to stored MRE regression fixtures
  - [x] CI gate for supported features

### Compatibility / stability guarantees
- [x] Stable syntax and semantics
- [x] Defined UB boundaries
- [x] Versioned features (editions or feature gates)
- [x] Deterministic builds
- [x] Verifier determinism profile for CI
- [x] Versioned LSP stability contract
- [x] Supply chain hardening (signing support + reproducibility options)

---

## 2.0 — Research-grade features (stretch goals)

### Advanced verification
- [x] Proof summaries (assumptions + what was proven)
- [x] Proof decomposition (module-level boundaries)
- [x] Interpolants / unsat cores for explanations
- [x] Synthesis
  - [x] Suggest fixes for failing proofs
  - [x] Generate invariants
- [x] Property-based testing integrated with counterexamples

### Verified compiler (research track)
- [x] Translation validation (IR → C/LLVM) with proofs or checkers
  - [x] Formally specify a core IR semantics model used as the oracle
  - [x] Prove or mechanically check key compiler passes (or verify per-pass invariants)
  - [x] Proof-carrying compilation experiments (attach evidence to artifacts)

### Language evolution
- [~] Indexed/dependent-ish types (carefully)
  - [x] Tensor shapes can be indexed by const-evaluated expressions (CTFE MVP)
- [x] Metaprogramming with guardrails
  - [x] Hygienic macros
  - [x] Compile-time evaluation (CTFE MVP for const u64 shape expressions; gated by `ctfe`)
- [~] Advanced pattern matching compilation
  - [x] Balanced decision tree for integer-literal matches
  - [ ] Constructor/enum patterns lowering
  - [ ] Nested pattern-matrix compilation (binders/guards)
  - [ ] Jump-table/switch lowering when dense

### Runtime
- [x] Optional GC or region allocator modes
  - [x] Env-controlled region arena for tensor allocation (`AURA_ALLOC_MODE=region`, `AURA_ARENA_BYTES`)
  - [ ] Optional GC mode (if pursued)
- [x] Verified subsets of stdlib
  - [x] Z3-gated verified core subset (`sdk/std/verified_core.aura`)

---

## Sentinel IDE (Desktop)

### Core editor experience
- [ ] Fast startup and project indexing
- [x] Robust file watching + dependency graphs
- [x] Diagnostics timeline (see when/why proofs changed)

### Interop workflows
- [x] Aura-Bindgen UI integration
  - [x] Select header(s) → generate wrappers → preview diff → write files
  - [x] Surface the trusted-boundary report inside the IDE

### Run & debug (Dev-VM aware)
- [x] “Run (Dev)” uses Dev-VM/JIT path for instant feedback
- [x] Step debugger integrates with Dev-VM execution model (MVP: statement-level)

### Proof UX (first-class)
- [~] Proof run controls
  - [x] Run + cancel streamed proofs
  - [x] Rerun affected
  - [x] Profiles (fast vs thorough)
- [~] Counterexample view
  - [x] Structured data plumbed via `Diagnostic.data`
  - [x] Rich value rendering + navigation
  - [x] Trace highlights in code
  - [x] Inline value injections (ghost text) from `aura.counterexample.v2`
  - [ ] Typed counterexample mapping (AST-shaped values, not just strings)
- [~] Proven path rendering
  - [x] “Proven glow” decoration exists
  - [x] Dimming for untrusted/unknown regions

### Debugging
- [~] Step debugger (interpreter + native)
  - [x] Dev-VM statement-level pause/continue/step
  - [~] Native: launch + supervised run + terminate + exit event (no attach/stepping)
- [x] Watch expressions (MVP)
- [x] Breakpoints + conditional breakpoints (MVP)
- [x] Debug protocol documentation (see [docs/debug-protocol.md](docs/debug-protocol.md))
- [x] Debug protocol robustness (invalid commands handled, no panics)
- [x] Dev-VM terminate support (cooperative cancel)
- [x] Debug protocol smoke test (hello + perfReport)

### Performance tooling
- [x] Profiler timeline (MVP)
- [x] Flame graphs (folded export, MVP)
- [~] Memory + allocation view (reachable-value breakdown; still approximate, MVP)

---

## LSP + Protocol

- [x] Proof stream custom protocol (start/cancel + streamed events)
- [x] Protocol documentation (see sdk/docs/z3-gate.md)
- [x] Capability negotiation (client advertises supported phases/features)
- [x] Telemetry (opt-in): proof timings, cache hits

---

## Android & Mobile

### Near-term (already enabled via SDK bootstrap)
- [x] Bootstrap install scripts (Android cmdline tools + sdkmanager)
- [x] Sample Gradle project buildable to an APK

### Mid-term
- [x] “One command” experience scripts exist
  - [x] `sdk/android/setup-android.ps1`
  - [x] `sdk/android/build-apk.ps1`
- [x] CI recipe for APK builds

### Long-term
- [ ] Native mobile runtime story
  - [ ] Aura ↔ Kotlin/Java FFI layer
  - [ ] Packaging Aura artifacts as assets or embedded libs
- [~] Cross-compilation targets
  - [x] aarch64-linux-android
  - [x] armv7-linux-androideabi

Bundling Android SDK/NDK inside Sentinel:
- [ ] Optional offline installer mode (very large; separate distribution)

---

## Governance / Contribution

- [~] Public feature tracking (issues/labels)
- [x] RFC process for language changes
- [x] Release channels (nightly / beta / stable)
- [x] Compatibility test suite

---

## Definition of Done (for checklist items)

- [ ] Tests exist (unit + integration where appropriate)
- [ ] Docs exist (reference + examples)
- [ ] Diagnostics designed for IDE use
- [ ] Migration plan exists if behavior changes

Additional DoD (for new strategic pillars):

- Aura-Bindgen: supports a documented C subset, generates deterministic output, and produces a trusted-boundary report; has tests over representative real headers.
- Dev-VM/JIT: can run typical projects without native toolchains, has parity tests vs native backends for supported semantics, and exposes debugger hooks for Sentinel.
- Backend trust: differential runs are CI-gated for supported features and failures minimize to stored repro fixtures.
- Performance: incremental proof feedback for a ~1,000-line file stays under 200ms in the typical edit loop.
- Safety: region-allocated “safe” code never segfaults; any potential crash must be rejected by the verifier/typechecker.
- Documentation: every new feature has a corresponding Aura Book chapter and a Cookbook example.

---

## Next concrete steps (recommended)

- [ ] Counterexamples: typed mapping + unsat-core “Explain” engine
- [ ] Proof performance: Merkle cache + incremental Z3 solving
- [ ] Region model: linear capabilities in the typechecker + region-based `std.collections`
- [ ] Keep Trust Gate green (fuzz/minimize/regressions) while expanding coverage
