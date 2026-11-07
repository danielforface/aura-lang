#!/usr/bin/env python3
"""Q28-010 independent Qiskit complex-amplitude parity for exact symbolic CLI.

Execute 64 random Clifford+T unitary circuits (1-4 qubits) and 16
projected, *subnormalized* dynamic branches. All root2/i128 coefficients
are transported via decimal strings, then evaluated only for comparison
against the independent complex128 Qiskit Statevector.
This is a bounded empirical test, not formal proof or hardware use.
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

PACKAGES = {"qiskit": "2.6.1", "numpy": "2.3.4"}
TOL = 4e-11
GATES = ("h", "x", "y", "z", "s", "t", "cx", "cz", "swap")


def gate(n: int, rng: random.Random) -> dict:
    kind = rng.choice(GATES if n > 1 else GATES[:6])
    a = rng.randrange(n)
    if kind in ("cx", "cz", "swap"):
        b = (a + 1 + rng.randrange(n - 1)) % n
        if kind == "swap":
            return {"kind": kind, "a": a, "b": b}
        return {"kind": kind, "control": a, "target": b}
    return {"kind": kind, "target": a}


def unitary(n: int, case: int) -> list:
    rng = random.Random(0xC010_2026 ^ (n * 10007 + case * 0x1F223))
    ops = []
    for q in range(n):
        ops.extend([
            {"op":"gate","gate":{"kind":"h","target":q}},
            {"op":"gate","gate":{"kind":"t","target":q}},
        ])
    for _ in range(31):
        ops.append({"op": "gate", "gate": gate(n, rng)})
    return ops


def branch_case(n: int, case: int) -> list:
    rng = random.Random(0xC010_9000 + n * 907 + case * 41)
    ops = []
    for q in range(n):
        ops.extend([
            {"op":"gate","gate":{"kind":"h","target":q}},
            {"op":"gate","gate":{"kind":"t","target":q}},
            {"op":"gate","gate":{"kind":"h","target":q}},
        ])
    for _ in range(6):
        ops.append({"op":"gate","gate":gate(n,rng)})
    # Construct a projective branch. Check nonzero support at each case in the
    # independent dense oracle rather than requiring arbitrary sampling.
    ops.append({"op":"measure","qubit":0,"into_bit":0,"outcome":True})
    ops.append({"op":"if","bit":0,"equals":True,
                "gate":{"kind":"x","target":min(n-1,1)}})
    return ops


def dense(n: int, ops: list) -> np.ndarray:
    state = Statevector.from_label("0" * n)
    register = [None] * 1
    for op in ops:
        if op["op"] == "measure":
            q = op["qubit"]
            outcome = op["outcome"]
            vals = np.asarray(state.data, dtype=np.complex128).copy()
            for i in range(len(vals)):
                if bool((i >> q) & 1) != outcome:
                    vals[i] = 0
            if not np.any(vals):
                raise AssertionError("synthetic postselection was impossible")
            state = Statevector(vals)
            register[op["into_bit"]] = outcome
            continue
        if op["op"] == "if":
            if register[op["bit"]] != op["equals"]:
                continue
        g = op["gate"]
        qc = QuantumCircuit(n)
        name = g["kind"]
        if name in ("cx", "cz"):
            getattr(qc, name)(g["control"], g["target"])
        elif name == "swap":
            qc.swap(g["a"], g["b"])
        else:
            getattr(qc, name)(g["target"])
        state = state.evolve(qc)
    return np.asarray(state.data, dtype=np.complex128)


def root2(spec: dict) -> float:
    return int(spec["rational"]) + int(spec["radical"]) * math.sqrt(2)


def exact_complex(entry: dict) -> complex:
    pair = entry["numerator"]
    power = entry["denominator_power_of_two"]
    return complex(root2(pair["real"]), root2(pair["imag"])) * 2.0 ** (-power)


def from_cli(binary: Path, n: int, ops: list) -> tuple[np.ndarray, dict]:
    request = {
        "schema": "aura.quantum.symbolic-request/1",
        "qubits": n, "classical_bits": 1,
        "max_memory_bytes": 16*1024*1024,
        "instructions": ops,
    }
    proc = subprocess.run([str(binary.resolve())], input=json.dumps(request),
        text=True,capture_output=True,timeout=20,check=False)
    if proc.returncode != 0 or proc.stderr.strip():
        raise AssertionError(f"symbolic Rust CLI failed: {proc.stderr}")
    output = json.loads(proc.stdout)
    if (output["schema"] != "aura.quantum.symbolic-result/1"
            or output["coefficient_encoding"] != "signed-i128-decimal-strings"
            or output["sampling"] != "none;forced-outcomes-only"
            or output["qpu_hardware_jobs"] != 0
            or output["evidence_class"] != "simulation"):
        raise AssertionError("incorrect symbolic contract or provenance")
    amps = np.asarray([exact_complex(a) for a in output["amplitudes"]])
    if len(amps) != 2**n or not np.all(np.isfinite(amps)):
        raise AssertionError("invalid symbolic state dimension")
    return amps, output


def verify(binary: Path, path: Path) -> None:
    actual_versions = {name: version(name) for name in PACKAGES}
    if actual_versions != PACKAGES or sys.version_info[:2] != (3,11):
        raise RuntimeError(f"Q28 symbolic oracle version mismatch: {actual_versions}")
    report = {
        "status":"SYMBOLIC_EXACT_RING_NUMERICAL_PARITY_PASS",
        "evidence_class":"simulation",
        "packages":actual_versions,
        "python":platform.python_version(),
        "qpu_hardware_jobs":0,
        "widths":[1,2,3,4],
        "unitary_cases":0,"projective_cases":0,
        "amplitudes_compared":0,
        "mutated_negative_controls":0,
        "max_complex_amplitude_delta":0.0,
        "max_subnormalized_mass_delta":0.0,
        "absolute_tolerance":TOL,
        "coefficient_encoding":"signed-i128-decimal-strings",
    }
    for n in range(1,5):
        for case in range(16):
            ops = unitary(n,case)
            expected = dense(n,ops)
            measured,out = from_cli(binary,n,ops)
            delta = float(np.max(np.abs(measured-expected)))
            if delta > TOL or not math.isfinite(delta):
                raise AssertionError(f"symbolic unitary mismatch n={n} c={case} delta={delta}")
            mass = float(np.sum(np.abs(measured)**2))
            if abs(mass-1.0) > TOL:
                raise AssertionError("symbolic unitary mass drift")
            report["unitary_cases"] += 1
            report["amplitudes_compared"] += len(expected)
            report["max_complex_amplitude_delta"] = max(report["max_complex_amplitude_delta"],delta)
            poisoned = measured.copy()
            poisoned[0] += 0.01
            if float(np.max(np.abs(poisoned-expected)))<=TOL:
                raise AssertionError("mutation control not detected")
            report["mutated_negative_controls"] += 1
        for case in range(4):
            ops = branch_case(n,case)
            expected = dense(n,ops)
            measured,out = from_cli(binary,n,ops)
            delta=float(np.max(np.abs(measured-expected)))
            actual_mass=float(np.sum(np.abs(measured)**2))
            original_mass=root2(out["final_joint_mass"]["numerator"])
            original_mass *= 2.0 ** (-out["final_joint_mass"]["denominator_power_of_two"])
            diff=abs(original_mass-actual_mass)
            if delta>TOL or diff>TOL:
                raise AssertionError(f"symbolic branch mismatch n={n} c={case}: {delta},{diff}")
            if len(out["trace"])!=1 or out["trace"][0]["outcome"] is not True:
                raise AssertionError("missing exact postselection trace")
            report["projective_cases"]+=1
            report["amplitudes_compared"]+=len(expected)
            report["max_complex_amplitude_delta"]=max(report["max_complex_amplitude_delta"],delta)
            report["max_subnormalized_mass_delta"]=max(report["max_subnormalized_mass_delta"],diff)
            poisoned=measured.copy()
            poisoned[0]+=0.01
            if float(np.max(np.abs(poisoned-expected)))<=TOL:
                raise AssertionError("branch mutation control not detected")
            report["mutated_negative_controls"]+=1
    if report["unitary_cases"]!=64 or report["projective_cases"]!=16 or report["amplitudes_compared"]!=600:
        raise AssertionError(f"incomplete symbolic oracle corpus {report}")
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(report,indent=2,sort_keys=True)+"\n")
    print("Q28_SYMBOLIC_PARITY "+json.dumps(report,sort_keys=True),flush=True)


if __name__=="__main__":
    if len(sys.argv)!=3:
        raise SystemExit("usage: check_qiskit_symbolic.py <rust-binary> <report.json>")
    verify(Path(sys.argv[1]),Path(sys.argv[2]))
