# Verification report schema v2 — bounded Q01 status contract

**Status:** scoped tests GREEN in [GitHub Actions run 39963769042](https://github.com/danielforface/aura-lang/actions/runs/39963769042); [Draft PR #17](https://github.com/danielforface/aura-lang/pull/17) OPEN/IN_REVIEW, unmerged. This is not language-wide qualification. This note is explanatory and mutable; the canonical MASTER_ROADMAP_V1.md is unchanged.

`aura verify --report <path>` produces JSON carrying a version marker `schema: "aura.trusted-core.v2"`, a typed `status`, backward-compatible `ok`, optional machine-readable `reason_code`, optional prose `error`, `trusted` surface and optional `verify` evidence.

| status | Meaning | expected ok | reason_code (current) | verify |
|---|---|---:|---|---|
| `proved` | Solver returned success for this program/model and proof metadata was gathered | true | omitted | object |
| `not_run` | Verification never ran, e.g. binary built without solver | false | `verifier_unavailable` | absent |
| `invalid_input` | Parsing/typechecking failed before proof | false | `parse_error` or `type_error` | absent |
| `failed` | Solver/verification step returned an error or did not establish proof | false | `verification_failed` | absent |

**Important:** `failed` does *not* necessarily mean `counterexample`. This initial schema intentionally does not guess whether a failure is `unknown`, `timeout`, `cancelled`, `unsupported` or a solver internal error. Those require typed verifier exceptions/results and separate adversarial tests (AURA-Q01-005/006). User-facing exit-code taxonomy beyond 0/nonzero is also out of scope.

In Rust, the writer derives `ok` from the typed enum, rejects success without proof metadata and rejects proof metadata for any non-`proved` result. The v1 boolean remains for compatibility but consumers requiring semantics should check `schema` and `status` together. This v2 JSON is a schema change that consumers must explicitly handle.

**Assurance limitations:** passing tests proves CLI serialization behavior on tested Ubuntu configurations only. Neither reports nor `proved` certify backend translation correctness, C integer semantics, trust assumptions, memory safety or general verifier soundness. See the frozen Q01/Q02/Q04 requirements.

**Test gate:** real binary `verify --report` in builds both with and without `z3`; expected no-solver, parse-error, false assertion and true assertion statuses, exit code and proof metadata. Collect CI links and results before marking IN_REVIEW. Keep this work in a stacked Draft PR; no autonomous merge.

## Addendum — structured Unknown (2026-10-24)

The later [Issue #18 / PR #19](https://github.com/danielforface/aura-lang/pull/19) extends this v2 enum with `unknown` and `reason_code=solver_unknown` *only* for explicitly typed `VerifyFailureKind::Unknown` originating from recognized Z3 `SatResult::Unknown` paths. Scoped [CI run 39973715747](https://github.com/danielforface/aura-lang/actions/runs/39973715747) passed. `failed` remains generic for other errors. Timeout, cancelled, unsupported, internal error and exact exit-code taxonomy remain open; no deterministic forced Z3-Unknown end-to-end test exists yet. This addendum does not retroactively change v2's original baseline review claim or the frozen roadmap.
