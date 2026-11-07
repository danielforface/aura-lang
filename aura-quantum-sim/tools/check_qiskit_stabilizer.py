#!/usr/bin/env python3
"""Independent pinned Qiskit Clifford oracle against real Rust binary outputs.

48 circuits across 1..6 qubits, 1008 full-basis Born probabilities;
48 explicit corrupted-oracle negative controls. Simulation, not QPU or proof.
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

EXPECTED = {"qiskit": "2.6.1", "numpy": "2.3.4"}
GATES = ("h", "x", "y", "z", "s", "cx", "cz", "swap")
TOL = 2e-11


def program(n, case):
    rng = random.Random(0xC009_0000 + 1009 * n + 37 * case)
    ops = []
    for q in range(n):
        ops.extend([
            {"op": "gate", "gate": {"kind": "h", "target": q}},
            {"op": "gate", "gate": {"kind": "s", "target": q}},
        ])
    for step in range(42):
        name = GATES[(case + step + rng.randrange(len(GATES))) % len(GATES)]
        if n == 1 and name in ("cx", "cz", "swap"):
            name = "h" if step % 2 == 0 else "s"
        q = rng.randrange(n)
        if name in ("cx", "cz"):
            other = (q + 1 + rng.randrange(n - 1)) % n
            gate = {"kind": name, "control": q, "target": other}
        elif name == "swap":
            other = (q + 1 + rng.randrange(n - 1)) % n
            gate = {"kind": name, "a": q, "b": other}
        else:
            gate = {"kind": name, "target": q}
        ops.append({"op": "gate", "gate": gate})
    return ops


def qiskit_probs(n, ops):
    qc = QuantumCircuit(n)
    for op in ops:
        g = op["gate"]
        name = g["kind"]
        if name in ("cx", "cz"):
            getattr(qc, name)(g["control"], g["target"])
        elif name == "swap":
            qc.swap(g["a"], g["b"])
        else:
            getattr(qc, name)(g["target"])
    return np.asarray(Statevector.from_instruction(qc).probabilities(), dtype=np.float64)


def rust_probs(binary, n, case, ops):
    request = {
        "schema": "aura.quantum.stabilizer-request/1",
        "qubits": n, "classical_bits": 0,
        "seed": 1000*n + case, "shots": 1,
        "max_memory_bytes": 16 * 1024 * 1024,
        "inspect_generators": True,
        "inspect_probabilities": True, "instructions": ops,
    }
    proc = subprocess.run(
        [str(binary.resolve())], input=json.dumps(request),
        text=True, capture_output=True, timeout=30, check=False,
    )
    if proc.returncode != 0 or proc.stderr.strip():
        raise AssertionError(f"Rust CLI failed n={n},case={case}: {proc.stderr}")
    out = json.loads(proc.stdout)
    if (out["schema"] != "aura.quantum.stabilizer-result/1"
            or out["precision"] != "exact-binary-Clifford-tableau"
            or out["evidence_class"] != "simulation"
            or out["qpu_hardware_jobs"] != 0
            or len(out["stabilizer_generators"]) != n):
        raise AssertionError("Stabilizer result/provenance incompatible")
    for label in out["stabilizer_generators"]:
        if len(label) != n + 1 or label[0] not in "+-" or not set(label[1:]) <= set("IXYZ"):
            raise AssertionError("Malformed signed stabilizer generator")
    result = np.asarray(out["basis_probabilities"], dtype=np.float64)
    if len(result) != 1 << n or not np.all(np.isfinite(result)):
        raise AssertionError("Wrong basis probability dimensions")
    return result


def main(binary, report):
    packages = {name: version(name) for name in EXPECTED}
    if packages != EXPECTED or sys.version_info[:2] != (3, 11):
        raise RuntimeError(f"unpinned oracle: packages={packages} py={sys.version}")
    result = {
        "status": "STABILIZER_NUMERICAL_PARITY_PASS",
        "evidence_class": "simulation",
        "qpu_hardware_jobs": 0,
        "packages": packages, "python": platform.python_version(),
        "q0_lsb": True, "cases": 0, "probabilities": 0,
        "mutation_negative_controls": 0, "max_abs_difference": 0.0,
        "max_norm_residual": 0.0, "tolerance": TOL,
    }
    for n in range(1, 7):
        for case in range(8):
            ops = program(n, case)
            expected = qiskit_probs(n, ops)
            actual = rust_probs(binary, n, case, ops)
            delta = float(np.max(np.abs(actual - expected)))
            residual = abs(float(actual.sum()) - 1.0)
            if not math.isfinite(delta) or delta > TOL or residual > TOL:
                raise AssertionError(
                    f"n={n},case={case}: delta={delta}, residual={residual}"
                )
            poisoned = actual.copy()
            poisoned[0] += 0.01
            if float(np.max(np.abs(poisoned - expected))) <= TOL:
                raise AssertionError("mutation control falsely accepted")
            result["cases"] += 1
            result["probabilities"] += len(actual)
            result["mutation_negative_controls"] += 1
            result["max_abs_difference"] = max(result["max_abs_difference"], delta)
            result["max_norm_residual"] = max(result["max_norm_residual"], residual)
    if result["cases"] != 48 or result["probabilities"] != 1008:
        raise AssertionError("Incomplete independent Clifford corpus")
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print("Q28_STABILIZER_PARITY " + json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: check_qiskit_stabilizer.py <binary> <report.json>")
    main(Path(sys.argv[1]), Path(sys.argv[2]))
