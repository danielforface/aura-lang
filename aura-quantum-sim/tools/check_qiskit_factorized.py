#!/usr/bin/env python3
"""Independent Qiskit full complex amplitude parity for bounded factor graph.
No sampling, QPU calls or physical hardware. 64 deterministic 1..4 wire
programs; independent Qiskit QuantumCircuit and Statevector.
"""
import json
import math
import platform
import random
import subprocess
import sys
from importlib.metadata import version
from pathlib import Path
import numpy as np
from qiskit import QuantumCircuit
from qiskit.quantum_info import Statevector

PKGS={"qiskit":"2.6.1","numpy":"2.3.4"}
LIMIT=3e-11

def gate(n,rng):
    k=rng.choice(("h","x","y","z","s","t","cx","cz","swap") if n>1
                 else ("h","x","y","z","s","t"))
    a=rng.randrange(n)
    if k in ("cx","cz","swap"):
        b=(a+1+rng.randrange(n-1))%n
        return {"kind":k, **({"a":a,"b":b} if k=="swap"
                              else {"control":a,"target":b})}
    return {"kind":k,"target":a}

def build(n,case):
    rng=random.Random(0xFACA_2026+n*991+case*17)
    ops=[{"op":"gate","gate":{"kind":"h","target":q}} for q in range(n)]
    ops.extend({"op":"gate","gate":gate(n,rng)} for _ in range(33))
    return ops

def qiskit(n,ops):
    qc=QuantumCircuit(n)
    for op in ops:
        g=op["gate"];kind=g["kind"]
        if kind in ("cx","cz"):
            getattr(qc,kind)(g["control"],g["target"])
        elif kind=="swap":
            qc.swap(g["a"],g["b"])
        else:
            getattr(qc,kind)(g["target"])
    return np.asarray(Statevector.from_instruction(qc).data,dtype=np.complex128)

def rust(binary,n,ops):
    req={"schema":"aura.quantum.factorized-request/1",
         "qubits":n,"classical_bits":0,"seed":17,"shots":1,
         "max_memory_bytes":16*1024*1024,
         "inspect_amplitudes":True,"inspect_factors":True,
         "instructions":ops}
    p=subprocess.run([str(binary.resolve())],input=json.dumps(req),
                     text=True,capture_output=True,timeout=30,check=False)
    if p.returncode!=0 or p.stderr.strip():
        raise AssertionError("factorized CLI rejected valid program: "+p.stderr)
    v=json.loads(p.stdout)
    if (v["schema"]!="aura.quantum.factorized-result/1"
        or v["precision"]!="IEEE-754-complex-f64"
        or v["truncation"]!="none;factor-width-hard-limit"
        or v["truncation_error_bound"]!=0.0
        or v["evidence_class"]!="simulation"
        or v["qpu_hardware_jobs"]!=0):
        raise AssertionError("unexpected quantum physics/provenance claim")
    amplitudes=np.array([complex(re,im) for re,im in v["basis_ordered_complex_pairs"]])
    if amplitudes.shape!=(2**n,) or not np.all(np.isfinite(amplitudes)):
        raise AssertionError("invalid amplitude dimensions")
    return amplitudes

def main(binary,out):
    packages={k:version(k) for k in PKGS}
    if packages!=PKGS or sys.version_info[:2]!=(3,11):
        raise RuntimeError("unpinned Q28 factor oracle")
    report={"status":"FACTOR_NUMERICAL_PARITY_PASS",
        "cases":0,"amplitudes":0,"mutation_negative_controls":0,
        "max_complex_delta":0.0,"max_normalization_residual":0.0,
        "threshold":LIMIT,"packages":packages,
        "python":platform.python_version(),"qubit_order":"q0-lsb",
        "truncation":"none","qpu_hardware_jobs":0,
        "evidence_class":"simulation"}
    for n in range(1,5):
        for case in range(16):
            ops=build(n,case)
            exact=qiskit(n,ops)
            actual=rust(binary,n,ops)
            delta=float(np.max(np.abs(exact-actual)))
            norm=abs(float(np.vdot(actual,actual).real)-1.0)
            if delta>LIMIT or norm>LIMIT or not math.isfinite(delta):
                raise AssertionError(f"Q28 factor parity n={n} case={case}: {delta},{norm}")
            altered=actual.copy()
            altered[0]+=0.01
            if float(np.max(np.abs(altered-exact)))<=LIMIT:
                raise AssertionError("negative control accepted poisoned amplitude")
            report["cases"]+=1
            report["amplitudes"]+=len(actual)
            report["mutation_negative_controls"]+=1
            report["max_complex_delta"]=max(report["max_complex_delta"],delta)
            report["max_normalization_residual"]=max(report["max_normalization_residual"],norm)
    if report["cases"]!=64 or report["amplitudes"]!=240:
        raise AssertionError("incomplete corpus")
    out.parent.mkdir(parents=True,exist_ok=True)
    out.write_text(json.dumps(report,indent=2,sort_keys=True)+"\n")
    print("Q28_FACTOR_PARITY "+json.dumps(report,sort_keys=True),flush=True)

if __name__=="__main__":
    if len(sys.argv)!=3:
        raise SystemExit("usage: check_qiskit_factorized.py <factorized-bin> <report.json>")
    main(Path(sys.argv[1]),Path(sys.argv[2]))
