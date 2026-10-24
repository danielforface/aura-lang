# Trusted Core Report (Audit Policy)

Aura can emit a machine-readable report that summarizes the **trusted surface** of a program/workspace.
This is intended as an audit artifact: it makes it obvious when unsafe/trusted regions expand.

## Generate a report

From the repo root:

- Single file:
  - `cargo run -p aura -- verify --report build/trusted-core.json main.aura`

- Workspace (multiple targets):
  - `cargo run -p aura -- verify --report build/trusted-core reports/`

The JSON schema is currently `aura.trusted-core.v2`. A report is **not** proof merely because the JSON file exists: require `status: "proved"`, `ok: true`, a zero process exit code and actual proof evidence. Without Z3 the command exits nonzero with `status: "not_run"`, `ok: false` even when source-level trust metadata can be produced.

## What it contains

- `trusted.unsafe_blocks`: source spans of `unsafe:` blocks, including nested blocks in a `cell`
- `trusted.extern_cells_trusted`: spans of `extern cell ... trusted`
- `trusted.extern_cells_untrusted`: spans of `extern cell ...` (not marked trusted)
- `trusted.explicit_assumes`: explicit `assume` statements (trusted premises, **not** independently proved)
- `trusted.preconditions`: `requires` contracts (obligations on callers, not arbitrary user assumptions)
- `trusted.assumption_origins`: typed `{kind, span, cell?}` records for each `assume` and `requires` with source positions
- With Z3 enabled, `verify` also summarizes proof notes (counts by plugin/kind).

## CI audit policy

CI runs [tools/trusted-core/check.sh](tools/trusted-core/check.sh) which:
1. Regenerates the report for `tests/compat/smoke.aura`
2. Diffs it against the checked-in baseline at `tools/trusted-core/baseline/compat-smoke.trusted-core.json`

If the diff is non-empty, CI fails. Updating the baseline is an explicit review signal that the trusted surface changed.

## Important compatibility note

The original v1 baseline under `tools/trusted-core/baseline/` is historical and must be regenerated/reviewed using v2's fail-closed semantics. The checker must never accept a no-Z3 `ok:true`; it may audit a `not_run` source trust-surface report, but must not mistake that audit for proof. The Q01-007 qualification task records its own CI evidence separately.
