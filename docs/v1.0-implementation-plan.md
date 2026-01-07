# Aura v1.0 "Daily Driver" Implementation Plan

**Target Release:** Q3 2026 (6 months from v0.3)  
**Status:** Strategic Planning (January 7, 2026)

---

## Executive Summary

After v0.3 (incremental proof + memory model foundation), the path to v1.0 "daily driver" requires **simultaneous acceleration of 5 strategic pillars** with weekly cross-team synchronization. This document prioritizes concrete work streams, identifies blockers, and defines "done" for each pillar.

---

## 📊 Pillar Readiness Matrix

| Pillar | Component | Status | Effort | Risk | Priority |
|--------|-----------|--------|--------|------|----------|
| **1. Explainability** | Typed Z3→AST mapping | ~30% | Medium | High | **P0** |
| **1. Explainability** | Variable trace engine | ~60% | Low | Low | **P0** |
| **2. Incremental Proofs** | Merkle cache integration | **100%** | — | Low | **DONE** |
| **2. Incremental Proofs** | Performance tuning (<200ms) | ~40% | Medium | Medium | **P0** |
| **3. Memory Safety** | Linear type enforcement | ~50% | High | High | **P1** |
| **3. Memory Safety** | Region stdlib (Vec/Map) | ~70% | Medium | Medium | **P1** |
| **4. Backend Trust Gate** | DWARF + GDB/LLDB | **100%** (MVP) | — | Low | **P0** |
| **4. Backend Trust Gate** | Differential testing CI | ~60% | High | High | **P1** |
| **5. Ecosystem** | aura pkg (package mgr) | ~50% | High | Medium | **P1** |
| **5. Ecosystem** | std.net audited | ~30% | High | High | **P2** |

---

## 🎯 Phase 1: Foundation (Weeks 1–8, Parallel Tracks)

### Track A: Pillar 1 — Explainability Engine (P0)

**Goal:** Developer can diagnose a proof failure in <5 seconds using typed counterexamples.

#### Tasks

**1a. Typed Z3→AST Mapper** (Week 1–3)
- **Deliverable:** `aura-verify/src/counterexample_mapper.rs`
  - Consume Z3 `Model` (SMT result)
  - Map primitive values → Aura type constructors
  - Recursively build structured values (records, enums, collections)
  - Emit JSON-serializable `TypedValue` with source ranges
  
- **Code Structure:**
  ```rust
  pub enum TypedValue {
    Primitive(String, String),  // type_name, value_str
    Record { fields: Vec<(String, TypedValue)> },
    Enum { variant: String, value: Box<TypedValue> },
    Array { element_type: String, elements: Vec<TypedValue> },
  }
  
  pub fn map_z3_model(
    model: &Z3Model,
    aura_type: &TypeRef,
    ctx: &MappingContext,
  ) -> Result<TypedValue, CounterexampleError>;
  ```

- **Success Criteria:**
  - [ ] Maps all primitives (u32, i64, bool, etc.)
  - [ ] Maps record types with correct field names
  - [ ] Maps enum variants + payloads
  - [ ] Handles nested structures (List<Struct>, etc.)
  - [ ] 4+ unit tests per case

- **Effort:** 3 weeks | **Blocker Risk:** Medium (Z3 API complexity)

---

**1b. Variable Trace Enhancement** (Week 2–4, parallel)
- **Deliverable:** Enhance `aura-lsp` proof streaming to emit rich variable traces
  - Wire `ProofSummary.assumptions` into LSP diagnostics
  - For each variable used in failed assertion, emit:
    - Definition location
    - Last assignment location
    - Value at proof point (from counterexample)
  
- **Code Location:** `aura-lsp/src/main.rs` → `emit_proof_diagnostics()`
  
- **Success Criteria:**
  - [ ] Trace emitted for 5+ variables in a failing proof
  - [ ] Sentinel renders each trace item as clickable breadcrumb
  - [ ] Integration test: complex struct mutation traces correctly

- **Effort:** 2 weeks | **Blocker Risk:** Low

---

