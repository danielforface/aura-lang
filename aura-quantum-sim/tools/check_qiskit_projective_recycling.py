#!/usr/bin/env python3
"""C-013 independent Qiskit projective-dynamic factor recycling oracle.

One actual unmodified Rust factorized JSON binary per independently seeded
dynamic circuit; Qiskit evolves gates and performs projective Z selection
using the *observed* Rust trace, never Qiskit Aer, mocks or QPU hardware.
Post-selection maintains the entire complex phase and tests every amplitude.
This validates conditional semantics, not a formal soundness theorem.
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
TOLERANCE=3e-11

def source_program(n,case):
    rng=random.Random(20261107+n*3701+case*97)
    ops=[]
    for q in range(1,n):
        ops.append({"op":"gate","gate":{"kind":"h","target":0}})
        ops.append({"op":"gate","gate":{"kind":"cx","control":0,"target":q}})
        if case%2==0:
            ops.append({"op":"measure","qubit":0,"into_bit":0})
            ops.append({"op":"if","bit":0,"equals":True,
                "gate":{"kind":"x","target":0}})
        else:
            ops.append({"op":"reset","qubit":0})
        # A genuine non-Clifford phase and real interference *after* the
        # random projective outcome. All targets remain independent.
        ops.append({"op":"gate","gate":{"kind":"t","target":q}})
        if rng.randrange(2):
            ops.append({"op":"gate","gate":{"kind":"h","target":q}})
        if q>1 and rng.randrange(2):
            ops.append({"op":"gate","gate":{
                "kind":"swap","a":q,"b":q-1}})
    return ops

def evolve(state,n,gate):
    qc=QuantumCircuit(n)
    kind=gate["kind"]
    if kind in ("cx","cz"):
        getattr(qc,kind)(gate["control"],gate["target"])
    elif kind=="swap":
        qc.swap(gate["a"],gate["b"])
    else:
        getattr(qc,kind)(gate["target"])
    return state.evolve(qc)

def collapse(state,n,qubit,outcome):
    data=np.asarray(state.data,dtype=np.complex128)
    indices=np.arange(1<<n,dtype=np.int64)
    chosen=((indices>>qubit)&1)==int(outcome)
    probability=float(np.sum(np.abs(data[chosen])**2))
    if not np.isfinite(probability) or probability<1e-13:
        raise AssertionError(f"impossible Qiskit observed outcome q={qubit},p={probability}")
    projected=np.zeros_like(data)
    projected[chosen]=data[chosen]/math.sqrt(probability)
    return Statevector(projected),probability

def simulate_independently(n,ops,rust_trace):
    state=Statevector.from_int(0,2**n)
    classical=[None]
    events=iter(rust_trace)
    count=0
    minimum_branch=1.0
    for instruction,op in enumerate(ops):
        if op["op"]=="gate":
            state=evolve(state,n,op["gate"])
        elif op["op"]=="if":
            if classical[op["bit"]]==op["equals"]:
                state=evolve(state,n,op["gate"])
        elif op["op"] in ("measure","reset"):
            try:
                event=next(events)
            except StopIteration as exc:
                raise AssertionError("missing source measurement event") from exc
            if (event["instruction"]!=instruction or
                event["qubit"]!=op["qubit"] or event["kind"]!=op["op"]):
                raise AssertionError(f"wrong trace event: {event}; op={op}")
            outcome=(event["value"] if op["op"]=="measure"
                     else event["previous_value"])
            if not isinstance(outcome,bool):
                raise AssertionError("invalid instrument outcome")
            state,p=collapse(state,n,op["qubit"],outcome)
            minimum_branch=min(minimum_branch,p)
            if op["op"]=="measure":
                classical[op["into_bit"]]=outcome
                if event["into_bit"]!=op["into_bit"]:
                    raise AssertionError("bad classical write")
            elif outcome:
                state=evolve(state,n,{"kind":"x","target":op["qubit"]})
            count+=1
        else:
            raise AssertionError("unsupported oracle instruction")
    try:
        next(events)
        raise AssertionError("trailing source event")
    except StopIteration:
        pass
    return np.asarray(state.data,dtype=np.complex128),classical,count,minimum_branch

def run_rust(binary,n,case,ops):
    request={
        "schema":"aura.quantum.factorized-request/1",
        "qubits":n,"classical_bits":1,
        "seed":2026+case*113+n*73,
        "shots":1,"max_memory_bytes":16*1024*1024,
        "inspect_factors":True,"inspect_amplitudes":True,
        "instructions":ops,
    }
    proc=subprocess.run([str(binary.resolve())],
        input=json.dumps(request),text=True,capture_output=True,
        timeout=25,check=False)
    if proc.returncode!=0 or proc.stderr.strip():
        raise AssertionError(f"valid source dynamic CLI failed {proc.stderr}")
    result=json.loads(proc.stdout)
    if (result["schema"]!="aura.quantum.factorized-result/1"
        or result["precision"]!="IEEE-754-complex-f64"
        or result["truncation"]!="none;factor-width-hard-limit"
        or result["qpu_hardware_jobs"]!=0
        or result["evidence_class"]!="simulation"
        or result["projective_wire_splits"]!=n-1
        or result["projective_split_deferred"]!=0
        or len(result["factors"])!=n
        or any(f["local_dimension"]!=2 for f in result["factors"])):
        raise AssertionError("invalid projective factorized metadata/state")
    raw=result["basis_ordered_complex_pairs"]
    if len(raw)!=(1<<n):raise AssertionError("incorrect local complex width")
    amps=np.asarray([complex(re,im) for re,im in raw],dtype=np.complex128)
    if not np.all(np.isfinite(amps)):raise AssertionError("non-finite quantum amplitudes")
    return result,amps

def main(binary,report_path):
    packages={name:version(name) for name in PKGS}
    if packages!=PKGS or sys.version_info[:2]!=(3,11):
        raise RuntimeError(f"Q28 scientific packages not pinned: {packages}")
    report={
        "status":"Q28_PROJECTIVE_DYNAMIC_NUMERICAL_PARITY_PASS",
        "cases":0,"complex_amplitudes":0,
        "projective_events":0,"negative_mutation_controls":0,
        "max_complex_delta":0.0,"max_norm_residual":0.0,
        "min_selected_branch_probability":1.0,
        "tolerance":TOLERANCE,
        "projection":"real_observed_Z_outcomes_independently_applied",
        "recycling":"lossless_measured_wire_extraction_no_schmidt_truncation",
        "precision":"complex-f64", "qpu_hardware_jobs":0,
        "packages":packages,"python":platform.python_version()
    }
    for n in range(2,9):
        for case in range(8):
            ops=source_program(n,case)
            receipt,actual=run_rust(binary,n,case,ops)
            expect,bits,event_count,min_p=simulate_independently(
                n,ops,receipt["results"][0]["trace"])
            if receipt["results"][0]["bits"]!=bits:
                raise AssertionError("classical register changed by split")
            delta=float(np.max(np.abs(actual-expect)))
            norm=abs(float(np.vdot(actual,actual).real)-1.0)
            if not math.isfinite(delta) or delta>TOLERANCE or norm>TOLERANCE:
                raise AssertionError(f"complex amplitude divergence n={n} case={case}: {delta}")
            modified=actual.copy()
            modified[0]+=1e-3
            if float(np.max(np.abs(modified-expect)))<=TOLERANCE:
                raise AssertionError("mutated negative oracle control accepted")
            report["cases"]+=1
            report["complex_amplitudes"]+=len(actual)
            report["projective_events"]+=event_count
            report["negative_mutation_controls"]+=1
            report["max_complex_delta"]=max(report["max_complex_delta"],delta)
            report["max_norm_residual"]=max(report["max_norm_residual"],norm)
            report["min_selected_branch_probability"]=min(
                report["min_selected_branch_probability"],min_p)
    if report["cases"]!=56 or report["complex_amplitudes"]!=4064:
        raise AssertionError("incomplete dynamic projective corpus")
    report_path.parent.mkdir(parents=True,exist_ok=True)
    report_path.write_text(json.dumps(report,indent=2,sort_keys=True)+"\n")
    print("Q28_PROJECTIVE_DYNAMIC_PARITY "+json.dumps(report,sort_keys=True),flush=True)

if __name__=="__main__":
    if len(sys.argv)!=3:
        raise SystemExit("usage: check_qiskit_projective_recycling.py <bin> <report.json>")
    main(Path(sys.argv[1]),Path(sys.argv[2]))
