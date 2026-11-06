# Aura Delivery Operating Policy — Production-First v1
**Date:** 2026-11-06 · **Authority:** maintainer-directed process correction · **Scope:** Agent A (Core), B (Lumina), C (Quantum), future agents and integration.

This is an **execution-policy amendment**, not a modification of frozen Master Roadmap V1/V2/V3 requirements or a declaration that quantum/UI/core features are qualified. It supersedes conflicting day-to-day procedural instructions that prescribe per-pulse central ledger edits, mandatory remote RED or a new Draft PR for every measurement. The veracity, safety and trust rules in the qualification program and `PUBLIC_CLAIMS_POLICY.md` **remain binding**.

## 1. Problem and evidence (verified snapshots)
- GitHub 2026-11-06 snapshot: **78 user PRs inspected, 70 open, 8 merged**, and 73 branch names on the first branches endpoint page. A huge, nested Draft history exists; it is an integration bottleneck, not evidence of 70 simultaneously failing implementations.
- Core Draft [#57](https://github.com/danielforface/aura-lang/pull/57) changed AST, sema, parser, IR, C emitter and real Native tests (**278 commits, 47 changed paths** at observation). [Issue #51](https://github.com/danielforface/aura-lang/issues/51) is a BUG ISSUE, not PR #51. It is false to call the full core lane documentation-only.
- Lumina Draft [#103](https://github.com/danielforface/aura-lang/pull/103) creates actual-engine but **diagnostically deferred** post-R callbacks; [#106](https://github.com/danielforface/aura-lang/pull/106) adds feature-gated ACK drop counters and Rust Editor integration tests. **Production standard helper and behavior unchanged in both**; the last experiments do not establish a user-visible production defect. [Issue #59](https://github.com/danielforface/aura-lang/issues/59) is a MULTI-AGENT COORDINATION issue, not automatically the IBus bug to close.
- Quantum [#99](https://github.com/danielforface/aura-lang/pull/99) contains **actual Rust reference simulator** and tests. [#108](https://github.com/danielforface/aura-lang/pull/108) independently checks Qiskit parity and benchmarks. It is wrong to assert Agent C has only paperwork; however endless oracle-only increments cannot substitute roadmap feature integration.
- Recent Q03 `local-record-native` RED [PR run 42213904423](https://github.com/danielforface/aura-lang/actions/runs/42213904423) was followed by [PR run 42214100867](https://github.com/danielforface/aura-lang/actions/runs/42214100867) four-job success on a newer code SHA. Verify newest exact SHA before calling it fully green. Latest B [PR run 42195826388](https://github.com/danielforface/aura-lang/actions/runs/42195826388) and C [PR run 42214329430](https://github.com/danielforface/aura-lang/actions/runs/42214329430) completed successfully on their observed heads.

## 2. Meaning of a phase (required for implementation work)
A **Delivery Phase** is an end-to-end vertical slice that produces at least ONE **tested user-observable function, proven fixed pre-existing defect, measurable accepted performance/reliability improvement, or a uniquely needed release-blocking integration feature**. Include code on its actual runtime/compiler path, API/program-level positive and negative acceptance, and a truthful release boundary. CI green only measures a committed candidate; **CI green alone is not delivery**.

Diagnostics, mock-only test scaffolding, new counters, documentation appendages or pass/fail snapshots are **investigation steps within a Delivery Phase**; they do NOT earn a separately completed Delivery Phase or a new nested PR. Name them `Investigation checkpoint` in the same Issue/PR and proceed to an implementation/decision within the same bounded scope.

**Valid exceptions:** isolated formal proof research, independent safety audit, regression-harness infrastructure, external standards conformance or policy/docs changes can be standalone deliverables **only with an explicit consequential output**: soundness counterexample, frozen interface/spec used by code, executable oracle used by development, or actual CI/merge unblock. Must be explicitly labeled RESEARCH/INFRA/POLICY, never counted as a shipped product feature. A no-code scientific investigation cannot silently satisfy production feature goals.

## 3. Diagnostic stop rule / engineering decision
Before an investigation write an Issue hypothesis, affected production path, required observation, discriminating test and one of: `FIX`, `NO_PRODUCTION_BUG / CLOSE_RESEARCH`, `UNSUPPORTED / FAIL_CLOSED`, `BLOCKED_ON_CONTRACT`.
- Allow **at most one new bounded investigative increment** after current evidence is sufficient to reach a decision. If still ambiguous, escalate for design review; do not indefinitely manufacture new observations of the same callback or numerical edge.
- If normal production behavior is not reproduced as faulty, **DO NOT invent a bug or weaken authority** to satisfy a mandate. Preserve regression tests, close the research hypothesis honestly, and move to the highest-priority functional slice.
- Once reproducibly faulty under a supported profile, implement the smallest correct production-path fix with a test that would fail without the fix; validate impact through normal public entry points.
- Give diagnostics a deletion/retention plan. Instrumentation behind feature gates must not become a permanent prerequisite for shipping.

## 4. TDD/CI — useful evidence, no broken-branch ceremony
- Start with source-level behavioral tests when practical. Use **local** `cargo test`, `cargo check`, targeted harnesses, `clang`, Qiskit reference or static analysis for GREEN and true semantic RED.
- **Never intentionally push compilation-broken code to routine GitHub Actions just to demonstrate missing symbols/methods (E0599), unimplemented functions, missing fixtures, or known failing imports.** Those are fixture/build incompleteness, not regression evidence.
- A genuine semantic RED is valuable if **the existing production behavior** violates an independent oracle. Capture locally or, if only a native Linux/DBus/real-hardware environment can reproduce it, through an explicitly labeled **isolated diagnostic workflow** with bounded budget, not the ordinary broad required CI path.
- Require a locally buildable candidate before normal remote push. In constrained environments, check the accessible compile/test subset locally and label unavailable paths honestly.
- Run lightweight path-scoped checks for development, one full required matrix on the final meaningful code+doc SHA. Avoid duplicate heavy Push and PR schedules where an equivalent single exact-SHA path is sufficient for qualification policy; if two are required by branch protection, satisfy them, but do not launch both after each diagnostic-only commit.
- Classify failures as `SEMANTIC`, `PRODUCTION_BUILD`, `TEST_FIXTURE`, `FLAKY_ENVIRONMENT`, or `DEPENDENCY_INFRA`; rerun flaky jobs with logs and isolate their source. Never hide a genuine failure as a flake or green on an older SHA.
- Add GitHub Actions concurrency cancellation and filters **when compatible with required checks**, not in ways that hide failed required jobs.

## 5. Branches and integration — stop increasing Draft debt
- Default **one active Issue + one feature PR per agent per coherent vertical slice**. Continue on that PR through implementation, positive/negative tests and final evidence; **no new micro-PR stacked above an unfinished Draft**.
- Historical stacks **must not be rebased, force-pushed, blindly closed or merged** to comply. Integration steward inventories parent → child relationships, tests merge order, stages reviewers and receives explicit maintainer authorization for merges/closures.
- Prefer new independent PRs from latest **reviewed integration/main base** where practical; when blocked by unmerged contractual prerequisites, record `BLOCKED_ON_INTEGRATION` in the existing Issue, rather than opening a new descendant Draft by default.
- Target: turn accepted slices into **reviewed, integrated code on a shared branch/main**; open Draft is a temporary review state, not a permanent certification archive.
- Preserve published version semantics, release ledgers and safety gates; partial production functionality may be experimental and correctly labeled.

## 6. Documentation without bureaucratic duplication
- One **Issue** = ownership/hypothesis/outcome; one **PR body** = files, behavioral change, tests, exact-SHA CI, review-ready checklist and next action.
- Scoped agent continuation notes (e.g. `docs/quantum/agent-c/`) only when necessary. Large `PROGRESS.md`, `CONTEXT_HANDOFF.md` and append-only `EVIDENCE_LOG.md` are **integration-owned**, updated serially at genuine milestone/review/merge or process change; not every commit or local checkpoint.
- If an integration update changes `PROGRESS.md` or `CONTEXT_HANDOFF.md`, change **both** to preserve a coherent cross-chat state; but **feature PRs are NOT required to touch either**.
- Never falsify a status or evidence chain to bypass a gate. Cite exact SHA, CI, platform and distinct scientific/production claims.
- Old records must be preserved, corrections appended as superseding decisions. V1/V2/V3 master rows remain frozen.

## 7. Priorities and escalation
1. **P0 compiler/IR/backend correctness and integration** (Agent A, Q03 / Issue #51), especially valid Aura source → ABI → emitted C → executable behavior; source/target claim correctness.
2. **Real user-facing UI function and correctness** (Agent B); do not spend successive phases measuring post-R conditions that were *only induced by diagnostic scheduling*. Apply product reproduction decision rule before editing native event authority.
3. **Independent, valuable quantum feature and reference correctness** (Agent C); protect Rust quantum namespace, benchmark independent oracles as needed but prioritize an actual simulator capability or Quantum IR boundary usable by a caller over infinite micro-assurance.
4. **Shared engineering health** (integration steward): CI reliability, PR queue reduction, reproducible clean checkout, stable APIs, releaseable reviewed subsets.

This is a priority ordering for **integration attention and shared CI capacity**, not authority to shut down valid parallel work or cross-write an agent-owned path.

## 8. Acceptance handoff card (short)
```text
Issue / Q ID:
Owner + protected paths:
Baseline SHA / proposed integration target:
Product behavior BEFORE → AFTER:
Evidence this is actual production behavior:
Production files changed / tests:
Local command + result:
Final required CI exact SHA + run links:
Remaining unsupported cases / risk:
Review decision READY / BLOCKED and next action:
```
A feature PR without `BEFORE → AFTER` impact or an explicitly approved exceptional deliverable is NOT a completed Delivery Phase.


## 9. Parallel work is safe only when dependencies, not just files, are resolved

**Read for every "ממשיכים":** [root AGENTS.md](../../AGENTS.md), [Phase Lifecycle](PHASE_LIFECYCLE_V1.md), [Cross-Agent Protocol](CROSS_AGENT_COORDINATION_V1.md), [Universal Continuation Prompt](CONTINUE_AGENT_PROMPT.md). This is an iterative decision gate, not a one-off setup or requirement to request approval for isolated D0 work.

A second agent may have read-only access to another lane's stable API while developing independently. Any breaking ABI/schema/effect change needs a producer-consumer contract with tests; a shared file needs **one writer**. A branch with unmerged inherited work has a **D4 ancestry dependency** even if there is no write collision; compare its own PR delta separately from main...head. A CI-green and "mergeable" child PR is NOT automatically safe for main. The integrator owns ancestry decisions and exact merged-tree tests. Status records and global lockfiles are serialized.

Example: B Autofocus has real production functionality on [Draft #106](https://github.com/danielforface/aura-lang/pull/106), but its 542-ahead-main history must be resolved before release; C has already added bounded dynamic simulation and JSON CLI in [Draft #108](https://github.com/danielforface/aura-lang/pull/108). Never send either agent back to reimplement an already delivered branch feature because an old instruction referred to it as future work.
