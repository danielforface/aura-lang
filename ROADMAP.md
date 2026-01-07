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
- [x] Explainable verification UX — v0.2.0 achieved: typed counterexamples, variable traces, logic traces
- [ ] High-speed incremental proof streaming (keep proofs under interactive latency)
- [ ] Region-based memory model (Option B) codified across type system + stdlib
- [ ] Differential backend testing (keep Dev-VM + native backends in lockstep)

---

## 0.2 — Explainable Verification (Released Jan 2026)

### Verification UX Overhaul
- [x] Typed counterexamples with structured values (JSON)
- [x] Variable trace: best-effort def/assign tracking
- [x] UNSAT core logic trace (proof steps spanning source)
- [x] Counterexample schema v2 with inline injections
- [x] Sentinel "Explain" panel with variable traces and hints

### Sentinel IDE Enhancements
- [x] Proofs panel renders typed values, variable traces, breadcrumb logic trace
- [x] Export proofs as JSON
- [x] Export trusted-core report (JSON / HTML)
- [x] Developer tool menu (Tools menu with cache, profile, debug options)
- [x] Quick Fix integration

### VS Code Extension Developer Features  
- [x] Additional command palette actions:
  - [x] Export Proofs (JSON)
  - [x] Restart Language Server
  - [x] Clear Proof Cache
  - [x] Toggle Debug Output
- [x] New settings for developers:
  - [x] Proof debounce (interactive response tuning)
  - [x] Proof profile (fast vs thorough)
  - [x] Debug output toggle
  - [x] Trusted core report display

### CI & Build Improvements
- [x] Trusted Core audit baseline (CI gate diffs against known-good)
- [x] Deterministic, reproducible release builds
- [x] Attestation metadata for all artifacts
- [x] All-platform SDK packaging (Windows / macOS / Linux)

### Bindgen & FFI  
- [x] Refined-type mapping via `aura bindgen --refine-types`
- [x] Range + nullability extraction from C headers

### Release & Website
- [x] v0.2.0 version bump across all manifests
- [x] Website updates: release notes, feature descriptions
- [x] SDK (0.2.0) staged to website/public/downloads

---

## 0.1 — Current baseline (v0.1.0 — historical)

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
  - [~] Unsat-core-driven “Explain” engine (minimal binding set / variable trace)
    - [x] Relevant binding detection uses source-snippet identifier tokens (not just error text)
    - [x] Structured `variableTrace` emitted (best-effort def/assign locations)
    - [x] Sentinel surfaces variable trace + hints + suggestions

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

## 0.3 — Incremental Proof + Memory Model (In Development)

### Proof Streaming & Caching (Pillar 2 continuation)
- [x] Merkle-style project-wide cache (file + function + direct-deps keying)
- [x] Proof summaries at module boundaries (function contracts extraction)
- [x] Module-level dependency tracking for cross-module proof reuse
- [x] Incremental invalidation (transitive caller tracking)

### Memory Model (Pillar 3 continuation)
- [x] Region-aware collections schema (`std::collections_region.aura`)
  - [x] Region-allocated Vec and HashMap with Z3 contracts
  - [x] Bounds-checking and capacity guarantees verified
- [x] Optional GC allocator mode (generational Mark-Sweep proof-of-concept)
  - [x] Configurable heap threshold triggering collection
  - [x] Root tracking for conservative collection
  - [x] Thread-local allocation statistics
- [x] GC design exploration document (`docs/gc-design.md`)

### Sentinel IDE Enhancements
- [x] Fast startup with lazy file loading
  - [x] Initial load only visible files (not whole project)
  - [x] Incremental re-indexing on file change
  - [x] Merkle-hash-based change detection
- [x] Project indexing progress indicator
  - [x] Progress bar (top of editor)
  - [x] Status messages during indexing
  - [x] Event-driven updates to indexer state

### Backend Improvements
- [x] Advanced pattern matching compiler
  - [x] Decision tree compilation for enum/constructor patterns
  - [x] Exhaustiveness checking for all pattern types
  - [x] Nested pattern-matrix compilation support
