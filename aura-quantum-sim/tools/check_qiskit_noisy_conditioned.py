#!/usr/bin/env python3
"""C-024 physical differential: real Rust noisy-conditioned binary vs Qiskit.

Qiskit DensityMatrix, Operator/Kraus and independent explicit Z instruments;
final-register selected event is calculated by independent unnormalized
weighted density branches, NOT by deriving it from the Rust unconditional
output. No RNG/seed/QPU. Numerical ghosts are explicitly counted.
"""
from __future__ import annotations
import json
import math
import platform
import subprocess
import sys
from importlib.metadata import version
from pathlib import Path

import numpy as np
from qiskit.quantum_info import DensityMatrix,Kraus
from check_qiskit_noisy_cptp import PACKAGES,TOL,apply_action,gate,noise
from check_qiskit_noisy_ensemble import (
    corpus as noisy_corpus,selected as z_project,
    GHOST_LIMIT,PHYSICAL_FLOOR,
)
REQUEST="aura.quantum.noisy-conditioned-request/1"
RESULT="aura.quantum.noisy-conditioned-result/1"
ERROR="aura.quantum.noisy-conditioned-error/1"

def convert(old:dict,conditions:list[dict])->dict:
    return {
        "schema":REQUEST,"qubits":old["qubits"],
        "classical_bits":old["classical_bits"],
        "max_memory_bytes":old["max_memory_bytes"],
        "instructions":old["instructions"],
        "conditions":conditions,
    }

def actual(binary:Path,req:dict)->tuple[bytes,dict]:
    p=subprocess.run([str(binary.resolve())],
        input=json.dumps(req,sort_keys=True).encode(),
        capture_output=True,timeout=45,check=False)
    if p.returncode!=0 or p.stderr:
        raise AssertionError(
            f"Rust real noisy postselection binary failed {p.returncode}: {p.stderr[:600]}")
    r=json.loads(p.stdout)
    if (r["schema"]!=RESULT or r["status"]!="simulated"
        or r["sampling"]!="none;postselected_born_ensemble"
        or r["shots"]!=0 or r["qpu_hardware_jobs"]!=0
        or r["noise_semantics"]!="idealized_single_wire_CPTP_Kraus"
        or r["condition_scope"]!="final_explicitly_measured_classical_register"
        or r["disclosure_policy"]!="selected_event_only;no_unselected_histogram_or_trace"
        or r["reset_policy"]!="nonselective;reset_observed_rejected"
        or r["basis_order"]!="q0_lsb;row_major_complex_pairs"
        or r["max_qubits"]!=4 or r["max_branches"]!=64):
        raise AssertionError("Rust conditioned provenance/schema/physical profile drift")
    for forbidden in (
        "seed","classical_probabilities","histogram","results",
        "instrument_trace","shot_trace","unselected_probability",
        "branch_weights","sampled_histogram",
    ):
        if forbidden in r:
            raise AssertionError(f"unselected classical information disclosed: {forbidden}")
    return p.stdout,r

