# Aura Cross-Agent Coordination Protocol v1 — Shared Surfaces, Provenance and Safe Parallel Delivery
**Date:** 2026-11-06. **Priority:** mandatory for all Agent A/B/C and later agents whenever a second lane may consume/modify the same code, contract, branch history, build artifact, CI check or project-wide document. Extends [Production-First Policy](DELIVERY_OPERATING_POLICY_V1.md), without weakening proof or ownership and without authorizing a merge.

## 0. A critical distinction: four kinds of "shared"
- **File write overlap:** two branches actually edit the same path or dynamically generated artifact. Only one writable owner until reviewed transfer; other lane may READ and implement independent adapter/test.
- **Semantic/interface overlap:** separate files depend on shared ABI, API, IR schema, protocol, error semantics, cache key, feature flags, or platform behavior. Paths may be disjoint yet changes can break another agent. A *versioned producer-consumer contract* and consumer tests are required BEFORE a breaking change.
- **History/merge overlap:** branch B contains hundreds of ancestor commits from A or other still-unmerged work. GitHub's `mergeable` against a parent PR base does NOT prove an independent releaseable patch or clean main integration. Create an explicit ancestry and inclusion/exclusion decision.
- **Shared operational resources:** CI workflows, Cargo workspace/lockfiles, integration docs, status and release claims. One coordinator serializes, or use path-scoped builds and separate agent notes.

