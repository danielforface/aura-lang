#!/usr/bin/env python3
"""C-014 independent full-complex Qiskit oracle for EXACT controlled gate factor shortcuts.

This compares real production JSON output, not a Rust reference to itself.
It tests valid 1q Pauli Z/X eigenstates against CX/CZ, their algebraic
phase consequences, and deliberate non-eigenstate negative controls:
the latter MUST merge, not silently approximate a nearly product state.
No QPU, network, Aer provider, shots statistics or formal proof claimed.
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
TOL=3e-11

def program(n,case):
    target=n-1
    kind=case%8
    def gate(g,*w):
        return {"op":"gate","gate":(
            {"kind":g,"control":w[0],"target":w[1]} if g in ("cx","cz")
            else {"kind":g,"target":w[0]}
        )}
    if kind==0:
        ops=[gate("x",0),gate("h",target),gate("cx",0,target)]
    elif kind==1:
        ops=[gate("x",0),gate("h",target),gate("cz",0,target)]
    elif kind==2:
        ops=[gate("h",0),gate("x",target),gate("cz",0,target)]
    elif kind==3:
        ops=[gate("h",0),gate("h",target),gate("cx",0,target)]
    elif kind==4:
        ops=[gate("h",0),gate("h",target),gate("z",target),
             gate("cx",0,target)]
    elif kind==5:
        ops=[gate("h",0),gate("t",0),gate("h",0),gate("cx",0,target)]
    elif kind==6:
        ops=[gate("h",0),gate("h",target),gate("cz",0,target)]
    else:
        ops=[gate("h",0),gate("t",target),gate("cz",0,target)]
    # Additional varied local interference/phase gates on all wires:
    # after the critical controlled operation so shortcut classification
    # is independently predictable, including non-Clifford amplitudes.
    rng=random.Random(20261107+n*1307+case*47)
    for i in range(11):
        q=rng.randrange(n)
        ops.append(gate(rng.choice(("h","t","s","z","x","y")),q))
    return ops, (kind not in (5,6))

def expect_with_qiskit(n,ops):
    qc=QuantumCircuit(n)
    for op in ops:
        g=op["gate"]
        if g["kind"] in ("cx","cz"):
            getattr(qc,g["kind"])(g["control"],g["target"])
        else:
            getattr(qc,g["kind"])(g["target"])
    return np.asarray(Statevector.from_instruction(qc).data,dtype=np.complex128)

def execute(binary,n,ops):
    request={
        "schema":"aura.quantum.factorized-request/1",
        "qubits":n,"classical_bits":0,"shots":1,"seed":20261107,
        "max_memory_bytes":16*1024*1024,
        "inspect_factors":True,"inspect_amplitudes":True,
        "instructions":ops,
    }
    proc=subprocess.run([str(binary.resolve())],
        input=json.dumps(request),capture_output=True,text=True,
        timeout=25,check=False)
    if proc.returncode!=0 or proc.stderr.strip():
        raise AssertionError(f"production CLI failed: {proc.stderr}")
    result=json.loads(proc.stdout)
    if (result["schema"]!="aura.quantum.factorized-result/1"
        or result["evidence_class"]!="simulation"
        or result["qpu_hardware_jobs"]!=0
        or result["truncation"]!="none;factor-width-hard-limit"
        or result["projective_wire_splits"]!=0):
        raise AssertionError("incorrect source/provenance/physical claim")
    values=result["basis_ordered_complex_pairs"]
    if len(values)!=(1<<n):
        raise AssertionError("wrong quantum complex basis size")
    amps=np.asarray([complex(a,b) for a,b in values],dtype=np.complex128)
    if not np.all(np.isfinite(amps)):
        raise AssertionError("nonfinite amplitudes accepted")
    return result,amps

def main(binary,outpath):
    pkgs={name:version(name) for name in PKGS}
    if pkgs!=PKGS or sys.version_info[:2]!=(3,11):
        raise RuntimeError(f"scientific packages not pinned: {pkgs}")
    report=dict(
        status="EXACT_PRODUCT_CONTROL_QISKIT_PARITY_PASS",
        cases=0,amplitudes=0,algebraic_bypass_cases=0,
        true_join_cases=0,negative_mutation_controls=0,
        max_complex_delta=0.0,max_norm_residual=0.0,
        tolerance=TOL,product_criterion="bit-exact_pauli_eigenvector_no_f64_threshold",
        qpu_hardware_jobs=0,evidence_class="simulation",
        packages=pkgs,python=platform.python_version()
    )
    for n in range(2,9):
        for case in range(16):
            ops,can_bypass=program(n,case)
            x,actual=execute(binary,n,ops)
            expected=expect_with_qiskit(n,ops)
            count=x["product_preserving_two_wire_gates"]
            if can_bypass:
                if count!=1 or len(x["factors"])!=n:
                    raise AssertionError(f"proved product gate unexpectedly joined n={n} case={case}")
                report["algebraic_bypass_cases"]+=1
            else:
                if count!=0 or len(x["factors"])!=n-1:
                    raise AssertionError(f"non-eigenstate bypassed incorrectly n={n} case={case}")
                report["true_join_cases"]+=1
            delta=float(np.max(np.abs(actual-expected)))
            norm=abs(float(np.vdot(actual,actual).real)-1.0)
            if not math.isfinite(delta) or delta>TOL or norm>TOL:
                raise AssertionError(f"quantum phase/probability mismatch n={n} case={case} d={delta} norm={norm}")
            mutated=actual.copy()
            mutated[-1]+=0.002j
            if float(np.max(np.abs(mutated-expected)))<=TOL:
                raise AssertionError("poisoned negative control was accepted")
            report["cases"]+=1
            report["amplitudes"]+=len(actual)
            report["negative_mutation_controls"]+=1
            report["max_complex_delta"]=max(report["max_complex_delta"],delta)
            report["max_norm_residual"]=max(report["max_norm_residual"],norm)
    if (report["cases"]!=112 or report["amplitudes"]!=8128
        or report["algebraic_bypass_cases"]!=84
        or report["true_join_cases"]!=28
        or report["negative_mutation_controls"]!=112):
        raise AssertionError("incomplete independent product-control corpus")
    outpath.parent.mkdir(parents=True,exist_ok=True)
    outpath.write_text(json.dumps(report,sort_keys=True,indent=2)+"\n")
    print("Q28_EXACT_CONTROL_QISKIT "+json.dumps(report,sort_keys=True),flush=True)

if __name__=="__main__":
    if len(sys.argv)!=3:
        raise SystemExit("usage: check_qiskit_exact_product_controls.py <factorized-bin> <report-path>")
    main(Path(sys.argv[1]),Path(sys.argv[2]))
