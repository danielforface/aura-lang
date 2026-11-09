#!/usr/bin/env python3
"""C-023: REAL seed-free noisy ensemble Rust CLI vs independent Qiskit Kraus.

Independent Qiskit *density matrices* for all recorded Z outcomes; the
reference neither reads nor calls Rust's branch enumerator or seeded shot.
Each outcome is projected with its Born weight and the noise / gates evolve
through Qiskit DensityMatrix.evolve(Kraus/Operator). Nonselective reset is a
single CPTP channel. Discards only separately reported Qiskit machine-roundoff
ghosts below 1e-13; corpus must have safely large admitted physical weights.
"""
from __future__ import annotations
import json
import math
import platform
import random
import subprocess
import sys
from importlib.metadata import version
from pathlib import Path

import numpy as np
from qiskit.quantum_info import DensityMatrix, Kraus
from check_qiskit_noisy_cptp import (
    PACKAGES,TOL,apply_action,gate,noise,
)

SCHEMA="aura.quantum.noisy-ensemble-request/1"
RESULT="aura.quantum.noisy-ensemble-result/1"
ERROR="aura.quantum.noisy-ensemble-error/1"
GHOST_LIMIT=1e-13
PHYSICAL_FLOOR=1e-9


def request(n:int,ops:list[dict],bits:int=2)->dict:
    return {"schema":SCHEMA,"qubits":n,"classical_bits":bits,
            "max_memory_bytes":16*1024*1024,"instructions":ops}


def transcript(binary:Path,req:dict)->tuple[bytes,dict]:
    p=subprocess.run([str(binary.resolve())],
        input=json.dumps(req,sort_keys=True).encode(),
        capture_output=True,timeout=45,check=False)
    if p.returncode!=0 or p.stderr:
        raise AssertionError(f"REAL noisy ensemble CLI failed: {p.returncode} "
                             f"{p.stderr[:800]}")
    r=json.loads(p.stdout)
    if (r["schema"]!=RESULT or r["status"]!="simulated"
        or r["sampling"]!="none;all_representable_explicit_z_outcomes"
        or r["reset_policy"]!="nonselective;reset_observed_rejected"
        or r["noise_semantics"]!="idealized_single_wire_CPTP_Kraus"
        or r["shots"]!=0 or r["qpu_hardware_jobs"]!=0
        or r["evidence_class"]!="simulation"
        or r["max_qubits"]!=4 or r["max_branches"]!=64
        or r["qubits"]!=req["qubits"]
        or r["classical_bits"]!=req["classical_bits"]):
        raise AssertionError("real CLI versioned provenance metadata mismatch")
    if any(key in r for key in ("seed","trace","results","shots_samples",
                                "sampled_histogram","hardware_job_id")):
        raise AssertionError("seed/shot trace leaked into unconditioned ensemble")
    return p.stdout,r


def selected(branch:np.ndarray,wire:int,outcome:bool,n:int
            )->tuple[np.ndarray,float]:
    dim=1<<n
    support=((np.arange(dim)>>wire)&1)==int(outcome)
    projected=branch*np.outer(support,support)
    p=float(np.trace(projected).real)
    return projected,p


def reference(req:dict)->tuple[np.ndarray,dict,int,int,int,float]:
    n=req["qubits"]
    initial=DensityMatrix.from_int(0,1<<n)
    branches=[(initial,[None]*req["classical_bits"],1.0)]
    n_measures=0
    ghosts=0
    largest_ghost=0.0
    smallest=1.0
    reset=Kraus([
        np.array([[1,0],[0,0]],dtype=complex),
        np.array([[0,1],[0,0]],dtype=complex),
    ])
    for op in req["instructions"]:
        kind=op["op"]
        if kind in ("gate","noise"):
            branches=[(apply_action(rho,op,n),bits,weight)
                      for rho,bits,weight in branches]
        elif kind=="reset":
            branches=[(rho.evolve(reset,qargs=[op["qubit"]]),bits,weight)
                      for rho,bits,weight in branches]
        elif kind=="if":
            bit=op["bit"]
            def conditional(rho:DensityMatrix,bits:list)->DensityMatrix:
                if bits[bit] is None:
                    raise AssertionError("unwritten conditional register")
                if bits[bit]==op["equals"]:
                    act=op["action"]
                    if act["kind"]=="gate":
                        return apply_action(rho,{"op":"gate","gate":act["gate"]},n)
                    if act["kind"]=="noise":
                        return apply_action(rho,{"op":"noise",
                                                  "channel":act["channel"]},n)
                    raise AssertionError("unknown conditional action")
                return rho
            branches=[(conditional(rho,bits),bits,weight)
                      for rho,bits,weight in branches]
        elif kind=="measure":
            n_measures+=1
            next_branches=[]
            for rho,bits,weight in branches:
                for outcome in (False,True):
                    projected,p=selected(np.asarray(rho.data,dtype=complex),
                                         op["qubit"],outcome,n)
                    if p<0:
                        raise AssertionError("negative instrument Born probability")
                    if p<GHOST_LIMIT:
                        if p>0:
                            ghosts+=1
                            largest_ghost=max(largest_ghost,p*weight)
                        continue
                    physical=p*weight
                    if physical<PHYSICAL_FLOOR:
                        raise AssertionError("Qiskit physical branch too rare to certify")
                    smallest=min(smallest,physical)
                    updated=bits.copy()
                    updated[op["into_bit"]]=outcome
                    next_branches.append((
                        DensityMatrix(projected/p),updated,physical))
            branches=next_branches
            if not branches or len(branches)>64:
                raise AssertionError("reference too many or zero physical branches")
        else:
            raise AssertionError(f"foreign operation {kind}")
    mass=sum(w for _,_,w in branches)
    if abs(mass-1.0)>TOL:
        raise AssertionError(f"Qiskit reference mass drift {mass}")
    dim=1<<n
    avg=np.zeros((dim,dim),dtype=complex)
    histogram={}
    for rho,bits,p in branches:
        avg+=p*np.asarray(rho.data,dtype=complex)
        key="".join("?" if x is None else ("1" if x else "0")
                    for x in reversed(bits))
        histogram[key]=histogram.get(key,0.0)+p
    if abs(np.trace(avg)-1)>TOL:
        raise AssertionError("final independent density trace drift")
    return avg,histogram,len(branches),n_measures,ghosts,smallest


