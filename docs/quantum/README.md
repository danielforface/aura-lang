# Aura Quantum — complete product-development program

**Decision:** Quantum WILL be developed as a first-class optional Aura domain (2026-11-05). **Implementation:** NOT_STARTED for Q19–Q38. **No quantum feature is yet qualified.** Maintainer's strategic approval does not certify a technical release or automatically supersede unmerged V2.

## Canonical documents

| Document | Purpose |
|---|---|
| [MASTER_ROADMAP_V3](../qualification/MASTER_ROADMAP_V3.md) | Full inherited Q00–Q18 plus 284 quantum tasks Q19–Q38, R16–R35 |
| [PRODUCT_CHARTER](PRODUCT_CHARTER.md) | Identity, target users, competition and non-goals |
| [SEMANTIC_AND_TRUST_CONTRACT](SEMANTIC_AND_TRUST_CONTRACT.md) | Normative research targets, soundness and physical semantics |
| [ARCHITECTURE_AND_INTEROP](ARCHITECTURE_AND_INTEROP.md) | Stack, QIR/OpenQASM, target profiles and technical interfaces |
| [QUALIFICATION_MATRIX](QUALIFICATION_MATRIX.md) | Test and claim gates with falsification controls |
| [AGENT_WORKSTREAMS](AGENT_WORKSTREAMS.md) | Exclusive ownership, shared evidence and parallel schedule |
| [SCIENTIFIC_REFERENCES](SCIENTIFIC_REFERENCES.md) | Official third-party specification links and reproducible baselines |
| [REFERENCE_PROGRAMS](REFERENCE_PROGRAMS.md) | Small canonical scientific and integration programs |
| [RFC_0001_QUANTUM_ENTRY](RFC_0001_QUANTUM_ENTRY.md) | Explicit major decisions and unresolved implementation RFCs |

## Required command discipline

- Read latest GitHub PR/HEAD, ACTIVE workstreams and all Q00–Q18 protections before opening work.
- Use `Q19`…`Q38` stable IDs, scoped test-first Draft PR, independent exact-SHA CI and common issue/comment evidence.
- Shared documents provide read-only coordination across branches; they do NOT magically synchronize. A designated integration agent reconciles all central mutable ledgers.
- Existing Agent A, Agent B, active Q03 and Lumina branches are not modified, rebased, merged or superseded by this planning operation.

## Verification vocabulary

PROVEN_EXACT / PROVEN_UP_TO_GLOBAL_PHASE require a matching supported proof engine and full checker provenance. APPROXIMATE_BOUND requires explicit certified bounds and assumptions. FALSIFIED_BY_WITNESS is a real counterexample for the declared task. SIMULATED and OBSERVED_ON_HARDWARE are evidence categories, NOT proofs. UNKNOWN, TIMEOUT, UNSUPPORTED, NOT_RUN, CANCELLED, ERROR all fail closed. See [SEMANTIC_AND_TRUST_CONTRACT](SEMANTIC_AND_TRUST_CONTRACT.md).


## 2026-11-06 current code note (supersedes planning-only snapshot)

Agent C has started actual Q28 implementation in [Draft #99](https://github.com/danielforface/aura-lang/pull/99) and subsequent work through [Draft #108](https://github.com/danielforface/aura-lang/pull/108). Standalone Rust reference simulator, test oracles, external Qiskit parity and scaling checks exist on UNMERGED branches. This means **Q28 development is IN_PROGRESS, not that all Q28 tasks or Quantum are production-qualified**. Other Q19–Q38 gates remain separately planned. Current [production-first policy](../qualification/DELIVERY_OPERATING_POLICY_V1.md) requires meaningful functional improvement on each delivery phase and a reviewed integration queue instead of growing nested diagnostic Drafts.
