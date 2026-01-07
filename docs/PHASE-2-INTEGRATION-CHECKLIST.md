# Phase 2 Master Integration Checklist

**Date:** January 7, 2026  
**Status:** Planning → Implementation  
**Duration:** 4 weeks (Feb 1 - Feb 28, 2026)  
**Owner:** Aura Team  

---

## Overview

Phase 1 completed: Pillars 1-4 (Explainability, Performance, Memory Safety, Differential Testing)

Phase 2 focuses on **full IDE integration** and **real-world usage validation**. All four pillars must work together seamlessly in a production environment.

### Goals

- ✅ Explain panel renders real proof failures from Sentinel LSP
- ✅ Profiling dashboard receives live telemetry from verification
- ✅ Debugger launches and controls real GDB/LLDB processes
- ✅ Memory-safe code proven end-to-end
- ✅ Sub-200ms P95 latency measured in practice
- ✅ Zero integration blockers (all components tested together)

---

## Pillar 1 Integration: Explainability Panel

### 1.1 Connect LSP Diagnostics to UI

**Owner:** Frontend Lead  
**Effort:** 8 hours  

- [ ] **Extract counterexample from LSP Diagnostic.data**
  - [ ] Parse `aura.counterexample.v2` schema in main.ts
  - [ ] Map Z3 model values to TypedValue tree
  - [ ] Handle nested structures (records, arrays, enums)
  - [ ] Test: 5+ different value types

- [ ] **Extract variable traces from LSP**
  - [ ] Parse variable_traces array from Diagnostic.data
  - [ ] Build timeline UI showing def→assign→final
  - [ ] Highlight variables in source on hover
  - [ ] Test: trace rendering for simple functions

- [ ] **Extract repair hints from LSP**
  - [ ] Parse repairHints array from Diagnostic.data
  - [ ] Render suggestion cards with icons
  - [ ] Implement "Apply Repair" button → WorkspaceEdit
  - [ ] Test: 3 repair types (strengthen, weaken, add invariant)

### 1.2 User Interactions

**Owner:** Frontend Lead  
**Effort:** 6 hours  

- [ ] **Expand/collapse value trees**
  - [ ] Click to expand nested values
  - [ ] Remember expansion state
  - [ ] Smooth animations
  - [ ] Test: deeply nested structures

- [ ] **Click variable to jump to source**
  - [ ] Extract location from trace
  - [ ] Go to definition / all references
  - [ ] Highlight occurrences in editor
  - [ ] Test: navigation accuracy

- [ ] **Click repair hint to apply**
  - [ ] Parse suggested code change
  - [ ] Create WorkspaceEdit
  - [ ] Send to LSP for verification
  - [ ] Re-run verifier on modified file
  - [ ] Test: 3 repair applications

### 1.3 Edge Cases

**Owner:** Frontend Lead  
**Effort:** 4 hours  

- [ ] **Large counterexamples**
  - [ ] Limit depth display (max 5 levels)
  - [ ] Show `...` for overflow
  - [ ] Lazy-load deep structures
  - [ ] Test: 1000+ field struct

- [ ] **Null/undefined values**
  - [ ] Render as special (gray, italic)
  - [ ] Don't crash on missing fields
  - [ ] Test: null pointers, None values

- [ ] **Non-UTF8 strings**
  - [ ] Handle binary data gracefully
  - [ ] Show hex representation
  - [ ] Test: embedded nulls, non-ASCII

---

## Pillar 2 Integration: Performance Telemetry

### 2.1 LSP → Dashboard Telemetry Stream

**Owner:** Backend Lead  
**Effort:** 10 hours  

- [ ] **LSP emits VerificationTelemetry on proof complete**
  - [ ] Add telemetry event to LSP completion
  - [ ] Include function name, file, latency breakdown
  - [ ] Track phase durations (parse, semantic, normalize, z3)
  - [ ] Include cache hit/miss status
  - [ ] Test: LSP emits for 10 functions

- [ ] **Sentinel receives telemetry stream**
  - [ ] Listen to `aura/telemetry` events
  - [ ] Parse VerificationTelemetry payload
  - [ ] Buffer 50 samples for trending
  - [ ] Test: stream remains live for 30+ events

- [ ] **Dashboard updates in real-time**
  - [ ] Metrics cards update as new samples arrive
  - [ ] P95 recalculates on each event
  - [ ] Recommendations regenerate
  - [ ] Trend chart scrolls with history
  - [ ] Test: latency changes reflect in UI

