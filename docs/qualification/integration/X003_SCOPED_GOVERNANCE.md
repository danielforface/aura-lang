# X-003 — Scoped Agent C Governance, Issue #100

Status: DRAFT GOVERNANCE REMEDIATION — NOT ADOPTED, NOT RUN ON AGENT C #108, NOT MERGED.

## Confirmed root cause
The V3 checker already has a later production-first rule: PROGRESS.md and CONTEXT_HANDOFF.md must change together only if either is edited. Thus the older Issue #100 statement about requiring both files on every feature PR is obsolete for V3 current HEAD. However the existing roadmap workflow does **not** watch pull requests whose base is V3, so an isolated Quantum child can have green numerical/Rust CI while governance was never executed.

## Actual proposal (integration-owned paths only)
- Extend the existing V3 roadmap workflow PR target filter to V3 while preserving old V1/V2/V3 task freeze and integrity tests.
- Add a separate C-only scope job for V3 child PRs. On adoption it loads a **trusted copy of policy from the base SHA**, not from the unreviewed head. For the first policy proposal, bootstrap itself via an explicit integration-only branch guard; separate real-git unit cases run in standard CI.
- Permit exclusively aura-quantum-sim/, docs/quantum/agent-c/ and .github/workflows/quantum-reference-sim.yml. Require source changes, changed C-### note identifying the Issue and matching Draft PR, no symlinks/submodules, no root, Core, Lumina, frozen roadmaps or central ledger writes.
- 18 real-git positive/negative regression cases ensure valid own-source PR passes; wrong branch, missing evidence, stale PR number, frozen/shared/foreign paths, global workflow, symlinks and empty diffs fail.

## Qualification boundary
This policy has no effect on *already existing* Agent C #108 because that PR targets C-003 and inherits an older workflow. Reviewed adoption to V3 and a separately approved application to older stacked ancestors are still required. No force/rebase/retarget is authorized. Nor does this grant #111 science proof or permit an automatic merge to main.
