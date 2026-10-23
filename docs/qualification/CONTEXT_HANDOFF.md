# Aura Program — Always-Current Fresh-Chat Handoff

**Read this first in any new conversation.** This is operational continuity, not a frozen spec.  
**Updated at:** 2026-10-23 (Q01 fix proposed; exact CI results still pending)  
**Repository:** https://github.com/danielforface/aura-lang  
**Current working branch:** `qual/Q01-001-002-fail-closed-verify` (stacked upon `program/aura-long-horizon-qualification-v1`; R0 PR #9 remains draft/unmerged)  
**Base:** `main`, initial commit `e8f1c35a4a56adbde983dce38bba59a8c4eee105`  
**Active gate/train:** Q00/R0 under review; Q01/R1 test-first work IN_PROGRESS in a separate stacked feature branch; **release:** none.  
**Scope of this focused branch:** [Draft PR #11](https://github.com/danielforface/aura-lang/pull/11), [issue #10](https://github.com/danielforface/aura-lang/issues/10), tasks AURA-Q01-001 / AURA-Q01-002; no autonomous merge.  
**Draft PR:** https://github.com/danielforface/aura-lang/pull/9 (OPEN, DRAFT, unmerged).  
**Next issue:** https://github.com/danielforface/aura-lang/issues/10 (P0, Q01 fail-closed).  
**Last verified R0 program branch head before starting Q01:** `b2bdf39282e9d6579e5c6f484762c72390e5a57d`; fetch current Q01 branch head/PR and CI each session.  
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
