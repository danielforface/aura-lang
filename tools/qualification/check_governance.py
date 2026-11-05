#!/usr/bin/env python3
"""PR gate: freeze canonical plan and require live handoff/progress on changes.

Run: python3 tools/qualification/check_governance.py [BASE_SHA]
When BASE_SHA omitted, performs local integrity checks without Git diff rules.
"""
from __future__ import annotations
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
PLAN = "docs/qualification/MASTER_ROADMAP_V1.md"
PLAN_V2 = "docs/qualification/MASTER_ROADMAP_V2.md"
PLAN_V3 = "docs/qualification/MASTER_ROADMAP_V3.md"
STATUS = "docs/qualification/PROGRESS.md"
HANDOFF = "docs/qualification/CONTEXT_HANDOFF.md"
ALLOWED_STATES = {"NOT_STARTED", "IN_PROGRESS", "BLOCKED", "IN_REVIEW", "VERIFIED", "RELEASED"}

def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=True, check=check)

def fail(message: str) -> None:
    raise SystemExit(f"Qualification governance FAILED: {message}")

def main() -> None:
    v1 = (ROOT / PLAN).read_text(encoding="utf-8")
    v2path = ROOT / PLAN_V2
    has_v2 = v2path.exists()
    v2 = v2path.read_text(encoding="utf-8") if has_v2 else None
    v3path = ROOT / PLAN_V3
    has_v3 = v3path.exists()
    v3 = v3path.read_text(encoding="utf-8") if has_v3 else None
    plan = v3 if v3 is not None else (v2 if v2 is not None else v1)
    progress = (ROOT / STATUS).read_text(encoding="utf-8")
    handoff = (ROOT / HANDOFF).read_text(encoding="utf-8")
    if "AURA-QUAL-PLAN-001" not in v1 or "Immutable contract" not in v1:
        fail("historical V1 canonical plan header/freeze policy missing")
    if has_v2:
        if "AURA-QUAL-PLAN-002" not in v2 or "PROPOSED SUPERSESSION" not in v2:
            fail("V2 proposal metadata / supersession policy is missing")
        v1rows = re.findall(r"^\| AURA-Q\d{2}-\d{3} \|.*$", v1, re.MULTILINE)
        v2rows = re.findall(r"^\| AURA-Q\d{2}-\d{3} \|.*$", v2, re.MULTILINE)
        if not set(v1rows).issubset(set(v2rows)):
            fail("V2 must preserve every original V1 acceptance row byte-for-byte")
        for n in range(12, 19):
            if not re.search(rf"^\| AURA-Q{n}-\d{{3}} \|", v2, re.MULTILINE):
                fail(f"V2 missing Q{n} gate task entries")
    if has_v3:
        if not has_v2:
            fail("V3 requires the preserved V2 parent")
        if "AURA-QUAL-PLAN-003" not in v3 or "PROPOSED TECHNICAL PLAN" not in v3:
            fail("V3 technical proposal metadata missing")
        v2rows = re.findall(r"^\| AURA-Q\d{2}-\d{3} \|.*$", v2, re.MULTILINE)
        v3rows = re.findall(r"^\| AURA-Q\d{2}-\d{3} \|.*$", v3, re.MULTILINE)
        if len(v2rows) != 190 or not set(v2rows).issubset(set(v3rows)):
            fail("V3 must preserve 190 V2 acceptance rows exactly")
        if len(v3rows) != 474 or len(v3rows) != len(set(v3rows)):
            fail("V3 requires 474 unique task rows: 190 inherited plus 284 quantum")
        for n in range(19, 39):
            tasks = [row for row in v3rows if row.startswith(f"| AURA-Q{n}-")]
            if not tasks:
                fail(f"V3 missing Q{n} acceptance tasks")
        for n in range(16, 36):
            if not re.search(rf"^\| R{n} —", v3, re.MULTILINE):
                fail(f"V3 missing quantum release train R{n}")
    if not re.search(r"\*\*Active gate:\*\*.*Q\d{2}", progress):
        fail("PROGRESS.md must state Active gate")
    if "Next safe action" not in handoff:
        fail("CONTEXT_HANDOFF.md must state Next safe action")
    ids = set(re.findall(r"AURA-Q\d{2}-\d{3}", plan))
    if len(ids) != len(set(ids)) or len(ids) < 70:
        fail("plan task list is unexpectedly small")
    # Validate each task row defines a unique stable identifier.
    task_rows = re.findall(r"^\| (AURA-Q\d{2}-\d{3}) \|", plan, re.MULTILINE)
    if len(task_rows) != len(set(task_rows)) or len(task_rows) < 70:
        fail("canonical task rows are duplicated or missing")
    for f, content in ((STATUS, progress), (HANDOFF, handoff)):
        unknown = sorted(set(re.findall(r"AURA-Q\d{2}-\d{3}", content)) - ids)
        if unknown:
            fail(f"{f} references undefined task IDs: {', '.join(unknown)}")
    gate_rows = re.findall(r"^\| (Q\d{2}) \| .*? \| (NOT_STARTED|IN_PROGRESS|BLOCKED|IN_REVIEW|VERIFIED|RELEASED) \|", progress, re.MULTILINE)
    gate_count = 39 if has_v3 else (19 if has_v2 else 12)
    if len(gate_rows) != gate_count or set(g for g, _ in gate_rows) != {f"Q{i:02d}" for i in range(gate_count)}:
        fail(f"PROGRESS.md must list exactly Q00..Q{gate_count-1:02d} with valid states")
    if len(sys.argv) > 1:
        base = sys.argv[1]
        if has_v3:
            existed_v3 = git("cat-file", "-e", f"{base}:{PLAN_V3}", check=False).returncode == 0
            if existed_v3 and git("show", f"{base}:{PLAN_V3}").stdout != v3:
                fail("frozen V3 changed in place; publish MASTER_ROADMAP_V4.md instead")
        if has_v2:
            existed_v2 = git("cat-file", "-e", f"{base}:{PLAN_V2}", check=False).returncode == 0
            if existed_v2 and git("show", f"{base}:{PLAN_V2}").stdout != v2:
                fail("frozen V2 changed in place; publish MASTER_ROADMAP_V3.md instead")
        existed = git("cat-file", "-e", f"{base}:{PLAN}", check=False).returncode == 0
        if existed:
            frozen = git("show", f"{base}:{PLAN}").stdout
            if frozen != v1:
                fail("frozen historical V1 roadmap changed")
            names = set(git("diff", "--name-only", f"{base}...HEAD").stdout.splitlines())
            if names and (STATUS not in names or HANDOFF not in names):
                fail("every substantive PR must change both PROGRESS.md and CONTEXT_HANDOFF.md")
        else:
            print("First-time bootstrap: canonical plan absent from base; freeze starts after merge.")
    print(f"Qualification governance OK: {len(task_rows)} canonical tasks, {len(gate_rows)} tracked gates, V2={has_v2}, V3={has_v3}.")

if __name__ == "__main__":
    main()
