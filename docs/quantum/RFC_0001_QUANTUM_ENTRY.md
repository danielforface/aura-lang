# RFC-0001 — Aura Quantum program admission

**Status:** Strategic direction APPROVED by maintainer (2026-11-05). **Technical interfaces:** DRAFT. **Implementation:** NOT_STARTED. **Qualification:** NONE. **Project release:** unchanged.

## Decision record

- Aura WILL enter quantum programming as a first-class optional domain.
- Quantum must extend existing proof-first identity without changing the truth of established Core or Lumina claims.
- The roadmap is additive, modular and gate-driven: Q19–Q38/R16–R35 after Q00–Q18.
- Roadmap V1/V2 are preserved as historical plan snapshots; Draft planning PRs remain unmerged until review.
- Leadership is a measurable target: verifiable interoperability, fault injection, scientific semantics, end-to-end DX and external adoption.

## Architecture decisions still OPEN

| Decision | Required evidence before acceptance |
|---|---|
| Embedded `quantum` syntax vs DSL | Parser/IR ownership impact, source mapping, onboarding evaluation |
| Handle state effects | Destructive/nondestructive semantics and region control-flow formalization |
| Exact checker engines | QCEC interoperability, ZX/DD selection, checker TCB review and nontermination budget |
| Supported OpenQASM/QIR versions | Conformance subset, external validators and explicit unsupported policy |
| Target backend adapters | Capability schema, permissions, budget, privacy, calibration expiration |
| Simulation engine | Small exact reference and numerical error budgets |
| QEC/pulse boundary | Separate scientific/hardware review; experimental release tier |
| Quantum LSP protocol | Source spans, proof statuses, stale-epoch handling and global cache keys |

## Anti-scope-creep rule

No changes to A/B agents, no main direct writes, no plan-in-place changes, no automatic QPU spending, and no claim that future goals are already supported. RFC updates are reviewed, linked to stable task IDs and append evidence. All work starts from explicit Issue ownership.
