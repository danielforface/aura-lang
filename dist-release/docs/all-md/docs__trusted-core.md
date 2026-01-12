# Trusted Core Report (Audit Policy)

Aura can emit a machine-readable report that summarizes the **trusted surface** of a program/workspace.
This is intended as an audit artifact: it makes it obvious when unsafe/trusted regions expand.

## Generate a report

From the repo root:

- Single file:
  - `cargo run -p aura -- verify --report build/trusted-core.json main.aura`

- Workspace (multiple targets):
  - `cargo run -p aura -- verify --report build/trusted-core reports/`

The JSON schema is currently `aura.trusted-core.v1`.

## What it contains

- `trusted.unsafe_blocks`: spans of `unsafe { ... }`
- `trusted.extern_cells_trusted`: spans of `extern cell ... trusted`
- `trusted.extern_cells_untrusted`: spans of `extern cell ...` (not marked trusted)
- With Z3 enabled, `verify` also summarizes proof notes (counts by plugin/kind).

## CI audit policy

CI runs [tools/trusted-core/check.sh](tools/trusted-core/check.sh) which:
1. Regenerates the report for `tests/compat/smoke.aura`
2. Diffs it against the checked-in baseline at `tools/trusted-core/baseline/compat-smoke.trusted-core.json`

If the diff is non-empty, CI fails. Updating the baseline is an explicit review signal that the trusted surface changed.
