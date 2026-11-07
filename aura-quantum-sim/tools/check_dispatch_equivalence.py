#!/usr/bin/env python3
"""Q28 C-011 external CPU dispatcher contract: real subprocess comparison.

The dispatcher must expose the unchanged full original JSON result for all
four engines. Invalid/unsupported cases must fail closed, no stdout, no QPU.
Uses only Python stdlib; no QPU, network, statistics or Qiskit needed.
"""
import copy
import json
import random
import subprocess
import sys
from pathlib import Path

SCHEMAS = {
    "clifford_stabilizer": ("aura-quantum-stabilizer-run", "aura.quantum.stabilizer-request/1",
                           "aura.quantum.stabilizer-result/1"),
    "pure_statevector": ("aura-quantum-run", "aura.quantum.circuit-request/1",
                         "aura.quantum.circuit-result/1"),
    "noisy_density": ("aura-quantum-noisy-run", "aura.quantum.noisy-request/1",
                      "aura.quantum.noisy-result/1"),
    "exact_symbolic": ("aura-quantum-symbolic-run", "aura.quantum.symbolic-request/1",
                       "aura.quantum.symbolic-result/1"),
}
DISPATCH_SCHEMA = "aura.quantum.dispatch-request/1"

def invoke(binary, request):
    proc=subprocess.run([str(binary.resolve())],input=json.dumps(request),
        capture_output=True,text=True,check=False,timeout=25)
    result=json.loads(proc.stdout) if proc.returncode==0 else None
    if proc.returncode==0 and proc.stderr.strip():
        raise AssertionError(f"unexpected stderr: {proc.stderr}")
    return proc,result

def positive(bins, request, expected):
    proc,output=invoke(bins/"aura-quantum-auto-run",request)
    if proc.returncode!=0:
        raise AssertionError(f"dispatch declined supported input: {proc.stderr}")
    if (output["selected_backend"]!=expected
            or output["schema"]!="aura.quantum.dispatch-result/1"
            or output["dispatch_policy"]!="capability-first-no-silent-fallback/1"
            or output["qpu_hardware_jobs"]!=0):
        raise AssertionError(f"wrong backend or provenance {output}")
    name,request_schema,result_schema=SCHEMAS[expected]
    direct=copy.deepcopy(request)
    direct.pop("mode")
    direct["schema"]=request_schema
    if expected!="clifford_stabilizer":
        direct.pop("inspect_generators",None)
        direct.pop("inspect_probabilities",None)
    if expected!="noisy_density":
        direct.pop("inspect_final_density",None)
    old,original=invoke(bins/name,direct)
    if old.returncode!=0 or output["engine_result"]!=original:
        raise AssertionError(f"direct quantum engine result changed by dispatcher: {old.stderr}")
    if original["schema"]!=result_schema or original["qpu_hardware_jobs"]!=0:
        raise AssertionError("engine provenance/schema mismatch")
    again,replay=invoke(bins/"aura-quantum-auto-run",request)
    if again.returncode!=0 or replay!=output:
        raise AssertionError("seeded run failed deterministic replay")

def negative(bins,request):
    proc,_=invoke(bins/"aura-quantum-auto-run",request)
    if proc.returncode!=2 or proc.stdout.strip():
        raise AssertionError(f"invalid input was executed: {proc}")
    error=json.loads(proc.stderr)
    if (error["schema"]!="aura.quantum.dispatch-error/1" or
        error["status"]!="error" or error["qpu_hardware_jobs"]!=0):
        raise AssertionError("invalid error provenance")

def program(n, mode, operations, sampled=True):
    r=dict(schema=DISPATCH_SCHEMA,mode=mode,qubits=n,classical_bits=1,
        max_memory_bytes=16*1024*1024,instructions=operations)
    if sampled:
        r.update(seed=15000+n,shots=8)
    return r

