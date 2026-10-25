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
broken_count = before.count(broken)
correct_count = before.count(correct)
if broken_count == 1 and correct_count == 0:
    # First clean build: apply exactly one known upstream member-reference fix.
    vendored.write_text(before.replace(broken, correct), encoding="utf-8")
    print(f"Patched one pinned z3-sys 0.8.1 column_info.h reference: {vendored}")
elif broken_count == 0 and correct_count == 1:
    # Rust cache may restore the same source after a previous CI preflight.
    # Re-accept only the exact expected postimage, never arbitrary drift.
    print(f"Confirmed exact prior pinned Z3 compatibility correction: {vendored}")
else:
    raise SystemExit(
        f"Unexpected vendored Z3 source drift in {vendored}: "
        f"broken occurrences={broken_count}, expected postimage occurrences={correct_count}"
    )
verified = vendored.read_text(encoding="utf-8")
if verified.count(broken) != 0 or verified.count(correct) != 1:
    raise SystemExit(f"Vendored Z3 source validation failed after preflight: {vendored}")