def branches_qiskit(req:dict)->tuple[list,int,int,float]:
    n=req["qubits"]
    branches=[(DensityMatrix.from_int(0,1<<n),
               [None]*req["classical_bits"],1.0)]
    measures=0
    ghosts=0
    min_physical=1.0
    reset=Kraus([
        np.array([[1,0],[0,0]],dtype=complex),
        np.array([[0,1],[0,0]],dtype=complex),
    ])
    for op in req["instructions"]:
        kind=op["op"]
        if kind in ("gate","noise"):
            branches=[(apply_action(rho,op,n),bits,mass)
                      for rho,bits,mass in branches]
        elif kind=="reset":
            branches=[(rho.evolve(reset,qargs=[op["qubit"]]),bits,mass)
                      for rho,bits,mass in branches]
        elif kind=="if":
            new=[]
            for rho,bits,mass in branches:
                bit=op["bit"]
                if bits[bit] is None:
                    raise AssertionError("independent Qiskit cannot read unwritten bit")
                if bits[bit]==op["equals"]:
                    action=op["action"]
                    if action["kind"] not in ("gate","noise"):
                        raise AssertionError("unknown conditional Kraus/gate")
                    rho=apply_action(rho,{
                        "op":action["kind"],
                        action["kind"]=="gate" and "gate" or "channel":
                            action["gate"] if action["kind"]=="gate"
                            else action["channel"],
                    },n)
                new.append((rho,bits,mass))
            branches=new
        elif kind=="measure":
            measures+=1
            next_branches=[]
            for rho,bits,mass in branches:
                for value in (False,True):
                    projected,p=z_project(
                        np.asarray(rho.data,dtype=complex),
                        op["qubit"],value,n)
                    if p<0:
                        raise AssertionError("Qiskit instrument negative Born mass")
                    if p<GHOST_LIMIT:
                        if p>0:ghosts+=1
                        continue
                    new_mass=p*mass
                    if new_mass<PHYSICAL_FLOOR:
                        raise AssertionError("Qiskit physically tiny branch outside oracle corpus")
                    min_physical=min(min_physical,new_mass)
                    next_bits=bits.copy()
                    next_bits[op["into_bit"]]=value
                    next_branches.append((
                        DensityMatrix(projected/p),next_bits,new_mass))
            branches=next_branches
        else:
            raise AssertionError(f"unsupported opcode {kind}")
    total=sum(mass for _,_,mass in branches)
    if abs(total-1.0)>TOL or not branches or len(branches)>64:
        raise AssertionError(f"independent Qiskit CPTP channel Born mass {total}")
    return branches,measures,ghosts,min_physical

def choose(
    branches:list,conditions:list[dict],dim:int,
)->tuple[float,int,np.ndarray,float]:
    selected=[(rho,mass) for rho,bits,mass in branches
              if all(bits[c["bit"]] is c["equals"] for c in conditions)]
    prob=sum(mass for _,mass in selected)
    if prob<=0:
        raise AssertionError("reference positive case must be selectable")
    matrix=np.zeros((dim,dim),dtype=complex)
    for rho,mass in selected:
        matrix+=(mass/prob)*np.asarray(rho.data,dtype=complex)
    purity=float(np.trace(matrix@matrix).real)
    return prob,len(selected),matrix,purity

def check(rust:dict,req:dict,reference:tuple,label:str)->tuple[float,int]:
    prob,count,matrix,purity=reference
    if (rust["selected_conditions"]!=req["conditions"]
        or rust["selected_branch_count"]!=count
        or rust["final_density_dimension"]!=matrix.shape[0]
        or rust["qubits"]!=req["qubits"]
        or rust["classical_bits"]!=req["classical_bits"]):
        raise AssertionError(f"{label}: selected event identity/shape changed")
    maximum=0.0
    for key,ref in (
        ("condition_probability",prob),("trace",1.0),("purity",purity),
    ):
        d=abs(float(rust[key])-ref)
        if not math.isfinite(d) or d>TOL:
            raise AssertionError(f"{label}: {key} {d} vs {ref}")
        maximum=max(maximum,d)
    arr=rust["final_density_row_major_complex_pairs"]
    if len(arr)!=matrix.size:
        raise AssertionError(f"{label} full matrix truncated")
    for i,(pair,z) in enumerate(zip(arr,matrix.ravel(order="C"))):
        if len(pair)!=2:
            raise AssertionError("missing complex pair")
        for j,(actual_v,ref_v) in enumerate(
            ((pair[0],float(z.real)),(pair[1],float(z.imag)))
        ):
            delta=abs(actual_v-ref_v)
            if not math.isfinite(delta) or delta>TOL:
                raise AssertionError(
                    f"{label}/rho[{i}][{j}]: Rust {actual_v} vs Qiskit {ref_v}")
            maximum=max(maximum,delta)
    if len(rust["final_basis_probabilities"])!=matrix.shape[0]:
        raise AssertionError("incomplete diagonal populations")
    for i,p in enumerate(rust["final_basis_probabilities"]):
        d=abs(p-float(matrix[i,i].real))
        if not math.isfinite(d) or d>TOL:
            raise AssertionError(f"{label} basis {i} drift")
        maximum=max(maximum,d)
    return maximum,matrix.size

