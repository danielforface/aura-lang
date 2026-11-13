#!/usr/bin/env python3
"""C-020 independent Qiskit partial-trace oracle for REAL Rust tomography CLI.

Reuses C-019's independently certified *density-matrix* ideal instrument
evolution, NOT Rust statevector trajectory.  Compares all 2x2 single-wire and
selected 4x4 pair density entries, Bloch XYZ, local purities and the branch
mixing gap.  Runs full real CLI JSON with negative/poisoned controls.
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
from qiskit.quantum_info import DensityMatrix, partial_trace
from check_qiskit_channel_moments import (
    PKGS, TOL, corpus, reference,
)

SCHEMA = "aura.quantum.channel-tomography-request/1"
RESULT = "aura.quantum.channel-tomography-result/1"
ERROR = "aura.quantum.channel-tomography-error/1"


def pairs_for(n: int) -> list[list[int]]:
    if n == 1:
        return []
    pairs = [[0, 1]]
    if n >= 3:
        pairs.append([0, n - 1])
    return pairs


def request(source: dict, selected_pairs: list[list[int]]) -> dict:
    return {
        "schema": SCHEMA, "qubits": source["qubits"],
        "classical_bits": source["classical_bits"],
        "max_memory_bytes": source["max_memory_bytes"],
        "instructions": source["instructions"],
        "pairs": selected_pairs,
    }


def call(binary: Path, body: dict) -> tuple[bytes, dict]:
    p = subprocess.run([str(binary.resolve())],
                       input=json.dumps(body, sort_keys=True).encode(),
                       capture_output=True, timeout=45, check=False)
    if p.returncode != 0 or p.stderr:
        raise AssertionError(
            f"real tomography CLI failed {p.returncode}: {p.stderr[:500]}")
    result = json.loads(p.stdout)
    if (result["schema"] != RESULT or result["status"] != "simulated"
        or result["density_scope"] !=
            "unconditioned_final_ideal_channel_reduced_subsystems"
        or result["measurement_outcome_policy"] !=
            "reset_hidden;explicit_measure_writes_classical"
        or result["sampling"] != "none;all_representable_instrument_branches"
        or result["shots"] != 0 or result["qpu_hardware_jobs"] != 0
        or result["evidence_class"] != "simulation"
        or result["max_qubits"] != 8
        or result["max_requested_pairs"] != 16
        or result["basis_order"] !=
            "global_q0_lsb;pair_[a,b]_local_basis_bit_a+2*bit_b"):
        raise AssertionError("Rust tomography provenance/contract mismatch")
    return p.stdout, result


def subdensity(rho: np.ndarray, n: int, selected: list[int]) -> np.ndarray:
    # Qiskit returns retained subsystems in increasing qubit order, so the
    # 2-wire state basis is q[a] as local LSB and q[b] as the next bit.
    traced = [q for q in range(n) if q not in selected]
    return np.asarray(partial_trace(DensityMatrix(rho), traced).data,
                      dtype=np.complex128)


def expected_local(
    branches: list[tuple[np.ndarray, list]], n: int, selected: list[int],
) -> dict:
    rho = sum((branch for branch, _ in branches),
              np.zeros((1 << n, 1 << n), dtype=np.complex128))
    local = subdensity(rho, n, selected)
    purity = float(np.trace(local @ local).real)
    mean_purity = 0.0
    for branch, _bits in branches:
        weight = float(np.trace(branch).real)
        conditional = subdensity(branch / weight, n, selected)
        mean_purity += weight * float(
            np.trace(conditional @ conditional).real)
    return {
        "matrix": local,
        "purity": purity,
        "mean_branch_purity": mean_purity,
        "branch_mixing_gap": mean_purity - purity,
    }


def require_close(actual: float, expected: float, label: str) -> float:
    delta = abs(float(actual) - float(expected))
    if not math.isfinite(delta) or delta > TOL * max(1.0, abs(expected)):
        raise AssertionError(
            f"tomography independent Qiskit mismatch {label}: "
            f"actual={actual}, expected={expected}, delta={delta}")
    return delta


def compare(actual: dict, expected: dict, n: int,
            selected_pairs: list[list[int]], name: str) -> tuple[float, int]:
    if (actual["final_branch_count"] != expected["final_branch_count"]
        or actual["instrument_count"] != expected["instrument_count"]
        or set(actual["classical_probabilities"]) !=
            set(expected["classical_probabilities"])):
        raise AssertionError(f"{name}: wrong instrument/branch keys")
    maximum = require_close(
        actual["total_branch_probability"],
        expected["numbers"]["total_branch_probability"],
        f"{name}/mass")
    for key,value in expected["classical_probabilities"].items():
        maximum = max(maximum, require_close(
            actual["classical_probabilities"][key],value,
            f"{name}/classical[{key}]"))
    if (len(actual["single_wire_reductions"])!=n
        or len(actual["selected_pair_reductions"])!=len(selected_pairs)):
        raise AssertionError(f"{name}: invalid number of marginals")
    branches = expected["reference_density_branches"]
    compared_entries = 0
    for wire, row in enumerate(actual["single_wire_reductions"]):
        if row["wire"] != wire:
            raise AssertionError(f"{name}: local wire order incorrect")
        oracle = expected_local(branches,n,[wire])
        rho=oracle["matrix"]
        if rho.shape!=(2,2):
            raise AssertionError("Qiskit single reduced shape")
        bloch=[2*rho[0,1].real,-2*rho[0,1].imag,
               rho[0,0].real-rho[1,1].real]
        for k,(a,e) in enumerate(zip(row["bloch_xyz"],bloch)):
            maximum=max(maximum,require_close(a,e,f"{name}/q{wire}/bloch{k}"))
        compared_entries+=4
        maximum=max(maximum,compare_density(row,oracle,f"{name}/q{wire}"))
    for j,row in enumerate(actual["selected_pair_reductions"]):
        selected=selected_pairs[j]
        if row["wires"]!=selected:
            raise AssertionError(f"{name}: pair ordering incorrect")
        oracle=expected_local(branches,n,selected)
        if oracle["matrix"].shape!=(4,4):
            raise AssertionError("Qiskit pair reduced shape")
        compared_entries+=16
        maximum=max(maximum,compare_density(row,oracle,f"{name}/pair{selected}"))
    return maximum,compared_entries


def compare_density(row: dict, oracle: dict, name: str) -> float:
    m=oracle["matrix"]
    actual=row["rho_row_major_complex_pairs"]
    if len(actual)!=m.size:
        raise AssertionError(f"{name} wrong matrix length")
    maxdiff=0.0
    for i,(seen,expected) in enumerate(zip(actual,m.ravel(order="C"))):
        if len(seen)!=2:
            raise AssertionError(f"{name} entry[{i}] not complex pair")
        maxdiff=max(maxdiff,require_close(
            seen[0],expected.real,f"{name}/rho[{i}].real"))
        maxdiff=max(maxdiff,require_close(
            seen[1],expected.imag,f"{name}/rho[{i}].imag"))
    for field in ("purity","mean_branch_purity","branch_mixing_gap"):
        maxdiff=max(maxdiff,require_close(
            row[field],oracle[field],f"{name}/{field}"))
    return maxdiff


def reject(binary:Path,req:dict,code:str)->None:
    p=subprocess.run([str(binary.resolve())],input=json.dumps(req),
                     text=True,capture_output=True,timeout=30)
    if p.returncode!=2 or p.stdout.strip():
        raise AssertionError("invalid tomography request did not fail closed")
    e=json.loads(p.stderr)
    if e["schema"]!=ERROR or e["code"]!=code or e["qpu_hardware_jobs"]!=0:
        raise AssertionError("tomography wrong error schema or simulated hardware")


def main(binary:Path,fixtures:Path,report_path:Path)->dict:
    versions={name:version(name) for name in PKGS}
    if versions!=PKGS or sys.version_info[:2]!=(3,11):
        raise RuntimeError("un-pinned C-020 scientific environment")
    suite=corpus(fixtures)
    report={
        "status":"Q28_CHANNEL_TOMOGRAPHY_QISKIT_PASS",
        "cases":0,"reduced_density_complex_entries":0,
        "real_pair_reductions":0,
        "mutated_negative_controls":0,
        "invalid_protocol_controls":0,
        "max_complex_or_purity_delta":0.0,
        "minimum_admitted_branch_probability":1.0,
        "highest_width":0,
        "qpu_hardware_jobs":0,
        "independent_reference":"Qiskit-DensityMatrix-PartialTrace-Kraus",
        "threshold":TOL,
        "packages":versions,"python":platform.python_version(),
    }
    for name,source in suite:
        selected=pairs_for(source["qubits"])
        req=request(source,selected)
        raw,actual=call(binary,req)
        oracle=reference(source)
        delta,entries=compare(actual,oracle,source["qubits"],selected,name)
        report["cases"]+=1
        report["reduced_density_complex_entries"]+=entries
        report["real_pair_reductions"]+=len(selected)
        report["highest_width"]=max(report["highest_width"],source["qubits"])
        report["max_complex_or_purity_delta"]=max(
            report["max_complex_or_purity_delta"],delta)
        report["minimum_admitted_branch_probability"]=min(
            report["minimum_admitted_branch_probability"],
            oracle["minimum_admitted_branch_probability"])
        # Mutate a true computed complex matrix entry: independent Qiskit
        # oracle must catch this, not self-compare the output to itself.
        mutated=json.loads(json.dumps(actual))
        mutated["single_wire_reductions"][0]["rho_row_major_complex_pairs"][0][0]+=0.02
        try:
            compare(mutated,oracle,source["qubits"],selected,name+"_poison")
        except AssertionError:
            report["mutated_negative_controls"]+=1
        else:
            raise AssertionError("tomography poisoned density accepted")
        if name in ("T_nonuniform","hidden_reset","Y_dephasing"):
            again,_=call(binary,req)
            if raw!=again:raise AssertionError("tomography no-seed replay differs")
    negative=request(suite[0][1],[[0,1]])
    for field,value in [
        ("hardware",True),("seed",1),("shots",100),
        ("qpu_backend","hardware")
    ]:
        reject(binary,{**negative,field:value},"invalid_json")
        report["invalid_protocol_controls"]+=1
    reject(binary,{**negative,"schema":"aura.quantum.channel-moments-request/1"},
           "unsupported_schema")
    report["invalid_protocol_controls"]+=1
    reject(binary,{**negative,"pairs":[[1,0]]},"circuit_rejected")
    report["invalid_protocol_controls"]+=1
    reject(binary,{**negative,"qubits":9},"profile_limit")
    report["invalid_protocol_controls"]+=1
    if (report["cases"]!=50 or report["mutated_negative_controls"]!=50
        or report["invalid_protocol_controls"]!=7):
        raise AssertionError("incomplete C-020 external scientific oracle")
    report_path.parent.mkdir(parents=True,exist_ok=True)
    report_path.write_text(
        json.dumps(report,sort_keys=True,indent=2)+"\n",encoding="utf-8")
    print("Q28_CHANNEL_TOMOGRAPHY_QISKIT "+
          json.dumps(report,sort_keys=True),flush=True)
    return report


if __name__=="__main__":
    if len(sys.argv)!=4:
        raise SystemExit("usage: check_qiskit_channel_tomography.py binary fixtures report")
    main(Path(sys.argv[1]),Path(sys.argv[2]),Path(sys.argv[3]))