### 2.2 Performance Targets

**Owner:** Verifier Lead  
**Effort:** 12 hours  

- [ ] **Measure P95 < 200ms on test suite**
  - [ ] Run 100 verifications on stdlib
  - [ ] Calculate P50, P95, P99
  - [ ] Log outliers and root causes
  - [ ] Tune Z3 params if needed
  - [ ] Test: hit target on 3 benchmarks

- [ ] **Cache effectiveness > 50%**
  - [ ] Implement cache key isolation
  - [ ] Measure hit rate per file
  - [ ] Identify cache thrashing patterns
  - [ ] Optimize invalidation logic
  - [ ] Test: 50%+ hit rate across suite

- [ ] **Dashboard shows realistic metrics**
  - [ ] No artificial data
  - [ ] Live from real verifier
  - [ ] Recommendations are actionable
  - [ ] Test: dashboard matches verifier logs

### 2.3 Recommendations Engine

**Owner:** Backend Lead  
**Effort:** 6 hours  

- [ ] **Generate contextual recommendations**
  - [ ] If P95 > 200ms: suggest fast profile
  - [ ] If cache < 30%: suggest splitting functions
  - [ ] If outliers exist: check for timeouts
  - [ ] Test: 5 different recommendation types

- [ ] **Make recommendations actionable**
  - [ ] Include code snippets
  - [ ] Point to specific functions
  - [ ] Suggest concrete changes
  - [ ] Test: user can apply directly

---

## Pillar 3 Integration: Memory Safety Proof

### 3.1 Type System Integration

**Owner:** Compiler Lead  
**Effort:** 8 hours  

- [ ] **Wire linear_types.rs into type checker**
  - [ ] On variable definition: Ownership = Owned
  - [ ] On move: Ownership = Moved
  - [ ] On borrow: Ownership = BorrowedImmut/Mut
  - [ ] On use: check Ownership allows it
  - [ ] Test: reject 5 use-after-move examples

- [ ] **Region-based stdlib checks**
  - [ ] Type check std::collections_region::Vec[T]
  - [ ] Verify bounds before indexing
  - [ ] Prove no out-of-bounds access
  - [ ] Test: stdlib module fully verified

- [ ] **Error messages point to root cause**
  - [ ] "moved at line X"
  - [ ] "last used at line Y"
  - [ ] Suggest fixes
  - [ ] Test: 3 error scenarios

### 3.2 Verification of Safe Code

**Owner:** Verifier Lead  
**Effort:** 10 hours  

- [ ] **Prove no use-after-free in verified stdlib**
  - [ ] Run verifier on Vec<T>, HashMap<K,V>
  - [ ] All operations prove safety
  - [ ] No capability violations
  - [ ] Test: all stdlib functions verified

- [ ] **Prove no data races in concurrent code**
  - [ ] Use Mutex, Channel, Atomic types
  - [ ] Verifier checks race-freedom
  - [ ] Memory ordering is correct
  - [ ] Test: producer/consumer pattern

- [ ] **Quantified proofs for generic types**
  - [ ] Vec::get(i) is safe for all i
  - [ ] HashMap operations don't corrupt state
  - [ ] Test: 10 generic proofs

### 3.3 Debugging Memory Issues

**Owner:** Debugger Lead  
**Effort:** 6 hours  

- [ ] **Debugger shows ownership state**
  - [ ] Variable panel: name, type, ownership
  - [ ] Highlight moved variables
  - [ ] Show scope/lifetime
  - [ ] Test: 3 debugging scenarios

- [ ] **Breakpoint at move site**
  - [ ] Break when value is moved
  - [ ] Show what moves it
  - [ ] Inspect pre-move state
  - [ ] Test: move tracking

---

## Pillar 4 Integration: Debugger + Verification

### 4.1 MI Protocol Implementation

**Owner:** Debugger Lead  
**Effort:** 12 hours  

- [ ] **GDB MI protocol completeness**
  - [ ] exec-run, exec-continue, exec-step, exec-next
  - [ ] exec-finish (step out)
  - [ ] break-insert, break-delete, break-modify
  - [ ] stack-list-frames, stack-list-variables
  - [ ] var-create, var-list-children (expressions)
  - [ ] data-read-memory (inspect arrays)
  - [ ] Test: each command with GDB/LLDB

- [ ] **LLDB MI compatibility**
  - [ ] Same commands work on macOS/Linux
  - [ ] Auto-detect GDB vs LLDB
  - [ ] Fallback for missing features
  - [ ] Test: both debuggers on 5 scenarios

