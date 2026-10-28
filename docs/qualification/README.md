# Aura Qualification Program — start here

**Purpose:** ongoing, evidence-based multi-release qualification of Aura without pretending to implement all missing features in one patch.

## One source of truth per purpose

| Document | Mutability | Purpose |
|---|---|---|
| [MASTER_ROADMAP_V1.md](MASTER_ROADMAP_V1.md) | **FROZEN, UNCHANGED** | Original 88 Q00–Q11 core acceptance tasks and R0–R8 trains |
| [MASTER_ROADMAP_V2.md](MASTER_ROADMAP_V2.md) | **PROPOSED reviewed supersession; freezes once approved** | Complete original scope plus 102 new Q12–Q18 modern Aura/Lumina Desktop/Web/Android/iOS UI tasks and R9–R15 |
| [PROGRESS.md](PROGRESS.md) | Mutable for *every work increment* | Current gate/train, task states, blockers, milestone evidence and work queue |
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
