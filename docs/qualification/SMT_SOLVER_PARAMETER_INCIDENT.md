# Q01/Q04: SMT solver parameter incident — Issue #24

**Status:** source fix code tests GREEN at [CI 40129765942](https://github.com/danielforface/aura-lang/actions/runs/40129765942), verifier/LSP final check PENDING when this note was written. [Draft PR #25](https://github.com/danielforface/aura-lang/pull/25), stacked and unmerged.

## Observed failure and root cause
The actual Aura verifier rejected `assert 1==1` and `forall(x:u32): x==x` in both Fast/Thorough proof profiles, while direct Z3 proved them. A 16-configuration Z3 matrix [run 40128912419](https://github.com/danielforface/aura-lang/actions/runs/40128912419) found `smt.random_seed=0` to be a strong discriminator in the pinned dependency environment: all eight masks including that key returned **SAT** for the negation of a contradiction, while all eight without it returned **UNSAT**. The problem reproduced in a raw Z3 test independent of Aura AST; avoid assuming all other solver versions behave similarly.

## Code correction and safety contract
The verifier no longer sets `smt.random_seed` in either of its solver call sites. Timeouts, `sat.random_seed` and UNSAT core extraction remain. The negated proof goal is a mandatory `assert_and_track`, never an optional premise. Known-bad raw reproduction remains quarantined as `#[ignore]` (not counted as passed). All eight safe parameter combinations are tested actively. Previously ignored real-CLI true assertions are now active in the child branch, together with false-property controls and existing status/no-Z3/durability regressions.

A proof must be marked `proved` only when Z3 returns UNSAT to the actual negated obligation under consistent assumptions, with explicit evidence. SAT means the proof was not established; `Unknown` is not proof; profile-specific unsupported operations are not refutations.

## Remaining limitations
This bounded fix is not validation of all Z3 builds/platforms, quantifier sorts and overflow semantics, external trust or source-to-native preservation. General SMT soundness, process exit status taxonomy, proof cache invalidation and auditable trust surface are still open under frozen MASTER_ROADMAP_V1. Do not call full Q01/Q04 complete. No PR may be merged without explicit maintainer approval.