def deny(binary:Path,req:dict,code:str)->None:
    p=subprocess.run([str(binary.resolve())],input=json.dumps(req),
                     text=True,capture_output=True,timeout=30)
    if p.returncode!=2 or p.stdout.strip():
        raise AssertionError("invalid noisy conditional event returned stdout")
    err=json.loads(p.stderr)
    if (err["schema"]!=ERROR or err["code"]!=code
        or err["qpu_hardware_jobs"]!=0):
        raise AssertionError(f"bad typed rejected outcome {err}")

def candidates(branches:list,ops:list[dict])->list[list[dict]]:
    written=sorted({x["into_bit"] for x in ops if x["op"]=="measure"})
    choices=[]
    for bit in written:
        for val in (False,True):
            condition=[{"bit":bit,"equals":val}]
            mass=sum(p for _,bits,p in branches if bits[bit] is val)
            if mass>=PHYSICAL_FLOOR:
                choices.append(condition)
    if len(written)>=2:
        first,second=written[:2]
        for v0 in (False,True):
            for v1 in (False,True):
                condition=[{"bit":first,"equals":v0},
                           {"bit":second,"equals":v1}]
                mass=sum(p for _,bits,p in branches
                         if bits[first] is v0 and bits[second] is v1)
                if mass>=PHYSICAL_FLOOR:
                    choices.append(condition)
    return choices