**1c. Sentinel "Explain" Panel Update** (Week 3–4, parallel)
- **Deliverable:** `editors/sentinel-app/src/explainPanel.ts` (rewrite)
  - Consume typed counterexamples from LSP
  - Render as expandable tree (primitives, records, arrays)
  - Highlight variables mentioned in UNSAT core
  - Suggest "repair" hints (e.g., "add `x > 0` as precondition")
  
- **Success Criteria:**
  - [ ] Typed values render with correct Aura syntax
  - [ ] Nested structures unfold on click
  - [ ] UNSAT core spans clickable → jump to assertion
  - [ ] Visual diff (expected vs actual) for failing assertions

- **Effort:** 2 weeks | **Blocker Risk:** Low

---

### Track B: Pillar 2 — Performance Tuning (P0)

**Goal:** Achieve <200ms latency for 1,000-line file proof on every keystroke.

#### Tasks

**2a. Profiling Infrastructure** (Week 1–2)
- **Deliverable:** Telemetry dashboard in Sentinel
  - Capture proof times per function
  - Track cache hit/miss rates
  - Identify outlier functions
  
- **Instrumentation:**
  ```rust
  // aura-lsp/src/profiling.rs
  pub struct ProofMetrics {
    pub function: String,
    pub parse_ms: u64,
    pub sema_ms: u64,
    pub verify_ms: u64,
    pub cache_hit: bool,
  }
  ```

- **Effort:** 1 week | **Blocker Risk:** Low

---

**2b. Z3 Incremental Solver Tuning** (Week 2–4)
- **Deliverable:** Optimize `aura-verify` Z3 session reuse
  - Profile solver `push/pop` overhead
  - Benchmark `check-sat-assuming` for partial re-verifications
  - Identify sweet spot: cache invalidation vs solver state
  
- **Expected Win:** 30–50% reduction in repeated verifications
  
- **Effort:** 2 weeks | **Blocker Risk:** Medium (Z3 internals)

---

**2c. Cache Tuning for Edit Loop** (Week 3–4, parallel)
- **Deliverable:** Auto-tune cache thresholds based on project size
  - Small projects (<100 functions): aggressive cache invalidation
  - Large projects (>5000 functions): conservative invalidation
  - Profile `merkle_cache` lookup/insert performance
  
- **Effort:** 1 week | **Blocker Risk:** Low

---

### Track C: Pillar 4 — Backend Trust Gate (P0, Foundation)

**Goal:** GDB/LLDB integration ready for Sentinel; differential testing CI framework in place.

#### Tasks

**4a. Sentinel Debugger UI** (Week 1–3)
- **Deliverable:** `editors/sentinel-app/src/debugger.ts` → native debugger panel
  - Launch button: "Debug (Native)"
  - Set breakpoints (click gutter)
  - Step over / into / out
  - Watch expressions
  - Stack frame inspection
  
- **Integration:** Use DWARF from `aura-backend-llvm/src/debugger.rs` (v0.3)
  
- **Success Criteria:**
  - [ ] Can pause at breakpoint
  - [ ] Can step through source (DWARF mapping works)
  - [ ] Watch expression evaluates + displays
  - [ ] Integration test: hello_world breakpoint + step

- **Effort:** 2 weeks | **Blocker Risk:** Medium (GDB/LLDB protocol complexity)

---

**4b. Differential Testing CI Gate** (Week 2–4, parallel)
- **Deliverable:** `ci/differential_test.yml` (GitHub Actions)
  - Build with `Dev-VM`, `C`, `LLVM` backends
  - Run golden test suite on each
  - Diff outputs; fail if discrepancy
  - Store regression fixtures
  
- **Golden Test Suite:** Existing `aura-test-suite/` repo
  - Arithmetic, control flow, pointers, arrays
  - FFI round-trip tests
  
- **Success Criteria:**
  - [ ] CI passes for baseline (known-good) tests
  - [ ] New test failure → regression fixture auto-saved
  - [ ] Weekly report: discrepancy count trending down

- **Effort:** 2 weeks | **Blocker Risk:** Low (infrastructure mostly exists)

---

### Track D: Pillar 3 — Linear Capability Enforcement (P1, Foundation)

**Goal:** Type-checker rejects all "use-after-move" patterns in Safe mode.

#### Tasks