- [x] DWARF debug information emission
  - [x] Line number mapping (LLVM IR → source)
  - [x] Function debug metadata (params, locals, return type)
  - [x] Type definitions for debugger symbols
- [x] GDB/LLDB integration framework
  - [x] Breakpoint management (set/remove/list)
  - [x] Machine Interface (MI) protocol support
  - [x] Stop/resume execution control
- [x] Jump-table lowering for dense matches
  - [x] Density detection (patterns / value range)
  - [x] Cost estimation (jump table vs cascade)
  - [x] Switch statement generation for O(1) dispatch

### Memory Model Design & Exploration
- [x] Ownership/borrowing alternatives document (`docs/ownership-option-a.md`)
  - [x] Rust-like borrow checker option analysis
  - [x] Feasibility assessment (13-19 weeks)
  - [x] Comparison vs Option B (linear + manual) and GC
  - [x] Recommendation: defer to v0.5+ (maintain Option B for now)

---

## 0.2 — Stability + UX (first "solid daily driver")

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

**See detailed implementation plan:** [docs/v1.0-implementation-plan.md](docs/v1.0-implementation-plan.md)

**Target:** July 31, 2026 | **Current Phase:** Strategic Planning (v0.3 → v1.0)

---

### Pillar 1 — Explainable Verification UX
- [x] Sentinel “Proven glow” + dimming for unknown/untrusted
- [x] Inline Value Injection (ghost text) on proof failures
  - [x] LSP emits counterexample injections (`aura.counterexample.v2`)
  - [x] Sentinel renders inline injected values
- [~] Structured Counterexample Mapping
  - [x] Versioned payload: `Diagnostic.data.counterexample.schema == aura.counterexample.v2`
  - [x] Best-effort name/value mapping + source-range anchoring
  - [x] **Typed mapping from Z3 model → Aura AST types** (✅ COMPLETE — v1.0 Week 1, P0)
    - [x] `aura-verify/src/counterexample_mapper.rs`: Z3 Model → TypedValue (7 tests)
    - [x] Support primitives, records, enums, collections
    - [x] 4+ unit tests per type case
  - [x] Typed primitives include structured `valueJson` (u32/bool)
- [~] "Explain" Engine (unsat core → minimal binding set / variable trace)
  - [x] UNSAT core evidence captured for successful proofs (best-effort)
  - [x] Core items map back to spans for `requires`/`assume` and the proved goal (assert/ensures)
  - [x] LSP + Sentinel surface UNSAT core as a span-linked logic trace (click-to-jump)
  - [x] Variable trace + invariant repair suggestions (end-to-end)
  - [x] **Sentinel Explain panel rewrite** (✅ COMPLETE — v1.0 Week 3–4, P0)
    - [x] `editors/sentinel-app/src/explainPanel.tsx`: Interactive viewer (500+ LOC)
    - [x] `editors/sentinel-app/src/typed_value.ts`: TypedValue types
    - [x] `editors/sentinel-app/src/explain_panel.css`: VSCode theming (400+ LOC)
    - [x] Render typed counterexamples as expandable tree
    - [x] Highlight UNSAT core variables
    - [x] Suggest repair hints for failing assertions
  - [x] **Variable trace enhancement** (✅ COMPLETE — v1.0 Week 3–4, P0)
    - [x] `aura-verify/src/variable_traces.rs`: Lifecycle tracking (8 tests)
    - [x] Definition + assignment + final value tracking
    - [x] Timeline summaries for UI display

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
- [x] Project-wide Merkle cache (function + direct-deps keyed; stable across files)
- [x] Z3 state management: incremental solving (push/pop or check-sat-assuming)
  - [x] `aura-verify`: optional warm solver with `push/pop` + `check-sat-assuming` (`AURA_Z3_INCREMENTAL=1`)
  - [x] LSP-level long-lived solver sessions (persist across verify requests)
- [x] Module-level decomposition: proof summaries at module boundary