## 1. Verified motivating example (snapshot; re-fetch current refs before work)
- [Lumina Draft #106](https://github.com/danielforface/aura-lang/pull/106) at `7b45b025cfc68239901dcdb4bca22a363862f47d` implemented real native `TextInput(autofocus: "true")`, 6/6 CI success. GitHub compare main...#106: **542 ahead, 0 behind**, merge base `e8f1c35a4a56adbde983dce38bba59a8c4eee105`, ~171 changed paths vs main. This does NOT make Autofocus a 542-commit feature.
- #106 is a 5-commit PR against its immediate Phase36 Draft base; the independent Autofocus delta is much smaller than the accumulated ancestry. PR #95 governance and #106 Lumina have 3 exact overlapping shared ledgers: `docs/qualification/PROGRESS.md`, `CONTEXT_HANDOFF.md` and `EVIDENCE_LOG.md`. #57 Core overlaps #106 in those 3 central files even though production paths are separate.
- [Core Draft #57](https://github.com/danielforface/aura-lang/pull/57) and B are **diverged**: compare A...B 336 ahead / 188 behind at observation. Neither unilateral merge nor naïve replay is safe. [Quantum Draft #108](https://github.com/danielforface/aura-lang/pull/108) is a real simulator implementation on an older V3 parent; its dependency history does not automatically include the latest execution policy.
- [IBus Draft #76](https://github.com/danielforface/aura-lang/pull/76), alternative crash test Draft #89 and plan PR #95 must be classified as ANCESTOR, PARALLEL, SUPERSEDED or NEEDED; mergeable is not enough. These are audit snapshots, not instructions to merge.

## 2. Dependency assessment: classify BEFORE editing and BEFORE handing off
For a proposed change, the author scans current Issues/claims, PR changed paths, branch head/base, graph ancestry and referenced contracts. Record the highest relevant class:
| Class | Trigger | Required response |
|---|---|---|
| D0 — ISOLATED | No path overlap, no contract change, no unmerged dependency | Proceed independently, focused tests and ordinary PR |
| D1 — CONSUMER | Uses stable read-only API/schema from another agent | Pin explicit producer ref and semantic version; consumer contract tests |
| D2 — CONTRACT_CHANGE | Changing ABI/API/schema/protocol/state-machine affecting another lane | Open ONE shared contract thread, proposal + tests + producer/consumer ACK; non-breaking parallel work may proceed |
| D3 — WRITE_COLLISION | Same file or path owner, including generated manifests/lockfile/workflow | STOP shared write; choose single owner, move other work to owned adapter or explicit reviewed handoff |
| D4 — ANCESTRY_CONTAMINATION | Branch includes unmerged ancestor changes or diverged histories | No direct main merge; use dependency DAG and explicit inclusion/exclusion plan; code may still be developed independently |
| D5 — SHARED_RESOURCE | Competes for global CI resources, calibration, service, docs/status ledger | Serialize the resource or partition it with documented budgets |

The classes are not mutually exclusive. Being D4 does NOT necessarily imply D3. A D3 conflict is not solved by saying both PRs are green. A D1 read-only use is not a reason to block all parallel work.

## 3. Minimal cross-agent handshake — only for real D2/D3/D4 conflicts
Post **one** GitHub Issue or linked comment (do not open a new micro-PR):
```text
CROSS_AGENT_CONTRACT v1
Owner lane / consumer lane:
Producer PR and exact HEAD:
Consumer PR and exact HEAD:
Dependency class (D1..D5):
Shared interface or file, source of truth:
Supported behavior and forbidden behavior:
Semver/schema/profile and rollback:
Producer tests + consumer tests:
Acceptance owner, decision APPROVED / BLOCKED / RESEARCH:
Merge prerequisites and parent chain:
Issue link and last reviewed SHA:
```
The producer owns the contract implementation; consumer owns integration tests. A change to shared API requires explicit ACK or approved compatibility adapter. No silent claim that an unmerged PR is available to other branches. No hidden messages between independent chats: GitHub PR/Issue is the shared bus.

## 4. Atomic integration rules for diverged branches
1. Compute current `base SHA`, `head SHA`, `merge-base`, `ahead/behind`, changed paths of **the PR's own base...head** AND **main...head**. These answer different questions.
2. Trace unmerged dependencies as a directed acyclic graph (DAG), including multiple branches created from another agent's outdated head. Mark each ancestor KEEP / REVIEW / SUPERSEDED / EXCLUDE; do not assume all 542 commits should land as one PR.
3. Have the integrator choose a reviewed target integration branch with accurate base. Select one of: (a) normal merge in correct ancestor order, (b) reviewed cherry-pick of the unique functional delta to a clean integration branch after dependencies are satisfied, (c) a tested adapter with the other change deferred. Preserve attribution and original PR links. No blanket rewriting of existing Draft branches.
4. Validate on **synthetic integration tree** or approved actual target snapshot: code build, contract tests, affected native/GUI/quantum tests and regressions. The branch's 6/6 CI against its *own parent* is necessary but not sufficient for another target.
5. Before modifying any ref, re-fetch expected SHA (optimistic concurrency / lease); reject if target moved, reconcile and retest. No force push or silent overwrite.
6. Only maintainer/reviewer-approved decision promotes to integration/main. `mergeable` is conflict detection for a given target, not product acceptance or authorization.
7. Single integrator serializes shared docs, Cargo root lockfiles, release ledger and cross-lane workflows; lane agents publish scoped notes in their own Issue/PR. Competing logs are synthesized after approved integration, not manually concatenated across branches.

## 5. Overlap resolution decision tree
- Another agent merely reads a stable function? **Proceed** with pinned version/tests.
- Other agent changing its signature or effects? **Pause only dependent part**, request a D2 contract and continue unrelated owned code.
- Both need same file? **One writer**, other supplies patch proposal or tests via reviewed handoff. Never dual-write.
- Both independently implement the same feature? Compare behavior and tests, designate canonical implementation, mark other candidate SUPERSEDED or RESEARCH after review; never merge competing code blindly.
- Existing feature is inherited from unmerged Core or V2 branch? **Isolate unique feature delta**, document dependencies, and request integration decision rather than treating its whole history as original authored work.
- Conflict only in progress/evidence documents? Integrator resolves status and append-only supersession; **do not** rebase another lane's source code just to reconcile logs.

## 6. Required evidence and stopping conditions
- Before coding: identify **owner, consumer, shared contract, current branch truth, integration target**. A D2/D3/D4 unresolved dependency is marked BLOCKED_ON_CONTRACT / BLOCKED_ON_SHARED_WRITE / BLOCKED_ON_INTEGRATION for the *affected portion only*.
- Before normal remote CI: successful local build in accessible environment, target-specific tests and no known missing-symbol RED; shared ABI/schema tests if applicable.
- Before declaring phase DONE: functional result on owned path, explicit cross-lane impact, reproducible tests, dependency status, exact SHA, PR state and next target integration action.
- The **read/decide/write/recheck** sequence is restated in [Phase Lifecycle Contract](PHASE_LIFECYCLE_V1.md) and [CONTINUE prompt](CONTINUE_AGENT_PROMPT.md). These links must be followed at EVERY phase, even when the user only says "ממשיכים".

**No false guarantees:** mere documentation cannot autonomously enforce correct agent behavior. The agent must actually read current links and GitHub facts. This protocol does not change V1/V2/V3 task IDs or claim any release.


## Standing cross-agent arbitrator — X (2026-11-07)

Agent X is the continuous mediation/architecture leader ([charter](agents/AGENT_X_INTEGRATION_DIRECTOR.md), [Issue #110](https://github.com/danielforface/aura-lang/issues/110)). X applies D0–D5 in every pulse and requests only *necessary* handshakes; normal D0 work proceeds. X gives agents with sound evidence GREEN encouragement, not gratuitous corrections. AMBER investigates justification and contracts; RED requires material verified harm; CROSS-LANE resolves dependencies by reviewed producer/consumer decisions. No autonomy to rewrite another's paths or approve main merge.
