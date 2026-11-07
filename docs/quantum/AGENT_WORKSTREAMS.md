# Quantum agent workstreams — parallel execution without shared-file collisions

**2026-11-06 LIVE correction:** Agent C is now actively developing Q28 in [Draft PR #99](https://github.com/danielforface/aura-lang/pull/99) through [#108](https://github.com/danielforface/aura-lang/pull/108); Q28 has real Rust simulator code and is branch-level IN_PROGRESS (NOT yet integrated/released/fully qualified). Agent A remains Core Draft #57; Agent B Lumina chain has moved through Draft #106. See [Delivery policy](../qualification/DELIVERY_OPERATING_POLICY_V1.md) and [Agent C immediate order](../qualification/agents/AGENT_C_QUANTUM_ORDER.md). Historical planning status is superseded; other Quantum Q gates remain future proposed/not qualified.

## Ownership proposals

| Lane | Q gates | Exclusive proposed files | Can begin before Quantum IR freeze? | Blocker to final qualification |
|---|---|---|---|---|
| Quantum Governance / research | Q19, Q38 | `docs/quantum/`, independent qualification specs | YES | Maintainer approval and external science review |
| Type/Effects | Q20–Q21 | `aura-quantum-types/` proposed | Interfaces/spec only | Core AST/sema contract version |
| Quantum IR | Q22–Q23 | `aura-quantum-ir/`, `aura-quantum-semantics/` proposed | YES, with prototypes | Core IR semantic bridge approval |
| Equivalence checker | Q24 | `aura-quantum-verify/` proposed | YES, stand-alone circuits | Frozen semantics, checker provenance |
| Optimizer and router | Q25–Q26 | `aura-quantum-opt/`, `aura-quantum-target/` | After IR interfaces | Q24 checked translation profiles |
| Interoperability | Q27 | `aura-quantum-interop/` | YES, file-format fixtures | QIR / OpenQASM pass conformance |
| Simulation | Q28 | `aura-quantum-sim/` | YES, mathematical reference | Q23 scientific oracle |
| Runtime/cloud | Q29–Q30 | `aura-quantum-runtime/` + provider folders | Mock only until consent | Auth/cost/permissions/ABI |
| Noise/QEC/Pulse | Q31–Q33 | separate `aura-quantum-noise/qec/pulse/` | Research/test fixtures | Hardware semantics, scientific review |
| Algorithm and SDK | Q34, Q36 | `aura-quantum-stdlib/`, `sdk/quantum/` | Standalone canonical samples | Typed syntax/IR versions |
| Sentinel Studio | Q35 | `editors/sentinel-app/src/quantum/` | YES, mocked read-only UI | Versioned evidence and LSP contract |
| Independent QA | Q37 | `tests/quantum-qualification/` | YES | Feature implementer cannot self-approve |

## Branch and documentation rules

1. Register claim in a GitHub Issue: Q ID, exact paths, allowed dependencies, immutable base commit, evidence links, owner, and external reviewers.
2. One writer per owned path; all others can READ. Shared `docs/qualification/{PROGRESS,CONTEXT_HANDOFF,EVIDENCE_LOG}.md` are **integration-agent owned** and updated in a serialized plan/qualification branch, never independently by every quantum agent.
3. Agent creates only a focused feature branch and Draft PR. No direct `main`, no merge/force push, no stealth rebase, no edits to V1/V2 or Agent A/B owned code.
4. Reference external branch status via GitHub Issues/PRs; do not assume Git branches share unstaged documentation changes.
5. Contract RFC, freeze, versioned JSON schema and stub/golden tests precede shared API modifications. Changes trigger consumer confirmation and forced negative tests.
6. Produce RED→GREEN evidence when correctness defect exists, but no forced fabricated RED for green initial baseline. Run CI on final docs-inclusive commit SHA.
7. Independent QA agent approves proof status only in declared scope; unsupported/timeout is never upgraded to PROVEN.
8. A single integration agent assembles reviewed contract-compatible PRs; cross-platform/QPU release remains separate GO/NO-GO.

## Immediate autonomous starting tickets (not opened yet)

- Q19-001 to Q19-006 — Charter, glossary, assurance and profiles; doc/science review only.
- Q27-001/Q27-003 — OpenQASM parser/QIR Base independent fixture and schema harness (no compiler modification).
- Q28-001/Q28-002 — Seeded reference statevector with Bell and GHZ golden vectors.
- Q24-001/Q24-002 — Bounded checker taxonomy and exact small-circuit equivalence fixtures.
- Q35-001/Q35-002 — Sentinel proof-status and MAY_ENTANGLE UI mock with stale-epoch tests.
- Q37-001/Q37-006 — Test/claim model and quantum independent CI skeleton.

## Merge / deployment limits

No actual PR opened except this proposal; no claim of Agent assignment without issue ownership confirmation. No cloud credentials, QPU reservation or billable execution authorized. This document is an execution contract only upon technical plan acceptance.


## Live agent C delivery correction — 2026-11-06

A Q28 reference simulator already exists on unmerged branch and has scientific parity/regression support; do not create a new scaffold, do not call the program wholly NOT_STARTED, and do not open one new stacked Draft per numerical test. Agent C next must build an actual callable supported simulator capability, then submit as one meaningful reviewable slice. Prior 4-deep Draft stack is retained pending explicit steward merge plan and independent scientific review. Central docs are **not** edited on each exploratory commit. [Agent C instructions](../qualification/agents/AGENT_C_QUANTUM_ORDER.md).


## Mandatory shared-interface and per-phase re-entry (2026-11-06)

All Quantum and non-Quantum agents must apply [root AGENTS.md](../../AGENTS.md), [Phase Lifecycle](../qualification/PHASE_LIFECYCLE_V1.md), [Cross-Agent Protocol](../qualification/CROSS_AGENT_COORDINATION_V1.md) and the [Universal “ממשיכים” Prompt](../qualification/CONTINUE_AGENT_PROMPT.md) at **every** phase, not only at onboarding. Quantum may develop independently against stable versioned source interfaces. Any change to Aura Core quantum ABI, LSP, IR effect contract, target schema or common workflow is D2/D3 and requires producer-consumer approval plus independent tests. C's existing #108 dynamic simulator/JSON CLI is real; don't ask it to restart C-001. All shared source ancestors require D4 integration review before a claim of clean main merge.