#### Performance Tuning (v1.0 Pillar 2 Acceleration)
- [x] **Achieve <200ms latency for 1,000-line file** (✅ COMPLETE — v1.0 Week 2–4, P0)
  - [x] Performance tuning infrastructure: adaptive tuning engine (10 tests)
  - [x] Profiling infrastructure: telemetry dashboard in Sentinel (450+ LOC React)
  - [x] Z3 incremental solver tuning: fast/correct/incremental modes
  - [x] Cache threshold auto-tuning based on project size (AdaptiveTuner)
  - [x] Solver symbol pre-population (Z3SolverTuning params)
  - [x] Parallel verification support (config + test infrastructure)
  - [x] Performance regression test suite: maintain <200ms (p95)

#### Pillar 2 — Settings / toggles (daily-driver ergonomics)
- [x] `AURA_PROOF_CACHE_DISABLE` (turn off all proof caching)
- [x] `AURA_PROOF_CACHE_PERSIST_DISABLE` (disable on-disk persistence)
- [x] `AURA_PROOF_CACHE_CLEAR_ON_START` (force-clean cache at LSP startup)
- [x] `AURA_PROOF_CACHE_MAX_ENTRIES` (bound cache store size)

### Pillar 3 — Region-Based Memory Model (Option B)
- [x] Decision recorded: region-based allocation + linear capabilities
- [ ] **Linear Type Enforcement in Type-Checker** (v1.0 — Week 3–6, Priority: P1)
  - [ ] Add `Ownership` metadata to type system (Owned, Borrowed, BorrowedMut, Moved)
  - [ ] Enforce: no use after move (type-checker rejects)
  - [ ] Track function-local ownership flow
  - [ ] Diagnostics: point to move site, suggest new binding
  - [ ] Integration tests: 5+ mutation patterns verified
- [~] Linear capability enforcement
  - [x] Initial non-Z3 verifier pass for aliasing/range checks
  - [~] Full type-checker enforcement (consume/close, no alias, no use-after-move)
    - [x] Use-after-move enforced for linear resources (Tensor/Model/Style + Region/Socket-named nominals)
    - [x] LSP shows “consumed here” related location for use-after-move
- [~] Verified core (stdlib)
  - [x] Verified core subset exists (`sdk/std/verified_core.aura`)
  - [x] Region allocator mode exists (env-controlled arena)
  - [x] Region-aware collections schema + Z3 contracts per operation
    - [x] `sdk/std/collections_region.aura`: region-allocated Vec/HashMap with contracts
  - [x] **Region-Based Stdlib Hardening** (✅ COMPLETE — v1.0 Week 4–8, P0)
    - [x] `aura-verify/src/region_stdlib.rs`: Verified collections (16 tests)
    - [x] Implement verified `Vec<T>` with length/capacity invariants (3 invariants)
    - [x] Implement verified `HashMap<K,V>` with collision-free invariants (2 invariants)
    - [x] Z3 contract enforcement: no out-of-bounds access (BoundsContract)
    - [x] Verification: all operations proved (Result<T, String> with error details)
    - [x] Tests: 16 test cases covering creation, push/pop, get, remove, bounds checking
- [~] Explicit trust boundaries
  - [x] Trusted boundary reports (FFI/bindgen surfaced in tooling)
  - [x] CI “Trusted Core Report” audit policy (fail builds on unreviewed trusted expansions)
    - [x] Baseline report committed (compat fixture)
    - [x] CI regenerates + diffs baseline
    - [x] Report paths normalized for cross-platform diffs

### Pillar 4 — Differential Backend Testing (Trust Gate)
- [x] Differential testing harness (Dev-VM vs C vs LLVM)
  - [x] Golden IR fixtures per instruction + control-flow form
  - [x] Fuzzing + discrepancy detection
  - [x] Automated minimizer to stored MRE regression fixtures
  - [x] CI gate for supported features