def verify(out:dict,reference_result:tuple,name:str)->tuple[float,int]:
    avg,hist,count,measures,ghosts,minimum=reference_result
    n=avg.shape[0]
    if (out["final_density_dimension"]!=n
        or out["final_branch_count"]!=count
        or out["measurement_count"]!=measures
        or set(out["classical_probabilities"])!=set(hist)):
        raise AssertionError(f"{name}: branch/histogram keys or dimensions incorrect")
    maximum=0.0
    for key,p in hist.items():
        delta=abs(out["classical_probabilities"][key]-p)
        if delta>TOL:raise AssertionError(f"{name}: wrong Born histogram {key}")
        maximum=max(maximum,delta)
    entries=out["final_density_row_major_complex_pairs"]
    if len(entries)!=avg.size:
        raise AssertionError(f"{name}: incomplete full density matrix")
    for i,(actual,z) in enumerate(zip(entries,avg.reshape(-1))):
        if not isinstance(actual,list) or len(actual)!=2:
            raise AssertionError(f"{name}: wrong complex pair at {i}")
        delta=max(abs(actual[0]-z.real),abs(actual[1]-z.imag))
        if not math.isfinite(delta) or delta>TOL:
            raise AssertionError(f"{name}: wrong density[{i}] {delta}")
        maximum=max(maximum,delta)
    independent_purity=float(np.trace(avg@avg).real)
    for key,reference_value in (
        ("trace",1.0),
        ("purity",independent_purity),
        ("probability_mass",1.0),
    ):
        delta=abs(out[key]-reference_value)
        if not math.isfinite(delta) or delta>TOL:
            raise AssertionError(f"{name}: {key} mismatch {delta}")
        maximum=max(maximum,delta)
    for i,v in enumerate(out["final_basis_probabilities"]):
        delta=abs(v-avg[i,i].real)
        if not math.isfinite(delta) or delta>TOL:
            raise AssertionError(f"{name}: basis[{i}] mismatch {delta}")
        maximum=max(maximum,delta)
    return maximum,avg.size


def negative(binary:Path,req:dict,code:str)->None:
    p=subprocess.run([str(binary.resolve())],input=json.dumps(req),
                     text=True,capture_output=True,timeout=30)
    if p.returncode!=2 or p.stdout.strip():
        raise AssertionError("invalid noisy ensemble request returned stdout")
    v=json.loads(p.stderr)
    if (v["schema"]!=ERROR or v["code"]!=code
        or v["qpu_hardware_jobs"]!=0):
        raise AssertionError(f"wrong failed-response envelope {v}")


