#!/usr/bin/env python3
"""Independent Q28 noisy-density reference using pinned Qiskit Kraus/DensityMatrix.

Observes *actual Rust executable* final complex matrices from 24 deterministic
1–3 qubit circuits, including all 9 unitary gate families, two Kraus maps and
unobserved reset. No hardware, no cloud provider, no formal proof.
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
from qiskit.quantum_info import DensityMatrix, Kraus

VERSIONS = {"qiskit": "2.6.1", "numpy": "2.3.4"}
TOL = 5e-11
GATES = ["h", "x", "y", "z", "s", "t", "cx", "cz", "swap"]
STRENGTHS = [0.0, 0.07, 0.25, 0.36, 0.5, 0.77, 1.0]


def check_versions() -> None:
    installed = {name: version(name) for name in VERSIONS}
    if installed != VERSIONS:
        raise RuntimeError(f"Q28-noisy oracle dependency mismatch: {installed} != {VERSIONS}")
    if sys.version_info[:2] != (3, 11):
        raise RuntimeError(f"Python 3.11 required: {sys.version}")


def generate(width: int, case: int) -> list[dict]:
    rng = random.Random(0xC008_2026 ^ (width << 24) ^ case * 0x9E3779B9)
    ops: list[dict] = []
    for q in range(width):
        ops.append({"op": "gate", "gate": {"kind": "h", "target": q}})
        ops.append({"op": "gate", "gate": {"kind": "t", "target": q}})
    for step in range(27):
        target = rng.randrange(width)
        family = step % (6 if width == 1 else 9)
        gate = GATES[family]
        if family < 6:
            ops.append({"op": "gate", "gate": {"kind": gate, "target": target}})
        elif family == 8:
            other = (target + 1 + rng.randrange(width - 1)) % width
            ops.append({"op": "gate", "gate": {"kind": "swap", "a": target, "b": other}})
        else:
            other = (target + 1 + rng.randrange(width - 1)) % width
            ops.append({"op": "gate", "gate": {
                "kind": gate, "control": target, "target": other
            }})
        if step % 3 == 0:
            strength = STRENGTHS[(step + case) % len(STRENGTHS)]
            if step % 2:
                ops.append({"op": "noise", "channel": {
                    "kind": "phase_flip", "qubit": target, "probability": strength,
                }})
            else:
                ops.append({"op": "noise", "channel": {
                    "kind": "amplitude_damping", "qubit": target, "gamma": strength,
                }})
        if step % 11 == 4:
            ops.append({"op": "reset", "qubit": target})
    return ops


def matrix_oracle(width: int, operations: list[dict]) -> np.ndarray:
    rho = DensityMatrix.from_label("0" * width)
    for op in operations:
        if op["op"] == "gate":
            gate = op["gate"]
            qc = QuantumCircuit(width)
            name = gate["kind"]
            if name in {"cx", "cz"}:
                getattr(qc, name)(gate["control"], gate["target"])
            elif name == "swap":
                qc.swap(gate["a"], gate["b"])
            else:
                getattr(qc, name)(gate["target"])
            rho = rho.evolve(qc)
        elif op["op"] == "noise":
            channel = op["channel"]
            q = channel["qubit"]
            if channel["kind"] == "phase_flip":
                p = channel["probability"]
                ident = np.eye(2, dtype=np.complex128)
                z = np.diag([1.0, -1.0]).astype(np.complex128)
                kraus = Kraus([math.sqrt(1.0 - p) * ident, math.sqrt(p) * z])
            elif channel["kind"] == "amplitude_damping":
                gamma = channel["gamma"]
                k0 = np.diag([1.0, math.sqrt(1.0 - gamma)]).astype(np.complex128)
                k1 = np.array(
                    [[0.0, math.sqrt(gamma)], [0.0, 0.0]], dtype=np.complex128
                )
                kraus = Kraus([k0, k1])
            else:
                raise AssertionError(f"unexpected channel: {channel!r}")
            rho = rho.evolve(kraus, qargs=[q])
        elif op["op"] == "reset":
            rho = rho.reset(qargs=[op["qubit"]])
        else:
            raise AssertionError(f"unsupported scientific fixture: {op!r}")
    return np.asarray(rho.data, dtype=np.complex128)


def run(binary: Path, width: int, case: int, ops: list[dict]) -> np.ndarray:
    request = {
        "schema": "aura.quantum.noisy-request/1",
        "qubits": width,
        "classical_bits": 0,
        "shots": 1,
        "seed": 100000 + width * 100 + case,
        "max_memory_bytes": 16 * 1024 * 1024,
        "inspect_final_density": True,
        "instructions": ops,
    }
    proc = subprocess.run(
        [str(binary.resolve())],
        input=json.dumps(request),
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    if proc.returncode != 0:
        raise AssertionError(f"Rust CLI failed n={width}, case={case}: {proc.stderr}")
    if proc.stderr.strip():
        raise AssertionError(f"Unexpected CLI stderr: {proc.stderr}")
    data = json.loads(proc.stdout)
    if (
        data["schema"] != "aura.quantum.noisy-result/1"
        or data["evidence_class"] != "simulation"
        or data["qpu_hardware_jobs"] != 0
        or data["shots"] != 1
    ):
        raise AssertionError(f"Invalid result provenance: {data!r}")
    final = data.get("final_density")
    size = 1 << width
    if not final or final["dimension"] != size:
        raise AssertionError("Missing complete final complex density matrix")
    entries = final["row_major_complex_pairs"]
    if len(entries) != size * size:
        raise AssertionError("Matrix dimensions/flattening mismatch")
    return np.asarray([complex(re, im) for re, im in entries], dtype=np.complex128).reshape((size,size))


def verify(binary: Path, output: Path) -> dict:
    check_versions()
    stats = {
        "status": "NUMERICAL_DENSITY_PARITY_PASS",
        "evidence_class": "simulation",
        "oracle": "Qiskit DensityMatrix + local Kraus + reset",
        "qpu_hardware_jobs": 0,
        "packages": VERSIONS,
        "python": platform.python_version(),
        "q0_lsb": True,
        "abs_complex_element_tolerance": TOL,
        "case_count": 0,
        "elements_compared": 0,
        "max_complex_element_delta": 0.0,
        "max_trace_residual": 0.0,
        "max_hermiticity_residual": 0.0,
        "min_observed_density_eigenvalue": 1.0,
        "mutation_negative_controls": 0,
        "worst_case": "",
    }
    for n in range(1, 4):
        for case in range(8):
            ops = generate(n, case)
            actual = run(binary, n, case, ops)
            expected = matrix_oracle(n, ops)
            delta = float(np.max(np.abs(actual - expected)))
            tr = abs(float(np.trace(actual).real) - 1.0)
            hermitian = float(np.max(np.abs(actual - actual.conj().T)))
            minimum_eigen = float(np.min(np.linalg.eigvalsh(actual)))
            values = [delta, tr, hermitian, minimum_eigen]
            if not all(map(math.isfinite, values)):
                raise AssertionError(f"nonfinite density data at n={n}, case={case}")
            if delta > TOL or tr > TOL or hermitian > TOL or minimum_eigen < -TOL:
                raise AssertionError(
                    f"n={n},case={case}: delta={delta}, trace={tr}, "
                    f"hermiticity={hermitian}, lambda_min={minimum_eigen}"
                )
            if delta > stats["max_complex_element_delta"]:
                stats["max_complex_element_delta"] = delta
                stats["worst_case"] = f"n{n}_case{case}"
            stats["max_trace_residual"] = max(stats["max_trace_residual"], tr)
            stats["max_hermiticity_residual"] = max(
                stats["max_hermiticity_residual"], hermitian
            )
            stats["min_observed_density_eigenvalue"] = min(
                stats["min_observed_density_eigenvalue"], minimum_eigen
            )
            corrupted = actual.copy()
            corrupted[0, 0] += 0.1
            if np.max(np.abs(corrupted - expected)) <= TOL:
                raise AssertionError("Mutation-negative control unexpectedly passed")
            stats["mutation_negative_controls"] += 1
            stats["case_count"] += 1
            stats["elements_compared"] += expected.size
    if stats["case_count"] != 24 or stats["mutation_negative_controls"] != 24:
        raise AssertionError("incomplete scientific corpus")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(stats, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print("Q28_NOISY_PARITY " + json.dumps(stats, sort_keys=True), flush=True)
    return stats


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: check_qiskit_noisy.py <rust-binary> <output-json>")
    verify(Path(sys.argv[1]), Path(sys.argv[2]))
