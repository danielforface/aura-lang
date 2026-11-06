# Aura Qualification Program — start here

> **Live agent entrypoint:** [root AGENTS.md](../../AGENTS.md), [every-phase lifecycle](PHASE_LIFECYCLE_V1.md), [cross-agent ownership/contracts](CROSS_AGENT_COORDINATION_V1.md), [universal “ממשיכים” prompt](CONTINUE_AGENT_PROMPT.md), and [structured ownership map](AGENT_OWNERSHIP.json). A/B/C must READ these at every phase, even if their active branch predates the docs.

**Purpose:** ongoing, evidence-based multi-release qualification of Aura without pretending to implement all missing features in one patch.

## New execution policy (2026-11-06): build the product

[Production-first policy](DELIVERY_OPERATING_POLICY_V1.md) supersedes earlier per-commit documentation and remote RED ceremonies, without weakening scientific/semantic or safety guarantees. [Workflow audit](WORKFLOW_AUDIT_2026-11-06.md) and [integration plan](INTEGRATION_AND_CI_PLAYBOOK.md) show the evidence and implementation priorities; [Agent A](agents/AGENT_A_CORE_ORDER.md), [B](agents/AGENT_B_LUMINA_ORDER.md) and [C](agents/AGENT_C_QUANTUM_ORDER.md) have distinct read-only continuation orders. Existing ownership branches do not auto-synchronize.

## One source of truth per purpose

| Document | Mutability | Purpose |
|---|---|---|
| [MASTER_ROADMAP_V1.md](MASTER_ROADMAP_V1.md) | **FROZEN, UNCHANGED** | Original 88 Q00–Q11 core acceptance tasks and R0–R8 trains |
| [MASTER_ROADMAP_V2.md](MASTER_ROADMAP_V2.md) | **PROPOSED reviewed supersession; freezes once approved** | Complete original scope plus 102 new Q12–Q18 modern Aura/Lumina Desktop/Web/Android/iOS UI tasks and R9–R15 |
| [MASTER_ROADMAP_V3.md](MASTER_ROADMAP_V3.md) | **PROPOSED technical plan; freeze after reviewed adoption** | Original 190 Q00–Q18 acceptance tasks preserved verbatim plus 284 Quantum Q19–Q38 tasks and R16–R35. Strategic quantum direction approved 2026-11-05; zero quantum qualification yet. |
| [PROGRESS.md](PROGRESS.md) | Integration-owned, updated on meaningful milestones | Current gate/train, task states, blockers, milestone evidence and work queue |
| [CONTEXT_HANDOFF.md](CONTEXT_HANDOFF.md) | Mutable for *every work increment* | Fresh-chat resumption: intent, decisions, GitHub connection, work performed, next action |
| [EVIDENCE_LOG.md](EVIDENCE_LOG.md) | Append-only (corrections add entries) | Static findings, reproductions, tests, CI and assurance evidence |
| [OPERATING_PROTOCOL.md](OPERATING_PROTOCOL.md) | Governance; versioned changes by review | Branch, PR, version, CI, release and documentation-update process |
| [RELEASE_LEDGER.md](RELEASE_LEDGER.md) | Append-only | Actual release train execution and version/tag record |

Existing project references remain authoritative where appropriate: [Public roadmap](../../ROADMAP_PUBLIC.md), [Qualification program](../../AURA_1_0_QUALIFICATION_PROGRAM.md), [Capability inventory](../../AURA_1_0_CAPABILITY_INVENTORY.md), [Project status](../../PROJECT_STATUS.md), [Public claims](../../PUBLIC_CLAIMS_POLICY.md) and [reference](../../sdk/docs/reference.md). Those documents are **not** automatically up-to-date evidence; date/SHA differences matter.

## For a fresh chat

Provide this repository URL and ask:

> Read `docs/qualification/CONTEXT_HANDOFF.md`, `PROGRESS.md`, `MASTER_ROADMAP_V2.md` (proposal/active status), `MASTER_ROADMAP_V1.md` (frozen), `OPERATING_PROTOCOL.md` and `EVIDENCE_LOG.md` in the current working branch. Check the real GitHub head, PR and permissions. Continue at `Next safe action` without changing the frozen roadmap. Update the handoff and progress with every increment, including any stopped/unfinished work. Never silently claim tests or commits happened.

## For a PR

Qualify one coherent slice, attach evidence and **change both PROGRESS and CONTEXT_HANDOFF** with the implementation. The governance workflow rejects edits to the frozen plan and untracked work once the program baseline is in `main`.

Current gate/branch/PR: see [PROGRESS.md](PROGRESS.md) (not this index).

## 2026-10-28 full-platform mandate and continuous existing-work resumption

The maintainer requires Aura to eventually deliver complete, modern, accessible and easy-to-build applications in Aura for Windows/macOS/Linux, Web and Android/iOS, **without reducing any verifier/systems-language correctness**. This is documented in proposed [Master Roadmap V2](MASTER_ROADMAP_V2.md), which retains V1's exact original acceptance rows. All new Q12–Q18 tasks are NOT_STARTED and separately qualified; core-only releases remain possible with honest target claims. **Current engineering is still Q03-001 on [Draft PR #57](https://github.com/danielforface/aura-lang/pull/57)**, NOT Lumina. Read newest appended [PROGRESS](PROGRESS.md) and [CONTEXT_HANDOFF](CONTEXT_HANDOFF.md), verify live GitHub CI/branch, and resume the existing active task before new-platform work. Never confuse this proposal with a merge or release.


## 2026-11-05 — Aura Quantum mandate and proposed Master V3

The maintainer committed to Quantum development as a future domain. [Master V3](MASTER_ROADMAP_V3.md) carries 474 unique requirements: all previous 190 preserved, plus 284 Q19–Q38 new tasks. The [Quantum reference index](../quantum/README.md) defines scientific semantics, architecture, interop, proof/status model, planned agent ownership, benchmark/QA and researcher-facing example programs. This is **planning-only**, all new quantum gates NOT_STARTED. No proof of executable quantum support, no QPU permission, no merge of prior Draft plans; Core and Lumina active agents retain their own work. Historic program and V1/V2 remain immutable.
