#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

TARGET="tests/compat/smoke.aura"
BASELINE="tools/trusted-core/baseline/compat-smoke.trusted-core.json"
OUTDIR="build/trusted-core-ci"
OUT="$OUTDIR/compat-smoke.trusted-core.json"

mkdir -p "$OUTDIR"

# Generate a trusted-core report (works with or without Z3; with Z3 it includes proof summaries).
cargo run -p aura -- verify --report "$OUT" "$TARGET"

if [[ ! -f "$BASELINE" ]]; then
  echo "baseline missing: $BASELINE" >&2
  echo "To create it locally: cp '$OUT' '$BASELINE'" >&2
  exit 2
fi

# Fail if trusted surface / evidence changes without updating the baseline.
diff -u "$BASELINE" "$OUT" > "$OUTDIR/trusted-core.diff" || {
  echo "Trusted core report differs from baseline." >&2
  echo "See: $OUTDIR/trusted-core.diff" >&2
  exit 1
}

echo "trusted-core: OK" 
