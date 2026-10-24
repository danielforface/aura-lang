#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

TARGET="tests/compat/smoke.aura"
BASELINE="tools/trusted-core/baseline/compat-smoke.trusted-core.json"
OUTDIR="build/trusted-core-ci"
OUT="$OUTDIR/compat-smoke.trusted-core.json"
AUDIT="$OUTDIR/compat-smoke.audit-projection.json"

mkdir -p "$OUTDIR"

# No Z3: verification must FAIL CLOSED, yet a truthful source trust-surface
# report can still be generated for audit. Never interpret this as proof.
set +e
cargo run -p aura --locked --no-default-features -- verify --report "$OUT" "$TARGET"
verify_rc=$?
set -e
if [[ "$verify_rc" -eq 0 ]]; then
  echo "SECURITY REGRESSION: no-Z3 verification unexpectedly reported success" >&2
  exit 1
fi
if [[ ! -f "$OUT" ]]; then
  echo "verification report missing despite requested audit output" >&2
  exit 1
fi

python3 - "$OUT" "$AUDIT" <<'PY'
import json, sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text())
assert report.get("schema") == "aura.trusted-core.v2", "unexpected report schema"
assert report.get("status") == "not_run", "not a no-Z3 audit, check parser/CLI failure"
assert report.get("ok") is False, "a no-Z3 audit is NEVER proof"
assert "verify" not in report, "unproven result cannot carry proof evidence"
trusted = report.get("trusted")
assert isinstance(trusted, dict), "trusted source metadata missing"
keys = (
    "assumption_origins",
    "explicit_assumes",
    "extern_cells_trusted",
    "extern_cells_untrusted",
    "preconditions",
    "unsafe_blocks",
)
for key in keys:
    assert isinstance(trusted.get(key), list), "missing trust category: " + key
audit = {
    "schema": report["schema"],
    "status": report["status"],
    "ok": report["ok"],
    "trusted": {k: trusted[k] for k in keys},
}
Path(sys.argv[2]).write_text(json.dumps(audit, sort_keys=True, indent=2) + "\n")
PY

if [[ ! -f "$BASELINE" ]]; then
  echo "audited baseline missing: $BASELINE" >&2
  echo "Review and copy '$AUDIT' after confirming trust spans." >&2
  exit 2
fi
diff -u "$BASELINE" "$AUDIT" > "$OUTDIR/trusted-core.diff" || {
  echo "Trusted core projection changed. Review source spans and update baseline explicitly." >&2
  cat "$OUTDIR/trusted-core.diff" >&2
  exit 1
}
echo "trusted-core v2 source audit: OK (proof status remains NOT_RUN)"