def corpus(fixtures:Path)->list[tuple[str,dict]]:
    items=[("real-noisy-feedback",json.loads(
        (fixtures/"noisy_ensemble_feedback_v1.json").read_text()))]
    single=[]
    for _ in range(6):
        single.extend([gate("h",0),{"op":"measure","qubit":0,"into_bit":0}])
    items.append(("all-64-histories",request(1,single,1)))
    types=("phase_damping","depolarizing","pauli_mixture",
           "amplitude_damping","phase_flip")
    for n in range(1,5):
        for i in range(12):
            rng=random.Random(20261109+n*200+i)
            ops=[gate("h",0),gate("s",n-1),gate("t",0)]
            if n>1:
                ops+=[gate("h",n-1),gate("cx",0,n-1)]
            for j in range(2):
                q=rng.randrange(n)
                kind=types[(i+j)%len(types)]
                if kind=="phase_damping":p={"gamma":0.25+0.05*j}
                elif kind=="amplitude_damping":p={"gamma":0.1+0.15*j}
                elif kind=="depolarizing":p={"probability":0.18+0.12*j}
                elif kind=="phase_flip":p={"probability":0.16+0.08*j}
                else:p={"p_x":0.11,"p_y":0.17,"p_z":0.23}
                ops.append(noise(kind,q,**p))
                ops.append(gate(("h","s","t")[rng.randrange(3)],q))
            ops.append({"op":"measure","qubit":0,"into_bit":0})
            ops.append({"op":"if","bit":0,"equals":True,
                "action":{"kind":"noise","channel":{
                    "kind":"pauli_mixture","qubit":n-1,
                    "p_x":0.14,"p_y":0.11,"p_z":0.10}}})
            ops.append({"op":"if","bit":0,"equals":False,
                "action":{"kind":"gate","gate":{
                    "kind":"h","target":n-1}}})
            ops.append(gate("h",0))
            ops.append({"op":"measure","qubit":0,"into_bit":1})
            if i%3==0:
                ops.append({"op":"reset","qubit":n-1})
            items.append((f"nonclifford-noisy-{n}q-{i}",
                          request(n,ops,2)))
    return items


def main(binary:Path,fixtures:Path,report_path:Path)->dict:
    installed={k:version(k) for k in PACKAGES}
    if installed!=PACKAGES or sys.version_info[:2]!=(3,11):
        raise RuntimeError(f"unqualified C-023 scientific env: {installed}")
    report={
        "status":"Q28_NOISY_ENSEMBLE_QISKIT_PASS",
        "cases":0,"full_complex_density_entries":0,
        "represented_instrument_histories":0,
        "max_component_delta":0.0,"smallest_admitted_physical_branch":1.0,
        "roundoff_ghost_count":0,"corrupted_density_controls_rejected":0,
        "invalid_protocol_controls":0,"highest_width":0,
        "qpu_hardware_jobs":0,
        "reference":"Qiskit-DensityMatrix-Kraus-Operator-independent-instrument-ensemble",
        "threshold":TOL,"packages":installed,
        "python":platform.python_version(),
    }
    examples=corpus(fixtures)
    for name,req in examples:
        _,rust=transcript(binary,req)
        oracle=reference(req)
        deviation,entries=verify(rust,oracle,name)
        report["cases"]+=1
        report["full_complex_density_entries"]+=entries
        report["represented_instrument_histories"]+=oracle[2]
        report["roundoff_ghost_count"]+=oracle[4]
        report["smallest_admitted_physical_branch"]=min(
            report["smallest_admitted_physical_branch"],oracle[5])
        report["max_component_delta"]=max(report["max_component_delta"],deviation)
        report["highest_width"]=max(report["highest_width"],req["qubits"])
        poisoned=json.loads(json.dumps(rust))
        poisoned["final_density_row_major_complex_pairs"][0][0]+=0.02
        try:
            verify(poisoned,oracle,name+"-poison")
        except AssertionError:
            report["corrupted_density_controls_rejected"]+=1
        else:
            raise AssertionError("poisoned ensemble erroneously qualified")
    base=examples[0][1]
    negatives=[
        ({**base,"seed":1},"invalid_json"),
        ({**base,"shots":256},"invalid_json"),
        ({**base,"hardware":True},"invalid_json"),
        ({**base,"qpu_backend":"device"},"invalid_json"),
        ({**base,"schema":"aura.quantum.noisy-request/1"},
         "unsupported_schema"),
        ({**base,"qubits":5},"profile_limit"),
        ({**base,"max_memory_bytes":1},"circuit_rejected"),
        ({**base,"instructions":[{"op":"reset_observed","qubit":0}]},
         "circuit_rejected"),
    ]
    for req,code in negatives:
        negative(binary,req,code)
        report["invalid_protocol_controls"]+=1
    if (report["cases"]!=50
        or report["corrupted_density_controls_rejected"]!=50
        or report["invalid_protocol_controls"]!=8
        or report["highest_width"]!=4):
        raise AssertionError(f"incomplete independent oracle {report}")
    report_path.parent.mkdir(parents=True,exist_ok=True)
    report_path.write_text(json.dumps(report,sort_keys=True,indent=2)+"\n",
                           encoding="utf-8")
    print("Q28_NOISY_ENSEMBLE_QISKIT "+json.dumps(report,sort_keys=True),flush=True)
    return report


if __name__=="__main__":
    if len(sys.argv)!=4:
        raise SystemExit("usage: check_qiskit_noisy_ensemble.py binary fixtures report")
    main(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