def main(bins,fixtures):
    counts={name:0 for name in SCHEMAS}
    for basename,want in [
        ("clifford","clifford_stabilizer"),
        ("nonclifford","pure_statevector"),
        ("noise","noisy_density"),
        ("exact","exact_symbolic"),
    ]:
        req=json.loads((fixtures/f"dispatch_{basename}_v1.json").read_text())
        positive(bins,req,want)
        counts[want]+=1

    for n in range(1,5):
        for idx in range(6):
            rng=random.Random(90001+n*101+idx*19)
            gates=[]
            for k in range(9):
                q=rng.randrange(n)
                kind=rng.choice(("h","x","y","s","z"))
                gates.append({"op":"gate","gate":{"kind":kind,"target":q}})
                if n>1 and k%4==0:
                    dst=(q+1+rng.randrange(n-1))%n
                    gates.append({"op":"gate","gate":{
                        "kind":"cx","control":q,"target":dst}})
            candidates=[
                ("clifford_stabilizer",program(n,"auto",gates+[
                    {"op":"measure","qubit":0,"into_bit":0}])),
                ("pure_statevector",program(n,"auto",[
                    {"op":"gate","gate":{"kind":"h","target":0}},
                    {"op":"gate","gate":{"kind":"t","target":0}},
                    {"op":"gate","gate":{"kind":"h","target":0}},
                    {"op":"measure","qubit":0,"into_bit":0}])),
                ("noisy_density",program(n,"auto",[
                    {"op":"gate","gate":{"kind":"h","target":0}},
                    {"op":"noise","channel":{"kind":"amplitude_damping",
                        "qubit":0,"gamma":[0.0,0.2,0.5][idx%3]}},
                    {"op":"measure","qubit":0,"into_bit":0}])),
                ("exact_symbolic",program(n,"exact_branch",[
                    {"op":"gate","gate":{"kind":"h","target":0}},
                    {"op":"gate","gate":{"kind":"t","target":0}},
                    {"op":"gate","gate":{"kind":"h","target":0}},
                    {"op":"measure","qubit":0,"into_bit":0,"outcome":True},
                    {"op":"if","bit":0,"equals":True,
                        "gate":{"kind":"x","target":0}}],False)),
            ]
            for want,req in candidates:
                positive(bins,req,want)
                counts[want]+=1

    fixture=json.loads((fixtures/"dispatch_clifford_v1.json").read_text())
    bad=[]
    def variant(**changes):
        v=copy.deepcopy(fixture)
        v.update(changes)
        return v
    bad.extend([
        variant(schema="aura.quantum.dispatch-request/2"),
        variant(mode="hardware"),
        variant(max_memory_bytes=0),
        variant(shots=0),
        variant(qubits=129),
        variant(instructions=[{"op":"hardware_run"}]),
        variant(instructions=[{"op":"gate","gate":{"kind":"rx","target":0}}]),
        variant(instructions=[{"op":"measure","qubit":0,"into_bit":0,"outcome":True}]),
        variant(mode="exact_branch"),
        variant(mode="statevector",instructions=[{"op":"reset_observed","qubit":0}]),
        variant(mode="stabilizer",instructions=[{"op":"gate","gate":{"kind":"t","target":0}}]),
        variant(qubits=16,instructions=([{"op":"gate","gate":{"kind":"h","target":0}},
               {"op":"gate","gate":{"kind":"t","target":0}}]
               + [{"op":"gate","gate":{"kind":"cx","control":q,"target":q+1}}
                  for q in range(10)])),
        variant(instructions=[{"op":"if","bit":0,"equals":True,
            "gate":{"kind":"x","target":0},"action":{"kind":"gate","gate":{"kind":"x","target":0}}}]),
        variant(instructions=[{"op":"noise","channel":{"kind":"phase_flip",
            "qubit":0,"probability":1.5}}]),
    ])
    no_seed=copy.deepcopy(fixture);no_seed.pop("seed");bad.append(no_seed)
    hardware=copy.deepcopy(fixture);hardware["submit_qpu"]=True;bad.append(hardware)
    forced=copy.deepcopy(fixture);forced.update(mode="exact_branch")
    forced.pop("seed");forced.pop("shots")
    forced["instructions"]=[{"op":"reset","qubit":0}]
    bad.append(forced)
    for case in bad:
        negative(bins,case)
    if sum(counts.values())!=100 or len(bad)!=17:
        raise AssertionError("incomplete deterministic dispatch matrix")
    print("Q28_DISPATCH_EXTERNAL_PROCESS_PASS "+
          json.dumps({"positives":100,"negatives":17,"routes":counts,
            "qpu_hardware_jobs":0},sort_keys=True),flush=True)

if __name__=="__main__":
    if len(sys.argv)!=3:
        raise SystemExit("usage: check_dispatch_equivalence.py <bin-dir> <fixtures-dir>")
    main(Path(sys.argv[1]),Path(sys.argv[2]))
