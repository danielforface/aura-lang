# Q01-005 — Typed unsupported feature handling (bounded proposal)

**Issue:** [#22](https://github.com/danielforface/aura-lang/issues/22). **Branch:** `qual/Q01-005-typed-unsupported`. **Review:** stacked Draft PR #23 on #21, no merge. This is not the immutable master roadmap.

The verifier's explicit refusal to analyze unsupported source-level expressions or quantifiers barred by the active SMT profile is not evidence of a counterexample or a false assertion. These known code paths now carry `VerifyFailureKind::Unsupported` from the verifier to the CLI report. Schema `aura.trusted-core.v2` extends its enum by `status="unsupported"`, `reason_code="verification_unsupported"`, `ok=false`, and omits proof evidence. The existing `Unknown`, `Failed`, `NotRun`, `InvalidInput`, `Proved` statuses retain their previous meanings.

**Scope of typed tagging:** verifier quantifier restrictions (Fast/Ci), excessive quantifier binders, unsupported boolean or integer AST forms, and range verifier rejecting a symbolic non-literal. Neither timeout nor solver-unknown nor an ordinary SAT counterexample is relabeled as unsupported. The classification must arise from a typed enum, not substring matching in the error message. Other unsupported/plugin/lowering conditions are *not* comprehensively classified.

**Test contract:** real `aura verify --smt-profile fast --report` with a valid `forall` tautology must return unsupported/nonzero and no evidence; the same source under Thorough should be provable. Typed tests ensure prose cannot smuggle unsupported status. Real Z3 normal true/false regressions and no-Z3 fail-closed regressions must remain GREEN. At this patch creation, do not claim CI outcomes or tested end-to-end support until observing exact workflow logs.

**Unqualified:** full timeout/cancelled/internal error/out-of-memory taxonomy, typed exit codes beyond zero/nonzero, proof/model backend fidelity and complete type coverage. Pending separate Q01/Q02/Q04 work. No autonomous merge or roadmap edits.

## Cross-task discovery: quantifier reflexivity is not yet qualified

[Issue #24](https://github.com/danielforface/aura-lang/issues/24) is a real independently reproduced false-negative: under Thorough, `assert forall(x: u32): x == x` returned `failed` in [baseline CI 40123015014](https://github.com/danielforface/aura-lang/actions/runs/40123015014) and [first patched CI 40123269360](https://github.com/danielforface/aura-lang/actions/runs/40123269360). Its failing regression is *preserved as `#[ignore]`*, not dropped or claimed fixed. The bounded Unsupported status patch only establishes that Fast rejects the profile-disallowed feature with the correct **typed** outcome. A separate Q01/Q04 model correction with RED→GREEN is a blocking task before qualifying quantified proofs as broadly supported.

## Second independent Thorough false-negative

[CI 40124194208](https://github.com/danielforface/aura-lang/actions/runs/40124194208) also found **plain `assert 1 == 1` under Thorough failed**, not only the universal tautology. That is now tracked in [Issue #24](https://github.com/danielforface/aura-lang/issues/24) as a proof-engine correctness defect, potentially affecting unassumed goals. Both named tests remain in the repository as `#[ignore]` **until** the dedicated fix is demonstrated with RED→GREEN; they are not pass evidence and do not qualify Thorough semantics. The scoped Unsupported taxonomy change is reviewed separately because it merely reports explicit refusal truthfully.
