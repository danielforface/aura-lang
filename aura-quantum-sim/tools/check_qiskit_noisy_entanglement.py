#!/usr/bin/env python3
"""C-025 actual Rust offline NPT/pair analysis vs independent Qiskit+NumPy.

External Qiskit DensityMatrix + Kraus computes dynamic noisy branches without
calling the Rust density or pair reducer. Then Qiskit partial_trace produces
local 4x4 matrices and NumPy eigvalsh computes the PARTIAL TRANSPOSE
spectrum, trace norm, entropy and MI. This is numerical science, not QPU,
device noise calibration, or a formal universal separability certificate.
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
from qiskit.quantum_info import DensityMatrix,partial_trace
from check_qiskit_noisy_cptp import PACKAGES,TOL,gate,noise
from check_qiskit_noisy_ensemble import corpus as channel_corpus
from check_qiskit_noisy_conditioned import branches_qiskit,choose

REQUEST="aura.quantum.noisy-pair-entanglement-request/1"
RESULT="aura.quantum.noisy-pair-entanglement-result/1"
ERROR="aura.quantum.noisy-pair-entanglement-error/1"
NUMERIC_TOL=1e-9
NPT_THRESHOLD=1e-10


def request(source:dict,pair:list[int],conditions:list[dict]|None)->dict:
    doc={
        "schema":REQUEST,"qubits":source["qubits"],
        "classical_bits":source["classical_bits"],
        "max_memory_bytes":source["max_memory_bytes"],
        "instructions":source["instructions"],
        "selected_pair":pair,
    }
    if conditions is not None:
        doc["conditions"]=conditions
    return doc


def run(binary:Path,req:dict)->tuple[bytes,dict]:
    p=subprocess.run([str(binary.resolve())],
        input=json.dumps(req,sort_keys=True).encode(),
        capture_output=True,timeout=45,check=False)
    if p.returncode or p.stderr:
        raise AssertionError(f"REAL Rust entanglement CLI failed code={p.returncode}"
                             f" {p.stderr[:700]}")
    out=json.loads(p.stdout)
    if (out["schema"]!=RESULT or out["status"]!="simulated"
        or out["sampling"]!="none;exact_in_model_noisy_ensemble"
        or out["profile"]!="noisy-two-qubit-ppt-density-f64/1"
        or out["shots"]!=0 or out["qpu_hardware_jobs"]!=0
        or out["evidence_class"]!="simulation"
        or out["entanglement_scope"]!="two_wire_reduced_mixed_state_PPT_NPT_witness"
        or out["basis_order"]!="global_q0_lsb;local_pair_a_plus_2b"
        or out["max_qubits"]!=4 or out["max_branches"]!=64
        or abs(float(out["ppt_witness_numeric_threshold"])-NPT_THRESHOLD)>1e-18):
        raise AssertionError("Rust entanglement output profile/provenance drift")
    for forbidden in (
        "seed","classical_probabilities","histogram","results",
        "trace_events","instrument_trace","shot_trace",
        "unselected_probability","full_density_row_major_complex_pairs",
        "branch_weights","shots_samples",
    ):
        if forbidden in out:
            raise AssertionError(f"Unrequested information leak: {forbidden}")
    if req.get("conditions") is None:
        if any(k in out for k in ("selected_conditions","condition_probability",
                                "selected_branch_count")):
            raise AssertionError("Unconditional request leaked selection data")
    elif out.get("selected_conditions")!=req["conditions"]:
        raise AssertionError("Final register selection was changed")
    return p.stdout,out


def entropy(matrix:np.ndarray)->float:
    eig=np.linalg.eigvalsh(matrix)
    if np.min(eig)<-NUMERIC_TOL or abs(np.sum(eig)-1)>NUMERIC_TOL:
        raise AssertionError("Independent Qiskit density eigenvalues invalid")
    return float(-sum(v*math.log2(v) for v in eig if v>0))


def reference(branches:list,n:int,pair:list[int],
              conditions:list[dict]|None)->dict:
    if conditions is None:
        selected=branches
    else:
        selected=[(rho,bits,p) for rho,bits,p in branches
                  if all(bits[c["bit"]] is c["equals"] for c in conditions)]
    probability=sum(w for _,_,w in selected)
    if probability<=0:
        raise AssertionError("oracle expected a physical chosen event")
    full=np.zeros((1<<n,1<<n),dtype=complex)
    for rho,bits,p in selected:
        full+=(p/probability)*np.asarray(rho.data,dtype=complex)
    complement=[wire for wire in range(n) if wire not in pair]
    local=np.asarray(partial_trace(DensityMatrix(full),complement).data,
                     dtype=complex)
    if local.shape!=(4,4):
        raise AssertionError(f"Qiskit partial trace incorrect shape {local.shape}")
    # Explicit q0 LSB second-wire partial transpose:
    # local[(a,b),(c,d)] -> local[(a,d),(c,b)].
    pt=np.zeros((4,4),dtype=complex)
    for r in range(4):
        for c in range(4):
            pt[r,c]=local[(r&1)|(c&2),(c&1)|(r&2)]
    spectrum=np.linalg.eigvalsh(pt)
    negativity=float(-sum(v for v in spectrum if v < -NPT_THRESHOLD))
    s_pair=entropy(local)
    s_a=entropy(np.asarray(
        partial_trace(DensityMatrix(local),[1]).data,dtype=complex))
    s_b=entropy(np.asarray(
        partial_trace(DensityMatrix(local),[0]).data,dtype=complex))
    mi=s_a+s_b-s_pair
    if mi<-NUMERIC_TOL:
        raise AssertionError("Numerically violated mutual information nonnegativity")
    one_a=np.asarray(partial_trace(DensityMatrix(local),[1]).data)
    one_b=np.asarray(partial_trace(DensityMatrix(local),[0]).data)
    return {
        "rho":local,"eig":spectrum,"negativity":negativity,
        "log_negativity":math.log2(1+2*negativity),
        "mi_bits":max(0.0,mi),
        "purity":float(np.trace(local@local).real),
        "single_purity":[float(np.trace(one_a@one_a).real),
                         float(np.trace(one_b@one_b).real)],
        "npt":bool(spectrum[0]<-NPT_THRESHOLD),
        "selection_probability":probability,
        "selected_histories":len(selected),
    }


def compare(out:dict,req:dict,e:dict,name:str)->tuple[float,int]:
    if out["selected_pair"]!=req["selected_pair"]:
        raise AssertionError(f"{name}: wrong selected pair")
    if req.get("conditions") is None:
        if out.get("condition_probability") is not None:
            raise AssertionError("unconditional Born leak")
    else:
        if out["selected_branch_count"]!=e["selected_histories"]:
            raise AssertionError(f"{name}: selected history count mismatch")
    maximum=0.0
    fields=[
        ("negativity",e["negativity"]),
        ("logarithmic_negativity_bits",e["log_negativity"]),
        ("mutual_information_bits",e["mi_bits"]),
        ("pair_purity",e["purity"]),
    ]
    if req.get("conditions") is not None:
        fields.append(("condition_probability",e["selection_probability"]))
    for key,expected in fields:
        got=float(out[key])
        delta=abs(got-expected)
        if not math.isfinite(delta) or delta>NUMERIC_TOL:
            raise AssertionError(
                f"{name}: {key} Rust={got} Qiskit={expected}, delta={delta}")
        maximum=max(maximum,delta)
    if out["npt_entanglement_witness_detected"]!=e["npt"]:
        raise AssertionError(f"{name}: NPT witness classification drift")
    entries=out["reduced_density_row_major_complex_pairs"]
    if len(entries)!=16:
        raise AssertionError("truncated reduced density matrix")
    for i,(pair,z) in enumerate(zip(entries,e["rho"].ravel(order="C"))):
        if len(pair)!=2:
            raise AssertionError("malformed local rho element")
        for a,b in ((pair[0],float(z.real)),(pair[1],float(z.imag))):
            delta=abs(a-b)
            if not math.isfinite(delta) or delta>NUMERIC_TOL:
                raise AssertionError(f"{name}: rho[{i}] {delta}")
            maximum=max(maximum,delta)
    for i,(actual,expected) in enumerate(
        zip(out["partial_transpose_eigenvalues_ascending"],e["eig"])
    ):
        d=abs(actual-expected)
        if not math.isfinite(d) or d>NUMERIC_TOL:
            raise AssertionError(f"{name}: partial transpose eig[{i}] drift {d}")
        maximum=max(maximum,d)
    for got,expected in zip(out["single_wire_purities"],e["single_purity"]):
        delta=abs(got-expected)
        if not math.isfinite(delta) or delta>NUMERIC_TOL:
            raise AssertionError(f"{name}: single-wire purity delta={delta}")
        maximum=max(maximum,delta)
    return maximum,16


def deny(binary:Path,req:dict,code:str)->None:
    p=subprocess.run([str(binary.resolve())],input=json.dumps(req),
                     text=True,capture_output=True,timeout=30)
    if p.returncode!=2 or p.stdout.strip():
        raise AssertionError("invalid entanglement query emitted result")
    e=json.loads(p.stderr)
    if e["schema"]!=ERROR or e["code"]!=code or e["qpu_hardware_jobs"]!=0:
        raise AssertionError(f"typed error envelope broken {e}")


def additions(fixtures:Path)->list[tuple[str,dict,list[int],list[dict]|None]]:
    out=[]
    base={
        "qubits":2,"classical_bits":0,"max_memory_bytes":16*1024*1024,
        "instructions":[gate("h",0),gate("cx",0,1)],
    }
    out.append(("analytic-bell",base,[0,1],None))
    out.append(("analytic-dephased-bell",{
        **base,"instructions":base["instructions"]+
        [noise("phase_damping",0,gamma=1.0)]},[0,1],None))
    out.append(("analytic-depolarized-bell",{
        **base,"instructions":base["instructions"]+
        [noise("depolarizing",0,probability=0.75)]},[0,1],None))
    out.append(("analytic-half-amplitude-bell",{
        **base,"instructions":base["instructions"]+
        [noise("amplitude_damping",1,gamma=0.5)]},[0,1],None))
    out.append(("complex-local-unitaries",{
        **base,"instructions":base["instructions"]+
        [gate("s",0),gate("t",1),gate("h",1)]},[0,1],None))
    ghz={
        "qubits":4,"classical_bits":0,"max_memory_bytes":16*1024*1024,
        "instructions":[gate("h",0),gate("cx",0,1),
                        gate("cx",1,2),gate("cx",2,3)],
    }
    out.append(("4q-ghz-nonadjacent",ghz,[0,3],None))
    fixture=json.loads((fixtures/"noisy_pair_entanglement_v1.json").read_text())
    src={k:fixture[k] for k in ("qubits","classical_bits",
                                "max_memory_bytes","instructions")}
    out.append(("fixture-selected-clean-bell",src,[0,1],
                [{"bit":0,"equals":False}]))
    out.append(("fixture-selected-dephased-bell",src,[0,1],
                [{"bit":0,"equals":True}]))
    out.append(("fixture-unconditional-mixed-bell",src,[0,1],None))
    return out


def main(binary:Path,fixtures:Path,artifact:Path)->dict:
    versions={name:version(name) for name in PACKAGES}
    if versions!=PACKAGES or sys.version_info[:2]!=(3,11):
        raise RuntimeError(f"unqualified Qiskit oracle environment {versions}")
    report={
        "status":"Q28_NOISY_PAIR_NPT_QISKIT_PASS",
        "source_circuits":0,"real_cli_analyses":0,
        "compared_reduced_complex_density_entries":0,
        "compared_pt_spectrum_eigenvalues":0,
        "selected_event_cases":0,
        "mutated_negativity_controls_caught":0,
        "invalid_protocol_controls":0,
        "max_abs_component_delta":0.0,
        "max_dense_qubits":0,
        "smallest_selected_event_probability":1.0,
        "roundoff_ghost_outcomes":0,
        "qpu_hardware_jobs":0,
        "oracle":"Qiskit DensityMatrix/Kraus/PartialTrace + NumPy eigvalsh/PT",
        "tolerance":NUMERIC_TOL,
        "packages":versions,"python":platform.python_version(),
    }
    all_cases=[]
    for name,src in channel_corpus(fixtures):
        if src["qubits"]<2:
            continue
        n=src["qubits"]
        pairs=[[0,1]]
        if n>=3:pairs.append([0,n-1])
        all_cases.append((name,src,pairs))
    for name,src,pair,conditions in additions(fixtures):
        all_cases.append((name,src,[pair]))
    for name,src,pairs in all_cases:
        branches,measures,ghosts,min_physical=branches_qiskit(src)
        report["roundoff_ghost_outcomes"]+=ghosts
        report["source_circuits"]+=1
        opts=[None]
        written=sorted({op["into_bit"] for op in src["instructions"]
                        if op["op"]=="measure"})
        if written:
            # For fixed analytic examples, select the event specified by its
            # label; for generated cases, check unconditional plus selected
            # true and two-bit postselection.
            opts.append([{"bit":written[0],"equals":True}])
            if len(written)>1:
                opts.append([{"bit":written[0],"equals":True},
                             {"bit":written[1],"equals":False}])
        if name.startswith("fixture-selected-"):
            opts=[[{"bit":0,"equals":
                name=="fixture-selected-dephased-bell"}]]
        elif name.startswith("fixture-unconditional-"):
            opts=[None]
        for pair in pairs:
            for conditions in opts:
                if conditions is not None:
                    selected_mass=sum(p for _,bits,p in branches if all(
                        bits[c["bit"]] is c["equals"] for c in conditions))
                    if selected_mass<=1e-9:
                        continue
                req=request(src,pair,conditions)
                raw,out=run(binary,req)
                expected=reference(branches,src["qubits"],pair,conditions)
                delta,count=compare(out,req,expected,f"{name} {pair} {conditions}")
                report["real_cli_analyses"]+=1
                report["compared_reduced_complex_density_entries"]+=count
                report["compared_pt_spectrum_eigenvalues"]+=4
                report["selected_event_cases"]+=int(conditions is not None)
                report["max_abs_component_delta"]=max(
                    report["max_abs_component_delta"],delta)
                report["max_dense_qubits"]=max(report["max_dense_qubits"],
                                                 src["qubits"])
                if conditions is not None:
                    report["smallest_selected_event_probability"]=min(
                        report["smallest_selected_event_probability"],
                        expected["selection_probability"])
                corrupted=json.loads(json.dumps(out))
                corrupted["negativity"]+=0.02
                try:compare(corrupted,req,expected,name+" corrupted")
                except AssertionError:
                    report["mutated_negativity_controls_caught"]+=1
                else:
                    raise AssertionError("poisoned output passed science gate")
                if name=="analytic-bell":
                    if abs(expected["negativity"]-0.5)>TOL \
                       or abs(expected["mi_bits"]-2.0)>TOL:
                        raise AssertionError("Bell analytical boundary mismatch")
                if name=="analytic-dephased-bell":
                    if abs(expected["negativity"])>TOL \
                       or abs(expected["mi_bits"]-1.0)>TOL:
                        raise AssertionError("classical/quantum MI distinction invalid")
                if name=="analytic-half-amplitude-bell":
                    if abs(expected["negativity"]-0.25)>TOL:
                        raise AssertionError("mixed amplitude damping NPT boundary")
                if name=="4q-ghz-nonadjacent":
                    if abs(expected["negativity"])>TOL \
                       or abs(expected["mi_bits"]-1.0)>TOL:
                        raise AssertionError("GHZ pair marginal quantum/classical confusion")
                if name=="fixture-selected-clean-bell":
                    if abs(expected["negativity"]-0.5)>TOL:
                        raise AssertionError("condition false should preserve Bell")
                if name=="fixture-selected-dephased-bell":
                    if abs(expected["negativity"])>TOL:
                        raise AssertionError("condition true should dephase Bell")
                if name=="analytic-bell":
                    again,_=run(binary,req)
                    if raw!=again:
                        raise AssertionError("unseeded entanglement CLI not deterministic")
    b=additions(fixtures)[-3]
    req=request(b[1],[0,1],[{"bit":0,"equals":False}])
    invalid=[
        ({**req,"seed":3},"invalid_json"),
        ({**req,"shots":2},"invalid_json"),
        ({**req,"hardware":True},"invalid_json"),
        ({**req,"qpu_backend":"provider"},"invalid_json"),
        ({**req,"schema":"aura.quantum.noisy-conditioned-request/1"},
         "unsupported_schema"),
        ({**req,"qubits":5},"profile_limit"),
        ({**req,"selected_pair":[1,0]},"circuit_rejected"),
        ({**req,"selected_pair":[1,1]},"circuit_rejected"),
        ({**req,"selected_pair":[0,1,2]},"invalid_json"),
        ({**req,"conditions":[]},"profile_limit"),
        ({**req,"conditions":[{"bit":0,"equals":True},
                               {"bit":0,"equals":False}]},"circuit_rejected"),
        ({**req,"conditions":[{"bit":2,"equals":True}]},"circuit_rejected"),
        ({**req,"max_memory_bytes":1},"circuit_rejected"),
        ({**req,"instructions":[{"op":"reset_observed","qubit":0}]},
         "circuit_rejected"),
    ]
    for invalid_req,code in invalid:
        deny(binary,invalid_req,code)
        report["invalid_protocol_controls"]+=1
    if (report["source_circuits"]<40
        or report["real_cli_analyses"]<150
        or report["selected_event_cases"]<80
        or report["mutated_negativity_controls_caught"]!=report["real_cli_analyses"]
        or report["invalid_protocol_controls"]!=14
        or report["max_dense_qubits"]!=4):
        raise AssertionError(f"incomplete C025 physical oracle {report}")
    artifact.parent.mkdir(parents=True,exist_ok=True)
    artifact.write_text(json.dumps(report,sort_keys=True,indent=2)+"\n",
                        encoding="utf-8")
    print("Q28_NOISY_PAIR_NPT_QISKIT "+json.dumps(report,sort_keys=True),
          flush=True)
    return report


if __name__=="__main__":
    if len(sys.argv)!=4:
        raise SystemExit("usage: check_qiskit_noisy_entanglement.py binary fixture report")
    main(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
