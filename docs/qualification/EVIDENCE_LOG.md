# Aura Qualification — Append-only Evidence and Findings Log

**Baseline:** `main@e8f1c35a4a56adbde983dce38bba59a8c4eee105`, source-reviewed 2026-10-23; existing historical documents may reflect earlier commits.  
**Evidence hierarchy:** actual reproduced run + artifact > inspected current code > old audit > hypothesis/target.  
**Correction policy:** never erase a historical finding; append a new entry `supersedes: EV-...` with why and a live evidence link. Do not confuse a code citation with proof of runtime behavior.

| Evidence ID | Task | Classification | Observation / test required | Source pointer |
|---|---|---|---|---|
| EV-001 | AURA-Q01-001 | static-code-confirmed | CLI verify returns success from non-Z3 path; reproduce exit code in no-feature build | `aura/src/main.rs` (`verify_file`) |
| EV-002 | AURA-Q01-002 | static-code-confirmed | report code may write `ok:true` without proof; reproduce JSON and exit | `aura/src/main.rs` (`verify_file_with_report`) |
| EV-003 | AURA-Q01-003 | static-code-confirmed | native development build does not universally require Z3 verification; inspect profile matrix | `aura/src/main.rs` (`build`, `build_one`) |
| EV-004 | AURA-Q01-004 | static-code-confirmed | contract/assert statements can be lowered as no-op | `aura-core/src/lower.rs` |
| EV-005 | AURA-Q02-002 | static-code-confirmed | arithmetic emits U32 for some non-U32 operations | `aura-backend-c/src/emit.rs` (`emit_binop`) |
| EV-006 | AURA-Q02-003 | static-code-confirmed | range checks/bounds have u32-based assumptions | `aura-backend-c/src/emit.rs`, `aura-core/src/sema.rs` |
| EV-007 | AURA-Q02-005 | static-code-confirmed | unknown type erases to U32 | `aura-core/src/lower.rs` |
| EV-008 | AURA-Q03-001 | static-code-confirmed | record IR emission group has an empty arm | `aura-backend-c/src/emit.rs` |
| EV-009 | AURA-Q03-002 | static-code-confirmed | enum IR emission group has an empty arm | `aura-backend-c/src/emit.rs` |
| EV-010 | AURA-Q03-003 | static-code-confirmed | applied/tuple/ref/refinement/tensor type information may be lost on lowering | `aura-core/src/lower.rs` |
| EV-011 | AURA-Q09-003 | static-code-confirmed | package verification signature option unused; lock verification mostly structural | `aura-pkg/src/commands.rs`, `aura-pkg/src/lockfile.rs` |
| EV-012 | AURA-Q09-002 | static-code-confirmed | init template resembles Rust and differs from documented Aura syntax | `aura-pkg/src/commands.rs` |
| EV-013 | AURA-Q10-001 | static-code-confirmed | test CLI runs verify on files and can succeed with zero tests | `aura/src/main.rs` |
| EV-014 | AURA-Q08-003 | static-code-confirmed | C bindgen uses regex and unknown-type fallback | `aura-bridge/src/lib.rs` |
| EV-015 | AURA-Q07-005 | static-code-confirmed | tensor/global-table limits, silent invalid access and mock AI | `aura-stdlib/src/stdlib.c` |
| EV-016 | AURA-Q08-005 | static-code-confirmed | build cache inputs do not comprehensively include C headers and linker/runtime environment | `aura/src/main.rs` |
| EV-017 | AURA-Q03-006 | inspection-required | existing compat build path doesn't establish actual AVM/C behavioral parity for full corpus | `tools/compat/run.sh`, `.github/workflows/` |
| EV-018 | AURA-Q00-005 | historical | old inventory marks composite/scalar features QUALIFIED at earlier SHA; cannot carry forward as current qualification | `AURA_1_0_CAPABILITY_INVENTORY.md`, `PROJECT_STATUS.md` |
| EV-019 | AURA-Q00-001 | connector-observed | GitHub installation found; branch create succeeded; no source compile performed | branch `program/aura-long-horizon-qualification-v1` |
| EV-020 | AURA-Q08-001 | target | C is recommended Tier 1 **candidate**, not yet qualified | `MASTER_ROADMAP_V1.md` |
| EV-021 | AURA-Q04-008 | target | latency under 200 ms P95 is target only, not measured | `PUBLIC_CLAIMS_POLICY.md` |

## How to append real verification evidence

Use a new ID (EV-022 onward), with `task-id | source commit | OS/arch/compiler/feature flags | exact command | expected vs actual result | CI/test/artifact link | classification | date`. A local command without archived output may support a reproduced finding, but not a publicly reproducible release qualification. On remediation, append a new entry referencing the previously found defect and the new passing test. **Do not mark static findings VERIFIED solely from reading code.**

## Open reproduction order

1. No-Z3 `verify` + `--report` false-success (EV-001/002).
2. Assertions under C native build profile and `verify` profile (EV-003/004).
3. Integer/float widening, signed values, overflow and narrow range (EV-005/006/007).
4. Record/enum construct/read/update/match on actual C backend (EV-008/009/010).
5. Package signature-verification adversarial fixture (EV-011).

