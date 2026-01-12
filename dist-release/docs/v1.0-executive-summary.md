# Aura v1.0 "Daily Driver" — Executive Summary

**Status:** Strategic Planning Phase (January 7, 2026)  
**Target Release:** July 31, 2026 (26 weeks from now)  
**Scope:** 5 Strategic Pillars with 4 parallel Phase 1 tracks

---

## 🎯 The 5 Strategic Pillars

### **Pillar 1: Explainability Engine** 🧠
*Developer diagnoses proof failures in <5 seconds*

| Component | Timeline | Priority | Effort |
|-----------|----------|----------|--------|
| Z3→AST Typed Mapper | Weeks 1–3 | **P0** | Medium |
| Variable Trace Enhancement | Weeks 2–4 | **P0** | Low |
| Sentinel Explain Panel | Weeks 3–4 | **P0** | Medium |

**Key Deliverable:** `aura-verify/src/counterexample_mapper.rs` — converts SMT models to Aura AST structures (records, enums, arrays) with recursive descent.

**Success:** Developer fixes a logical bug using only the Explain panel, no SMT knowledge required.

---

### **Pillar 2: Incremental Proof Pipeline** ⚡
*Proofs indistinguishable from typing speed (<200ms)*

| Component | Timeline | Priority | Effort |
|-----------|----------|----------|--------|
| Profiling Infrastructure | Weeks 1–2 | **P0** | Low |
| Z3 Solver Tuning | Weeks 2–4 | **P0** | Medium |
| Cache Optimization | Weeks 3–4 | **P0** | Low |

**Key Win:** 30–50% latency reduction via `push/pop` + `check-sat-assuming` tuning.

**Target Metric:** p95 latency = **<200ms** for typical edit cycle (1,000 lines).

---

### **Pillar 3: Memory Safety Identity** 🔒
*Zero segfaults in Safe mode; deterministic memory lifecycle*

| Component | Timeline | Priority | Effort |
|-----------|----------|----------|--------|
| Linear Type Enforcement | Weeks 3–6 | **P1** | High |
| Region Stdlib Hardening | Weeks 4–8 | **P1** | Medium |

**Key Enforcer:** Type-checker rejects all `use-after-move` patterns with "move site" diagnostics.

**Proof Requirement:** All Vec/HashMap operations verified (Z3 passing); 10k fuzz ops = 0 violations.

---

### **Pillar 4: Backend Trust Gate** 🔗
*Proven code = executed code (differential testing)*

| Component | Timeline | Priority | Effort |
|-----------|----------|----------|--------|
| Sentinel Debugger UI | Weeks 1–3 | **P0** | Medium |
| Differential Testing CI | Weeks 2–4 | **P1** | High |

**Key Deliverable:** GDB/LLDB integration in Sentinel; breakpoints, step, watches.

**CI Gate:** Fail builds if Dev-VM ≠ LLVM ≠ C for golden tests.

---

### **Pillar 5: Ecosystem & Platform** 📦
*Build real systems using audited stdlib + package manager*

| Component | Timeline | Priority | Effort |
|-----------|----------|----------|--------|
| aura pkg (package mgr) | Weeks 17–19 | **P1** | High |
| Audited std.net/concurrent | Weeks 19–22 | **P1** | High |
| Documentation | Weeks 22–24 | **P1** | Medium |

**Target:** Developers build TCP server from stdlib in <30 min using reference + examples.

---

## 📊 Phase Breakdown

### **Phase 1: Foundation (Weeks 1–8)**
4 parallel tracks, all essential for v1.0:

```
Track A (Explainability):     Weeks 1–4   │  Z3 Mapper + Variable Traces + Explain Panel
Track B (Performance):         Weeks 1–4   │  Profiling + Solver Tuning + Cache Opts
Track C (Backend Trust):       Weeks 1–4   │  Debugger UI + Differential CI
Track D (Memory Safety):       Weeks 3–8   │  Linear Types + Region Stdlib
```

