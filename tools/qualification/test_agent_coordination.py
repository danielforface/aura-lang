#!/usr/bin/env python3
"""Cheap self-contained structural test: agent ownership, per-phase docs and shared policy links."""
from pathlib import Path
import json
ROOT = Path(__file__).resolve().parents[2]
base = ROOT / "docs/qualification"
data = json.loads((base / "AGENT_OWNERSHIP.json").read_text(encoding="utf-8"))
assert data["schema_version"] == 1
assert set(data["agents"]) == {"A", "B", "C"}
for p in data["policy_documents"]:
    assert (ROOT / p).is_file(), p
for lane, spec in data["agents"].items():
    assert (ROOT / spec["lane_order"]).is_file(), lane
prefixes = [(lane, p) for lane, spec in data["agents"].items() for p in spec["exclusive_prefixes"]]
assert len(prefixes) == len(set(p for _, p in prefixes))
assert all(not (a != b and (p.startswith(q) or q.startswith(p)))
           for a, p in prefixes for b, q in prefixes)
assert all(p.endswith("/") and not p.startswith("/") and ".." not in p for _, p in prefixes)
shared = set(data["integration_only_exact"]) | set(data["historical_frozen"])
assert len(shared) == len(data["integration_only_exact"]) + len(data["historical_frozen"])
assert all(not p.startswith(root) for _, root in prefixes for p in shared)
assert set(data["dependency_classes"]) == {"D0","D1","D2","D3","D4","D5"}
read = lambda p: (ROOT / p).read_text(encoding="utf-8")
root_text = read("AGENTS.md")
life = read("docs/qualification/PHASE_LIFECYCLE_V1.md")
contract = read("docs/qualification/CROSS_AGENT_COORDINATION_V1.md")
prompt = read("docs/qualification/CONTINUE_AGENT_PROMPT.md")
for text in (root_text, life, contract, prompt):
    assert "D0" in text and "D5" in text
    assert "ממשיכים" in text
assert "mergeable" in contract and "merge-base" in contract
assert all(("Stage "+str(i)) in life for i in range(5))
assert "AGENT / LANE" in life and "BEFORE" in life
assert "PR #95" in prompt and "Issue #59" in prompt or "issues/59" in prompt
print("Agent coordination PASS: lane ownership nonoverlap; policy anchors, D0-D5 and phase re-entry present")

# Agent X is a coordinator and serialized integration-doc steward, NOT an
# additional production writer inside A/B/C's exclusive paths.
x = data["integration_agent"]
assert x["id"] == "X" and x["standing_issue"] == 110
assert x["owns_code_of_other_agents"] is False
assert x["may_merge_without_maintainer_approval"] is False
assert x["requires_evidence_for_correction"] is True
assert x["default_if_on_track"] == "GREEN_SUPPORT_AND_DO_NOT_DISTURB"
assert (ROOT / x["charter"]).is_file()
assert (ROOT / x["startup_prompt"]).is_file()
xc = read(x["charter"])
xs = read(x["startup_prompt"])
for phrase in ("GREEN", "AMBER", "RED", "CROSS-LANE", "JUSTIFIED_VARIANCE", "ממשיכים", "NO_ACTION_NEEDED"):
    assert phrase in xc, phrase
assert all(token in xs for token in ("X-001", "AGENT", "ממשיכים", "issues/110", "issues/109"))
print("Agent X leadership PASS: standing mandate, evidence-gated intervention, no autonomous merge")
