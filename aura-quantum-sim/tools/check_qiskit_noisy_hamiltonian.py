#!/usr/bin/env python3
"""C-026 independent real Rust mixed-noisy Pauli Hamiltonian Qiskit oracle.

Exact-in-model Qiskit DensityMatrix/Kraus + Z instrument branches are
independent of Rust's C-023/C-024 enumerator. Independently form H from
Qiskit SparsePauliOp with correct reversed (q0-first) word order, then
compute Tr(rho H), Tr(rho H@H), variance and every per-term expectation.
REAL compiled offline Rust CLI via subprocess, not a Rust function stub.
No quantum hardware, physical calibration, QIR ABI or sampled shots.
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
from qiskit.quantum_info import SparsePauliOp
from check_qiskit_noisy_cptp import PACKAGES,TOL,gate,noise
from check_qiskit_noisy_ensemble import corpus as noisy_corpus
from check_qiskit_noisy_conditioned import branches_qiskit

REQUEST="aura.quantum.noisy-hamiltonian-request/1"
RESULT="aura.quantum.noisy-hamiltonian-result/1"
ERROR="aura.quantum.noisy-hamiltonian-error/1"
NUMERIC_TOL=8e-10

def case(source:dict,terms:list[dict],bits:list[dict]|None)->dict:
    doc={
        "schema":REQUEST,"qubits":source["qubits"],
        "classical_bits":source["classical_bits"],
        "max_memory_bytes":source["max_memory_bytes"],
        "instructions":source["instructions"],
        "terms":terms,
    }
    if bits is not None:
        doc["conditions"]=bits
    return doc

def actual(binary:Path,request:dict)->tuple[bytes,dict]:
    p=subprocess.run([str(binary.resolve())],
        input=json.dumps(request,sort_keys=True).encode(),
        capture_output=True,timeout=40,check=False)
    if p.returncode or p.stderr:
        raise AssertionError(
            f"REAL Rust noisy Hamiltonian returned code={p.returncode} "
            f"stderr={p.stderr[:1000]}")
    r=json.loads(p.stdout)
    if (r["schema"]!=RESULT or r["status"]!="simulated"
        or r["expectation_mode"]!="analytic_noisy_mixed_channel_final"
        or r["variance_definition"]!="Tr(rho H^2)-Tr(rho H)^2;not_shot_variance"
        or r["pauli_word_order"]!="ASCII_q0_first;basis_q0_lsb"
        or r["sampling"]!="none;full_representable_Born_channel"
        or r["shots"]!=0 or r["qpu_hardware_jobs"]!=0
        or r["max_qubits"]!=4 or r["max_branches"]!=64
        or r["max_pauli_terms"]!=64
        or r["evidence_class"]!="simulation"
        or r["qubits"]!=request["qubits"]
        or r["classical_bits"]!=request["classical_bits"]
        or r["term_count"]!=len(request["terms"])):
        raise AssertionError("wrong real Rust CLI physics profile or provenance")
    if request.get("conditions") is None:
        for key in ("condition_probability","selected_branch_count",
                    "selected_conditions"):
            if key in r:
                raise AssertionError(f"unexpected selected event field {key}")
    elif r.get("selected_conditions")!=request["conditions"]:
        raise AssertionError("wrong selected classical register event")
    for forbidden in ("seed","histogram","classical_probabilities",
                      "results","instrument_trace","shot_trace",
                      "full_density_row_major_complex_pairs",
                      "unselected_probability","branch_weights"):
        if forbidden in r:
            raise AssertionError(f"unrequested private/source information {forbidden}")
    return p.stdout,r

def reference(branches:list,request:dict)->dict:
    n=request["qubits"]
    selection=request.get("conditions")
    selected=(branches if selection is None else [
        (rho,bits,p) for rho,bits,p in branches
        if all(bits[c["bit"]] is c["equals"] for c in selection)
    ])
    probability=sum(w for _,_,w in selected)
    if probability<=0:
        raise AssertionError("independent selected event must be positive")
    dim=1<<n
    rho=np.zeros((dim,dim),dtype=complex)
    for matrix,bits,mass in selected:
        rho+=(mass/probability)*np.asarray(matrix.data,dtype=complex)
    terms=request["terms"]
    labels=[(t["paulis"][::-1],float(t["coefficient"])) for t in terms]
    H=SparsePauliOp.from_list(labels).to_matrix()
    H2=H@H
    mu_complex=np.trace(rho@H)
    second_complex=np.trace(rho@H2)
    if abs(mu_complex.imag)>TOL or abs(second_complex.imag)>TOL:
        raise AssertionError("Qiskit Hamiltonian not Hermitian")
    mean=float(mu_complex.real)
    second=float(second_complex.real)
    var=second-mean**2
    if var< -TOL:
        raise AssertionError(f"independent physically impossible variance {var}")
    var=max(var,0.0)
    per_term=[]
    for t in terms:
        label=t["paulis"][::-1]
        bare=SparsePauliOp.from_list([(label,1.0)]).to_matrix()
        e=np.trace(rho@bare)
        if abs(e.imag)>TOL:
            raise AssertionError("independent Pauli expected real")
        per_term.append((float(e.real),float(t["coefficient"]*e.real)))
    if abs(np.trace(rho)-1.0)>TOL:
        raise AssertionError("independent density not normalized")
    return {
        "mean":mean,"second":second,"variance":var,
        "event_probability":probability,
        "selected_branches":len(selected),
        "terms":per_term,
    }

def verify(rust:dict,req:dict,ref:dict,label:str)->float:
    maximum=0.0
    for key,expected in [
        ("weighted_sum",ref["mean"]),
        ("weighted_second_moment",ref["second"]),
        ("weighted_variance",ref["variance"]),
    ]:
        got=float(rust[key])
        delta=abs(got-expected)
        # Two large squared moments may cancel; all test coefficients <= 3.
        if not math.isfinite(delta) or delta>NUMERIC_TOL:
            raise AssertionError(
                f"{label} {key}: Rust {got} vs Qiskit {expected}, δ={delta}")
        maximum=max(maximum,delta)
    if req.get("conditions") is not None:
        if rust["selected_branch_count"]!=ref["selected_branches"]:
            raise AssertionError("selected branch count mismatch")
        d=abs(rust["condition_probability"]-ref["event_probability"])
        if d>NUMERIC_TOL:
            raise AssertionError("wrong physical Born event probability")
        maximum=max(maximum,d)
    rterms=rust["terms"]
    if len(rterms)!=len(ref["terms"]):
        raise AssertionError("lost term estimates")
    for inp,actual,out in zip(req["terms"],rterms,ref["terms"]):
        if (actual["paulis"]!=inp["paulis"]
            or actual["coefficient"]!=inp["coefficient"]):
            raise AssertionError("term order/sign/coefficients mismatched")
        for key,expected in [
            ("expectation",out[0]),("contribution",out[1])
        ]:
            delta=abs(actual[key]-expected)
            if not math.isfinite(delta) or delta>NUMERIC_TOL:
                raise AssertionError(
                    f"{label} {key} {inp['paulis']} actual={actual[key]}"
                    f" expected={expected}, δ={delta}")
            maximum=max(maximum,delta)
    return maximum

def invalid(binary:Path,req:dict,expected:str)->None:
    p=subprocess.run([str(binary.resolve())],
        input=json.dumps(req),text=True,capture_output=True,
        timeout=25,check=False)
    if p.returncode!=2 or p.stdout.strip():
        raise AssertionError("fail-closed invalid request emitted result")
    e=json.loads(p.stderr)
    if e["schema"]!=ERROR or e["code"]!=expected or e["qpu_hardware_jobs"]!=0:
        raise AssertionError(f"invalid request wrong typed envelope {e}")

def term_corpus(n:int,index:int)->list[dict]:
    rng=random.Random(20261113+n*101+index)
    letters="IXYZ"
    terms=[{"paulis":"I"*n,"coefficient":0.125},
           {"paulis":"X"+"I"*(n-1),"coefficient":-0.32},
           {"paulis":"Y"+"I"*(n-1),"coefficient":0.41},
           {"paulis":"Z"+"I"*(n-1),"coefficient":0.7}]
    if n>=2:
        terms.extend([
            {"paulis":"XX"+"I"*(n-2),"coefficient":0.24},
            {"paulis":"ZZ"+"I"*(n-2),"coefficient":-0.56},
            {"paulis":"XY"+"I"*(n-2),"coefficient":0.17},
        ])
    terms.extend([
        {"paulis":"".join(rng.choice(letters) for _ in range(n)),
         "coefficient":rng.choice([-0.45,0.2,0.37,0.0])}
        for _ in range(3)
    ])
    return terms

def selections(branches:list,src:dict)->list[list[dict]|None]:
    chosen=[None]
    measured=sorted(set(op["into_bit"] for op in src["instructions"]
                        if op["op"]=="measure"))
    if measured:
        bit=measured[0]
        for value in (False,True):
            mass=sum(p for _,bits,p in branches if bits[bit] is value)
            if mass>1e-9:chosen.append([{"bit":bit,"equals":value}])
    if len(measured)>1:
        a,b=measured[:2]
        for va,vb in [(False,False),(False,True),
                      (True,False),(True,True)]:
            mass=sum(p for _,bits,p in branches
                     if bits[a] is va and bits[b] is vb)
            if mass>1e-9:
                chosen.append([
                    {"bit":a,"equals":va},
                    {"bit":b,"equals":vb},
                ])
    return chosen

def main(binary:Path,fixtures:Path,artifact:Path)->dict:
    versions={name:version(name) for name in PACKAGES}
    if versions!=PACKAGES or sys.version_info[:2]!=(3,11):
        raise RuntimeError(f"unqualified pinned Qiskit environment {versions}")
    sources=noisy_corpus(fixtures)
    special=json.loads((fixtures/"noisy_hamiltonian_v1.json").read_text())
    sources.append(("real-noisy-H-fixture",{
        key:special[key] for key in (
            "qubits","classical_bits","max_memory_bytes","instructions"
        )
    }))
    analytic=[
        ("XZ_anticommute",{
            "qubits":1,"classical_bits":0,
            "max_memory_bytes":16*1024*1024,"instructions":[]
        },[
            {"paulis":"X","coefficient":1.0},
            {"paulis":"Z","coefficient":1.0},
        ]),
        ("Bell_commuting_noncommuting",{
            "qubits":2,"classical_bits":0,
            "max_memory_bytes":16*1024*1024,
            "instructions":[gate("h",0),gate("cx",0,1)]
        },[
            {"paulis":"XX","coefficient":1.0},
            {"paulis":"ZZ","coefficient":1.0},
            {"paulis":"YY","coefficient":-0.2},
        ]),
        ("amplitude_damped_excited_Z",{
            "qubits":1,"classical_bits":0,
            "max_memory_bytes":16*1024*1024,
            "instructions":[gate("x",0),
                noise("amplitude_damping",0,gamma=0.25)]
        },[{"paulis":"Z","coefficient":1.0}]),
    ]
    for name,src,terms in analytic:
        sources.append((name,{**src,"oracle_terms":terms}))
    report={
        "status":"Q28_NOISY_HAMILTONIAN_QISKIT_PASS",
        "source_circuits":0,"real_binary_evaluations":0,
        "classically_postselected_events":0,"joint_two_bit_events":0,
        "compared_term_estimates":0,
        "independent_H2_checks":0,
        "mutated_variance_controls_rejected":0,
        "invalid_protocol_controls":0,
        "maximum_numeric_deviation":0.0,
        "minimum_selected_probability":1.0,
        "independent_quantum_instrument_roundoff_ghosts":0,
        "largest_dense_width":0,"qpu_hardware_jobs":0,
        "oracle":"Qiskit-DensityMatrix-Kraus-SparsePauliOp-Tr_rho_H2",
        "tolerance":NUMERIC_TOL,
        "packages":versions,"python":platform.python_version()
    }
    for k,(name,src) in enumerate(sources):
        branches,measures,ghosts,smallest=branches_qiskit(src)
        report["independent_quantum_instrument_roundoff_ghosts"]+=ghosts
        report["source_circuits"]+=1
        terms=src.get("oracle_terms") or (
            special["terms"] if name=="real-noisy-H-fixture"
            else term_corpus(src["qubits"],k)
        )
        for opts in selections(branches,src):
            req=case(src,terms,opts)
            raw,got=actual(binary,req)
            ref=reference(branches,req)
            deviation=verify(got,req,ref,f"{name}:{opts}")
            report["real_binary_evaluations"]+=1
            report["independent_H2_checks"]+=1
            report["compared_term_estimates"]+=len(terms)
            report["maximum_numeric_deviation"]=max(
                report["maximum_numeric_deviation"],deviation)
            report["largest_dense_width"]=max(
                report["largest_dense_width"],src["qubits"])
            if opts is not None:
                report["classically_postselected_events"]+=1
                report["joint_two_bit_events"]+=int(len(opts)==2)
                report["minimum_selected_probability"]=min(
                    report["minimum_selected_probability"],
                    ref["event_probability"])
            mutated=json.loads(json.dumps(got))
            mutated["weighted_variance"]+=0.025
            try:
                verify(mutated,req,ref,f"{name}:poison")
            except AssertionError:
                report["mutated_variance_controls_rejected"]+=1
            else:
                raise AssertionError("poisoned noisy Hamiltonian variance qualified")
            if name=="XZ_anticommute":
                if (abs(ref["mean"]-1)>TOL or abs(ref["second"]-2)>TOL
                    or abs(ref["variance"]-1)>TOL):
                    raise AssertionError("anticommuting XZ H2 boundary failed")
            if name=="amplitude_damped_excited_Z":
                if (abs(ref["mean"]+0.5)>TOL
                    or abs(ref["variance"]-0.75)>TOL):
                    raise AssertionError("amplitude damping mixed variance boundary")
            if name=="real-noisy-H-fixture" and opts is None:
                again,_=actual(binary,req)
                if raw!=again:
                    raise AssertionError("unseeded/noisy Hamiltonian should be reproducible")
    base=case(sources[0][1],term_corpus(sources[0][1]["qubits"],0),None)
    negative=[
        ({**base,"seed":12},"invalid_json"),
        ({**base,"shots":512},"invalid_json"),
        ({**base,"hardware":True},"invalid_json"),
        ({**base,"qpu_backend":"simulator-provider"},"invalid_json"),
        ({**base,"schema":"aura.quantum.moments-request/1"},
         "unsupported_schema"),
        ({**base,"qubits":5},"profile_limit"),
        ({**base,"terms":[]},"profile_limit"),
        ({**base,"terms":[{"paulis":"Q","coefficient":1.0}]},
         "circuit_rejected"),
        ({**base,"terms":[{"paulis":"I","coefficient":10001.0}]},
         "circuit_rejected"),
        ({**base,"terms":[{"paulis":"XXX","coefficient":0.3}]},
         "circuit_rejected"),
        ({**base,"max_memory_bytes":1},"circuit_rejected"),
        ({**base,"conditions":[]},"profile_limit"),
        ({**base,"conditions":[{"bit":7,"equals":True}]},
         "circuit_rejected"),
        ({**base,"instructions":[{"op":"reset_observed","qubit":0}]},
         "circuit_rejected"),
    ]
    for req,code in negative:
        invalid(binary,req,code)
        report["invalid_protocol_controls"]+=1
    corrupt=json.loads(json.dumps(base))
    corrupt["terms"][0]["provider"]="QPU"
    invalid(binary,corrupt,"invalid_json")
    report["invalid_protocol_controls"]+=1
    if (report["source_circuits"]<50
        or report["real_binary_evaluations"]<100
        or report["classically_postselected_events"]<60
        or report["joint_two_bit_events"]<30
        or report["invalid_protocol_controls"]!=15
        or report["mutated_variance_controls_rejected"]!=report["real_binary_evaluations"]
        or report["largest_dense_width"]!=4):
        raise AssertionError(f"C-026 oracle incomplete: {report}")
    artifact.parent.mkdir(parents=True,exist_ok=True)
    artifact.write_text(json.dumps(report,sort_keys=True,indent=2)+"\n",
                        encoding="utf-8")
    print("Q28_NOISY_HAMILTONIAN_QISKIT "+json.dumps(report,sort_keys=True),
          flush=True)
    return report

if __name__=="__main__":
    if len(sys.argv)!=4:
        raise SystemExit("usage: check_qiskit_noisy_hamiltonian.py binary fixtures report")
    main(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
