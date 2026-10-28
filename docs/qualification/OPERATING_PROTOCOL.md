# Aura Qualification Operating Protocol v2 (proposed; maintainer review required)

This defines the repeatable *process*, not the frozen technical acceptance contents. Changes to protocol require reviewed PR, rationale in handoff, and consistent updates to progress.

## 1. Branches and review model

- Long-lived **program branch**: `program/aura-long-horizon-qualification-v1` for the initial bootstrap proposal only, based on `main`.
- After governance is accepted, use focused branches from current `main`: `qual/Q01-001-fail-closed-verify`, `qual/Q02-002-c-typed-binop`, etc.
- Open **Draft PR** first, reference exact task IDs, affected assurance boundaries and test evidence, then request maintainer review; no autonomous merge.
- Never force push, bypass branch protection, introduce unrelated refactors or write directly to main.
- When parallel tasks touch same areas, land prerequisite work first or rebase after explicit conflict inspection.

## 2. Authority and freeze policy

- `MASTER_ROADMAP_V1.md` is *definition only*: goals, fixed IDs, completion criteria, dependency edges. Its content must not change after initial baseline merge.
- `PROGRESS.md` is live state (including **active gate**, active train, backlog, statuses and blockers). Never mark VERIFIED or RELEASED without artifact.
- `CONTEXT_HANDOFF.md` stores what is not obvious from Git history: user intent, past decisions, why choices were made, current collaboration expectations, last actual work, lost work, next action and connection recipe.
- `EVIDENCE_LOG.md` and `RELEASE_LEDGER.md` grow append-only; corrections are new superseding entries.
- The 2026-10-28 user-requested scope amendment is the proposed `MASTER_ROADMAP_V2.md`: all V1 acceptance requirements remain unchanged and new Q12–Q18 are additive future gates. V1 is immutable; V2 becomes frozen upon reviewed approval/merge; subsequent amendments require V3+, never in-place edits.

## 3. Every work increment (including partial work)

1. Read handoff/progress/plan/evidence and ask GitHub for current branch SHA/PR/CI status. Verify you are actually authenticated with write access.
2. Pick one stable task ID and state its impact on verification or execution. Establish current behavior with a negative test whenever possible.
3. Make the smallest change to source and tests. Record the exact commands run, environment and outcomes; state NOT RUN explicitly where appropriate.
4. Append evidence. Update `PROGRESS.md` task/gate/train status and release ledger **if an actual release occurred**.
5. Update `CONTEXT_HANDOFF.md` **in the same change** (even if the work is incomplete or blocked), listing changed paths, decisions, actual errors, PR and the next safe action.
6. Open/update PR. Verify CI rather than infer success from workflow YAML. If stopped mid-task, commit accurate status/handoff before handing off.
7. Treat release numbering and public claims as an independent reviewed step, not an automatic consequence of merging code.

**Guarantee boundary:** CI can enforce matching files in the PR diff after baseline; it cannot force humans or AI to write truthful content. The agent must keep the narrative accurate. Fresh-chat authentication may require reconnection and should never be stored as a token.

## 4. PR acceptance template

- Task ID(s), priority, parent gate, baseline issue / reproducible failing fixture.
- Scope, assumptions, out-of-scope backends/targets and trust-boundary impact.
- Source and test files changed, command/environment and actual outcomes (including failures).
- Proof/CI links, differential results, effect on supported-feature/tier table.
- Explicit migration/compatibility/release ramifications; rollback plan.
- Evidence ID(s), updated PROGRESS and CONTEXT_HANDOFF, reviewer approval.
- If no CI run can be obtained, mark IN_REVIEW/BLOCKED; never VERIFIED.

## 5. Gate review and release

No gate is CLOSED just because tasks were coded. Each listed acceptance requirement must carry a tested, linked outcome on the qualified target. A beta release may cover an explicit limited subset; stable-language-wide claims require Q11 Go/No-Go plus all relevant P0 trust/semantic/memory gaps resolved. Every real release is recorded in `RELEASE_LEDGER.md` with tag, artifact, supported OS/backend, tests, changelog and remaining experimental features.

## 6. Governance CI behavior

`tools/qualification/check_governance.py` checks (on PR):
- V1 remains byte-identical and V2 preserves every V1 original acceptance row verbatim; after adoption V2 also freezes in place;
- while V2 exists, the stage tracker must list Q00–Q18, with newly proposed gates NOT_STARTED and no invented CI;
- frozen master file equals base branch's content, once the base branch contains it;
- both `PROGRESS.md` and `CONTEXT_HANDOFF.md` changed on any substantive PR once bootstrap merged;
- all task IDs referenced in status/handoff exist in the master plan (format sanity);
- minimum metadata and valid gate states.

Bootstrap PR may add the frozen baseline. For any later revision, publish a new version instead of modifying the old. This is a policy guard, **not** a substitute for compiler tests or proof soundness.

## 7. Fresh-chat resumption and no hidden context

New chat opening request:

 > Work directly in `danielforface/aura-lang`. Read the latest appended `docs/qualification/CONTEXT_HANDOFF.md` and `PROGRESS.md`, proposed/active `MASTER_ROADMAP_V2.md`, frozen `MASTER_ROADMAP_V1.md`, `EVIDENCE_LOG.md` and `OPERATING_PROTOCOL.md`. Discover and verify GitHub connection. Inspect active branch/PR, live CI and current source. Continue the exact Next safe action. Do not alter the frozen plan or make unsupported test claims. Update handoff and progress along with each change; keep the program auditable across future chats.

If head differs from recorded notes, **GitHub is the authority for code and branches, not an out-of-date handoff**; reconcile carefully and document the discrepancy. Do not recreate branches/files without checking first.

## 8. Scope expansion with no implementation diversion

Future development chats MUST see the new eventual Desktop/Web/iOS/Android application-platform goal (Q12–Q18), but MUST continue Q03-001 on Draft PR #57 as the current implementation stream until maintainer direction changes it. A roadmap amendment never authorizes a source rebase/merge, automatic implementation switch, removal of earlier requirements, or all-platform stability claim. Preserve V1, require reviewed supersession for V2, keep PROGRESS/HANDOFF updated together and EVIDENCE_LOG append-only. Core releases and separately qualified UI releases remain distinct; no software evidence exists merely because a document is updated.
