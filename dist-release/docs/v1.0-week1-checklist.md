# Aura v1.0 Phase 1 — Week 1 Kickoff Checklist

**Week 1 of 26**  
**Date:** January 13–17, 2026  
**Gate Criteria:** All 4 tracks have spike prototypes + working branches

---

## 🎯 Weekly Goals

### Overall
- [ ] All 4 tracks kickoff with assigned leads
- [ ] Spike prototypes complete (Z3 mapper, variable traces, debugger UI sketch, linear types draft)
- [ ] GitHub project board populated (Weeks 1–8)
- [ ] CI/CD pipeline ready for rapid iteration

---

## 📋 Track A: Pillar 1 — Explainability Engine

### Task 1a: Z3→AST Typed Mapper (Spike)

**Owner:** TBD (Rust + Z3 expertise)  
**Status:** [ ] Not started

**Spike Objective (2 days, Jan 13–14):**
- [ ] Study Z3 Python bindings output format
- [ ] Design TypedValue enum (primitives, records, arrays)
- [ ] Create mock Z3→TypedValue for u32, bool, simple struct
- [ ] Proof-of-concept: Can we round-trip a simple value?

**Deliverable:**
```rust
pub enum TypedValue {
  Primitive(String, String),  // type_name, value
  Record { fields: Vec<(String, TypedValue)> },
  Enum { variant: String, value: Box<TypedValue> },
  Array { element_type: String, elements: Vec<TypedValue> },
}

fn map_primitive(z3_value: &str, aura_type: &str) -> TypedValue { }
```

**Success Criteria:** [ ] Can map u32(42), bool(true), struct { x: 1, y: 2 }

**Effort:** 1 FTE week 1–2  
**Risk:** Medium (Z3 API learning curve)

---

### Task 1b: Variable Trace Enhancement (Spike)

**Owner:** TBD (LSP expertise)  
**Status:** [ ] Not started

**Spike Objective (2 days, Jan 13–14):**
- [ ] Examine current `ProofSummary.assumptions` (from v0.3)
- [ ] Design variable trace JSON schema:
  ```json
  {
    "variable": "x",
    "defined_at": { "uri": "file://...", "range": {...} },
    "last_assigned_at": { "uri": "...", "range": {...} },
    "value_at_proof": "42"
  }
  ```
- [ ] Wire into LSP `Diagnostic.data.variableTrace`
- [ ] Test: Emit trace for 1 variable in a failing assert

**Deliverable:** LSP diagnostic now includes `variableTrace` array

**Success Criteria:** [ ] `aura-lsp --verify test.aura` shows 1+ variable traces in output

**Effort:** 1 FTE week 1–2  
**Risk:** Low (straightforward LSP extension)

---

## 📋 Track B: Pillar 2 — Performance Tuning

### Task 2a: Profiling Infrastructure (Spike)

**Owner:** TBD (Telemetry expertise)  
**Status:** [ ] Not started

**Spike Objective (1.5 days, Jan 13):**
- [ ] Create `aura-lsp/src/profiling.rs` module
- [ ] Define `ProofMetrics` struct (function, parse_ms, sema_ms, verify_ms, cache_hit)
- [ ] Instrument 1 function path (e.g., `run_verify()`) with timing
- [ ] Emit metrics to LSP telemetry endpoint
- [ ] Sentinel can receive + store metrics (no UI yet)

**Deliverable:**
```rust
pub struct ProofMetrics {
  pub function: String,
  pub parse_ms: u64,
  pub sema_ms: u64,
  pub verify_ms: u64,
  pub cache_hit: bool,
}
```

**Success Criteria:** [ ] Profiling data flows LSP → Sentinel log

**Effort:** 0.5 FTE week 1  
**Risk:** Low

---

### Task 2b: Z3 Solver Profiling Setup (Spike)

**Owner:** TBD (Verifier expertise)  
**Status:** [ ] Not started

**Spike Objective (1.5 days, Jan 14–15):**
- [ ] Measure Z3 `push/pop` overhead on 10 incremental solves (existing test suite)
- [ ] Compare vs `check-sat-assuming` on same problems
- [ ] Document findings: which is faster? By how much?
- [ ] Hypothesis: for <100 vars, push/pop faster; for >1000, check-sat-assuming wins

**Deliverable:** Benchmark report (markdown) with data

**Success Criteria:** [ ] Have clear data on push/pop vs check-sat-assuming tradeoff

**Effort:** 0.5 FTE week 1  
**Risk:** Low

---

## 📋 Track C: Pillar 4 — Backend Trust Gate

### Task 4a: Sentinel Debugger UI Sketch (Spike)

**Owner:** TBD (Sentinel/TypeScript expertise)  
**Status:** [ ] Not started

**Spike Objective (2 days, Jan 13–14):**
- [ ] Create `editors/sentinel-app/src/debugPanel.ts` (stub)
- [ ] UI: "Debug (Native)" button + breakpoint gutter markers
- [ ] Stub GDB/LLDB launcher (using existing `debugger.rs` from v0.3)
- [ ] Can we hit a breakpoint in hello_world.aura? (manual test)
- [ ] Can we read 1 variable from stack frame?

