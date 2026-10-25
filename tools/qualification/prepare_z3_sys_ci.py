#!/usr/bin/env python3
"""CI-only, fail-closed backport for a single vendored Z3 0.8.1 C++ typo.

Do NOT silently accept a different z3-sys release/source layout. This file
patches downloaded build input on the CI runner, not a Git-managed dependency,
and must be retired after a separately qualified solver dependency migration.
"""
import json
from pathlib import Path
import sys

if len(sys.argv) != 2:
    raise SystemExit("usage: prepare_z3_sys_ci.py <cargo-metadata.json>")

metadata = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
packages = [p for p in metadata["packages"] if p["name"] == "z3-sys" and p["version"] == "0.8.1"]
if len(packages) != 1:
    raise SystemExit(f"Expected exactly one pinned z3-sys 0.8.1, found {len(packages)}")

vendored = Path(packages[0]["manifest_path"]).parent / "z3/src/math/lp/column_info.h"
if not vendored.is_file():
    raise SystemExit(f"Expected pinned Z3 vendored source missing: {vendored}")
before = vendored.read_text(encoding="utf-8")
broken = "m_lower_bound == c.m_low_bound"
correct = "m_lower_bound == c.m_lower_bound"
if before.count(broken) != 1:
    raise SystemExit(f"Unexpected vendored Z3 source drift: expected one exact {broken!r} in {vendored}")
vendored.write_text(before.replace(broken, correct), encoding="utf-8")
after = vendored.read_text(encoding="utf-8")
assert broken not in after and after.count(correct) == before.count(correct) + 1
print(f"Patched exactly one known z3-sys 0.8.1 vendored column_info.h reference: {vendored}")
