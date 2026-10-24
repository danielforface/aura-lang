# Q01 typed SMT Unknown — bounded status propagation
**Issue:** [#18](https://github.com/danielforface/aura-lang/issues/18). **State:** scoped CI green ([run 39973715747](https://github.com/danielforface/aura-lang/actions/runs/39973715747)); [Draft PR #19](https://github.com/danielforface/aura-lang/pull/19) IN_REVIEW, unmerged; full taxonomy remains open.

`VerifyError.kind` carries a structural `VerifyFailureKind::{Failed,Unknown}`. Known Z3 `SatResult::Unknown` paths in the verifier and geometry are tagged explicitly and become report-v2 `status=unknown`, `reason_code=solver_unknown`, `ok=false`. Generic errors remain failed. Classifying based on text is forbidden.

**Limitations:** no deterministic *real Z3 Unknown* end-to-end fixture yet; report unit tests exercise typed propagation deterministically, CLI regression verifies true/false and no-Z3 paths. Timeout, cancelled, unsupported, internal_error and machine-specific exit codes are not classified yet. None of this establishes proof-to-native correctness.

**CI:** compile impacted CLI/verifier/LSP crates, both no-Z3 and real-Z3; run typed mapping tests and CLI report/fail-closed regressions. Fix any build breaks before claiming completion. Keep stack draft/unmerged. Do not edit frozen roadmap.