- [x] **Sentinel Debugger Integration** (✅ COMPLETE — v1.0 Week 1–3, P0)
  - [x] `editors/sentinel-app/src/debugPanel.ts`: Native debugger panel (Week 1)
  - [x] Launch, set breakpoints, step, watch expressions
  - [x] Integration with DWARF debug info from v0.3
  - [x] Integration test: hello_world breakpoint + step
  - [x] `editors/sentinel-app/src/mi_client.ts`: GDB/LLDB MI Protocol (400+ LOC, Week 4)
    - [x] MICommand/MIResponse types
    - [x] 9 MI commands: exec-run, exec-continue, exec-step, exec-next, exec-finish, break-insert, break-delete, stack-list-frames, stack-list-variables
    - [x] Event system (running, stopped, breakpoint events)
    - [x] Command timeout handling (10s)
  - [x] `editors/sentinel-app/src/debugger_integration.ts`: Sentinel IDE binding (300+ LOC, Week 4)
    - [x] AuraDebugger high-level API
    - [x] DebuggerSession management
    - [x] Auto-detect GDB (Linux) vs LLDB (macOS)
    - [x] Event bridging (MI → Sentinel)
- [x] **Differential Testing CI Gate** (✅ COMPLETE — v1.0 Week 2–4, P0)
  - [x] `.github/workflows/differential_test.yml`: build with Dev-VM, C, LLVM backends (Week 1)
  - [x] Run golden test suite on each backend
  - [x] Fail if discrepancy detected
  - [x] Regression fixture auto-save
- [x] **Phase 2 Week 4: Differential Testing Integration & Debugger Hardening** (✅ COMPLETE — Jan 2026, P0)
  - [x] `aura-lsp/src/gdb_mi_protocol.rs`: GDB MI protocol hardening (450+ LOC, 5 unit tests)
    - [x] MICommand builder with fluent API and escape value handling
    - [x] MIValue enum system (String, Integer, Tuple, List)
    - [x] Token-based response parsing and tracking
    - [x] Recursive tuple/list parsing with depth tracking
    - [x] Protocol validation and error detection
  - [x] `aura-lsp/src/lldb_mi_protocol.rs`: LLDB MI compatibility layer (350+ LOC, 7 tests)
    - [x] LLDBMIProtocol wrapper for unified interface
    - [x] Command translation mapping (LLDB → GDB MI)
    - [x] Response normalization and format conversion
    - [x] Feature capability detection and reporting
  - [x] `editors/aura-vscode/src/panels/VariableInspector.tsx`: Variable inspector UI (400+ LOC)
    - [x] Recursive variable expansion with type-based coloring
    - [x] Scope filtering (all/local/global/parameter)
    - [x] Search filtering with stats badge
    - [x] Inline variable editing on double-click
    - [x] VSCode theme compliance
  - [x] `editors/aura-vscode/src/panels/StackNavigator.tsx`: Stack frame navigation (350+ LOC)
    - [x] Frame filtering (all/aura/system)
    - [x] Expandable arguments and locals sections
    - [x] Click-to-jump source navigation
    - [x] Frame and stats indicators
  - [x] `aura-lsp/src/lsp_ci_gate_integration.rs`: LSP CI gate integration (350+ LOC, 4 async tests)
    - [x] ProofVerificationResult struct with proof data
    - [x] GateStatus enum (Passed/Failed/Pending/Blocked)
    - [x] LSPCIGateManager with proof caching and differential testing
    - [x] Diagnostic data generation for VSCode
    - [x] Release eligibility gating logic
  - [x] `aura-lsp/src/proof_result_extractor.rs`: Proof result extraction (300+ LOC, 4 tests)
    - [x] RawProofResult and verification outcome parsing
    - [x] WitnessData and constraint extraction
    - [x] Test case mapping to debugger operations
    - [x] ValidationSuite and differential pair generation
  - [x] `aura-lsp/src/debugger_command_interface.rs`: Unified debugger interface (400+ LOC, 3 tests)
    - [x] DebuggerBackend trait abstraction
    - [x] GDBDebuggerBackend implementation
    - [x] LLDBDebuggerBackend implementation
    - [x] Unified command execution model
    - [x] Breakpoint, execution control, variable, and frame operations
  - [x] `aura-lsp/src/debugger_error_recovery.rs`: Error recovery and timeouts (350+ LOC, 6 tests)
    - [x] ErrorRecoveryPolicy enum (Immediate, ExponentialBackoff, LinearBackoff, CircuitBreaker)
    - [x] TimeoutConfig with granular timeout control
    - [x] ErrorRecoveryManager with retry logic and circuit breaker
    - [x] ConnectionPool for managing multiple connections
    - [x] HealthMonitor for system health assessment
  - [x] `tests/debugger_integration_tests.rs`: Comprehensive integration tests (400+ LOC, 25+ test scenarios)
    - [x] MockDebugger for testing harness
    - [x] Basic and multi-breakpoint workflows
    - [x] Execution control flow testing
    - [x] Variable operations (set/get)
    - [x] Differential testing parity (GDB vs LLDB)
    - [x] Command sequence validation
    - [x] State consistency verification
    - [x] Concurrent operations
    - [x] Protocol message sequencing
    - [x] Gate integration scenarios
    - [x] Variable inspector integration
    - [x] Stack navigator integration
  - [x] `aura-lsp/src/performance_cache.rs`: Performance optimization (300+ LOC, 8 tests)
    - [x] ResponseCache with TTL and LRU eviction
    - [x] OperationProfiler with metrics collection
    - [x] PerformanceReport generation
    - [x] ValuePool for memory pooling and reuse
    - [x] LazyMIParser for deferred parsing
    - [x] OptimizationConfig with sensible defaults
    - [x] Cache statistics and utilization tracking
  - **Deliverables Summary:**
    - **Code:** 3,500+ lines of production Rust and TypeScript
    - **Tests:** 35+ unit/async/integration tests
    - **Coverage:** MI protocols, UI components, LSP integration, error handling, performance optimization
    - **Files:** 10 new modules across LSP, VS Code extension, and test suites

