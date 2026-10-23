# Q01 interim native assurance policy (experimental; NOT a full-language guarantee)

**Status:** proposed in [Draft PR #13](https://github.com/danielforface/aura-lang/pull/13), pending CI/review.  
**Program tasks:** AURA-Q01-003, AURA-Q01-004; [issue #12](https://github.com/danielforface/aura-lang/issues/12).  
**Authority:** this is a **mutable implementation policy note**, not the frozen [MASTER_ROADMAP_V1.md](MASTER_ROADMAP_V1.md).

## Scope

For the current experimental CLI implementation, native C/WASM compilation **must not silently strip explicit** `requires`, `ensures`, `assert`, or `assume` constructs. `aura-core/src/lower.rs` currently lowers these statement kinds to no-ops, so the compiler applies a conservative, fail-closed *pre-codegen* check:

| Path | Explicit source proof constructs | No explicit source constructs |
|---|---|---|
| `aura build --backend c/wasm --profile dev` | With Z3 feature: attempt proof before emission; without Z3: explicit build error | Type-check/lower/emit still allowed; **not** formally verified |
| `aura build --backend c/wasm --profile release` | Same proof-or-error policy | Type-check/lower/emit still allowed; **not** formally verified |
| `aura build --backend c/wasm --profile verify` | Must verify; no Z3 is error; duplicate verification currently possible | Explicit verify gate still runs |
| `aura run --mode llvm --backend c` | Reaches direct `build()`, subject to the same interim native guard | Native runtime path may execute without a formal proof; not stable |
| `aura build --backend llvm` | Existing independent LLVM+Z3 feature gate; not covered by this C/WASM table | Existing experimental LLVM policy |
| `aura run --mode avm/hybrid` | Separate AVM runtime/interpreter contract; **not** qualified by this patch | Same |
| `aura verify` | Requires actual Z3 verification, or nonzero error when feature absent (Q01-001/002) | Same |

This behavior is an **interim assurance gate**, *not* a completed normative semantics for all profiles. In particular, `assume` represents an accepted assumption rather than an independently proved fact; trusted boundaries must be reported distinctly.

## Cache safety rule

For source with explicit proof constructs, `build_cached` must **skip cached artifact restoration** and enter a fresh codegen/assurance path. C-header bridging also bypasses cache until header contents, generated shims, transitive inputs, toolchain identity, proof inputs and trust annotations are included in a sound cache key (AURA-Q08-005). Correctness takes precedence over cache speed.

The syntax-aware scan uses Aura's AST and the recursive `report::analyze_verify_surface` helper. No regex is used as the guarantee. If the compiler discovers a construct it cannot prove or safely preserve, the safe fallback is rejection rather than silently removing the contract.

## Known exclusions and open proof obligations

- A source `assert` proved under the current verifier's model **does not** by itself certify correctness of C output; translation-validation and SMT/native semantic equivalence belong to Q02–Q04.
- Implicit obligations (e.g., constrained range proofs not represented as these statement forms) still require separate tracking and gating.
- Contracts introduced indirectly by macro expansion, extern shims, stdlib imports and generated code require complete frontend semantics and trust accounting before stable status. Recursive syntax scanning is conservative, not a completeness proof.
- `requires`/`ensures` total functional semantics, runtime checks, error/cancellation behavior, `unsafe` assumptions and full proof-result taxonomy remain qualification work.
- No binary artifacts are released, and neither this note nor a passing test grants a stable language-wide claim.

## Qualification tests

`cargo test -p aura --locked --no-default-features --test native_contract_modes -- --nocapture` on Ubuntu (CI) covers false assertions in dev/release, a nested assertion, and a positive unannotated dev build. Record real RED and GREEN run IDs in `EVIDENCE_LOG.md`; the code change is unqualified until observed. Test with Z3-enabled proof success/failure and additional platforms in later bounded increments.

**Release discipline:** keep this PR draft and stacked until reviewed; do not merge autonomously. Update `PROGRESS.md` and `CONTEXT_HANDOFF.md` whenever work advances.

## Source-check compatibility note (issue #14, pending CI)

A separate CLI command `aura check` is proposed for compatibility and IDE-like validation: parse/typecheck **only**, `formal verification NOT RUN`. `build --mode avm` is intentionally still a formal-verification gate and fails with missing Z3; the older `tools/compat/run.sh` is adapted to use `check` rather than relabel typecheck as proof. This is under review and has its own test/CI evidence gate; it must not be confused with native output assurance.
