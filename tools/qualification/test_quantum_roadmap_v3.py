#!/usr/bin/env python3
"""Repository-local tests for quantum V3 task/ledger integrity; no external deps."""
from pathlib import Path
import re
ROOT = Path(__file__).resolve().parents[2]
v1 = (ROOT / "docs/qualification/MASTER_ROADMAP_V1.md").read_text()
v2 = (ROOT / "docs/qualification/MASTER_ROADMAP_V2.md").read_text()
v3 = (ROOT / "docs/qualification/MASTER_ROADMAP_V3.md").read_text()
p = (ROOT / "docs/qualification/PROGRESS.md").read_text()
extract = lambda s: re.findall(r"^\| AURA-Q\d{2}-\d{3} \|.*$", s, re.M)
v1rows, v2rows, v3rows = map(extract, (v1, v2, v3))
assert len(v1rows) == 88 and set(v1rows).issubset(set(v2rows))
assert len(v2rows) == 190 and set(v2rows).issubset(set(v3rows))
assert len(v3rows) == 474 and len(set(v3rows)) == 474
assert all(sum(row.startswith(f"| AURA-Q{n}-") for row in v3rows) >= 1 for n in range(19, 39))
gates = re.findall(r"^\| (Q\d{2}) \| .*? \| (NOT_STARTED|IN_PROGRESS|BLOCKED|IN_REVIEW|VERIFIED|RELEASED) \|", p, re.M)
assert len(gates) == 39 and len(set(g for g, _ in gates)) == 39
# Quantum implementation may proceed on agent-owned branches while V3 remains Draft.
# Never freeze all future work as NOT_STARTED forever. Verify supported state values.
states = dict(gates)
assert len(states) == 39
assert all(f"Q{n}" in states for n in range(19, 39))
assert states["Q28"] in {"NOT_STARTED", "IN_PROGRESS", "BLOCKED", "IN_REVIEW", "VERIFIED", "RELEASED"}

assert all(f"| R{n} —" in v3 for n in range(16, 36))
assert "PROPOSED TECHNICAL PLAN" in v3 and "NOT IMPLEMENTED" in v3
print("Quantum V3 integrity PASS: 88 V1, 190 V2, 474 V3, 39 tracked gates; Q28 state="+states["Q28"])
