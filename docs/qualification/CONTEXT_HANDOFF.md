# Aura Program — Always-Current Fresh-Chat Handoff

**Read this first in any new conversation.** This is operational continuity, not a frozen spec.  
**Updated at:** 2026-10-24 (issue #20 test-first report safety reproduction pending)  
**Repository:** https://github.com/danielforface/aura-lang  
**Current working branch:** `qual/Q01-002-report-durability` (stacked on #19 → #17 → #15 → #13 → #11 → #9)  
**Base:** `main`, initial commit `e8f1c35a4a56adbde983dce38bba59a8c4eee105`  
**Active gate/train:** Q00/R0 in review, Q01/R1 IN_PROGRESS with tested CLI/native/compatibility paths GREEN; issue #14 IN_REVIEW in Draft PR #15. **No releases or merges.**  
**Current focused scope:** [issue #18](https://github.com/danielforface/aura-lang/issues/18): structural SMT Unknown propagation to CLI reports, under CI. Parent #17 green but unmerged.  
**Draft PR chain:** [#9](https://github.com/danielforface/aura-lang/pull/9) → [#11](https://github.com/danielforface/aura-lang/pull/11) → [#13](https://github.com/danielforface/aura-lang/pull/13) → [#15](https://github.com/danielforface/aura-lang/pull/15) → [#17](https://github.com/danielforface/aura-lang/pull/17) → [#19](https://github.com/danielforface/aura-lang/pull/19). **All Draft/unmerged**; user approval needed before any merge.  
**Current issue:** https://github.com/danielforface/aura-lang/issues/18 (AURA-Q01-005/006); #16 green/unmerged.  
**Source of truth for current HEAD/CI:** fetch live `qual/Q01-005-typed-solver-unknown` and its Draft PR; historical SHAs are not current HEAD.  
**Plan:** `docs/qualification/MASTER_ROADMAP_V1.md` — frozen definition. **Do not revise it to reflect progress.**

## 1. Why we are doing this — the user intent

The user owns/maintains Aura, an active pre-stable proof-driven systems programming language, and requests **direct, long-term engineering work inside the GitHub repository**. A prior deep source audit identified substantial maturity gaps before Aura can honestly be called a meaningful, reliable systems language. The user **does not want a one-shot attempt to upgrade everything**. They want a **multi-stage, many-release program** built *on top of* existing roadmaps, executing small increments with concrete tests, evidence, review, and release milestones.

**Non-negotiable special requirement:** a user may move to a completely new chat **at any arbitrary point**, without warning. Therefore, keep this handoff document **accurate on every step**, and submit its update alongside progress/implementation changes. It must explain the purpose, role, prior decisions, work already done, present context, blockers and exact next action; Git alone cannot convey these intentions. A fresh chat should be able to continue without asking the user to retell the story. This is a synopsis of intent/context, **not an unrestricted transcript**: record decisions and outcomes, don't store secrets, tokens, personal details or unverified assumptions.

The user explicitly approved GitHub integration across their projects; **do not repeatedly suggest restricting repository scope**. Work on this repository only unless explicitly told otherwise. Use GitHub tools to actually read/write; do not imply browser OAuth alone guarantees write access.

## 2. Prior audit — key engineering conclusions and known limitations

Aura has a substantial Rust workspace (`Cargo.toml` workspace version `0.2.0`, 22 crates), including lexer/parser/AST/sema/IR, Z3 verifier, AVM, native C and experimental LLVM backend, LSP/Sentinel, packages, SDK, runtime, Android/AI/Lumina experiments. The existing `AURA_1_0_QUALIFICATION_PROGRAM.md` and `ROADMAP_PUBLIC.md` are ambitious and must be preserved. Old `AURA_1_0_CAPABILITY_INVENTORY.md` may mark features `QUALIFIED` despite insufficient evidence, and was audited against an earlier source revision.

**Trust-critical source-reviewed risks to reproduce first:**
1. `aura/src/main.rs`: a non-Z3 `aura verify` path can return success, and report mode can claim `ok:true` without solver proof (AURA-Q01-001/002).
2. `aura-core/src/lower.rs`: `requires/ensures/assert/assume` can disappear from emitted native IR, while the usual C build path does not require proof (AURA-Q01-003/004).
3. `aura-backend-c/src/emit.rs`: several record/enum IR operations have an empty code-emission branch (AURA-Q03-001/002).
4. `aura-backend-c/src/emit.rs`: arithmetic and range checking can hardcode `u32` even though typed `u64/i64/f32/f64` were added (AURA-Q02-002/003).
5. `aura-core/src/lower.rs`: unresolved `Unknown` maps to `U32`; generic/tuple/ref/tensor/refinement type details are erased (AURA-Q02-005 / AURA-Q03-003).
6. `aura-pkg/src/commands.rs`: package verification omits requested signature verification and init can generate Rust-like `pub fn` not canonical Aura `cell`; manifest/path expectations diverge (AURA-Q09-001/002/003).
7. `aura/src/main.rs`: `aura test` currently verifies files rather than running behavioral tests and may return success with none (AURA-Q10-001).
8. `aura-bridge/src/lib.rs`: regex-based C header extraction and fallback for unknown types to `u32` (AURA-Q08-003).
9. `aura-stdlib/src/stdlib.c`: bounded prototype tensor handles, silent invalid indices and mock AI/IO (AURA-Q07-004/005).
10. Native build cache may omit C headers, linker inputs, compiler/version/target dependencies (AURA-Q08-005); some CI workflows verify syntax/compiler stages, not semantic differential results (AURA-Q03-006).

These are **static-source review findings**, not newly executed repro tests. Do not upgrade to `reproduced`/`verified` without actual command results, commit and environment. Never state that Aura is already formally verified end-to-end, universally memory safe or production-JIT capable.

## 3. GitHub access and operational setup

The user initially had OAuth authorization but no installed GitHub App; attempting `create_branch` returned `403 Resource not accessible by integration`. The user installed **ChatGPT Codex Connector** with read/write code, actions, issues, PR and workflow privileges, and selected all repositories by preference. Retesting `list_installations` returned installation ID `174399905` for `danielforface` and `create_branch` succeeded. A read of `ROADMAP_PUBLIC.md` from the branch verified access.

**Never embed tokens.** In any fresh chat, discover/use the connected GitHub app and check `get_repo`/branch/PR plus effective writes; if access is missing, ask for reconnection instead of claiming a mutation succeeded. GitHub connector authentication need not persist identically across chats.

Branch `program/aura-long-horizon-qualification-v1` was created from `main@e8f1c35a4a56adbde983dce38bba59a8c4eee105`; avoid writing to main directly. Actual GitHub branch/PR/HEAD must be queried each session (this field may be stale). Scope is a bootstrap plan plus continuous implementation via small reviewable follow-up PRs. Never merge without user approval.

## 4. What has actually been done in this session

- Verified owner and repository access; owner is `danielforface`, permission push/admin, default branch `main`.
- Confirmed previously missing App install; later successful write test created `program/aura-long-horizon-qualification-v1`.
- Read current `ROADMAP_PUBLIC.md`, `PROJECT_STATUS.md`, `AURA_1_0_QUALIFICATION_PROGRAM.md`, `AURA_1_0_CAPABILITY_INVENTORY.md`, `PUBLIC_CLAIMS_POLICY.md`, `README.md` and `Cargo.toml` on main.
- Selected immutable canonical plan v1 plus mutable progress, evidence, ledger and handoff, with governance CI guard and a brief entry in old public roadmap rather than rewriting it.
- Committed initial R0 documentation/governance scaffold as [`04dbdc0`](https://github.com/danielforface/aura-lang/commit/04dbdc05925315235d83a5e9dcdb398b0c5baaed): 88 frozen task IDs, 12 gates, 9 trains, mutable tracker/handoff/evidence/release ledger, operating protocol, check script and GitHub Actions workflow; preserved existing Gates A–G and added navigation to original public roadmap. No language source changed.
- Opened [Draft PR #9](https://github.com/danielforface/aura-lang/pull/9) against main (no merge). Confirmed files are present on branch. [Qualification governance run 39921540244](https://github.com/danielforface/aura-lang/actions/runs/39921540244), Dependency Review and Website Static Export reported SUCCESS on initial bootstrap head. Core CI was IN_PROGRESS at last check, so no full-CI success claim.
- Created [GitHub issue #10](https://github.com/danielforface/aura-lang/issues/10) as first scoped P0 technical follow-up: reproduce no-Z3 verify success and false ok:true report, then fix fail-closed behavior with tests.
- **This status/handoff/evidence follow-up commit** records those events together. Check current PR/CI after it lands; no compiler tests or runtime repro were run in this session.

## 5. Rules and decisions already agreed

- **D-001:** Multiple small releases, no forced big-bang 1.0 or deadline.
- **D-002:** The canonical plan content is set now and then **immutable**; if scope absolutely changes, add a superseding version while preserving v1. Progress never edits the master plan.
- **D-003:** Progress and fresh-chat handoff are separate, always updated in same unit of work; evidence is append-only; all permanent work lives in the repository.
- **D-004:** Keep existing roadmaps/qualification program, add execution overlay and stable task IDs.
- **D-005:** Prioritize soundness/trust and actual native semantics over broad new features, UI, AI, JIT or extra backends.
- **D-006:** Draft PR against main, incremental source fixes on narrow branches, test/CI evidence before marking VERIFIED, no autonomous merge.
- **D-007:** User wants all-repositories GitHub App access retained; do not nag about scoping this integration.
- **D-008:** This handoff is required even when Git history seems self-explanatory: decisions, intent and unfinished context matter beyond diffs.

## 6. Current queue, blockers, explicit next safe action

**Current train:** R0 program foundation, active task Q00.  
**Currently in review:** [Draft PR #9](https://github.com/danielforface/aura-lang/pull/9), with governance PASS on its initial commit and complete CI status still requiring a fresh check.  
**Blockers:** no compiler tests, executable repro or release artifacts have been validated in this session; historical inventory is old; PR #9 review/merge not authorized. Governance workflow PASSED at initial PR head; other CI checks must be rechecked after latest commit.

**Next safe action (read repo state first):**
1. Fetch `docs/qualification/PROGRESS.md` and this file from the active branch, fetch latest [Draft PR #9](https://github.com/danielforface/aura-lang/pull/9) diff and *current* CI statuses; prior governance PASS is linked above.
2. Review the docs/CI guard, especially immutability and handoff update policy. **Do not merge** without explicit user/maintainer approval. Correct issues in a small commit updating both live documents.
3. After maintainer approval to merge R0, use [issue #10](https://github.com/danielforface/aura-lang/issues/10) on a **new narrow implementation branch** for `AURA-Q01-001`/`AURA-Q01-002`; first reproduce missing-solver `verify` false-success and `ok:true` reporting with actual commands.
4. Make verifier fail closed and add adversarial CLI regression tests; capture real tests/CI/commit in evidence.
5. Continue Q01 profile/assertion truth, then Q02 numeric correctness, Q03 composite lowering. Re-evaluate all tasks against current code each time.

**Do NOT** mark `AURA-Q00-004` or any compiler task VERIFIED merely because a script exists. **Do NOT** start coding Q01 without a reproducible test/baseline if a clean checkout is available.

## 7. Session handoff update contract — mandatory

At the **end of every implementation/documentation increment and before leaving a chat**, update this file and `PROGRESS.md` together on the working branch/PR. Explicitly write: date, actual latest HEAD/PR, what changed (paths and task IDs), test commands and results including NOT RUN, decisions and rationale, risk/blockers, task state transitions, pending review, actual release/tag (if any), and exact next action. Keep this short enough to scan but complete enough to execute. If an action fails or is interrupted, record it rather than implying success. The new assistant must read these files **before coding** and must update them again during its work rather than assume this conversation will follow it.

## 8. How to restart in a completely new chat

Send repository URL plus: **"Continue the Aura long-horizon qualification program. Read `docs/qualification/CONTEXT_HANDOFF.md`, `PROGRESS.md`, `MASTER_ROADMAP_V1.md`, `OPERATING_PROTOCOL.md`, `EVIDENCE_LOG.md` from the latest program branch. Verify GitHub connection/PR/branch state, then continue the recorded next safe action. Never rewrite the frozen plan and update progress + handoff with every increment."**

The latest GitHub files, not memory, are the source of continuity. See `OPERATING_PROTOCOL.md` for failure, PR and release rules.


## 9. Continuity update — first technical Q01 incremental work (2026-10-23)

**New decision D-009:** Start reversible/test-only *stacked* technical work while R0 governance Draft PR #9 is awaiting explicit maintainer merge approval. The feature branch is based on R0's head; its own PR should target the program branch, not `main`, until R0 is merged. No direct-main changes and no mixing compiler fixes into R0 bootstrap proposal. When #9 merges, rebase/retarget the technical PR after checking actual GitHub state.

**Verified CI for PR #9 at `b2bdf3928`:** [Core CI PASS](https://github.com/danielforface/aura-lang/actions/runs/39921912182), [Governance PASS](https://github.com/danielforface/aura-lang/actions/runs/39921912310), Website Static Export PASS, Dependency Review PASS. PR remains OPEN/DRAFT and unmerged.

**Current technical slice:** create a no-Z3 integration test that executes the real `CARGO_BIN_EXE_aura` with a semantically valid but nonprovable assertion; also exercise `--report` and confirm `ok:false`, an explanatory missing-solver error and nonzero exit. Dedicated workflow builds with `--no-default-features` and invokes only this integration test. **At test-first commit time results are NOT RUN; expect RED on current source.** First observe actual CI output before changing production code, then add a minimal fail-closed implementation, await GREEN and record evidence. Be careful that Core CI on PR #9 does not imply the new Q01 tests have passed.

**Next safe action:** look at Q01 branch CI; verify RED is due to expected assertions rather than invalid Aura fixture/build problems. Fix test setup if invalid. Then repair the non-Z3 code branches in `aura/src/main.rs`; build/report must never falsely imply proof. Update `PROGRESS.md`, this handoff and evidence together. Do not edit `MASTER_ROADMAP_V1.md` or merge either PR without user approval.

## 10. Technical continuity update (2026-10-23)

- Created stacked [Draft PR #11](https://github.com/danielforface/aura-lang/pull/11) from `qual/Q01-001-002-fail-closed-verify` into the R0 program branch. R0 [PR #9](https://github.com/danielforface/aura-lang/pull/9) remains unmerged. Never merge either without user authorization.
- Test-first commit `9c6172a21e1dec551e51a044c25ffbb2df4b744f` added real CLI test `aura/tests/verify_fail_closed.rs` and dedicated `.github/workflows/q01-verification-fail-closed.yml`. Its [first CI run](https://github.com/danielforface/aura-lang/actions/runs/39927689905) was IN_PROGRESS when observed. **Do not claim RED** until the log is inspected.
- Subsequent source patch touches only `aura/src/main.rs`: when feature `z3` is missing, `verify_file` now fails explicitly rather than returning success; `verify_file_with_report` writes a failure report with `ok:false`, an error naming `z3`, and no fabricated proof evidence. Clarified Verify help. This is limited to AURA-Q01-001/002; broader verifier taxonomy, runtime assertion enforcement and C correctness remain open.
- **Current validation status:** patched code has not yet produced an observed CI result at moment of commit; local Rust compiler unavailable. Recheck Q01 GitHub Actions runs, including first-commit expected RED and patched HEAD GREEN. Distinguish real reproduced failure from fixture/compiler errors. Preserve test results with exact run IDs in EVIDENCE_LOG, PROGRESS and this handoff.
- **Next safe action:** fetch latest Q01 PR HEAD and workflow status. Investigate RED/compile failures, correct implementation or tests, append evidence and update BOTH live docs in the same commit. Keep frozen plan unchanged. If ready for review leave Draft PR open and ask approval before merge.

## 11. CI baseline reproduction result (2026-10-23)

- At test-only SHA `9c6172a21e1dec551e51a044c25ffbb2df4b744f`, [GitHub Actions run 39927640657](https://github.com/danielforface/aura-lang/actions/runs/39927640657) compiled Aura then executed `cargo test -p aura --locked --no-default-features --test verify_fail_closed -- --nocapture` on Ubuntu. The integration suite reported `0 passed; 2 failed`. Both no-Z3 `verify` and no-Z3 `verify --report` produced **exit code 0** even though the fixture asserted `x == 2` when `x = 1`. This is reproduced evidence (EV-025), not just static source review; an issue #10 comment also records it.
- First implementation fix SHA `649cf30078c2bfef6283e34783336fa398854e7e` changes only `aura/src/main.rs` and handoff/progress/evidence. At the time of this context update, patched [run 39928636094](https://github.com/danielforface/aura-lang/actions/runs/39928636094) and [run 39928644146](https://github.com/danielforface/aura-lang/actions/runs/39928644146) were still IN_PROGRESS. **Do not claim fix passed until actual run logs confirm success.**
- **Next safe action:** inspect patched CI runs for success and output, and investigate if failures are due to compilation, fixture or implementation. For any further code/doc change, update PROGRESS and handoff together; keep master roadmap frozen. PR #11 remains stacked and Draft; PR #9 remains unmerged.

## 12. Verified CI observation and next task — 2026-10-23 (latest)

**The previous in-progress status above is historical and superseded by this section.** Paired test-first evidence is complete for this narrow path: [baseline run 39927640657](https://github.com/danielforface/aura-lang/actions/runs/39927640657) on `9c6172a` had **0 passed, 2 failed**; [patched run 39929440082](https://github.com/danielforface/aura-lang/actions/runs/39929440082) on `4868112391d19bfa3a665a8a87edaf870a86cb24` had **2 passed, 0 failed** in the same Ubuntu/no-Z3 integration command (`cargo test -p aura --locked --no-default-features --test verify_fail_closed -- --nocapture`). GREEN logs explicitly show both tests `... ok`. This establishes limited **no-solver CLI regression remediation**, not completeness of solver status taxonomy nor general contract soundness. [Draft PR #11](https://github.com/danielforface/aura-lang/pull/11) is unmerged, awaiting review; issue #10 is not closed; Q01-001/002 remain IN_REVIEW pending maintainer acceptance.

**Next safe action:** inspect current GitHub branch/PR/CI; begin separate test-first scope `AURA-Q01-003/004` for build profile and assertion/contract semantics, preferably in another stacked branch based on PR #11 if R0 and Q01 are not yet merged. Never silently discard a native assert or treat `assume` as proof. Capture real negative tests and classify behavior before fixing; update progress + this handoff every increment. **Do not merge #9/#11 automatically**, or modify `MASTER_ROADMAP_V1.md`. If code/branch HEAD has advanced, reconcile these notes with the actual repository.

## 13. Q01-003/004 native assurance tests initiated (2026-10-23)

**New Issue:** [#12](https://github.com/danielforface/aura-lang/issues/12); **new branch:** `qual/Q01-003-004-native-contract-gate`, parent Q01 PR #11 head `ab23a7b5ae164085709722339ba0ed3f6c5755c7`. It is stacked for review: PR #9 (R0) → PR #11 (no-Z3 verify) → upcoming native-contract PR, never directly on main and never merge without owner approval.

**Source-reviewed risk:** `aura-core/src/lower.rs` erases contracts/assertions with `Ok(())`; `aura/src/main.rs` `build_one` only calls `verify_file` for `profile verify` (or AVM), not default dev or release; C backend emits unverified output. The build cache restores existing artifacts before a proof gate, so the assurance policy must account for cache hits. This is source review, not runtime reproduction at commit time.

**Test-only increment:** new `aura/tests/native_contract_modes.rs` and `.github/workflows/q01-native-contract-gate.yml` exercise real CLI `build --mode hybrid --backend c` under no-Z3 `dev`/`release` with an explicitly false assertion, plus a positive contract-free dev build. Test-first CI is expected RED on unsafe branches but status NOT RUN at document creation. No implementation changes were made in this commit.

**Immediate next safe action:** fetch latest Q01-003/004 CI run and per-test stdout/stderr; distinguish verified false-success from fixture/C compile errors. If RED proves defect, choose minimal fail-closed interim assurance gate (native C/WASM must prove obligations or reject; preserve unannotated build). **Check cache-hit and alternate native build paths**, do not use source regex as a surrogate for syntax tree. Update progress + this handoff + evidence in each commit; do not mutate frozen master roadmap or merge PRs autonomously. Once work is validated, open/update Draft PR against `qual/Q01-001-002-fail-closed-verify`, then review stacked merge order.

## 14. Q01-003/004 native C/WASM interim gate implementation (2026-10-23)

- [Draft PR #13](https://github.com/danielforface/aura-lang/pull/13) targets the previous Q01 branch; [issue #12](https://github.com/danielforface/aura-lang/issues/12) defines acceptance. Test-only commit `794a83c2de7e0813cd1952772b8203c3850d8b3d` contains 3 real-CLI tests and an Actions workflow (expected first RED). **Actual baseline CI had not yet completed at the time of implementation**. Do not assert reproduction without inspecting output.
- Implementation changes `aura/src/main.rs` in two paths: `build_cached` parses augmented source and skips cache for detected assurance constructs or C bridge headers; the actual `build` path, after semantic analysis and before IR lowering, calls the Z3 verifier when native C/WASM code contains assumptions/obligations, or returns an explicit error when the binary lacks Z3. This prevents the known silent contract-drop route in no-Z3 dev/release and blocks production codegen without proof for annotated C/WASM. Contract-free dev build remains possible.
- Caveats: this is a *conservative interim gate*, not a proof of full compiler soundness. It does not yet address implicit non-AST refinement obligations, compiler/SMT semantics mismatches, complex extern interactions, implicit bridging trust, or taxonomy of verifier outcomes. `--profile verify` may prove twice on contract-bearing native source; performance to optimize after safety. Cache is deliberately bypassed for bridged input until full cache inputs are sound (Q08-005).
- **Next safe action:** check PR #13 GitHub Actions baseline first-commit RED (confirm expected semantics) and patched GREEN at new head. If test harness/compile fails, fix and rerun; inspect actual reports and verify all 3 tests. Append measured evidence and update PROGRESS + handoff in same change. Keep PR #9 and PR #11 unmerged until maintainer authorization; leave PR #13 draft and stacked. NEVER edit frozen roadmap.

## 15. Additional bounded Q01 refinement (2026-10-23)

- Created `docs/qualification/Q01_NATIVE_ASSURANCE_POLICY.md`: exact *interim* C/WASM matrix for dev/release/verify with and without Z3, cache bypass, AVM/LLVM exclusions and explicit non-guarantees. This note is not a normative frozen roadmap amendment.
- Hardened `aura/src/report.rs` `analyze_verify_surface` to walk `MacroDef` and `Render` bodies (which contain nested blocks) rather than overlooking their assertions/assumptions. Added nested conditional assertion negative test to `aura/tests/native_contract_modes.rs`.
- **At commit creation:** initial baseline workflow and patched workflow were still compiling; the latest code's CI has **NOT BEEN VERIFIED**. On resume, fetch all PR #13 run statuses, check baseline RED and latest GREEN, archive precise outputs in EVIDENCE_LOG; if failing, fix and rerun. Check stacked R0 #9, Q01 #11 and #13 branches; none merged automatically.

## 16. Native gate CI failure triage and correction (2026-10-23)

**First-commit baseline RED confirmed:** [run 39938385042](https://github.com/danielforface/aura-lang/actions/runs/39938385042), head `794a83c`, compiled and executed 3 native CLI tests. `dev_native_build_must_not_erase_false_assert` and `release_native_build_must_not_erase_false_assert` failed **because both returned exit 0 and wrote module.c**, despite unprovable assertion. The positive contract-free build passed. This proves a real semantic gap; evidence EV-030.

**Patch blocked by Rust compilation:** [run 39938775467](https://github.com/danielforface/aura-lang/actions/runs/39938775467) at `5c7f050` and [run 39939359111](https://github.com/danielforface/aura-lang/actions/runs/39939359111) at `eff15d7` failed in `aura/src/main.rs` line ~1253: `error[E0277]: the trait bound ErrReport: Diagnostic is not satisfied`. Root cause: `parse_source_with_config` already yields a `miette::Report`, which must not be wrapped by `miette::Report::new`. **Neither failed run establishes safety of the intended patch.**

**Correction committed in this increment:** `aura/src/main.rs` now uses direct `?` for parsing, and short-circuits bridged headers to uncached real build *before* parsing combined text (which can contain bare header paths). New CI result NOT YET OBSERVED at commit creation. Recheck latest PR #13 head/run and inspect actual logs. Investigate any new failures, record GREEN with run URL if achieved, update PROGRESS+handoff+evidence together. Leave all stacked PRs draft/unmerged; frozen plan unchanged.

## 17. Compatibility blocker and workflow refinement (2026-10-23)

- Discovered a source-derived integration risk: `tools/compat/run.sh` executes `cargo run -q -p aura --locked --no-default-features -- build "$f" --mode avm` for compatibility fixtures; `build_one` calls `verify_file` for all `mode==Avm` and prints `avm: verified`. After Q01-001/002 correctly makes no-Z3 `verify` fail closed, compatibility suite may fail, as it previously conflated parse/typecheck with proof. Logged in [issue #14](https://github.com/danielforface/aura-lang/issues/14) and [PR #13 comment](https://github.com/danielforface/aura-lang/pull/13). **Do not mark reproduced** until command executed; do not weaken proof correctness to fix CI. Resolve via explicit typecheck-only compatibility path or real-Z3 verification, with tests.
- Streamlined `.github/workflows/q01-native-contract-gate.yml`: stop duplicate runs caused by simultaneous `push` and `pull_request` on the stacked feature branch (main/program PR coverage retained), and enable `Swatinem/rust-cache@v2` used by Core CI for iterative test speed. These are workflow changes, not proof quality evidence.
- **Latest source change:** commit `da39c0c` fixes miette E0277 and bridge cache bypass. Its Actions [run 39948322245](https://github.com/danielforface/aura-lang/actions/runs/39948322245) and [run 39948332418](https://github.com/danielforface/aura-lang/actions/runs/39948332418) were still IN_PROGRESS at the last observation. **Next safe action:** fetch their outcomes/logs and the new workflow HEAD run; inspect failing tests or capture real GREEN, update PROGRESS+CONTEXT_HANDOFF+EVIDENCE_LOG with exact run links and actual outcomes, then review stacked PRs. No merging to main without user approval. Frozen plan unchanged.

## 18. Top-of-document reconciliation (2026-10-23)

Corrected stale header references carried over from initial R0/Q01 branch. Current work is **Q01-003/004 in Draft PR #13**, not a new Q01 issue being planned. Exact next task remains patched CI validation and compatibility issue #14 reproduction. This documentation-only reconciliation does not imply success of in-progress CI. Follow newest section over historical earlier status notes. Current source patch before this note is `da39c0c`; current workflow+docs head is `0479214`. Frozen roadmap unchanged, no merges or releases.

## 19. Native no-Z3 regression GREEN; Z3-proof coverage initiated (2026-10-23)

- [CI run 39948322245](https://github.com/danielforface/aura-lang/actions/runs/39948322245) at code commit `da39c0cd9d63f562a838b51a76f00057e9083c7b` **PASSED**, logging all four real-CLI Ubuntu/no-Z3 tests: false assert rejected in `dev`, `release`, nested `if`; no-contract dev build allowed. Compare baseline RED [39938385042](https://github.com/danielforface/aura-lang/actions/runs/39938385042), **1/3 passed**. EV-033 captures measured result. **Still not a full qualification:** no real Z3 positive proof, verifier soundness, cache poisoning or unsupported backend semantics established by these four tests.
- Added `aura/tests/native_contract_z3.rs` (3 real CLI cases with Z3: true assertion accepted, false assertion rejected in dev/release) and a separate Ubuntu Actions job installing `libz3-dev`; results **NOT RUN at commit creation**. Goal is to test the proof-enabled branch of the new native gate, not to claim all verification is sound.
- **Next safe action:** check current PR #13 run(s), look at both no-Z3 and Z3 job results. If solver runtime/dependency or proof mismatch occurs, diagnose with actual logs, then fix minimally; document every observation. After that, reproduce [issue #14](https://github.com/danielforface/aura-lang/issues/14) no-Z3 `tools/compat/run.sh` regression and make compatible semantics explicit. Keep PR #9/#11/#13 Draft/unmerged; do not edit frozen plan.

## 20. Q01-003/004 Z3 AND no-Z3 GREEN; issue #14 explicit compatibility check implementation (2026-10-23)

- [GitHub run 39949851377](https://github.com/danielforface/aura-lang/actions/runs/39949851377) on PR #13 source `28fe3ccb3a01695589bec9bde2ae98e3067cd683`: Z3 job **3 passed/0 failed** (provable assertion accepted in dev, false assertion rejected in dev and release); no-Z3 job **4 passed/0 failed** (C dev/release/nested false assertion rejected and unannotated dev build succeeded). This narrows previously open Q01 contract-path concerns, but does not prove completeness of implicit obligations, C soundness, caching, AVM run or all target combinations. Evidence EV-034.
- Initiated issue [#14](https://github.com/danielforface/aura-lang/issues/14) on a dedicated *stacked* branch `qual/Q01-compat-explicit-typecheck` branched from PR #13; no prior PRs merged. The old `tools/compat/run.sh` called `aura build --mode avm` with `--no-default-features`, but that mode demands `verify_file` and now refuses to lie without a solver. **We must preserve that refusal** and use an explicitly non-proving validation path for syntax/type compatibility.
- New CLI `aura check` (in `aura/src/main.rs`) uses the same parser and semantic checker with range proofs deferred, but does not invoke SMT or represent unproved code as verified; stdout explicitly states `formal verification NOT RUN`. `tools/compat/run.sh` now calls `aura check`. Added `aura/tests/compat_check.rs` for well-typed false `assert` accepted as typechecked-but-NOT-proved, invalid syntax rejected, and AVM build still failing closed without Z3. Added `.github/workflows/q01-compat-check.yml` to execute tests, full fixture suite and solver-feature compile check.
- **New branch CI: NOT YET OBSERVED at implementation commit;** cannot claim the compatibility bug was dynamically reproduced or fixed until actual CLI/workflow results. Next: open stacked draft PR against `qual/Q01-003-004-native-contract-gate`, inspect CI logs, fix failures iteratively to GREEN, record evidence, update progress/handoff with each change. Never change frozen roadmap or merge automatically.

## 21. PR #15 confirmed open and first CI run started (2026-10-23)

- [Draft PR #15](https://github.com/danielforface/aura-lang/pull/15) opened against `qual/Q01-003-004-native-contract-gate`, referencing [Issue #14](https://github.com/danielforface/aura-lang/issues/14). Initial source SHA `d9ad4ab502203ee66b4edc3b1f28206f649b4dac`; no merges or releases performed.
- Dedicated [CI run 39956263061](https://github.com/danielforface/aura-lang/actions/runs/39956263061) was still **IN_PROGRESS at installing native dependencies** at last inspection, not yet green nor red. This documentation-only commit will schedule a new run; inspect both actual outcomes, do not rely on status in this note. The program's permanent handoff/progress documents were updated together, frozen master unchanged.
- **Next safe action:** read current PR #15/branch and CI logs (notably `aura/tests/compat_check.rs`, `tools/compat/run.sh`, solver-feature cargo check). If a compile or fixture error occurs, correct it and repeat to GREEN, recording real evidence. If GREEN, mark issue #14 IN_REVIEW pending maintainer review, update EVIDENCE_LOG with commands/runs and this handoff+PROGRESS in same commit. PR #9/#11/#13/#15 remain stacked/draft. No autonomous merge.

## 22. Issue #14 compatibility gate GREEN — successful scoped iteration (2026-10-23)

- **Actual GitHub Actions successful:** [run 39956263061](https://github.com/danielforface/aura-lang/actions/runs/39956263061), code SHA `d9ad4ab502203ee66b4edc3b1f28206f649b4dac`. CLI integration suite `cargo test -p aura --locked --no-default-features --test compat_check -- --nocapture`: **3 passed, 0 failed**. Test cases: semantically valid but unprovable `assert` can be parsed/typechecked without a proof claim; invalid syntax rejected; `build --mode avm` without solver *remains fail-closed*.
- `bash tools/compat/run.sh` **PASSED** on all three existing fixtures `basic_math.aura`, `smoke.aura`, `vector_basic.aura`. Each `aura check` stdout explicitly said **formal verification NOT RUN**. `cargo check -p aura --locked --no-default-features --features z3` also completed successfully. GitHub logged all three step conclusions success. Evidence [EV-037](EVIDENCE_LOG.md). No unsupported claim about AVM runtime execution or semantic parity.
- [Draft PR #15](https://github.com/danielforface/aura-lang/pull/15) is now **IN_REVIEW** based on scoped GREEN code-CI; the stack remains [#9](https://github.com/danielforface/aura-lang/pull/9) → [#11](https://github.com/danielforface/aura-lang/pull/11) → [#13](https://github.com/danielforface/aura-lang/pull/13) → #15. **All four PRs unmerged**; no permission to merge automatically. Q01 stage as a whole is still IN_PROGRESS (implicit obligations/cache and full proof statuses not qualified).
- **Next safe action in a new chat:** re-fetch actual GitHub branches/PR/CI and this handoff, review dependency-ordered PRs, request maintainer approval before merges. If continuing engineering without merging, create a new narrowly scoped branch from current validated #15 head for the next Q01 gap (explicit proof status taxonomy, cache negative tests). Keep frozen master unmodified; use test-first and update PROGRESS+CONTEXT_HANDOFF+EVIDENCE_LOG with every increment. Never interpret `check` success as proof.

## 23. Typed, versioned verification-report work begun (2026-10-23)

- Prior [PR #15](https://github.com/danielforface/aura-lang/pull/15) is still DRAFT/unmerged. [Workflow run 39960113689](https://github.com/danielforface/aura-lang/actions/runs/39960113689) on its latest earlier SHA `a4dd08b` completed SUCCESS. All upstream PRs #9/#11/#13/#15 stayed unmerged; **maintainer consent is required to merge**, but not to continue proposing focused branches.
- Created [issue #16](https://github.com/danielforface/aura-lang/issues/16) and stacked branch `qual/Q01-006-versioned-proof-report` from PR #15. The original report `schema=aura.trusted-core.v1` only had independent boolean `ok`/error; new bounded v2 serialized `status` enum is `proved|not_run|invalid_input|failed` with `reason_code`, deriving `ok` exclusively from typed result, and rejecting inconsistent proof evidence. Adds `docs/qualification/VERIFY_REPORT_STATUS_V2.md`, real-CLI Z3/no-Z3 tests and Actions workflow. DOES NOT categorize solver timeouts/unknown/cancellation automatically; further Q01-005 work remains.
- **At code commit: new CI NOT YET OBSERVED.** The next chat or this one must fetch new issue #16 Draft PR/head CI logs, fix any Rust compilation, report mapping or fixture failures until GREEN; never claim success before logs. Continue updating PROGRESS+this handoff and append-only evidence; never modify frozen `MASTER_ROADMAP_V1.md`. Do not merge without user authorization.

## 24. Q01-006 versioned report v2 scoped CI acceptance (2026-10-23)

- [Draft PR #17](https://github.com/danielforface/aura-lang/pull/17) OPEN/DRAFT, targeted at PR #15 branch; initial code SHA `8929561da7ef10ed65453145ff4542bf42ce2bb4`. Existing stacked #9/#11/#13/#15 are also Draft/unmerged; `main` still at prior baseline at this observation. No action required from user **to continue engineering**; to **merge into main**, maintainer must explicitly authorize the entire dependency-ordered stack after review.
- [Actual GitHub Actions run 39963769042](https://github.com/danielforface/aura-lang/actions/runs/39963769042) COMPLETED SUCCESS at code SHA `8929561da`, on Ubuntu. `no-z3-report` job: `cargo test -p aura --locked --no-default-features --test verify_report_status -- --nocapture` **2 passed / 0 failed**; existing `verify_fail_closed` **2 passed / 0 failed**. `z3-report` job: `cargo test -p aura --locked --no-default-features --features z3 --test verify_report_status -- --nocapture` **2 passed / 0 failed**. Logs explicitly show every test `... ok`. EV-039 archives exact outcomes.
- Implemented `schema=aura.trusted-core.v2`, typed `status` `proved|not_run|invalid_input|failed`, `reason_code`, derived legacy `ok`, and report-writer invariant ensuring proof metadata exists iff `proved`; unsupported solver error kinds remain undifferentiated intentionally. Source docs `VERIFY_REPORT_STATUS_V2.md` describe v2 as a schema change with limited scope. This is **NOT** a complete AURA-Q01-005/006 implementation: `unknown/timeout/cancelled/unsupported/internal-error` and distinct typed process exit codes still need work; Q01 gate stays IN_PROGRESS.
- **Next safe action:** live-check PR #17 and parent PRs/CI; request maintainer review before any merge. For further engineering without a merge, open a new narrow branch based on current #17 head for structured solver-error classification/exit-code cases, or proof-cache adversarial regressions (Q04-004/Q08-005). Attach tests, CI and evidence; update PROGRESS + CONTEXT_HANDOFF with each commit. Never edit frozen MASTER_ROADMAP_V1.md or claim all verification soundness.

## 25. Structural Unknown continuation (2026-10-23)

- All prior PRs #9/#11/#13/#15/#17 are DRAFT/unmerged; #17 [latest CI 39965574132](https://github.com/danielforface/aura-lang/actions/runs/39965574132) SUCCESS. User need not act to continue engineering; explicit approval required for merging in dependency order.
- New [Issue #18](https://github.com/danielforface/aura-lang/issues/18) on `qual/Q01-005-typed-solver-unknown`, based on #17. Replaced text-only verifier error classification with typed `VerifyFailureKind::{Failed,Unknown}`; tagged known Z3 Unknown returns in solver, verifier and geometry. Adjusted Rust constructors in CLI/LSP, and converted typed Unknown to report-v2 status/reason. Added deterministic unit tests preventing text-based false classification, plus workflow for both solver feature configurations. **No new CI results observed at this commit.**
- Remaining: timeout/cancellation/unsupported/internal errors, process exit taxonomy, real forced Z3 Unknown reproducibility and full soundness. New [SOLVER_FAILURE_CLASSIFICATION.md](SOLVER_FAILURE_CLASSIFICATION.md) explains scope. Next action: inspect new Draft PR and GitHub Actions, fix compile/runtime test failures, update PROGRESS+handoff+EVIDENCE_LOG together, keep frozen plan unchanged and do not merge automatically.

## 26. Structured SMT Unknown — scoped CI GREEN (2026-10-24)

- Actual [Actions run 39973715747](https://github.com/danielforface/aura-lang/actions/runs/39973715747) at `b27c6cd59e5854e9b0cdce11a8ff5c30bbe41109` finished SUCCESS: **Z3 job:** `typed_failure_tests` 2 passed / 0 failed, real-Z3 report CLI 2 passed / 0 failed, `cargo check` verifier+LSP passed. **No-Z3 job:** affected workspace cargo checks passed and both CLI test suites passed, 2/2 each. This validates the typed routing and previously tested paths, NOT deterministic Z3-Unknown end-to-end or full proof taxonomy. EV-041 appended. PR #19 remains Draft/unmerged with all ancestors.
- Next safe action: inspect latest GitHub CI and branch head (docs commits schedule new run), proceed to bounded next Q01-005/006 improvement with explicit typed failure semantics and adversarial tests. A new chat should use live PR #19 and `docs/qualification/PROGRESS.md` + this handoff rather than old journal steps. Never edit frozen `MASTER_ROADMAP_V1.md`; do not merge without user approval.

## 27. Report output durability, test-first increment (2026-10-24)

- Issue [#20](https://github.com/danielforface/aura-lang/issues/20) opened; branch `qual/Q01-002-report-durability` based on parent #19. Source-review defect: `verify_file_with_report` reads/augments input before touching report destination, leaving a stale old `ok:true` JSON on early read failure, and discards report-write errors on parse/sema/solver paths; source/report same path may corrupt source. These are SOURCE REVIEW, **NOT reproduced by a new CI yet**.
- Test-only commit adds `aura/tests/verify_report_durability.rs` four real CLI cases and `.github/workflows/q01-report-durability.yml`; no production fix. Expected 3 RED and 1 GREEN on source baseline, but do not claim until real job output. Keep existing valid status behavior and no-Z3 guard.
- **Next safe action:** fetch current branch and new Draft PR, obtain test-first CI result, ensure errors are semantic rather than harness mistakes. Patch report destination preflight and early-error emission, preserve source if `--report` aliases input. Repeat to GREEN and archive exact CI. Later atomic crash-safe output publication, concurrent symlink races and multi-target paths remain further hardening. Update PROGRESS+CONTEXT_HANDOFF+EVIDENCE_LOG in same commit, master frozen, no auto merges.
