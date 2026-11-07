# Evidence-based workflow audit — 2026-11-06
**Classification:** review snapshot, not code release. Consult live GitHub for updated heads and CI conclusions.

## Claim review
| Criticism | Finding | Consequence |
|---|---|---|
| No real code from Core | **Refuted.** [PR #57](https://github.com/danielforface/aura-lang/pull/57) changes compiler frontend, IR, C emitter, verifier, native/source tests. 278 commits, 47 changed paths observed. | Keep Core highest integration priority; do not discard its real fixes. |
| Q03 PR #51 repeatedly fails | **Misidentified.** [#51](https://github.com/danielforface/aura-lang/issues/51) is an Issue; PR #57 addresses it. [42213904423](https://github.com/danielforface/aura-lang/actions/runs/42213904423) was semantic RED at `local-record-native`; [42214100867](https://github.com/danielforface/aura-lang/actions/runs/42214100867) later 4/4 green. Latest HEAD can still be pending. | Avoid labeling legitimate test-first RED an enduring production regression. Stop remote RED that adds no useful semantic proof. |
| Lumina did no production work | **Refuted historically.** Atomic native Hangul D+C and fault isolation were implemented earlier. | Protect safety fixes. |
| Lumina P36/37 are product fixes | **Not established.** [PR #103](https://github.com/danielforface/aura-lang/pull/103) uses diagnostic-only deferred helper callback scheduling; [#106](https://github.com/danielforface/aura-lang/pull/106) feature-gated counters, tests, unchanged default runtime. | Force FIX / CLOSE_RESEARCH decision; avoid further counter-only Phase. |
| Issue #59 is the IBus bug | **Incorrect.** [Issue #59](https://github.com/danielforface/aura-lang/issues/59) governs cross-agent ownership. | Keep it coordination-only; open/link a narrow product bug issue if reproduced. |
| CI was used for deliberate missing-function RED | **Confirmed.** [PR #106](https://github.com/danielforface/aura-lang/pull/106) cites E0599 `diagnostic_ack_drop_counts` in remote CI as a test-only RED rather than semantic bug. | Ban this from normal required CI; compile incomplete tests locally and use semantic RED where meaningful. |
| Quantum is documentation only | **Refuted.** [#99](https://github.com/danielforface/aura-lang/pull/99) has Rust statevector simulator; [#108](https://github.com/danielforface/aura-lang/pull/108) Qiskit oracle and scaling work. | Keep Agent C independent but feature-first, and track Q28 as branch-level IN_PROGRESS. |
| Draft PR nesting is substantial | **Confirmed.** 70 open out of 78 user PRs inspected; 73 repository branches on first page, including many historical layered Q01/Q03/UI branches. | Stop new micro-stacks; appoint integration steward and plan reviewed merges, avoid auto-merging. |
| Claims policy forbids production edits | **Not in the policy.** `PUBLIC_CLAIMS_POLICY.md` regulates truthful claims; `AURA_1_0_QUALIFICATION_PROGRAM.md` regulates correctness. | Fix day-to-day process rather than weakening soundness. |
| Governance document churn | **Confirmed.** `check_governance.py` required both central ledgers to change on *every* substantive PR. | Relax this to either neither central ledger or both, with milestone-based integration ownership. |

## Specific CI data, observed on 2026-11-06
- Core recent red: both Push [42213898136](https://github.com/danielforface/aura-lang/actions/runs/42213898136) and PR [42213904423](https://github.com/danielforface/aura-lang/actions/runs/42213904423) failed ONLY Native job at pre-fix EV-203 commit; newer [42214100867](https://github.com/danielforface/aura-lang/actions/runs/42214100867) PR 4/4 succeeded on different SHA.
- B final PR [42195826388](https://github.com/danielforface/aura-lang/actions/runs/42195826388) three job success at `92f0e0a9`; standard production helper unchanged.
- C final PR [42214329430](https://github.com/danielforface/aura-lang/actions/runs/42214329430) three job success at `e384680c`; separately checked parity/benchmark only within declared scope.

## Root-causes and anti-corruption boundaries
1. **Misaligned output unit**: phase counted as a docs/CI checkpoint rather than actual observable capability.
2. **Expensive early remote integration**: known missing methods deliberately used to trigger broad CI.
3. **Stack debt**: unmerged Drafts are treated as final destinations, making the progress dashboard detached from releaseable code.
4. **Ledgers are a write hotspot**: mandatory changes to two enormous shared docs on all PRs create churn and rechecks.
5. **Scope can become a self-sustaining research loop**: a real safety anomaly must end with a decision, not endless telemetry.

## Causality limit
This audit is based on current public GitHub PR descriptions, changed paths, selected patches, workflow/job conclusions, relevant issue and governance documentation. It does **not** reproduce an IBus customer-visible error or run all 70 open PR suites independently. It does not authorize a speculative C/D callback acceptance or a bulk merge. See [Production-first policy](DELIVERY_OPERATING_POLICY_V1.md).


## Addendum — cross-agent integration discovery after Agent B Autofocus (2026-11-06)

See [Cross-Agent Protocol](CROSS_AGENT_COORDINATION_V1.md). B's Native TextInput Autofocus at Draft #106 is already implemented and 6/6 in observed Push/PR runs, yet the branch is 542 commits ahead of main while the own PR delta is 5 commits/9 paths. It is an ancestry-dependency (D4), not necessarily a source-path write collision. Core A and B are diverged; shared PROGRESS/HANDOFF/EVIDENCE are the exact overlap between A/B/#95. Agent C's #108 has implemented offline JSON CLI and dynamic Rust simulator. An integration decision must inspect **own feature delta vs inherited delta**, contract consumers, actual merge base, separate parallel PR #76/#89, and an isolated target integration test. This is not evidence to cancel productive parallel implementation.
