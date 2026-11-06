# Aura Phase Lifecycle Contract v1 — mandatory at EVERY "ממשיכים"
**Date:** 2026-11-06. **Scope:** ALL agents A/B/C, future agents and integrator. See [Cross-agent coordination](CROSS_AGENT_COORDINATION_V1.md) and [Production-first](DELIVERY_OPERATING_POLICY_V1.md). This is the executable mental checklist and handoff contract; follow on each turn, not just the first turn of a chat.

## Stage 0 — Re-anchor (every phase, before touching code)
1. **Live** GitHub: identify user-assigned lane (A Core / B Lumina / C Quantum / Integrator), active Issue, actual open Draft PR, current exact head/base/merge-base, latest CI and current code. The last chat message or historical PROGRESS header is *not* authority.
2. Read root [AGENTS.md](../../AGENTS.md) at current policy branch, this file, [Cross-agent protocol](CROSS_AGENT_COORDINATION_V1.md), [Production-first policy](DELIVERY_OPERATING_POLICY_V1.md), [Operating protocol](OPERATING_PROTOCOL.md), your lane-specific order under [agents/](agents/), and the relevant Master V1/V2/V3 tasks. If the branch lacks these files, READ them from the live PR #95 docs branch directly — never infer absence means policy removed.
3. Before editing, run path/contract/history classification D0–D5; check [Issue #59](https://github.com/danielforface/aura-lang/issues/59) for ownership, [Issue #109](https://github.com/danielforface/aura-lang/issues/109) for integration, the current PR own changed-file list and other agent PR heads. Determine what is actually exclusive versus shared.
4. Define ONE compact delivery goal: observable product BEFORE→AFTER, minimal scope, exact needed tests, release impact, dependency owner, rollback and reviewer. Preserve work already implemented (B autofocus and C dynamic simulator are examples; never restart from stale prompt).

## Stage 1 — Decide (prevent analysis/CI loops)
1. Classify objective as DELIVERY / VERIFIED_BUG_FIX / SCIENTIFIC_ORACLE / RELEASE_UNBLOCK / POLICY. A diagnostic is a checkpoint inside a delivery task, not automatically a completed Phase.
2. If the previous investigation already establishes NO_PRODUCTION_BUG_PROVEN, stop re-probing the same test-only variant; advance functional work. If a supported production bug IS proven, implement and validate the fix with authority/fail-closed guarantees.
3. For D2 shared interface: issue **one** producer/consumer contract request and use a stable mock/adapter to continue independent work until ACK; do not quietly change another lane's interface. For D3 write conflicts: never write shared path. For D4 history: allow owned development but block unreviewed integration.
4. Check project priority: Core correctness and useful integration > actual user Lumina behavior > scientific Quantum feature and integration; do not pre-emptively stop healthy disjoint work. Highest priority is a shared integration-review decision, not unbounded diagnostic telemetry.

## Stage 2 — Build (real work)
1. Work only in owned paths. Write or modify **production/runtime/compiler code** when this is a Delivery Phase. Research tasks have explicitly justified scientific or release-unblocking output.
2. Run local targeted acceptance first. Real semantic RED may be demonstrated locally; DO NOT push known missing-method E0599/compile-broken fixtures through ordinary remote CI as a ceremonial proof.
3. Keep one active Issue and existing feature PR for the meaningful vertical slice; no new descendant Draft just to hold an isolated test, counter, or docs update. Do not merge/rebase/force/close without approval.
4. Maintain source-to-result correctness, type/effect/proof/quantum physical distinctions, input privacy and security. A shared contract must have consumer negative and positive tests.
5. If blocked, continue unblocked owned work and file exact BLOCKED reason; do not report scope as done or bypass a protection.

## Stage 3 — Integration preflight (before each push/PR change)
1. Recheck exact head/base (they may change during the phase), all path overlaps, contract consumers, and target PR ancestry. Distinguish mergeable against parent from mergeable against main/integration.
2. Run target-specific local tests and report actual negative/positive results; no invented success.
3. Only send a buildable candidate to ordinary remote CI; where needed use isolated diagnostics. Keep CI resource budget proportionate.
4. Feature PR may omit central PROGRESS/HANDOFF/EVIDENCE; integration steward owns those on true milestones. If touching both mandatory central docs, pair them and respect append-only evidence.
5. Do not declare READY_FOR_MERGE without reviewed integration tree, release-contract boundaries, independent CI and explicit owner approval.

## Stage 4 — Handoff (end of EACH phase)
Answer compactly in Hebrew with this exact factual card:
```text
AGENT / LANE:
ISSUE, PR, BRANCH, EXACT HEAD and BASE:
POLICY CHECK: AGENTS / PHASE / COORDINATION / LANE order READ at this phase
DELIVERY TYPE + PRODUCT BEFORE → AFTER:
PRODUCTION FILES and ACTUAL TESTS:
DEPENDENCY CLASSES D0–D5 + producer/consumer + shared-path decision:
LOCAL RESULTS and REMOTE EXACT-HEAD CI (PASSED/FAILED/PENDING/NOT RUN):
INTEGRATION READINESS (NOT READY / REVIEW READY / APPROVED) and what is still unmerged:
NEXT SAFE PRODUCTION ACTION (same PR, no automatic new Draft):
```
Do not write "6/6" absent six confirmed jobs on the same exact head. A Draft PR and its CI are not a main merge or an end-user release. The next "ממשיכים" starts again at Stage 0.

## Stage 5 — Continuation trigger (every new message)
- If the user writes **"ממשיכים"** alone, assume same lane/Issue/feature, but re-fetch GitHub live truth and every policy anchor; continue code toward the stated product acceptance. No repeated user question and no automatic switch to generic research.
- If the user provides a changed priority or a new upstream contract, update only that dimension and re-classify D0–D5.
- New agent or new chat: read [Universal Continuation Prompt](CONTINUE_AGENT_PROMPT.md) and own lane instructions BEFORE choosing files. Links to central documents in other branches are read-only references; no claim they were merged.
