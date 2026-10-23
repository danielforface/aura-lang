# Aura Qualification Program — start here

**Purpose:** ongoing, evidence-based multi-release qualification of Aura without pretending to implement all missing features in one patch.

## One source of truth per purpose

| Document | Mutability | Purpose |
|---|---|---|
| [MASTER_ROADMAP_V1.md](MASTER_ROADMAP_V1.md) | **Frozen** after baseline, versioned by supersession | Long-horizon goals, stable IDs, dependencies and acceptance criteria |
| [PROGRESS.md](PROGRESS.md) | Mutable for *every work increment* | Current gate/train, task states, blockers, milestone evidence and work queue |
| [CONTEXT_HANDOFF.md](CONTEXT_HANDOFF.md) | Mutable for *every work increment* | Fresh-chat resumption: intent, decisions, GitHub connection, work performed, next action |
| [EVIDENCE_LOG.md](EVIDENCE_LOG.md) | Append-only (corrections add entries) | Static findings, reproductions, tests, CI and assurance evidence |
| [OPERATING_PROTOCOL.md](OPERATING_PROTOCOL.md) | Governance; versioned changes by review | Branch, PR, version, CI, release and documentation-update process |
| [RELEASE_LEDGER.md](RELEASE_LEDGER.md) | Append-only | Actual release train execution and version/tag record |

Existing project references remain authoritative where appropriate: [Public roadmap](../../ROADMAP_PUBLIC.md), [Qualification program](../../AURA_1_0_QUALIFICATION_PROGRAM.md), [Capability inventory](../../AURA_1_0_CAPABILITY_INVENTORY.md), [Project status](../../PROJECT_STATUS.md), [Public claims](../../PUBLIC_CLAIMS_POLICY.md) and [reference](../../sdk/docs/reference.md). Those documents are **not** automatically up-to-date evidence; date/SHA differences matter.

## For a fresh chat

Provide this repository URL and ask:

> Read `docs/qualification/CONTEXT_HANDOFF.md`, `PROGRESS.md`, `MASTER_ROADMAP_V1.md`, `OPERATING_PROTOCOL.md` and `EVIDENCE_LOG.md` in the current working branch. Check the real GitHub head, PR and permissions. Continue at `Next safe action` without changing the frozen roadmap. Update the handoff and progress with every increment, including any stopped/unfinished work. Never silently claim tests or commits happened.

## For a PR

Qualify one coherent slice, attach evidence and **change both PROGRESS and CONTEXT_HANDOFF** with the implementation. The governance workflow rejects edits to the frozen plan and untracked work once the program baseline is in `main`.

Current gate/branch/PR: see [PROGRESS.md](PROGRESS.md) (not this index).