**Deliverable:** Basic debugger panel UI + manual test video

**Success Criteria:** [ ] Breakpoint stops execution; variable is visible (not auto-formatted yet)

**Effort:** 1 FTE week 1–2  
**Risk:** Medium (GDB/LLDB protocol learning)

---

### Task 4b: Differential Test CI Skeleton (Spike)

**Owner:** TBD (CI/DevOps)  
**Status:** [ ] Not started

**Spike Objective (1 day, Jan 15):**
- [ ] Create `ci/differential_test.yml` (GitHub Actions)
- [ ] Can it build 1 test with Dev-VM, C, LLVM backends?
- [ ] Can it diff outputs (should be identical)?
- [ ] On mismatch, does it save regression fixture?

**Deliverable:** Working CI pipeline (runs on every commit)

**Success Criteria:** [ ] Green CI for baseline; fixture saved on failure

**Effort:** 0.5 FTE week 1  
**Risk:** Low (infrastructure mostly exists)

---

## 📋 Track D: Pillar 3 — Memory Safety

### Task 3a: Linear Type Enforcement Design (Spike)

**Owner:** TBD (Type system expertise)  
**Status:** [ ] Not started

**Spike Objective (2 days, Jan 13–14):**
- [ ] Study current `aura-core/src/type_system.rs`
- [ ] Design `Ownership` enum:
  ```rust
  pub enum Ownership {
    Owned,        // can be moved
    Borrowed,     // immutable borrow
    BorrowedMut,  // exclusive mutable
    Moved,        // error state
  }
  ```
- [ ] Propose: where to add this metadata in TypeRef/TypeInfo?
- [ ] Write spec: type-checking rules for move detection
- [ ] Prototype: reject 1 simple use-after-move case

**Deliverable:** Design doc + 1 failing test (expected to be rejected)

**Success Criteria:** [ ] `let x = 5; let y = x; print(x);` → type error

**Effort:** 1 FTE week 1–2  
**Risk:** High (type system complexity)

---

### Task 3b: Region Stdlib Test Suite Scaffold (Spike)

**Owner:** TBD (Test/Verifier)  
**Status:** [ ] Not started

**Spike Objective (1 day, Jan 15):**
- [ ] Create `tests/fuzz_region_collections.aura` (skeleton)
- [ ] Outline: random Vec/HashMap operations (push, get, insert, remove)
- [ ] Define property invariants (len ≤ capacity, no OOB)
- [ ] Can we run 100 random ops without crash? (current state)

**Deliverable:** Fuzz harness ready for Week 4–8 hardening

**Success Criteria:** [ ] Fuzz runs, collects stats (current: probably unsafe)

**Effort:** 0.5 FTE week 1  
**Risk:** Low

---

## 🔄 Cross-Track Sync

**Weekly Sync:** Every Monday 10 AM  
**Agenda:**
- [ ] Each track: spike status (blocker check)
- [ ] Pillar 1 ↔ Pillar 2: Is Explain panel performance-aware?
- [ ] Pillar 3 ↔ Pillar 4: Does linear type checker + debugger work together?

---

## 📊 Week 1 Success Criteria

**All Tracks Must Have:**
- [ ] Spike prototype working (can demo)
- [ ] GitHub branch ready for Week 2 (not merged yet)
- [ ] 2–3 unit tests passing (for infrastructure)
- [ ] Blockers identified + mitigation plan
- [ ] Estimate: effort/complexity for Weeks 2–4

**Team Confidence:** Can we commit to Phase 1 Gate (Feb 28) with current spikes?

---

## 📝 Handoff to Week 2 (Jan 20)

**Track A → Week 2:**
- [ ] Expand TypedValue mapper to records + enums
- [ ] Add 4+ unit tests per type case
- [ ] Measure Z3 API performance (model extraction speed)

**Track B → Week 2:**
- [ ] Telemetry UI in Sentinel (chart of times over edit history)
- [ ] Profile aura-lsp on real project (1,000-line file)
- [ ] Initial optimization pass (low-hanging fruit)

**Track C → Week 2:**
- [ ] Debugger: set breakpoint → stop at breakpoint (functional)
- [ ] CI: add more golden tests to suite (>10)
- [ ] Document GDB/LLDB protocol integration plan

**Track D → Week 2:**
- [ ] Type-checker: enforce move rule in full scope
- [ ] Add error diagnostics (show move site)
- [ ] Region Vec: implement 1 operation (push) with proof

---

## 🎯 This Week's Daily Standups

**Standup Format:** 15 min, 4:30 PM (after hours)

**Mon 1/13:** Spike kickoff + unblock checklist  
**Tue 1/14:** Track progress check  
**Wed 1/15:** Mid-spike blocker discussion  
**Thu 1/16:** Integration planning (tracks start cross-talking)  
**Fri 1/17:** Week 1 wrap + Week 2 readiness  

---

**Document:** v1.0-week1-checklist.md  
**Owner:** @danie  
**Last Updated:** January 7, 2026  
**Next Update:** Friday, January 17, 2026 (after Week 1 closeout)
