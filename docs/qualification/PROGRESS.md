# Aura Qualification — Live Progress

**Updated:** 2026-10-23  
**Baseline source:** `main@e8f1c35a4a56adbde983dce38bba59a8c4eee105`  
**Working branch:** `program/aura-long-horizon-qualification-v1`  
**Active gate:** **Q00 — Baseline, governance and continuity**  
**Active release train:** **R0 — Program foundation**  
**Status:** **IN_PROGRESS** (R0 governance scaffold committed; Draft PR #9 open; no compiler fixes attempted)  
**Release:** none created in this work; no new stable-language claim.  
**Draft PR:** [#9 — program foundation](https://github.com/danielforface/aura-lang/pull/9), opened 2026-10-23; awaiting maintainer review. **First technical issue:** [#10 — fail-closed verify](https://github.com/danielforface/aura-lang/issues/10).  
**Next planned task:** finish R0 PR/CI validation, then reproduce `AURA-Q01-001/002` through issue #10; keep `AURA-Q00-005` source re-audit active.

This file is an evolving dashboard, **not** the immutable plan. Gate completion means supporting evidence exists in [EVIDENCE_LOG.md](EVIDENCE_LOG.md); a status table is not evidence.

## Program stage tracker

| Gate | Track | State | Verified exit evidence |
|---|---|---|---|
| Q00 | Governance, baseline and continuity | IN_PROGRESS | PENDING |
| Q01 | Fail-closed verifier and CLI | NOT_STARTED | PENDING |
| Q02 | Typed numeric semantics | NOT_STARTED | PENDING |
| Q03 | Composite/IR/backend fidelity | NOT_STARTED | PENDING |
| Q04 | Formal soundness and trust accounting | NOT_STARTED | PENDING |
| Q05 | Ownership, memory and concurrency | NOT_STARTED | PENDING |
| Q06 | Language core and modules | NOT_STARTED | PENDING |
| Q07 | Standard library and runtime | NOT_STARTED | PENDING |
| Q08 | Native backend, ABI and FFI | NOT_STARTED | PENDING |
| Q09 | Packages and reproducible releases | NOT_STARTED | PENDING |
| Q10 | CLI tests, IDE and UX | NOT_STARTED | PENDING |
| Q11 | External qualification and stable decision | NOT_STARTED | PENDING |

## Active Q00 task tracker

| Task ID | State | Notes / source of evidence |
|---|---|---|
| AURA-Q00-001 | IN_REVIEW | Working branch exists; [Draft PR #9](https://github.com/danielforface/aura-lang/pull/9) open and awaiting review |
| AURA-Q00-002 | IN_REVIEW | References to legacy Gates A–G and 1.0 qualification preserved and mapped |
| AURA-Q00-003 | IN_REVIEW | Frozen plan + live progress + evidence + handoff + release ledger included in bootstrap |
| AURA-Q00-004 | IN_REVIEW | [Qualification governance workflow](https://github.com/danielforface/aura-lang/actions/runs/39921540244) passed on bootstrap head `04dbdc0`; freeze-after-merge scenario still awaits review |
| AURA-Q00-005 | IN_PROGRESS | Static high-risk code review logged; real reproduction and comprehensive re-audit pending |
| AURA-Q00-006 | NOT_STARTED | Clean-checkout smoke instructions and observed output pending |
| AURA-Q00-007 | NOT_STARTED | Explicit tier/profile/version policy pending maintainer confirmation |
| AURA-Q00-008 | NOT_STARTED | Triage, security policy and decision records pending |

**Important:** `IN_REVIEW` is not `VERIFIED`. A GitHub PR or successful source commit alone cannot qualify compiler/runtime behavior.

## Immediate next queue (do not skip trust gates)

1. Finish and review R0 governance PR, verify new CI guard is effective, preserve branch history and handoff.
2. Establish reproducible baseline build + `aura verify` with and without Z3, plus typed numeric and C backend repro fixtures; record *actual* results, not predictions.
3. Open a small Q01 PR that makes missing solver fail closed and fixes report truthfulness (AURA-Q01-001/002), with negative regression tests.
4. Next Q01 PR: specify and enforce assertion behavior for dev/release/verify profiles (AURA-Q01-003/004).
5. Progress incrementally to Q02 numeric and Q03 composite fidelity, updating this file and handoff in the same change.

## Current blockers and uncertainties

- **No runtime/build tests performed in this documentation bootstrap.** Static findings are not run-confirmed defects.
- Baseline capability inventory is anchored to an older source commit; stale `QUALIFIED` labels must be independently revalidated.
- A Draft PR and CI run are required before moving Q00 task states to VERIFIED.
- Current branch exists through GitHub App write access; fresh chats must **recheck** connector availability rather than assume it.

## Task completion template (append new entries below, never silently edit scope)

`TASK-ID | state transition | PR/commit | test command + environment + actual outcome | risk/known gaps | date`

## Progress events

- **2026-10-23:** created and verified the qualification branch (separate from `main`); began R0 plan/governance documentation. No code modifications or language release claims.
- **2026-10-23:** pushed documentation/governance bootstrap commit [`04dbdc0`](https://github.com/danielforface/aura-lang/commit/04dbdc05925315235d83a5e9dcdb398b0c5baaed) (10 files, 88 fixed tasks), opened [Draft PR #9](https://github.com/danielforface/aura-lang/pull/9), observed governance [run 39921540244 PASS](https://github.com/danielforface/aura-lang/actions/runs/39921540244), dependency review and website export PASS; Core CI was still IN_PROGRESS when checked. Created [issue #10](https://github.com/danielforface/aura-lang/issues/10) for first Q01 fail-closed fix. No runtime regression tests executed in this session. Recheck current CI before approving.
