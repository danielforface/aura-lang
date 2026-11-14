#!/usr/bin/env python3
"""External Aura no-Z3 debug consumer trust contract, not the full workspace test."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

def verify(binary):
    exe=binary.resolve()
    if not exe.is_file():
        raise AssertionError("public Aura executable missing")
    with tempfile.TemporaryDirectory(prefix="x006-ab-") as d:
        src=Path(d)/"main.aura"
        src.write_text("# X006 no-Z3 trust negative\n\ncell main() ->:\n    val x: u32 = 1\n    val y: u32 = x + 2\n    val z: u32 = y * 3\n",encoding="utf-8")
        env=os.environ.copy()
        env.update({"AURA_DEBUG_PROTOCOL":"1","AURA_AVM_NO_Z3":"1"})
        p=subprocess.run([str(exe),"run",str(src),"--mode","avm"],input="{not-json}\n",capture_output=True,text=True,env=env,timeout=60)
    assert p.returncode==1,(p.returncode,p.stdout[-800:],p.stderr[-800:])
    events=[json.loads(line[len("AURA_DEBUG_EVENT "):]) for line in p.stdout.splitlines() if line.startswith("AURA_DEBUG_EVENT ")]
    assert [e.get("event") for e in events]==["hello","perfReport"],events
    assert events[0].get("protocol")==1
    assert "perf.timeline" in events[0].get("capabilities",[])
    assert isinstance(events[1].get("report",{}).get("timeline"),list)
    assert "AURA_DEBUG_PROTOCOL: ignored invalid command" in p.stderr,p.stderr
    assert "avm: Z3 gate rejected execution: verification not run: Z3 gate disabled" in p.stderr,p.stderr
    assert '"verified":true' not in p.stdout and '"proof_status":"passed"' not in p.stdout
    print("X006_AVM_NO_Z3_DEBUG_CONSUMER_PASS exit=1 hello=1 perfReport=1 malformed=rejected proof=false")

if __name__=="__main__":
    arg=argparse.ArgumentParser()
    arg.add_argument("--binary",required=True,type=Path)
    try:
        verify(arg.parse_args().binary)
    except (AssertionError, OSError, ValueError, TypeError, subprocess.SubprocessError) as e:
        sys.exit("X006_AVM_NO_Z3_DEBUG_CONSUMER_FAILED: "+str(e))
