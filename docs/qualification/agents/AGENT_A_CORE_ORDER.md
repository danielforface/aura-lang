# Agent A — Immediate delivery order (Core Q03)
**Status:** directive for next continuation, not a code change or claim of merge. **Owner:** Agent A; Q03 / Draft PR #57; [Issue #51](https://github.com/danielforface/aura-lang/issues/51).

1. Read [Production-first policy](../DELIVERY_OPERATING_POLICY_V1.md) and fetch CURRENT PR #57 SHA, complete ancestry and pending CI. Do not treat historic SHA as latest or latest RED as unresolved if newer GREEN proved the fix.
2. Keep Core exclusive ownership: `aura-ast/`, `aura-parse/`, `aura-core/`, `aura-ir/`, `aura-backend-c/`, `aura-verify/`, related compiler tests. Do not touch B's Lumina or C's simulator.
3. Audit Issue #51 acceptance against actual functional source→Checker→Lowerer→IR Oracle→native C emitter→compiled O0/O2 program. Identify **the single most consequential missing Record/native behavior** blocking a usable subset or review/merge; commit a production fix with behavioral positive and negative tests, or close the supported-slice review if the behavior is already correct. No new unrelated edge-case program unless severity and acceptance relevance are demonstrated.
4. A 278-commit Draft stack is NOT the delivery unit. Prepare an integration plan for dependencies and releaseable subset, including exact supported/unsupported Record ABI, parent PR ordering, changed-file conflict and next reviewer action. DO NOT auto-merge, force-push, flatten or recreate the existing stack.
5. Reproduce genuine semantic RED locally first, or in isolated needed native environment. No new default remote CI failure for an unimplemented test method or known compile-only fixture error.
6. One code+test+concise PR-body evidence change at a time. Full final exact SHA CI once code is ready; avoid a new Draft on each EV/test. Keep source/proof boundaries fail-closed and do not declare Q03 VERIFIED without full support evidence.
7. Escalate shared C/LLVM/FFI requirements separately with explicit interfaces, rather than opportunistically expanding a single PR. End with product BEFORE→AFTER, runnable source fixture, exact CI, merge readiness and any real P0 blockers.

**Success:** a demonstrable, reviewable and integratable product improvement — not another EV tally. Follow the maintainer review gates for actual merge to shared code.


## Required on EVERY subsequent Core phase — shared dependencies

Open [root AGENTS.md](../../../AGENTS.md), [Phase Lifecycle](../PHASE_LIFECYCLE_V1.md), [Cross-Agent Protocol](../CROSS_AGENT_COORDINATION_V1.md) and [universal continuation prompt](../CONTINUE_AGENT_PROMPT.md) **at Stage 0, before pushes and at handoff**, even if your Core branch does not contain the files. Obtain latest policy from Draft #95. Do not assume B/C consume the latest parser/IR contract: B's #106 inherited old Core ancestry; compare A vs B branch history and require D2 API consumer tests before modifying shared AST/IR/LSP signatures. Existing Q03 branch #57 contains real native implementation; continue outstanding functional blocker or reviewable integration slice rather than redoing already accepted tests. A/B/C central docs overlap is owned by integration steward.