- [ ] **Robust error handling**
  - [ ] Timeout after 10s
  - [ ] Retry on transient failure
  - [ ] Clear error messages
  - [ ] Test: 5 error scenarios

### 4.2 Sentinel Debugger Panel

**Owner:** Frontend Lead  
**Effort:** 10 hours  

- [ ] **Toolbar controls**
  - [ ] Launch, Continue, Pause, Step Over, Step Into, Step Out
  - [ ] Enable/disable based on state
  - [ ] Keyboard shortcuts (F5, F10, F11, Shift+F11)
  - [ ] Test: control flow

- [ ] **Stack frame display**
  - [ ] List all frames with function names
  - [ ] Click to select frame
  - [ ] Show file:line:column
  - [ ] Test: deeply nested calls

- [ ] **Variable inspection**
  - [ ] Auto-expand locals in current frame
  - [ ] Show type, value, address
  - [ ] Expand nested structures
  - [ ] Test: 10 variable types

- [ ] **Watch expressions**
  - [ ] Add arbitrary expressions
  - [ ] Evaluate on stop
  - [ ] Show value updates
  - [ ] Test: 5 watch scenarios

- [ ] **Breakpoint management**
  - [ ] Toggle via margin click
  - [ ] Conditional breakpoints
  - [ ] Hit count tracking
  - [ ] Persistent across sessions
  - [ ] Test: breakpoint lifecycle

### 4.3 Differential Testing CI Gate

**Owner:** CI Lead  
**Effort:** 8 hours  

- [ ] **Build on all three backends**
  - [ ] Dev-VM (interpreter)
  - [ ] C backend (gcc/clang)
  - [ ] LLVM backend (LLVM IR → native)
  - [ ] Test: build succeeds on all

- [ ] **Run golden tests**
  - [ ] Same test on each backend
  - [ ] Compare output/behavior
  - [ ] Report discrepancies
  - [ ] Test: 20 test cases

- [ ] **Fail build on backend mismatch**
  - [ ] Detect divergence in Dev-VM vs C vs LLVM
  - [ ] Flag as regression
  - [ ] Auto-save MRE fixture
  - [ ] Notify team
  - [ ] Test: introduce artificial divergence, verify detection

### 4.4 Real Debugging Sessions

**Owner:** Debugger Lead  
**Effort:** 8 hours  

- [ ] **Debug a simple function**
  - [ ] Set breakpoint at line
  - [ ] Step through body
  - [ ] Inspect variables
  - [ ] Modify and continue
  - [ ] Verify behavior change
  - [ ] Test: hello_world.aura example

- [ ] **Debug a recursive function**
  - [ ] Pause in recursive frame
  - [ ] Walk up/down stack
  - [ ] Inspect frame-local state
  - [ ] Verify invariants hold
  - [ ] Test: fibonacci.aura

- [ ] **Debug a concurrent program**
  - [ ] Break in multiple threads
  - [ ] Switch between threads
  - [ ] Inspect shared state
  - [ ] Verify no race condition
  - [ ] Test: producer/consumer example

---

## Cross-Pillar Integration Tests

### Integration Test Suite

**Owner:** QA Lead  
**Effort:** 16 hours  

#### Test 1: Prove + Explain + Debug
```
1. Write function with assertion
2. Verification fails
3. Explain panel shows counterexample (Pillar 1)
4. Profiling shows P95 = 120ms (Pillar 2)
5. Click variable → jumps to line in debugger (Pillar 4)
6. Debugger shows state at failure point (Pillar 4)
7. Apply repair hint (Pillar 1)
8. Re-verify → passes (Pillar 1, 2)
```
- [ ] End-to-end test script
- [ ] Automated verification
- [ ] Screenshot validation

#### Test 2: Memory Safety Proof + Debugging
```
1. Write Vec[T] code with potential UB
2. Type system prevents compile (Pillar 3)
3. Fix to use verified Vec from stdlib
4. Verification proves safety (Pillar 1, 3)
5. Debug session shows variables safely (Pillar 4)
6. Performance meets targets (Pillar 2)
```
- [ ] Test multiple stdlib types
- [ ] Verify each operation
- [ ] Debug under load

#### Test 3: Performance Optimization Loop
```
1. Prove function (Pillar 1)
2. Dashboard shows P95 = 250ms (Pillar 2)
3. Recommendation: break into smaller pieces
4. Apply suggestion (Pillar 1)
5. Re-measure P95 = 180ms (Pillar 2)
6. Cache hit rate improves (Pillar 2)
```
- [ ] Benchmark before/after
- [ ] Automate optimization
- [ ] Validate metrics

