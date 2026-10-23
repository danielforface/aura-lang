#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIXTURES="$REPO_ROOT/tests/compat"

if [ ! -d "$FIXTURES" ]; then
  echo "Missing compat fixtures at $FIXTURES" >&2
  exit 1
fi

echo "== Aura compat suite =="

cd "$REPO_ROOT"
shopt -s nullglob
FILES=("$FIXTURES"/*.aura)
if [ ${#FILES[@]} -eq 0 ]; then
  echo "No compat fixtures (*.aura) found in $FIXTURES" >&2
  exit 1
fi

for f in "${FILES[@]}"; do
  echo "-- syntax/type check (not a formal proof): $f"
  cargo run -q -p aura --locked --no-default-features -- check "$f"
done

echo "OK"
