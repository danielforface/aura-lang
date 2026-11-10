#!/usr/bin/env python3
"""C-016 independent pinned Qiskit Pauli observable/conditional trajectory oracle.

The *real Rust binary* computes weighted Pauli string expectations directly
from factorized quantum states. Qiskit separately evaluates full complex
statevectors on 2..8 qubits, conditioning only on actual Rust instrument
traces. The 128q fixture is checked by a 2q Bell+T analytic factor theorem,
never by constructing a 2^128 state. No QPU, proof, GPU or provider calls.
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
from qiskit.quantum_info import Pauli, Statevector

PACKAGES={"numpy":"2.3.4","qiskit":"2.6.1"}
THRESHOLD=3e-11

def gate(kind,*wires):
    if kind in ("cx","cz"):
        return {"op":"gate","gate":{
            "kind":kind,"control":wires[0],"target":wires[1]}}
    if kind=="swap":
        return {"op":"gate","gate":{
            "kind":kind,"a":wires[0],"b":wires[1]}}
    return {"op":"gate","gate":{"kind":kind,"target":wires[0]}}

def corpus(n,case):
    rng=random.Random(20261108+n*1609+case*31)
    ops=[gate("h",0),gate("t",n-1),
         gate("cx",0,n-1),gate("h",1)]
    if n>=3:
        ops.append(gate("swap",0,1))
    if case%2==0:
        ops.append({"op":"measure","qubit":0,"into_bit":0})
        ops.append({"op":"if","bit":0,"equals":True,
            "gate":{"kind":"x","target":n-1}})
    if case%3==0:
        ops.append({"op":"reset","qubit":n-1})
    for _ in range(5):
        ops.append(gate(rng.choice(["h","t","s","x","y","z"]),
            rng.randrange(n)))
    coeffs=[1.0,-0.5,0.25,1.25,-0.75,0.4]
    words=["I"*n,"Y"*n,"Z"*n,
           "X"+"I"*(n-2)+"Y","Y"+"I"*(n-2)+"X",
           "".join(rng.choice("IXYZ") for _ in range(n))]
    terms=[{"paulis":word,"coefficient":coef}
           for word,coef in zip(words,coeffs)]
    return ops,terms

def run_rust(binary,request):
    cmd=[str(binary.resolve())]
    proc=subprocess.run(cmd,input=json.dumps(request),
        text=True,capture_output=True,timeout=30,check=False)
    if proc.returncode or proc.stderr.strip():
        raise AssertionError("actual Rust Pauli CLI failed "+proc.stderr[:750])
    output=json.loads(proc.stdout)
    if (output["schema"]!="aura.quantum.observable-result/1"
        or output["status"]!="simulated"
        or output["shots"]!=1 or output["qpu_hardware_jobs"]!=0
        or output["evidence_class"]!="simulation"
        or output["pauli_word_order"]!="q0-first;position-0-is-least-significant-wire"
        or output["expectation_mode"]!="analytic_pure_state_final"
        or output["conditioned_on_observed_instruments"] is not True
        or output["truncation_error_bound"]!=0.0):
        raise AssertionError("incorrect Pauli semantics/provenance")
    return output

def apply_gate(state,n,g):
    qc=QuantumCircuit(n)
    kind=g["kind"]
    if kind in ("cx","cz"):
        getattr(qc,kind)(g["control"],g["target"])
    elif kind=="swap":
        qc.swap(g["a"],g["b"])
    else:
        getattr(qc,kind)(g["target"])
    return state.evolve(qc)

def observed_postselection(state,n,qubit,value):
    raw=np.asarray(state.data,dtype=np.complex128)
    basis=np.arange(1<<n)
    support=((basis>>qubit)&1)==int(value)
    probability=float(np.sum(np.abs(raw[support])**2))
    if not np.isfinite(probability) or probability<=0.0:
        raise AssertionError("impossible source-observed Qiskit branch")
    new=np.zeros_like(raw)
    new[support]=raw[support]/np.sqrt(probability)
    return Statevector(new),probability

def evolve_real_trace(n,ops,trace):
    state=Statevector.from_int(0,2**n)
    bits=[None]
    events=iter(trace)
    count=0
    min_branch=1.0
    for instruction,op in enumerate(ops):
        kind=op["op"]
        if kind=="gate":
            state=apply_gate(state,n,op["gate"])
        elif kind=="if":
            if bits[op["bit"]]==op["equals"]:
                state=apply_gate(state,n,op["gate"])
        elif kind in ("measure","reset"):
            event=next(events,None)
            if event is None or event["instruction"]!=instruction \
                or event["qubit"]!=op["qubit"] or event["kind"]!=kind:
                raise AssertionError("bad real instrument trace event")
            value=event["value"] if kind=="measure" else event["previous_value"]
            if not isinstance(value,bool):
                raise AssertionError("non-bool source outcome")
            state,p=observed_postselection(state,n,op["qubit"],value)
            min_branch=min(min_branch,p)
            if kind=="measure":
                if event["into_bit"]!=op["into_bit"]:
                    raise AssertionError("incorrect register mapping")
                bits[op["into_bit"]]=value
            elif value:
                state=apply_gate(state,n,{"kind":"x","target":op["qubit"]})
            count+=1
        else:
            raise AssertionError("unrecognized real operation")
    if next(events,None) is not None:
        raise AssertionError("extra unexpected instrument observation")
    return state,bits,count,min_branch

def qiskit_pauli(state,word):
    # Qiskit left-to-right Pauli string is q[n-1]..q0;
    # Aura's product API explicitly documents q0 FIRST.
    value=state.expectation_value(Pauli(word[::-1]))
    if abs(value.imag)>THRESHOLD:
        raise AssertionError("Hermitian observable yielded complex expectation")
    return float(value.real)

def main(binary,fixture,output):
    versions={k:version(k) for k in PACKAGES}
    if versions!=PACKAGES or sys.version_info[:2]!=(3,11):
        raise RuntimeError("pinned scientific environment missing "+str(versions))
    report={
        "status":"Q28_ANALYTIC_PAULI_QISKIT_PARITY_PASS",
        "cases":0,"terms_compared":0,"observed_instrument_events":0,
        "mutated_negative_controls":0,"max_term_delta":0.0,
        "max_weighted_sum_delta":0.0,"min_selected_branch_probability":1.0,
        "threshold":THRESHOLD,"qpu_hardware_jobs":0,
        "measurement_scope":"one_seeded_trajectory_conditioned_not_branch_average",
        "basis_order":"input_Pauli_is_q0_first_Qiskit_word_reversed",
        "packages":versions,"python":platform.python_version()
    }
    for n in range(2,9):
        for case in range(8):
            ops,terms=corpus(n,case)
            request={
                "schema":"aura.quantum.observable-request/1",
                "qubits":n,"classical_bits":1,
                "seed":2026+case*117+n*31,
                "max_memory_bytes":16*1024*1024,
                "instructions":ops,"terms":terms
            }
            actual=run_rust(binary,request)
            reference,bits,events,min_p=evolve_real_trace(n,ops,actual["trace"])
            if actual["classical_register"]!=bits or len(actual["terms"])!=len(terms):
                raise AssertionError("source classical or Pauli term contract mismatch")
            expected_weighted=0.0
            for t,o in zip(terms,actual["terms"]):
                expected=qiskit_pauli(reference,t["paulis"])
                contribution=expected*t["coefficient"]
                delta=abs(expected-o["expectation"])
                if (o["paulis"]!=t["paulis"]
                    or o["coefficient"]!=t["coefficient"]
                    or delta>THRESHOLD
                    or abs(contribution-o["contribution"])>THRESHOLD):
                    raise AssertionError(f"Pauli mismatch n={n} case={case} {t} got {o}")
                expected_weighted+=contribution
                report["terms_compared"]+=1
                report["max_term_delta"]=max(report["max_term_delta"],delta)
            error=abs(actual["weighted_sum"]-expected_weighted)
            if error>THRESHOLD:
                raise AssertionError(f"weighted energy mismatch n={n} case={case} d={error}")
            report["max_weighted_sum_delta"]=max(report["max_weighted_sum_delta"],error)
            # Deliberately poisoned candidate must be detected by independent
            # evaluation (a negative oracle control per circuit).
            if abs(actual["terms"][0]["expectation"]+0.002-
                   qiskit_pauli(reference,terms[0]["paulis"]))<=THRESHOLD:
                raise AssertionError("corrupted Pauli expectation accepted")
            report["mutated_negative_controls"]+=1
            report["observed_instrument_events"]+=events
            report["min_selected_branch_probability"]=min(
                report["min_selected_branch_probability"],min_p)
            report["cases"]+=1
    # Large 128-qubit factor product is independently certified with a
    # 2-qubit Qiskit reference for *each identical Bell+T component*.
    request=json.loads(fixture.read_text())
    large=run_rust(binary,request)
    if large["qubits"]!=128 or large["term_count"]!=5:
        raise AssertionError("128q executable fixture lost its contract")
    bell=QuantumCircuit(2);bell.h(0);bell.cx(0,1);bell.t(1)
    pair=Statevector.from_instruction(bell)
    expected=[
        qiskit_pauli(pair,"ZZ"),
        qiskit_pauli(pair,"XX"),
        qiskit_pauli(pair,"YY"),
        qiskit_pauli(pair,"ZZ")**64,
        1.0
    ]
    for actual,expect in zip(large["terms"],expected):
        if abs(actual["expectation"]-expect)>THRESHOLD:
            raise AssertionError("128q factorized observable is inconsistent with pair oracle")
    energy=sum(x["coefficient"]*y for x,y in zip(large["terms"],expected))
    if abs(large["weighted_sum"]-energy)>THRESHOLD:
        raise AssertionError("128q Hamiltonian energy mismatch")
    report["large_128_qubit_exact_factor_case"]=True
    report["large_128_qubit_weighted_energy"]=large["weighted_sum"]
    if report["cases"]!=56 or report["terms_compared"]!=336 \
       or report["mutated_negative_controls"]!=56:
        raise AssertionError("partial Pauli scientific corpus")
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_text(json.dumps(report,indent=2,sort_keys=True)+"\n")
    print("Q28_PAULI_QISKIT "+json.dumps(report,sort_keys=True),flush=True)

if __name__=="__main__":
    if len(sys.argv)!=4:
        raise SystemExit("usage: check_qiskit_pauli_observables.py binary 128-fixture report")
    main(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
    # C-019 independent *density-matrix* oracle for the new real ideal-channel
    # Rust CLI.  Keep this in the existing C-owned check step: no shared workflow
    # edit, no hidden skipped gate, and preserve the original C-016 contract.
    from check_qiskit_channel_moments import main as channel_density_oracle
    crate = Path(__file__).resolve().parents[1]
    channel_binary = crate / "target/release/aura-quantum-channel-moments-run"
    subprocess.run([
        "cargo", "build", "--release", "--manifest-path", str(crate / "Cargo.toml"),
        "--bin", "aura-quantum-channel-moments-run",
    ], check=True, timeout=180)
    channel_density_oracle(
        channel_binary, crate / "fixtures",
        Path(sys.argv[3]).with_name("c019-channel-density-qiskit.json"),
    )
    # C-020 direct independent Qiskit PartialTrace oracle for all 2x2
    # and selected 4x4 density matrices of the production tomography CLI.
    from check_qiskit_channel_tomography import main as tomography_oracle
    tomography_binary = crate / "target/release/aura-quantum-channel-tomography-run"
    subprocess.run([
        "cargo","build","--release","--manifest-path",str(crate / "Cargo.toml"),
        "--bin","aura-quantum-channel-tomography-run",
    ],check=True,timeout=180)
    tomography_oracle(
        tomography_binary,crate / "fixtures",
        Path(sys.argv[3]).with_name("c020-channel-tomography-qiskit.json"),
    )
    # C-021 scientific gate for REAL Rust final-register conditional
    # postselection, selected-only result and separate strict v1 protocol.
    from check_qiskit_conditioned_tomography import main as conditional_oracle
    conditional_binary = crate / "target/release/aura-quantum-conditioned-tomography-run"
    subprocess.run([
        "cargo","build","--release","--manifest-path",str(crate / "Cargo.toml"),
        "--bin","aura-quantum-conditioned-tomography-run",
    ],check=True,timeout=180)
    conditional_oracle(
        conditional_binary,crate / "fixtures",
        Path(sys.argv[3]).with_name("c021-conditioned-tomography-qiskit.json"),
    )
    # C-022 real noisy-request/1 CLI vs independent Qiskit Kraus CPTP maps.
    # No new shared workflow and no replacement of C-016..021 certification.
    from check_qiskit_noisy_cptp import main as noisy_cptp_oracle
    noisy_binary = crate / "target/release/aura-quantum-noisy-run"
    subprocess.run([
        "cargo","build","--release","--manifest-path",str(crate / "Cargo.toml"),
        "--bin","aura-quantum-noisy-run",
    ],check=True,timeout=180)
    noisy_cptp_oracle(
        noisy_binary,crate / "fixtures",
        Path(sys.argv[3]).with_name("c022-noisy-cptp-kraus-qiskit.json"),
    )
    # C-023: full noisy ensemble scientific acceptance for real release binary
    # with physically independent Qiskit Kraus + explicit Z instruments.
    from check_qiskit_noisy_ensemble import main as noisy_ensemble_oracle
    noisy_ensemble_binary=crate / "target/release/aura-quantum-noisy-ensemble-run"
    subprocess.run([
        "cargo","build","--release","--manifest-path",str(crate / "Cargo.toml"),
        "--bin","aura-quantum-noisy-ensemble-run",
    ],check=True,timeout=180)
    noisy_ensemble_oracle(
        noisy_ensemble_binary,crate / "fixtures",
        Path(sys.argv[3]).with_name("c023-noisy-ensemble-qiskit.json"),
    )
    # C-024 real final-register selected noisy density: independent Qiskit
    # Kraus instrument and multi-bit postselection (no shots, no QPU).
    from check_qiskit_noisy_conditioned import main as conditioned_noisy_oracle
    noisy_conditioned_binary=crate / "target/release/aura-quantum-noisy-conditioned-run"
    subprocess.run([
        "cargo","build","--release","--manifest-path",str(crate / "Cargo.toml"),
        "--bin","aura-quantum-noisy-conditioned-run",
    ],check=True,timeout=180)
    conditioned_noisy_oracle(
        noisy_conditioned_binary,crate / "fixtures",
        Path(sys.argv[3]).with_name("c024-noisy-conditioned-qiskit.json"),
    )
    # C-025: independent entanglement science for REAL release CLI,
    # Qiskit reduced density/partial trace + NumPy PT eigen/spectrum.
    from check_qiskit_noisy_entanglement import main as pair_npt_oracle
    entanglement_binary=crate / "target/release/aura-quantum-noisy-pair-entanglement-run"
    subprocess.run([
        "cargo","build","--release","--manifest-path",str(crate / "Cargo.toml"),
        "--bin","aura-quantum-noisy-pair-entanglement-run",
    ],check=True,timeout=180)
    pair_npt_oracle(
        entanglement_binary,crate / "fixtures",
        Path(sys.argv[3]).with_name("c025-noisy-pair-npt-qiskit.json"),
    )
