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
    plan = v2 if v2 is not None else v1
    progress = (ROOT / STATUS).read_text(encoding="utf-8")
    handoff = (ROOT / HANDOFF).read_text(encoding="utf-8")
    if "AURA-QUAL-PLAN-001" not in v1 or "Immutable contract" not in v1:
        fail("historical V1 canonical plan header/freeze policy missing")
    if has_v2:
        if "AURA-QUAL-PLAN-002" not in v2 or "PROPOSED SUPERSESSION" not in v2:
            fail("V2 proposal metadata / supersession policy is missing")
        v1rows = re.findall(r"^\\| AURA-Q\\d{2}-\\d{3} \\|.*$", v1, re.MULTILINE)
        v2rows = re.findall(r"^\\| AURA-Q\\d{2}-\\d{3} \\|.*$", v2, re.MULTILINE)
        if not set(v1rows).issubset(set(v2rows)):
            fail("V2 must preserve every original V1 acceptance row byte-for-byte")
        for n in range(12, 19):
            if not re.search(rf"^\\| AURA-Q{n}-\\d{{3}} \\|", v2, re.MULTILINE):
                fail(f"V2 missing Q{n} gate task entries")
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
    gate_count = 19 if has_v2 else 12
    if len(gate_rows) != gate_count or set(g for g, _ in gate_rows) != {f"Q{i:02d}" for i in range(gate_count)}:
        fail(f"PROGRESS.md must list exactly Q00..Q{gate_count-1:02d} with valid states")
    if len(sys.argv) > 1:
        base = sys.argv[1]
        if has_v2:
            existed_v2 = git("cat-file", "-e", f"{base}:{PLAN_V2}", check=False).returncode == 0
            if existed_v2 and git("show", f"{base}:{PLAN_V2}").stdout != v2:
                fail("frozen V2 changed in place; publish MASTER_ROADMAP_V3.md instead")
        existed = git("cat-file", "-e", f"{base}:{PLAN}", check=False).returncode == 0
        if existed:
            frozen = git("show", f"{base}:{PLAN}").stdout
            if frozen != plan:
                fail("frozen canonical roadmap changed; add MASTER_ROADMAP_V2.md instead")
            names = set(git("diff", "--name-only", f"{base}...HEAD").stdout.splitlines())
            if names and (STATUS not in names or HANDOFF not in names):
                fail("every substantive PR must change both PROGRESS.md and CONTEXT_HANDOFF.md")
        else:
            print("First-time bootstrap: canonical plan absent from base; freeze starts after merge.")
    print(f"Qualification governance OK: {len(task_rows)} canonical tasks, {len(gate_rows)} tracked gates, V2={has_v2}.")

if __name__ == "__main__":
    main()
