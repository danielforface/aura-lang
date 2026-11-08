#!/usr/bin/env python3
"""C-021 REAL conditioned Rust CLI vs independent Qiskit final-register
postselection and partial trace. No stochastic sampling and no QPU calls.

Consumes the C-019 pure-DensityMatrix / Kraus oracle's UNNORMALIZED final
branches, not the Rust postselection or statevector.  Each selected event
uses rho_E=sum_{b in E} rho_b / sum_{b in E} Tr(rho_b).  Compare subsystem
partial traces, purity and conditional branch mixing on the real binary.
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
from check_qiskit_channel_moments import PKGS,TOL,corpus,reference
from check_qiskit_channel_tomography import (
    compare_density,expected_local,pairs_for,require_close,
)

REQUEST="aura.quantum.conditioned-tomography-request/1"
RESULT="aura.quantum.conditioned-tomography-result/1"
ERROR="aura.quantum.conditioned-tomography-error/1"


def conditioned_request(source:dict,pairs:list[list[int]],
                        predicates:list[dict])->dict:
    return {
        "schema":REQUEST,
        "qubits":source["qubits"],
        "classical_bits":source["classical_bits"],
        "max_memory_bytes":source["max_memory_bytes"],
        "instructions":source["instructions"],
        "pairs":pairs,
        "conditions":predicates,
    }


def run(binary:Path,req:dict)->tuple[bytes,dict]:
    proc=subprocess.run([str(binary.resolve())],
        input=json.dumps(req,sort_keys=True).encode(),
        capture_output=True,timeout=45,check=False)
    if proc.returncode or proc.stderr:
        raise AssertionError(
            f"REAL Rust conditioned tomography failed: {proc.returncode} "
            f"{proc.stderr[:800]}")
    result=json.loads(proc.stdout)
    if (result["schema"]!=RESULT or result["status"]!="simulated"
        or result["shots"]!=0 or result["qpu_hardware_jobs"]!=0
        or result["density_scope"]!=
           "conditioned_final_ideal_channel_reduced_subsystems"
        or result["sampling"]!="none;born_postselected_final_register"
        or result["condition_scope"]!=
           "final_explicitly_measured_classical_register"
        or result["history_output_policy"]!=
           "selected_only;no_unrequested_classical_histogram_or_trace"
        or result["max_qubits"]!=8 or result["max_branches"]!=256
        or result["basis_order"]!=
           "global_q0_lsb;pair_[a,b]_local_basis_bit_a+2*bit_b"):
        raise AssertionError("Rust conditioned physical/provenance contract drift")
    for forbidden in ("classical_probabilities","classical_register",
                      "histogram","trace","branches","results",
                      "total_branch_probability","unselected_probability"):
        if forbidden in result:
            raise AssertionError(f"Unrequested history exposed: {forbidden}")
    return proc.stdout,result


def select_reference(density_reference:dict,predicates:list[dict])->tuple[list,float]:
    selected=[]
    mass=0.0
    for rho,bits in density_reference["reference_density_branches"]:
        if all(bits[c["bit"]] is c["equals"] for c in predicates):
            selected.append((rho,bits))
            mass+=float(np.trace(rho).real)
    return selected,mass


def compare(actual:dict,src:dict,density_reference:dict,
            conditions:list[dict],pairs:list[list[int]],
            name:str)->tuple[float,int]:
    branch_list,prob=select_reference(density_reference,conditions)
    if prob<=0:
        raise AssertionError("reference must not test an impossible event as success")
    if (actual["selected_conditions"]!=conditions
        or actual["selected_branch_count"]!=len(branch_list)
        or actual["instrument_count"]!=density_reference["instrument_count"]
        or len(actual["single_wire_reductions"])!=src["qubits"]
        or len(actual["selected_pair_reductions"])!=len(pairs)):
        raise AssertionError(f"{name}: postselection count or requested predicates mismatch")
    maximum=require_close(actual["condition_probability"],prob,
                          f"{name}/BornConditionProbability")
    normalized=[(rho/prob,bits) for rho,bits in branch_list]
    entries=0
    for wire,row in enumerate(actual["single_wire_reductions"]):
        if row["wire"]!=wire:
            raise AssertionError("Wrong local wire order")
        expected=expected_local(normalized,src["qubits"],[wire])
        rho=expected["matrix"]
        bloch=[2*rho[0,1].real,-2*rho[0,1].imag,
               rho[0,0].real-rho[1,1].real]
        for i,(value,ref) in enumerate(zip(row["bloch_xyz"],bloch)):
            maximum=max(maximum,require_close(value,ref,
                                               f"{name}/Bloch q{wire}:{i}"))
        maximum=max(maximum,compare_density(
            row,expected,f"{name}/rho q{wire}"))
        entries+=4
    for index,row in enumerate(actual["selected_pair_reductions"]):
        pair=pairs[index]
        if row["wires"]!=pair:
            raise AssertionError(f"{name}: pair order mismatch")
        expected=expected_local(normalized,src["qubits"],pair)
        maximum=max(maximum,compare_density(
            row,expected,f"{name}/pair{pair}"))
        entries+=16
    return maximum,entries


def negative(binary:Path,req:dict,expected_code:str)->None:
    out=subprocess.run([str(binary.resolve())],
        input=json.dumps(req),text=True,capture_output=True,timeout=45)
    if out.returncode!=2 or out.stdout.strip():
        raise AssertionError("Rejected conditioned query emitted stdout")
    data=json.loads(out.stderr)
    if (data["schema"]!=ERROR or data["code"]!=expected_code
        or data["qpu_hardware_jobs"]!=0):
        raise AssertionError(f"Invalid typed rejection {data}")


def candidate_conditions(src:dict,density_reference:dict)->list[list[dict]]:
    written=sorted({op["into_bit"] for op in src["instructions"]
                    if op["op"]=="measure"})
    if not written:
        return []
    selections=[]
    for bit in written:
        for flag in (False,True):
            c=[{"bit":bit,"equals":flag}]
            _,mass=select_reference(density_reference,c)
            if mass>=1e-9:
                selections.append(c)
    if len(written)>=2:
        for a in (False,True):
            for b in (False,True):
                c=[{"bit":written[0],"equals":a},
                   {"bit":written[1],"equals":b}]
                _,mass=select_reference(density_reference,c)
                if mass>=1e-9:
                    selections.append(c)
    return selections


def main(binary:Path,fixtures:Path,output:Path)->dict:
    versions={k:version(k) for k in PKGS}
    if versions!=PKGS or sys.version_info[:2]!=(3,11):
        raise RuntimeError("C-021 requires pinned Qiskit 2.6.1/NumPy 2.3.4/Python3.11")
    report={
        "status":"Q28_CONDITIONED_TOMOGRAPHY_QISKIT_PASS",
        "source_cases":0,"selected_events":0,
        "selected_physical_histories":0,
        "compared_complex_density_entries":0,
        "selected_pair_reductions":0,
        "max_abs_delta":0.0,
        "minimum_event_probability":1.0,
        "mutation_controls_detected":0,
        "invalid_protocol_controls":0,
        "highest_qubit_width":0,
        "qpu_hardware_jobs":0,
        "oracle":"Qiskit DensityMatrix Kraus + final-register postselection + PartialTrace",
        "basis_order":"q0_is_LSB",
        "threshold":TOL,
        "packages":versions,"python":platform.python_version(),
    }
    examples=corpus(fixtures)
    for name,src in examples:
        independent=reference(src)
        variants=candidate_conditions(src,independent)
        if not variants:
            continue
        report["source_cases"]+=1
        pairs=pairs_for(src["qubits"])
        for k,predicates in enumerate(variants):
            req=conditioned_request(src,pairs,predicates)
            raw,actual=run(binary,req)
            deviation,entries=compare(actual,src,independent,
                                      predicates,pairs,f"{name}:{k}")
            report["selected_events"]+=1
            report["selected_physical_histories"]+=actual["selected_branch_count"]
            report["compared_complex_density_entries"]+=entries
            report["selected_pair_reductions"]+=len(pairs)
            report["highest_qubit_width"]=max(
                report["highest_qubit_width"],src["qubits"])
            report["max_abs_delta"]=max(report["max_abs_delta"],deviation)
            report["minimum_event_probability"]=min(
                report["minimum_event_probability"],
                actual["condition_probability"])
            poisoned=json.loads(json.dumps(actual))
            poisoned["single_wire_reductions"][0][
                "rho_row_major_complex_pairs"][0][0]+=0.02
            try:
                compare(poisoned,src,independent,predicates,pairs,
                        f"{name}:{k}:poison")
            except AssertionError:
                report["mutation_controls_detected"]+=1
            else:
                raise AssertionError("Poisoned selected-density result accepted")
            if name=="T_nonuniform":
                p0=(1.0+math.sqrt(0.5))/2.0
                predicted=p0 if predicates[0]["equals"] is False else 1.0-p0
                if len(predicates)==1 and predicates[0]["bit"]==0:
                    require_close(actual["condition_probability"],predicted,
                                  "HTH Born asymmetry")
            if name=="C018_real_Bell_feedback" and k==0:
                again,_=run(binary,req)
                if raw!=again:
                    raise AssertionError("Unseeded conditioned CLI nondeterminism")
    # Strict positive/negative user-facing CLI protocol and resource gates.
    bell=examples[0][1]
    base=conditioned_request(bell,[[0,1]],[{"bit":0,"equals":True}])
    for key,value in (("hardware",True),("shots",100),("seed",7),
                      ("qpu_backend","provider")):
        negative(binary,{**base,key:value},"invalid_json")
        report["invalid_protocol_controls"]+=1
    negative(binary,{**base,"schema":"aura.quantum.channel-tomography-request/1"},
             "unsupported_schema")
    report["invalid_protocol_controls"]+=1
    negative(binary,{**base,"conditions":[{"bit":0,"equals":True},
                                          {"bit":0,"equals":False}]},
             "circuit_rejected")
    report["invalid_protocol_controls"]+=1
    negative(binary,{**base,"conditions":[]},"profile_limit")
    report["invalid_protocol_controls"]+=1
    negative(binary,{**base,"conditions":[{"bit":1,"equals":True}]},
             "circuit_rejected")
    report["invalid_protocol_controls"]+=1
    deterministic=conditioned_request({
        "qubits":1,"classical_bits":1,"max_memory_bytes":16*1024*1024,
        "instructions":[{"op":"measure","qubit":0,"into_bit":0}],
    },[],[{"bit":0,"equals":True}])
    negative(binary,deterministic,"circuit_rejected")
    report["invalid_protocol_controls"]+=1
    negative(binary,{**base,"pairs":[[1,0]]},"circuit_rejected")
    report["invalid_protocol_controls"]+=1
    negative(binary,{**base,"qubits":9},"profile_limit")
    report["invalid_protocol_controls"]+=1
    if (report["source_cases"]<20 or report["selected_events"]<40
        or report["mutation_controls_detected"]!=report["selected_events"]
        or report["invalid_protocol_controls"]!=11
        or report["highest_qubit_width"]!=8):
        raise AssertionError(f"Truncated selected channel oracle {report}")
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_text(json.dumps(report,sort_keys=True,indent=2)+"\n",
                      encoding="utf-8")
    print("Q28_CONDITIONED_TOMOGRAPHY_QISKIT "+
          json.dumps(report,sort_keys=True),flush=True)
    return report


if __name__=="__main__":
    if len(sys.argv)!=4:
        raise SystemExit(
            "usage: check_qiskit_conditioned_tomography.py real_binary fixtures report")
    main(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
