#!/usr/bin/env python3
"""C-022 science oracle: REAL Rust noisy-run full complex density vs Qiskit Kraus.

Existing noise engine is NOT reimplemented. This is a different physical
representation using Qiskit DensityMatrix.evolve(Kraus) and Qiskit Operator.
Qiskit conditions follow observed *real CLI trace*; branch probability is
checked independently, but RNG bitstream correctness is not claimed by this
comparison. Explicitly idealized parametric CPTP models, NO QPU.
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
from qiskit import QuantumCircuit
from qiskit.quantum_info import DensityMatrix, Kraus, Operator

PACKAGES={"qiskit":"2.6.1","numpy":"2.3.4"}
TOL=8e-11
SCHEMA="aura.quantum.noisy-request/1"
RESULT="aura.quantum.noisy-result/1"
ERR="aura.quantum.noisy-error/1"

I=np.eye(2,dtype=complex)
X=np.array([[0,1],[1,0]],dtype=complex)
Y=np.array([[0,-1j],[1j,0]],dtype=complex)
Z=np.diag([1,-1]).astype(complex)


def gate(kind:str,*wires:int)->dict:
    if kind in ("cx","cz"):
        return {"op":"gate","gate":{"kind":kind,"control":wires[0],
                                      "target":wires[1]}}
    if kind=="swap":
        return {"op":"gate","gate":{"kind":kind,"a":wires[0],"b":wires[1]}}
    return {"op":"gate","gate":{"kind":kind,"target":wires[0]}}


def noise(kind:str,wire:int,**params)->dict:
    return {"op":"noise","channel":{"kind":kind,"qubit":wire,**params}}


def request(n:int,ops:list[dict],seed:int=31,bits:int=0)->dict:
    return {"schema":SCHEMA,"qubits":n,"classical_bits":bits,
            "shots":1,"seed":seed,"max_memory_bytes":16777216,
            "inspect_final_density":True,"instructions":ops}


def channel_kraus(spec:dict)->Kraus:
    kind=spec["kind"]
    if kind=="phase_flip":
        p=spec["probability"]
        return Kraus([math.sqrt(1-p)*I,math.sqrt(p)*Z])
    if kind=="amplitude_damping":
        g=spec["gamma"]
        return Kraus([np.diag([1,math.sqrt(1-g)]).astype(complex),
                      np.array([[0,math.sqrt(g)],[0,0]],dtype=complex)])
    if kind=="phase_damping":
        g=spec["gamma"]
        return Kraus([np.diag([1,math.sqrt(1-g)]).astype(complex),
                      np.diag([0,math.sqrt(g)]).astype(complex)])
    if kind=="depolarizing":
        p=spec["probability"]
        probs=(1-p,p/3,p/3,p/3)
    elif kind=="pauli_mixture":
        px,py,pz=(spec[k] for k in ("p_x","p_y","p_z"))
        probs=(1-(px+py+pz),px,py,pz)
    else:
        raise AssertionError(f"unsupported oracle Kraus type {kind}")
    operators=[math.sqrt(p)*m for p,m in zip(probs,(I,X,Y,Z)) if p>0]
    if not operators:raise AssertionError("invalid zero-sum Kraus channel")
    return Kraus(operators)


def gate_operator(n:int,g:dict)->Operator:
    c=QuantumCircuit(n)
    k=g["kind"]
    if k in ("cx","cz"):
        getattr(c,k)(g["control"],g["target"])
    elif k=="swap":
        c.swap(g["a"],g["b"])
    else:
        getattr(c,k)(g["target"])
    return Operator(c)


def apply_action(state:DensityMatrix,operation:dict,n:int)->DensityMatrix:
    kind=operation["op"]
    if kind=="gate":
        return state.evolve(gate_operator(n,operation["gate"]))
    if kind=="noise":
        ch=operation["channel"]
        return state.evolve(channel_kraus(ch),qargs=[ch["qubit"]])
    raise AssertionError(f"unexpected action {kind}")


def reference(case:dict,rust:dict)->tuple[np.ndarray,float,int]:
    n=case["qubits"]
    state=DensityMatrix.from_int(0,2**n)
    bits=[None]*case["classical_bits"]
    trace=iter(rust["results"][0]["trace"])
    measured=0
    for at,op in enumerate(case["instructions"]):
        k=op["op"]
        if k in ("gate","noise"):
            state=apply_action(state,op,n)
        elif k in ("measure","reset_observed"):
            event=next(trace)
            if event["instruction"]!=at or event["kind"]!=k or event["qubit"]!=op["qubit"]:
                raise AssertionError("real Rust measurement provenance incorrect")
            value=bool(event["value"])
            dim=1<<n
            support=((np.arange(dim)>>op["qubit"])&1)==int(value)
            arr=np.asarray(state.data,dtype=complex)
            projected=arr*np.outer(support,support)
            prob=float(np.trace(projected).real)
            if prob<=1e-9 or prob>1+TOL:
                raise AssertionError("reported outcome impossible or below oracle resolution")
            state=DensityMatrix(projected/prob)
            if k=="measure":
                if event["into_bit"]!=op["into_bit"]:
                    raise AssertionError("wrong classical target in trace")
                bits[op["into_bit"]]=value
            elif value:
                state=state.evolve(gate_operator(n,{"kind":"x","target":op["qubit"]}))
            measured+=1
        elif k=="reset":
            event=next(trace)
            if event["instruction"]!=at or event["kind"]!="reset" \
               or event["qubit"]!=op["qubit"]:
                raise AssertionError("hidden reset should produce no revealed outcome")
            reset=Kraus([
                np.array([[1,0],[0,0]],dtype=complex),
                np.array([[0,1],[0,0]],dtype=complex),
            ])
            state=state.evolve(reset,qargs=[op["qubit"]])
            measured+=1
        elif k=="if":
            if bits[op["bit"]] is None:
                raise AssertionError("read before write in independent reference")
            if bits[op["bit"]] == op["equals"]:
                action=op["action"]
                state=apply_action(state,{"op":action["kind"],
                    **({"gate":action["gate"]} if action["kind"]=="gate"
                       else {"channel":action["channel"]})},n)
        else:raise AssertionError(f"invalid op {k}")
    if next(trace,None) is not None:raise AssertionError("excess observed Rust trace")
    if bits!=rust["results"][0]["bits"]:
        raise AssertionError("final classical bits inconsistent with reference")
    arr=np.asarray(state.data,dtype=complex)
    if np.max(np.abs(arr-arr.conj().T))>TOL or abs(np.trace(arr)-1)>TOL:
        raise AssertionError("Qiskit Kraus output invalid Hermiticity/trace")
    return arr,float(np.real(np.trace(arr@arr))),measured


def actual_run(binary:Path,req:dict)->tuple[bytes,dict]:
    proc=subprocess.run([str(binary.resolve())],
        input=json.dumps(req,sort_keys=True).encode(),
        capture_output=True,timeout=30,check=False)
    if proc.returncode or proc.stderr:
        raise AssertionError(
            f"real Rust noisy CLI failed exit={proc.returncode} {proc.stderr[:800]}")
    out=json.loads(proc.stdout)
    if out["schema"]!=RESULT or out["status"]!="simulated" \
       or out["evidence_class"]!="simulation" or out["qpu_hardware_jobs"]!=0 \
       or out["shots"]!=1 or out["qubits"]!=req["qubits"] \
       or out["seed"]!=req["seed"] or len(out["results"])!=1:
        raise AssertionError("wrong noisy CLI provenance")
    return proc.stdout,out


def compare(rust:dict,expected:np.ndarray,purity:float,label:str)->float:
    size=expected.shape[0]
    out=rust["final_density"]
    if out["dimension"]!=size or len(out["row_major_complex_pairs"])!=size*size:
        raise AssertionError(f"{label} wrong complete final density size")
    arr=np.asarray(out["row_major_complex_pairs"],dtype=np.float64)
    if arr.shape!=(size*size,2):
        raise AssertionError(f"{label} output not complex-pair matrix")
    z=arr[:,0]+1j*arr[:,1]
    maxdelta=float(np.max(np.abs(z.reshape(size,size)-expected)))
    if not np.isfinite(maxdelta) or maxdelta>TOL:
        raise AssertionError(f"{label} full complex Qiskit density mismatch {maxdelta}")
    for key,ref in [("trace",1.0),("purity",purity)]:
        d=abs(float(out[key])-ref)
        if not math.isfinite(d) or d>TOL:
            raise AssertionError(f"{label} {key} actual={out[key]} expected={ref}")
        maxdelta=max(maxdelta,d)
    for i,p in enumerate(out["basis_probabilities"]):
        d=abs(float(p)-float(expected[i,i].real))
        if not math.isfinite(d) or d>TOL:
            raise AssertionError(f"{label} Born basis {i} mismatch")
        maxdelta=max(maxdelta,d)
    return maxdelta


def negative(binary:Path,doc:dict,code:str)->None:
    proc=subprocess.run([str(binary.resolve())],
        input=json.dumps(doc),text=True,capture_output=True,timeout=30)
    if proc.returncode!=2 or proc.stdout.strip():
        raise AssertionError("invalid noisy CPTP input unexpectedly accepted")
    d=json.loads(proc.stderr)
    if d["schema"]!=ERR or d["code"]!=code or d["qpu_hardware_jobs"]!=0:
        raise AssertionError(f"wrong fail-closed error envelope {d}")


def corpus(fixtures:Path)->list[tuple[str,dict]]:
    cases=[("real_bell_three_channels",
            json.loads((fixtures/"noisy_cptp_channels_v1.json").read_text()))]
    extremes=[
        ("depolarizing",{"probability":0.0}),
        ("depolarizing",{"probability":0.75}),
        ("depolarizing",{"probability":1.0}),
        ("phase_damping",{"gamma":0.0}),
        ("phase_damping",{"gamma":0.75}),
        ("phase_damping",{"gamma":1.0}),
        ("pauli_mixture",{"p_x":0.0,"p_y":0.0,"p_z":0.0}),
        ("pauli_mixture",{"p_x":1.0,"p_y":0.0,"p_z":0.0}),
        ("pauli_mixture",{"p_x":0.0,"p_y":1.0,"p_z":0.0}),
        ("pauli_mixture",{"p_x":0.0,"p_y":0.0,"p_z":1.0}),
        ("pauli_mixture",{"p_x":0.2,"p_y":0.3,"p_z":0.1}),
    ]
    for i,(kind,params) in enumerate(extremes):
        cases.append((f"kraus_boundary_{i}",request(1,[
            gate("h",0),gate("s",0),noise(kind,0,**params)
        ],seed=i+4)))
    probs=[0.1,0.23,0.4,0.61,0.8]
    for n in range(1,5):
        for i in range(9):
            rng=random.Random(20261109+n*100+i)
            ops=[gate("h",0),gate("s",n-1)]
            if n>1:ops.append(gate("cx",0,n-1))
            for k in range(3):
                q=rng.randrange(n)
                nk=("phase_damping","depolarizing","pauli_mixture")[k]
                if nk=="phase_damping":
                    params={"gamma":probs[(i+k)%len(probs)]}
                elif nk=="depolarizing":
                    params={"probability":probs[(i+2*k)%len(probs)]}
                else:
                    params={"p_x":0.15,"p_y":0.20,"p_z":0.25}
                ops.extend([noise(nk,q,**params),
                            gate(("h","s","t","y")[rng.randrange(4)],q)])
            if n>1:
                ops.append(gate(("cz","swap","cx")[i%3],0,n-1))
            cases.append((f"nonclifford_{n}q_{i}",request(n,ops,seed=n*1000+i)))
    dynamic=[
        gate("h",0),gate("h",1),
        noise("phase_damping",1,gamma=0.49),
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"if","bit":0,"equals":True,"action":{
            "kind":"noise","channel":{"kind":"depolarizing",
                                       "qubit":1,"probability":0.75}}},
        noise("pauli_mixture",1,p_x=0.2,p_y=0.1,p_z=0.3),
        {"op":"reset","qubit":0},
        {"op":"measure","qubit":1,"into_bit":1},
        {"op":"if","bit":1,"equals":False,"action":{
            "kind":"noise","channel":{"kind":"phase_damping",
                                       "qubit":1,"gamma":0.8}}},
    ]
    for seed in range(16):
        cases.append((f"dynamic_conditional_{seed}",
                      request(2,dynamic,seed=seed+301,bits=2)))
    return cases


def main(binary:Path,fixtures:Path,reportfile:Path)->dict:
    installed={name:version(name) for name in PACKAGES}
    if installed!=PACKAGES or sys.version_info[:2]!=(3,11):
        raise RuntimeError(f"unqualified Qiskit environment {installed}")
    cases=corpus(fixtures)
    report={
        "status":"Q28_NOISY_CPTP_KRAUS_QISKIT_PASS",
        "cases":0,"full_complex_density_entries":0,
        "true_instrument_events":0,
        "mutated_density_controls_detected":0,
        "invalid_cptp_protocol_controls":0,
        "max_complex_purity_delta":0.0,
        "highest_dense_width":0,
        "qpu_hardware_jobs":0,
        "oracle":"Qiskit DensityMatrix.evolve(Kraus)+Operator",
        "probability_convention":"depolarizing_p_is_nonidentity_Pauli_probability",
        "threshold":TOL,
        "packages":installed,"python":platform.python_version(),
    }
    for name,req in cases:
        raw,rust=actual_run(binary,req)
        expected,purity,instruments=reference(req,rust)
        delta=compare(rust,expected,purity,name)
        report["cases"]+=1
        report["full_complex_density_entries"]+=expected.size
        report["true_instrument_events"]+=instruments
        report["highest_dense_width"]=max(report["highest_dense_width"],req["qubits"])
        report["max_complex_purity_delta"]=max(report["max_complex_purity_delta"],delta)
        poisoned=json.loads(json.dumps(rust))
        poisoned["final_density"]["row_major_complex_pairs"][0][0]+=0.025
        try:compare(poisoned,expected,purity,name+"_poison")
        except AssertionError:report["mutated_density_controls_detected"]+=1
        else:raise AssertionError("poisoned density passed independent Kraus gate")
        if name=="real_bell_three_channels":
            again,_=actual_run(binary,req)
            if raw!=again:raise AssertionError("seeded noisy result not reproducible")
    base=cases[0][1]
    for channel in [
        {"kind":"pauli_mixture","qubit":0,"p_x":0.6,"p_y":0.5,"p_z":0.0},
        {"kind":"pauli_mixture","qubit":0,"p_x":-0.1,"p_y":0.0,"p_z":0.0},
        {"kind":"depolarizing","qubit":0,"probability":1.1},
        {"kind":"phase_damping","qubit":0,"gamma":-0.1},
        {"kind":"phase_damping","qubit":99,"gamma":0.3},
    ]:
        poisoned=json.loads(json.dumps(base))
        poisoned["instructions"][2]["channel"]=channel
        negative(binary,poisoned,"circuit_rejected")
        report["invalid_cptp_protocol_controls"]+=1
    for key,value in [("hardware",True),("qpu_backend","provider")]:
        negative(binary,{**base,key:value},"invalid_json")
        report["invalid_cptp_protocol_controls"]+=1
    negative(binary,{**base,"schema":"aura.quantum.channel-tomography-request/1"},
             "unsupported_schema")
    report["invalid_cptp_protocol_controls"]+=1
    malformed=json.loads(json.dumps(base))
    malformed["instructions"][2]["channel"]["hardware"]="qpu"
    negative(binary,malformed,"invalid_json")
    report["invalid_cptp_protocol_controls"]+=1
    if (report["cases"]!=64 or report["mutated_density_controls_detected"]!=64
        or report["invalid_cptp_protocol_controls"]!=9
        or report["highest_dense_width"]!=4):
        raise AssertionError(f"incomplete noisy Kraus qualification {report}")
    reportfile.parent.mkdir(parents=True,exist_ok=True)
    reportfile.write_text(json.dumps(report,sort_keys=True,indent=2)+"\n",
                          encoding="utf-8")
    print("Q28_NOISY_CPTP_KRAUS_QISKIT "+
          json.dumps(report,sort_keys=True),flush=True)
    return report


if __name__=="__main__":
    if len(sys.argv)!=4:
        raise SystemExit("usage: check_qiskit_noisy_cptp.py noisy_binary fixtures report")
    main(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
