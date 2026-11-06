# Integration debt reduction and CI budget playbook (execution queue)
**Stage:** PROPOSED action queue, not authorization to merge. **Coordination owner:** designated integration steward/maintainer; agents retain own production paths.

## Freeze rules
- No newly stacked micro-Drafts above unfinished Drafts. Continue the existing Agent A/B/C branch toward a complete **vertical slice**, then STOP branching until review/merge decision.
- Do not bulk-rebase old descendants, rewrite history, close sibling PRs, or merge to main automatically. Inventory duplicates/ancestry before deciding.
- Create a single review queue organized as **dependency DAG**, not as 70 independently mergeable features.

## Candidate lanes
| Priority | Active candidate | Owner | Hard completion condition |
|---|---|---|---|
| P0 | [Core Q03 PR #57](https://github.com/danielforface/aura-lang/pull/57), [Issue #51](https://github.com/danielforface/aura-lang/issues/51) | A | Runnable Aura source→C→O0/O2 + negative ABI guards; exact CI and reviewed supported-subset contract; narrow merge sequence. |
| P1 | [Lumina PR #106](https://github.com/danielforface/aura-lang/pull/106) descendant of [#103](https://github.com/danielforface/aura-lang/pull/103) | B | Decision on actual normal-prod input loss. If reproduced, implement verified non-lossy safe fix; if not, close research and resume production UI, never spoof provenance. |
| P1 | [Quantum PR #108](https://github.com/danielforface/aura-lang/pull/108) descendant of [#99](https://github.com/danielforface/aura-lang/pull/99) | C | Reference simulator reviewed and exposed through real callable API, with scientific oracle tests; next phase must extend capability rather than only add benchmark telemetry. |
| P0 cross-lane | CI and staging governance | Integrator | Cancel obsolete runs where permitted; evaluate path filters/concurrency; provenance-preserving merge strategy with real reviewer sign-off. |

## Review & integration protocol
1. Fetch live exact PR head/base and parent chain. Enumerate changed paths; protect disjoint ownership.
2. Inspect what is code vs test vs documentation, not commit or EV-count. Group semantically related old Drafts into reviewable feature landing units WITHOUT rewriting or force-pushing others.
3. Reproduce failures on the exact attempted integration tree (not only the old PR branch) before approval. Fail-closed on unsupported compilation and actual semantic gaps.
4. For mergeable accepted features, propose explicit ancestry-order integration/merge plan; wait for maintainer approval, required CI, and conflict resolution. A Draft PR is not a merge approval.
5. After real integration, update shared PROGRESS/HANDOFF and append one synthesized evidence entry. Don't append a redundant EV for each commit.
6. Close or supersede obsolete overlapping Drafts only through individual reviewed decisions with preserved links to implemented code/tests.

## CI budget principles
- Fast local Rust check/test and Clang smoke **before** push.
- Fast path-scoped check during development; only required complete matrix when behavior is ready.
- Diagnostic-only Linux X11/Hangul environment: isolated manual/diagnostic workflow where possible, not an intentional compile-broken Push/PR main gate.
- Concurrency cancellation for obsolete SHA and dependency-sensitive branch workflows; avoid duplicate heavy Push+PR when not contractually required.
- Distinguish ordinary test RED caused by a *real old behavior* from false RED caused by missing new API.
- Track CI failure classifications and work on flaky runner infrastructure as a discrete integration task, not inside functional PRs.
- Scientific parity and QPU/hardware experiments are separate profiles and budgets.

## Review readiness / metrics
| Useful measure | Meaning |
|---|---|
| Accepted source-to-runtime features integrated | Production substance, not number of commits |
| Verified bug closures | Actual root-cause and recovery evidence |
| Median PR age + count of unmerged ancestor stacks | Integration debt |
| Routine CI minutes per accepted feature | Process efficiency |
| Main/integration reproducibility | Real users can build the tested artifact |
| Failure cause distribution | Distinguish flake, semantic failures and fixture errors |
| Quantum oracle disagreement rate | Real scientific capability integrity |

No target is a verified KPI until measured from actual data. Historic PR closure requires reviewer sign-off, not quota gaming.


## Current shared-surface DAG and required review — 2026-11-06

See [Cross-Agent Protocol](CROSS_AGENT_COORDINATION_V1.md) for D0–D5, atomic integration tests, ancestry isolation and consumer approvals; [Phase Lifecycle](PHASE_LIFECYCLE_V1.md) rechecks these rules each "ממשיכים". Branches can diverge while source paths are disjoint.

- **Lumina #106** at observed `7b45b025cfc68239901dcdb4bca22a363862f47d</BT>: native TextInput Autofocus has meaningful C/Rust functionality, 6/6 CI; original PR is 5 commits / 9 changed paths versus its own Phase36 base. But compare **main...head** is 542 ahead/0 behind/171 affected paths. Never merge as though its entire history is an isolated Autofocus delta. Investigate ancestry and separate already-reviewed upstream work via a maintainer-approved landing strategy.
- **Core #57** at observed `c13f0e9781abc418ceb0f55f76c7ab48920599ae</BT> diverges from B; A...B 336/188, so branch switching and cherry-picking require conflict and contract tests, not a generic merge request.
- **Quantum #108** at observed `f8a5fbeff04ebb11ff063d46cd60493679bf97d0</BT>: bounded dynamic Rust simulator/JSON CLI is a real completed branch feature; need integration and proof boundary review, not a request to reproduce basic CLI from scratch.
- **Shared path collision:** #106, #57, #95 all modify PROGRESS, HANDOFF and EVIDENCE. Integrator only reconciles these docs after picking a tested code integration target; preserve old append-only evidence, do not blindly concatenate three independently evolving histories.
- **Parallel branches** #76 and #89: review ancestor/parallel/superseded status before inclusion. A parallel unused Draft is not automatically needed for #106.

Require `OWN_FEATURE_DELTA` (PR base...head), `INHERITED_DELTA` (main...head), dependency DAG, target-specific contract test, reviewed integration status, and explicit KEEP/DEFER/EXCLUDE disposition. No unauthorized merge or history rewrite. Values are observation snapshots and must be recomputed before integration.