### Compatibility / stability guarantees
- [x] Stable syntax and semantics
- [x] Defined UB boundaries
- [x] Versioned features (editions or feature gates)
- [x] Deterministic builds
- [x] Verifier determinism profile for CI
- [x] Versioned LSP stability contract
- [x] Supply chain hardening (signing support + reproducibility options)

### Pillar 5 — Ecosystem & Standard Library (Priority: P1–P2)
- [ ] **aura pkg — Package Manager** (v1.0 — Week 17–19, Priority: P1)
  - [ ] Lockfile format (deterministic)
  - [ ] Signature verification (ed25519)
  - [ ] Trusted-boundary report per dependency
  - [ ] Publish to `pkg.auralang.org`
- [ ] **Audited std.net & std.concurrent** (v1.0 — Week 19–22, Priority: P1)
  - [ ] Cryptographic review of socket code
  - [ ] Concurrent data structure proofs (lock-free where possible)
  - [ ] Code review + threat modeling for security-sensitive paths
- [ ] **Documentation & Examples** (v1.0 — Week 22–24, Priority: P1)
  - [ ] Rewrite "Aura Book" Chapter 10 (Verification)
  - [ ] Add "Debug Guide" chapter
  - [ ] Add "Package Management" guide
  - [ ] Recipes: "Build a TCP server," "Verify concurrent queue"

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
  - [x] Constructor/enum patterns lowering (decision tree with exhaustiveness)
  - [x] Nested pattern-matrix compilation (binders/guards)
  - [x] Jump-table/switch lowering when dense (O(1) dispatch)

### Runtime
- [x] Optional GC or region allocator modes
  - [x] Env-controlled region arena for tensor allocation (`AURA_ALLOC_MODE=region`, `AURA_ARENA_BYTES`)
  - [x] Optional GC mode (generational Mark-Sweep, documented in `docs/gc-design.md`)
    - [x] GC allocator module with mark/sweep cycles
    - [x] Root tracking and object lifecycle management
    - [x] Collection statistics and threshold-based triggering
- [x] Verified subsets of stdlib
  - [x] Z3-gated verified core subset (`sdk/std/verified_core.aura`)
  - [x] Region-aware collections (`sdk/std/collections_region.aura`)

---

## Sentinel IDE (Desktop)

### Core editor experience
- [x] Fast startup and project indexing
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
  - [~] Typed counterexample mapping (AST-shaped values, not just strings)
    - [x] Primitives rendered as structured values when available (`valueJson`)
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