**3a. Type-Checker Enhancement** (Week 3–6)
- **Deliverable:** `aura-core/src/type_system.rs` → linear type tracking
  - Add `Ownership` to type metadata:
    ```rust
    pub enum Ownership {
      Owned,                // can be moved
      Borrowed,             // immutable borrow
      BorrowedMut,          // exclusive mutable
      Moved,                // error state
    }
    ```
  - Enforce: after `x` is moved, no further use of `x` is allowed
  - Track function-local ownership flow
  
- **Error Diagnostics:**
  - Point to move site
  - Suggest: "x was moved here; cannot use again"
  - Offer quick fix: introduce new binding
  
- **Success Criteria:**
  - [ ] `let x = ...; let y = x; print(x);` → error (use-after-move)
  - [ ] Linear resource types (Region, Socket) enforce closure
  - [ ] 5+ integration tests covering mutation patterns
  - [ ] Diagnostics have "move site" breadcrumb

- **Effort:** 4 weeks | **Blocker Risk:** High (type system redesign)

---

**3b. Region-Based Stdlib Hardening** (Week 4–8)
- **Deliverable:** Refactor `sdk/std/collections_region.aura` → production-ready
  - Implement verified `Vec<T>` with length/capacity invariants
  - Implement verified `HashMap<K,V>` with collision-free invariants
  - Add contract enforcement: no out-of-bounds access
  - Fuzz test with random operations
  
- **Z3 Contracts Example:**
  ```aura
  fn vec_get<T>(v: &Vec<T>, index: usize) -> Option<T>
    requires index < v.len()
    ensures result.is_some()
  ```

- **Success Criteria:**
  - [ ] `Vec` allocates from region arena (no malloc)
  - [ ] Bounds checks compile-time-verified when possible
  - [ ] Fuzzing: 10k random operations, 0 violations
  - [ ] All operations proved (Z3 passing)

- **Effort:** 4 weeks | **Blocker Risk:** Medium (Z3 contract complexity)

---

## 🚀 Phase 2: Integration & Hardening (Weeks 9–16)

### 2a. Cross-Pillar Integration

**Week 9–10: Explainability ↔ Incremental Proofs**
- Wire Merkle cache stats into Explain panel
- Show "cache hit" → faster explanation generation
- Benchmark: typed counterexample rendering should be <50ms

**Week 10–11: Linear Types ↔ Region Collections**
- Ensure `Vec<T>` in region arena can only be moved, not copied
- Type-checker enforces "no aliasing" rules for region-allocated types
- Test: move Vec between functions without copy

**Week 11–12: Backend Trust ↔ Explainability**
- DWARF line mapping enables: "stepping into function shows typed values"
- Sentinel debugger watches render as TypedValue (from Pillar 1)
- Test: set watch on struct field, step, see update

---

### 2b. Performance Tuning Sprint (Week 12–14)

- **Latency Target:** <200ms for 1,000-line file
- **Profiling:** Real-world projects (aura-stdlib, aura-lsp itself)
- **Optimizations:**
  - Parallel verification (rayon for independent functions)
  - Solver symbol pre-population (avoid repeated introductions)
  - Cache preloading on LSP startup

---

### 2c. Testing & Validation (Week 14–16)

- **Unit Tests:** 80+ new tests (20 per pillar)
- **Integration Tests:** 10+ end-to-end scenarios
  - "Fix a failing proof using Explain panel" (happy path)
  - "Modify a data structure; verify deterministic behavior" (safety)
  - "Debug a complex algorithm; use step/watch" (debugger)
- **Performance Regression Test:** <200ms latency maintained

---

## 📋 Phase 3: Polish & Docs (Weeks 17–24)

### 3a. Ecosystem (Pillars 5)

**Week 17–19: aura pkg (Package Manager)**
- Implement lockfile format (deterministic)
- Implement signature verification (ed25519)
- Implement trusted-boundary auto-report per dependency
- Publish to `pkg.auralang.org`

**Week 19–22: Audited std.net & std.concurrent**
- Cryptographic review of socket code
- Concurrent data structure proofs (lock-free where possible)
- Code review + threat modeling for security-sensitive paths