- **EV-022 (2026-10-23, CI-OBSERVED, AURA-Q00-004):** [GitHub Actions qualification governance run 39921540244](https://github.com/danielforface/aura-lang/actions/runs/39921540244) completed SUCCESS for initial bootstrap head `04dbdc05925315235d83a5e9dcdb398b0c5baaed`; this verifies the script executes on the bootstrap PR, NOT that a later attempted plan modification has been adversarially rejected.
- **EV-023 (2026-10-23, CONNECTOR-OBSERVED, AURA-Q00-001):** [Draft PR #9](https://github.com/danielforface/aura-lang/pull/9) opened against main, no merge; [issue #10](https://github.com/danielforface/aura-lang/issues/10) created as scoped, *not-yet-reproduced* Q01 technical follow-up. Website Static Export and Dependency Review also reported SUCCESS on initial head; Core CI remained IN_PROGRESS when checked. No compiler tests performed.

- **EV-024 (2026-10-23, TEST-FIRST PREPARED, AURA-Q01-001/002):** initial test-only Q01 commit `9c6172a21e1dec551e51a044c25ffbb2df4b744f` introduced no-Z3 command and JSON report negative tests. [Workflow run 39927689905](https://github.com/danielforface/aura-lang/actions/runs/39927689905) was still IN_PROGRESS when implementation was prepared. Reproduction is NOT YET CONFIRMED; check actual logs later. Source patch submitted on [Draft PR #11](https://github.com/danielforface/aura-lang/pull/11), not qualified without actual tests.

- **EV-025 (2026-10-23, REPRODUCED, AURA-Q01-001/002):** [GitHub Actions baseline run 39927640657](https://github.com/danielforface/aura-lang/actions/runs/39927640657) on SHA `9c6172a21e1dec551e51a044c25ffbb2df4b744f`, Ubuntu runner, command `cargo test -p aura --locked --no-default-features --test verify_fail_closed -- --nocapture`: test compilation SUCCEEDED; test result **FAILED, 0 passed / 2 failed as expected**. Logs: `verify_without_z3_exits_nonzero_instead_of_claiming_success` saw `ExitStatus(unix_wait_status(0))`; `verify_report_without_z3_is_failure_not_false_proof` also observed zero exit. Minimal fixture `cell main() ->: val x: u32 = 1; assert x == 2`. This confirms false-positive CLI success. Patched code at `649cf300` was awaiting CI at time of entry; passing fix not yet established.

- **EV-026 (2026-10-23, REPRODUCED-AND-REMEDITED-IN-CI, AURA-Q01-001/002):** [patched GitHub Actions run 39929440082](https://github.com/danielforface/aura-lang/actions/runs/39929440082) on SHA `4868112391d19bfa3a665a8a87edaf870a86cb24`, Ubuntu, command `cargo test -p aura --locked --no-default-features --test verify_fail_closed -- --nocapture`: cargo compilation succeeded; **2 passed / 0 failed** (both `verify_without_z3_exits_nonzero_instead_of_claiming_success` and `verify_report_without_z3_is_failure_not_false_proof`). Baseline [EV-025 / run 39927640657](https://github.com/danielforface/aura-lang/actions/runs/39927640657) was **0 passed / 2 failed** with exit 0 for same unprovable program. Note: this evidence qualifies only no-Z3 CLI path; it does not establish success with actual Z3, runtime assertion preservation, or a released artifact. [PR #11](https://github.com/danielforface/aura-lang/pull/11) still draft/unmerged.

- **EV-027 (2026-10-23, TEST-FIRST PREPARED, AURA-Q01-003/004):** issue [#12](https://github.com/danielforface/aura-lang/issues/12); stacked branch `qual/Q01-003-004-native-contract-gate` includes negative CLI tests (no-Z3 C `dev` and `release` with false assertion) and a positive contract-free native build. Expected RED is only a hypothesis until GitHub Actions completes. Source evidence: `aura-core/src/lower.rs` contract match returns `Ok(())`, `aura/src/main.rs` default profile bypasses verification, cache restoration precedes build. Test execution: NOT RUN as of initial commit.

- **EV-028 (2026-10-23, SOURCE-PATCH-PENDING-CI, AURA-Q01-003/004):** [Draft PR #13](https://github.com/danielforface/aura-lang/pull/13) proposes an AST-aware cache bypass for proof-bearing source and a Z3-or-reject gate before native C/WASM IR lowering. This is static patch evidence ONLY; test-first baseline was still building when patch was prepared. Proof soundness and complete contract semantics remain open; no verified status claimed.

- **EV-029 (2026-10-23, CODE-REVIEW-ONLY, AURA-Q01-003/004):** source audit found `report::analyze_verify_surface` previously skipped nested `Stmt::MacroDef` and `Stmt::Render` bodies; initial Q01 patch extends traversal and adds nested conditional negative CLI fixture plus scope documentation. Runtime proof still pending CI. This does not prove complete AST/implicit-obligation coverage.