#### Test 4: Differential Testing Catches Bug
```
1. Modify C backend code
2. Introduce subtle divergence from LLVM
3. Run differential test (Pillar 4)
4. Catch divergence automatically
5. Generate MRE
6. Notify team with regression report
```
- [ ] Inject bugs in backends
- [ ] Verify detection
- [ ] Validate MRE minimization

---

## Milestone Timeline

### Week 1 (Feb 1-7): Pillar 1 Integration
- [ ] LSP diagnostics → Explain panel
- [ ] Variable traces rendering
- [ ] Repair hints UI
- [ ] **(Target: 15 integration tests passing)**

### Week 2 (Feb 8-14): Pillar 2 Integration  
- [ ] Telemetry stream implementation
- [ ] Dashboard real-time updates
- [ ] Performance tuning validation
- [ ] **(Target: P95 < 200ms on test suite)**

### Week 3 (Feb 15-21): Pillar 3 Integration
- [ ] Type checker wiring
- [ ] Stdlib verification
- [ ] Debugger memory state
- [ ] **(Target: All stdlib functions verified)**

### Week 4 (Feb 22-28): Pillar 4 + Cross-Pillar
- [ ] MI protocol hardening
- [ ] Debugger panel polish
- [ ] Differential testing CI gate
- [ ] **(Target: 4 integration tests passing, zero blockers)**

---

## Risk Mitigation

### High-Risk Items

| Risk | Probability | Impact | Mitigation |
| --- | --- | --- | --- |
| Telemetry stream lags | Medium | High | Buffer async, test latency SLA |
| Memory inspection crashes | Medium | High | Robust null checks, fuzzing |
| Debugger hangs on timeout | Medium | High | Implement 10s timeout, retry logic |
| Differential test false positives | High | Medium | Determinism fixes, seed control |
| Proof latency regression | Medium | High | Benchmark on each commit, CI gate |

### Fallback Plans

- **If telemetry slow:** Buffer locally, send batched
- **If debugger unstable:** Ship with "experimental" flag, option to disable
- **If differential tests flaky:** Lock to known-good commit, investigate separately
- **If P95 misses:** Reduce verification scope, split into simpler proofs

---

## Success Criteria

### Functional Completeness

- ✅ All 4 pillars integrated (no silos)
- ✅ Zero crashes in typical workflows
- ✅ All UI components responsive (<100ms)
- ✅ Debugger works on GDB + LLDB
- ✅ Performance metrics accurate and actionable

### Performance

- ✅ P95 latency < 200ms (measured live)
- ✅ Dashboard updates within 500ms of proof complete
- ✅ Debugger responds to commands within 1s
- ✅ UI stays responsive during heavy proving

### Quality

- ✅ 30+ integration tests (all passing)
- ✅ 0 known blockers or critical bugs
- ✅ Differential tests detect intentional divergence
- ✅ Memory safety proven for all stdlib

### Usability

- ✅ New user can complete debug workflow in 5 min
- ✅ Error messages are clear and actionable
- ✅ Help docs link from each component
- ✅ Example projects demonstrate all features

---

## Sign-Off

**Phase 2 Ready to Begin:** January 7, 2026  
**Expected Completion:** February 28, 2026  
**Owner:** Aura Team Lead  

---

## Appendix: Component Dependencies

```
Pillar 1 (Explainability)
  ├─ LSP Diagnostics (backend)
  ├─ explainPanel.tsx (frontend)
  ├─ explain_integration.ts (wiring)
  └─ Tests: 8/8 passing

Pillar 2 (Performance)
  ├─ performance_tuning.rs (backend)
  ├─ profiling_dashboard.tsx (frontend)
  ├─ profiling_integration.ts (wiring)
  └─ Tests: 10/10 passing

Pillar 3 (Memory Safety)
  ├─ linear_types.rs (compiler)
  ├─ region_stdlib.rs (stdlib)
  ├─ Type checker integration
  └─ Tests: 28/28 passing

Pillar 4 (Differential Testing)
  ├─ mi_client.ts (debugger protocol)
  ├─ debugger_integration.ts (abstraction)
  ├─ debugger_panel_integration.ts (UI)
  ├─ debugPanel.ts (legacy)
  ├─ .github/workflows/differential_test.yml (CI)
  └─ Tests: 6/6 passing

All Dependencies: 100% Complete ✅
```