**Week 22–24: Documentation**
- Rewrite "Aura Book" ch. 10 (Verification)
- Add "Debug Guide" chapter
- Add "Package Management" guide
- Recipes: "Build a TCP server," "Verify concurrent queue"

---

### 3b. Release Preparation (Week 24–26)

- [ ] All integration tests passing
- [ ] Benchmark suite locked in (`benchmarks/` dir)
- [ ] Changelog + upgrade guide ready
- [ ] Website updated with v1.0 feature showcase
- [ ] Sentinel installer tested on Windows/macOS/Linux
- [ ] SDK versioned 1.0.0, signed, published

---

## 🎯 Definition of Done (v1.0)

### Latency ✅
- [ ] Incremental proofs for 1,000-line file: **<200ms** (p95)
- [ ] Explain panel renders in **<50ms**
- [ ] Debugger breakpoint stops in **<100ms**

### Safety ✅
- [ ] Type-checker rejects all use-after-move in Safe mode
- [ ] Region-allocated Vec/HashMap: 0 segfaults in fuzzing (10k ops)
- [ ] Differential testing: Dev-VM ≡ LLVM ≡ C (for all golden tests)

### Utility ✅
- [ ] New user can fix logical bug using Explain panel alone
- [ ] New user can build TCP server from stdlib + example
- [ ] New user can package and publish Aura crate in <10 minutes

### Stability ✅
- [ ] Stdlib covers 90% of systems programming use cases
- [ ] LSP + Sentinel: 99.9% uptime in stress tests
- [ ] Compatibility: code written for v1.0 still compiles v1.1+

---

## 📆 Timeline & Checkpoints

| Milestone | Date | Gate Criteria |
|-----------|------|---------------|
| Phase 1 (Tracks A–D) | Feb 28 | All P0 tasks + integration tests |
| Phase 2 (Integration) | Apr 15 | <200ms latency achieved |
| Phase 3 (Ecosystem + Docs) | May 31 | aura pkg works end-to-end |
| v1.0 Alpha Release | Jun 15 | All DoD items met |
| v1.0 Beta (user feedback) | Jul 1 | 50+ external users testing |
| **v1.0 Final Release** | **Jul 31** | All critical feedback resolved |

---

## 🚨 Risk Mitigation

### High-Risk Items

| Risk | Pillar | Mitigation |
|------|--------|-----------|
| Z3 model mapping is complex | 1 | Prototype typed mapping on simple types first; use Z3 Python bindings for exploration |
| Linear type enforcement requires type system rewrite | 3 | Limit to "Move" semantic; defer full borrow checker to v1.1 |
| Differential testing infra is heavy | 4 | Use existing golden test suite; don't expand coverage yet |
| aura pkg requires ecosystem trust | 5 | Soft launch: internal-only registry first, public later |

### Contingency Cuts

If timeline slips:
1. **Week 20 → Defer:** aura pkg signing (use unsigned temporarily)
2. **Week 22 → Defer:** std.net audit (ship as experimental)
3. **Week 24 → Defer:** Full documentation refresh (minimal updates only)

---

## 📊 Resource Allocation (Recommended)

- **Pillar 1 (Explainability):** 2 engineers, 8 weeks
- **Pillar 2 (Performance):** 1 engineer (part-time profiling), 8 weeks
- **Pillar 3 (Memory Safety):** 2 engineers, 8 weeks
- **Pillar 4 (Backend Trust):** 1 engineer + CI specialist, 6 weeks
- **Pillar 5 (Ecosystem):** 2 engineers, 10 weeks (overlaps Phase 3)

**Total:** ~8–9 FTE for 6 months

---

## ✅ Next Steps (This Week)

1. [ ] Review this plan in team sync
2. [ ] Assign track leads (A–D, Phase 1)
3. [ ] Create GitHub milestones for Weeks 1–8
4. [ ] Schedule weekly sync: Pillar 1 ↔ Pillar 2 (cross-dependency)
5. [ ] Prototype: Z3 model mapper (spike, 2 days)
6. [ ] Prototype: Variable trace rendering (spike, 2 days)

---

**Document Owner:** @danie  
**Last Updated:** January 7, 2026  
**Next Review:** Weekly, Mondays 10 AM (team sync)