def main(binary:Path,fixtures:Path,artifact:Path)->dict:
    installed={x:version(x) for x in PACKAGES}
    if installed!=PACKAGES or sys.version_info[:2]!=(3,11):
        raise RuntimeError(f"C-024 pinned Qiskit env mismatch {installed}")
    corpus=noisy_corpus(fixtures)
    # A genuinely asymmetric Born probability as a separate analytic case.
    asymmetric={"schema":"aura.quantum.noisy-ensemble-request/1",
        "qubits":1,"classical_bits":1,"max_memory_bytes":16*1024*1024,
        "instructions":[
            gate("h",0),gate("t",0),gate("h",0),
            {"op":"measure","qubit":0,"into_bit":0},
            noise("depolarizing",0,probability=0.75),
        ]}
    corpus.append(("hth-depolarized-postselection",asymmetric))
    report={
        "status":"Q28_NOISY_CONDITIONED_QISKIT_PASS",
        "source_circuits":0,
        "selected_events":0,
        "selected_physical_history_occurrences":0,
        "compared_full_complex_density_entries":0,
        "joint_two_bit_events":0,
        "mutated_density_controls_caught":0,
        "strict_invalid_controls":0,
        "max_abs_component_delta":0.0,
        "smallest_event_probability":1.0,
        "smallest_physical_branch":1.0,
        "roundoff_ghost_outcomes":0,
        "max_dense_qubits":0,
        "qpu_hardware_jobs":0,
        "oracle":"Qiskit-DensityMatrix-Kraus-plus-independent-Born-postselection",
        "tolerance":TOL,
        "packages":installed,"python":platform.python_version(),
    }
    for name,src in corpus:
        b,measures,ghosts,smallest=branches_qiskit(src)
        options=candidates(b,src["instructions"])
        if not options:
            continue
        report["source_circuits"]+=1
        report["roundoff_ghost_outcomes"]+=ghosts
        report["smallest_physical_branch"]=min(
            report["smallest_physical_branch"],smallest)
        for k,conditions in enumerate(options):
            request=convert(src,conditions)
            raw,rust=actual(binary,request)
            expected=choose(b,conditions,1<<src["qubits"])
            d,entries=check(rust,request,expected,f"{name}:{k}")
            if rust["measurement_count"]!=measures:
                raise AssertionError("measurement count mismatch")
            report["selected_events"]+=1
            report["compared_full_complex_density_entries"]+=entries
            report["selected_physical_history_occurrences"]+=expected[1]
            report["joint_two_bit_events"]+=int(len(conditions)==2)
            report["max_dense_qubits"]=max(report["max_dense_qubits"],
                                            src["qubits"])
            report["max_abs_component_delta"]=max(
                report["max_abs_component_delta"],d)
            report["smallest_event_probability"]=min(
                report["smallest_event_probability"],expected[0])
            corrupted=json.loads(json.dumps(rust))
            corrupted["final_density_row_major_complex_pairs"][0][0]+=0.05
            try:
                check(corrupted,request,expected,f"{name}:{k}:corrupted")
            except AssertionError:
                report["mutated_density_controls_caught"]+=1
            else:
                raise AssertionError("corrupted noisy postselection accepted")
            if name=="hth-depolarized-postselection":
                p0=(1+math.sqrt(0.5))/2
                expected_p=p0 if conditions[0]["equals"] is False else 1-p0
                if len(conditions)==1:
                    if abs(expected[0]-expected_p)>TOL:
                        raise AssertionError("HTH independent analytic Born discrepancy")
            if name=="real-noisy-feedback" and k==0:
                again,_=actual(binary,request)
                if again!=raw:
                    raise AssertionError("unseeded noisy postselection nondeterminism")
    base=convert(corpus[0][1],[{"bit":0,"equals":True}])
    negatives=[
        ({**base,"seed":5},"invalid_json"),
        ({**base,"shots":1024},"invalid_json"),
        ({**base,"hardware":True},"invalid_json"),
        ({**base,"qpu_backend":"device"},"invalid_json"),
        ({**base,"schema":"aura.quantum.noisy-ensemble-request/1"},
         "unsupported_schema"),
        ({**base,"qubits":5},"profile_limit"),
        ({**base,"max_memory_bytes":1},"circuit_rejected"),
        ({**base,"conditions":[]},"profile_limit"),
        ({**base,"conditions":[{"bit":0,"equals":False},
                                {"bit":0,"equals":True}]},"circuit_rejected"),
        ({**base,"conditions":[{"bit":3,"equals":True}]},"circuit_rejected"),
        ({**base,"instructions":[
            {"op":"measure","qubit":0,"into_bit":0},
            {"op":"reset_observed","qubit":0},
        ]},"circuit_rejected"),
        ({**base,"instructions":[
            {"op":"measure","qubit":0,"into_bit":0},
        ],"conditions":[{"bit":0,"equals":True}]},"circuit_rejected"),
    ]
    for req,code in negatives:
        deny(binary,req,code)
        report["strict_invalid_controls"]+=1
    malformed=json.loads(json.dumps(base))
    malformed["conditions"][0]["hardware"]=True
    deny(binary,malformed,"invalid_json")
    report["strict_invalid_controls"]+=1
    if (report["source_circuits"]<45 or report["selected_events"]<120
        or report["joint_two_bit_events"]<40
        or report["strict_invalid_controls"]!=13
        or report["mutated_density_controls_caught"]!=report["selected_events"]
        or report["max_dense_qubits"]!=4):
        raise AssertionError(f"C-024 scientific corpus incomplete: {report}")
    artifact.parent.mkdir(parents=True,exist_ok=True)
    artifact.write_text(json.dumps(report,sort_keys=True,indent=2)+"\n",
                        encoding="utf-8")
    print("Q28_NOISY_CONDITIONED_QISKIT "+
          json.dumps(report,sort_keys=True),flush=True)
    return report

if __name__=="__main__":
    if len(sys.argv)!=4:
        raise SystemExit("usage: check_qiskit_noisy_conditioned.py binary fixtures report")
    main(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