**Gate:** All P0 tasks complete + integration tests passing (Feb 28).

### **Phase 2: Integration & Hardening (Weeks 9–16)**
- Cross-pillar wiring (Explain ↔ Proof Perf ↔ Linear Types ↔ Debugger)
- Performance sprint to <200ms
- Fuzz testing + regression suite
- **Gate:** <200ms latency achieved (Apr 15)

### **Phase 3: Ecosystem & Polish (Weeks 17–24)**
- aura pkg package manager
- Audited stdlib (net, concurrent)
- Book rewrite + cookbook
- **Gate:** End-to-end ecosystem working (May 31)

---

## ✅ Definition of Done (v1.0 Release)

### Latency ⏱️
- [x] Incremental proofs: **<200ms (p95)** for 1,000-line file
- [x] Explain panel: **<50ms** render time
- [x] Debugger: **<100ms** breakpoint stop

### Safety 🛡️
- [x] Type-checker rejects all use-after-move
- [x] Region Vec/HashMap: **0 segfaults** (10k fuzz ops)
- [x] Differential testing: Dev-VM ≡ LLVM ≡ C (golden suite)

### Utility 🎯
- [x] Developer fixes logic bug using Explain alone (no SMT knowledge)
- [x] Developer builds TCP server from stdlib in <30 min
- [x] New package publishable in <10 min via aura pkg

### Stability 📈
- [x] Stdlib covers 90% of systems programming use cases
- [x] LSP + Sentinel: 99.9% uptime in stress tests
- [x] v1.0 code still compiles in v1.1+ (backward compat)

---

## 📈 Resource & Timeline

**Team:** 8–9 FTE over 6 months  
**Parallel Tracks:** 4 (Days 1–56, Feb 28 gate)  
**Critical Path:** Pillar 1 + Pillar 2 (explainability + performance)

```
Jan 7                  ┌─ Phase 1 (Weeks 1-8, 4 Tracks)
                       │    └─ Feb 28 (P0 Gate)
                       │
                       ├─ Phase 2 (Weeks 9-16, Integration)
                       │    └─ Apr 15 (Latency Gate)
                       │
                       ├─ Phase 3 (Weeks 17-24, Ecosystem)
                       │    └─ May 31 (Ecosystem Gate)
                       │
                       ├─ v1.0 Alpha (Jun 15)
                       ├─ v1.0 Beta (Jul 1)
                       └─ v1.0 Final Release (Jul 31)
```

---

## 🚨 Top Risks & Mitigations

| Risk | Pillar | Mitigation | Backup |
|------|--------|-----------|--------|
| Z3 model mapping is complex | 1 | Prototype simple types first | Use Python binding for exploration |
| Linear types require rewrite | 3 | Limit to Move semantic first | Defer full borrow checker to v1.1 |
| Differential testing infra | 4 | Use existing golden suite | Don't expand coverage, just gates |
| aura pkg ecosystem trust | 5 | Soft launch (internal only) | Public launch in v1.1 |

**Contingency Cuts** (if timeline slips):
- Week 20: Defer aura pkg signing (use unsigned)
- Week 22: Defer std.net audit (experimental label)
- Week 24: Defer full docs (minimal updates only)

---

## 🏁 Next Steps (This Week)

1. **Team Review** → Confirm 5 pillars + phases  
2. **Assign Leads** → Track A/B/C/D owners  
3. **Create Milestones** → GitHub issues for Week 1–8  
4. **Spike: Z3 Mapper** → 2-day prototype (simple types)  
5. **Spike: Variable Traces** → 2-day prototype (LSP emission)  
6. **Weekly Sync** → Monday 10 AM, Pillar 1 ↔ Pillar 2 cross-updates

---

**For detailed breakdown:** See [docs/v1.0-implementation-plan.md](v1.0-implementation-plan.md)

**Document:** v1.0-executive-summary.md  
**Owner:** @danie  
**Last Updated:** January 7, 2026  
**Next Review:** Weekly team sync
