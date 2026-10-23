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
STATUS = "docs/qualification/PROGRESS.md"
HANDOFF = "docs/qualification/CONTEXT_HANDOFF.md"
ALLOWED_STATES = {"NOT_STARTED", "IN_PROGRESS", "BLOCKED", "IN_REVIEW", "VERIFIED", "RELEASED"}

def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=True, check=check)

def fail(message: str) -> None:
    raise SystemExit(f"Qualification governance FAILED: {message}")

def main() -> None:
    plan = (ROOT / PLAN).read_text(encoding="utf-8")
    progress = (ROOT / STATUS).read_text(encoding="utf-8")
    handoff = (ROOT / HANDOFF).read_text(encoding="utf-8")
    if "AURA-QUAL-PLAN-001" not in plan or "Immutable contract" not in plan:
        fail("canonical plan header/freeze policy missing")
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
    if len(gate_rows) != 12 or set(g for g, _ in gate_rows) != {f"Q{i:02d}" for i in range(12)}:
        fail("PROGRESS.md must list exactly Q00..Q11 with valid states")
    if len(sys.argv) > 1:
        base = sys.argv[1]
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
    print(f"Qualification governance OK: {len(task_rows)} canonical tasks, {len(gate_rows)} tracked gates.")

if __name__ == "__main__":
    main()
