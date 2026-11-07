#!/usr/bin/env python3
"""Strict Agent C child-PR scope gate for V3. Does not replace roadmap checker."""
from __future__ import annotations
import argparse
import re
import subprocess
import sys
from pathlib import Path

V3="plan/aura-quantum-full-stack-v3"
ALLOWED=("aura-quantum-sim/","docs/quantum/agent-c/")
WORKFLOW=".github/workflows/quantum-reference-sim.yml"
NOTE=re.compile(r"docs/quantum/agent-c/C-\d{3}\.md\Z")
C_BRANCH=re.compile(r"quantum/agent-c-[a-z0-9][a-z0-9-]*\Z")

class PolicyError(Exception):
    pass

def git(repo,*args):
    p=subprocess.run(["git","-C",str(repo),*args],capture_output=True,check=False)
    if p.returncode:
        raise PolicyError("git "+args[0]+": "+p.stderr.decode("utf-8","replace")[:300])
    return p.stdout

def verify(repo,base,head,pr_number,head_ref,base_ref):
    if base_ref!=V3: raise PolicyError("incorrect base ref")
    if pr_number<1: raise PolicyError("invalid PR number")
    git(repo,"rev-parse","--verify",base+"^{commit}")
    git(repo,"rev-parse","--verify",head+"^{commit}")
    git(repo,"merge-base","--is-ancestor",base,head)
    names=[p.decode("utf-8") for p in git(repo,"diff","--name-only","-z","--no-renames",base+"..."+head).split(b"\0") if p]
    if not names: raise PolicyError("empty PR diff")
    c_owned=any(p.startswith(ALLOWED) or p==WORKFLOW for p in names)
    if not c_owned and not head_ref.startswith("quantum/"):
        return "X003_SCOPE_NOT_APPLICABLE paths="+str(len(names))
    if not C_BRANCH.fullmatch(head_ref): raise PolicyError("unclaimed quantum branch "+head_ref)
    bad=[p for p in names if not p.startswith(ALLOWED) and p!=WORKFLOW]
    if bad: raise PolicyError("foreign/shared paths blocked: "+repr(bad[:10]))
    owned=[p for p in names if p.startswith("aura-quantum-sim/")]
    notes=[p for p in names if NOTE.fullmatch(p)]
    if not owned: raise PolicyError("no simulator production code")
    if not notes: raise PolicyError("no changed C-### evidence note")
    for path in names:
        info=git(repo,"ls-tree","-z",head,"--",path)
        if info and not (info.startswith(b"100644 blob ") or info.startswith(b"100755 blob ")):
            raise PolicyError("symlink/submodule/foreign object: "+path)
    if not any(git(repo,"ls-tree",head,"--",p) for p in owned):
        raise PolicyError("simulator source deleted in full")
    ok=False
    for path in notes:
        if not git(repo,"ls-tree",head,"--",path):
            continue
        txt=git(repo,"show",head+":"+path).decode("utf-8")
        if re.search(rf"(?<!\d)#{pr_number}(?!\d)",txt) and re.search(r"(?i)\bissue\b[^\n]{0,70}#\d+",txt) and re.search(r"(?i)\bdraft\b",txt):
            ok=True
            break
    if not ok: raise PolicyError("evidence lacks current Draft PR and Issue claim")
    return f"X003_SCOPE_PASS lane=C paths={len(names)} source={len(owned)} evidence={len(notes)} pr={pr_number}"

def main():
    a=argparse.ArgumentParser()
    a.add_argument("--repo",type=Path,default=Path("."))
    for name in ("base-sha","head-sha","head-ref","base-ref"):a.add_argument("--"+name,required=True)
    a.add_argument("--pr-number",type=int,required=True)
    x=a.parse_args()
    try:print(verify(x.repo,x.base_sha,x.head_sha,x.pr_number,x.head_ref,x.base_ref))
    except (PolicyError,UnicodeError) as e:sys.exit("X003_SCOPE_FAILED: "+str(e))

if __name__=="__main__":
    main()
