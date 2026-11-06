#!/usr/bin/env python3
"""Q28 C-004 independent numerical parity against pinned Qiskit Statevector.

Input is deterministic case/gate/complex-amplitude CSV exported by a Rust
example. This is a test-only code path, not an interoperable production ABI.
"""
from __future__ import annotations

import csv
import json
import math
import platform
import sys
from collections import defaultdict
from importlib.metadata import version
from pathlib import Path

import numpy as np
from qiskit import QuantumCircuit
from qiskit.quantum_info import Statevector

EXPECTED = {"qiskit": "2.6.1", "numpy": "2.3.4"}
TOLERANCE = 2e-11


def prepare_circuit(qubits: int, gates: str) -> QuantumCircuit:
    circuit = QuantumCircuit(qubits)
    allowed = {"h", "x", "y", "z", "s", "t", "cx", "cz", "swap"}
    for encoded in gates.split(";"):
        parts = encoded.split(":")
        name = parts[0]
        if name not in allowed:
            raise ValueError(f"unsupported gate: {name!r}")
        positions = [int(item) for item in parts[1:]]
        expected_arity = 1 if name in {"h", "x", "y", "z", "s", "t"} else 2
        if len(positions) != expected_arity:
            raise ValueError(f"wrong arity: {encoded!r}")
        if any(q < 0 or q >= qubits for q in positions):
            raise ValueError(f"invalid qubit: {encoded!r}")
        if len(positions) == 2 and positions[0] == positions[1]:
            raise ValueError(f"duplicated qubit: {encoded!r}")
        getattr(circuit, name)(*positions)
    return circuit


def check_parity(source: Path, report_path: Path) -> dict[str, object]:
    package_versions = {name: version(name) for name in EXPECTED}
    if package_versions != EXPECTED:
        raise RuntimeError(f"dependency mismatch: {package_versions} != {EXPECTED}")
    if sys.version_info[:2] != (3, 11):
        raise RuntimeError(f"Python runtime not pinned to 3.11: {sys.version}")

    cases: dict[str, list[dict[str, str]]] = defaultdict(list)
    with source.open("r", newline="", encoding="utf-8") as stream:
        reader = csv.DictReader(stream)
        if set(reader.fieldnames or []) != {
            "case_id", "qubits", "gates", "basis", "re", "im"
        }:
            raise ValueError(f"unrecognized fixture columns: {reader.fieldnames}")
        for row in reader:
            cases[row["case_id"]].append(row)
    if len(cases) != 32:
        raise AssertionError(f"expected 32 fixture circuits, got {len(cases)}")

    highest_component_difference = 0.0
    highest_probability_difference = 0.0
    worst_case = ""
    total_amplitudes = 0
    per_width: dict[int, int] = defaultdict(int)

    for case_id in sorted(cases):
        rows = cases[case_id]
        qubits = int(rows[0]["qubits"])
        gates = rows[0]["gates"]
        if any(row["gates"] != gates or int(row["qubits"]) != qubits for row in rows):
            raise AssertionError(f"inconsistent case metadata in {case_id}")
        count = 1 << qubits
        if len(rows) != count:
            raise AssertionError(f"{case_id} returned {len(rows)} instead of {count} amps")
        expected_indices = list(range(count))
        basis_indices = [int(row["basis"]) for row in rows]
        if basis_indices != expected_indices:
            raise AssertionError(f"noncanonical basis order for {case_id}: {basis_indices}")

        rust = np.array(
            [complex(float(row["re"]), float(row["im"])) for row in rows],
            dtype=np.complex128,
        )
        if not np.all(np.isfinite(rust)):
            raise AssertionError(f"nonfinite Rust amplitudes in {case_id}")

        # Qiskit Statevector and Rust engine both use q0 as the LSB basis bit.
        reference = np.asarray(
            Statevector.from_instruction(prepare_circuit(qubits, gates)).data,
            dtype=np.complex128,
        )
        max_error = float(np.max(np.abs(reference - rust)))
        max_probability_error = float(
            np.max(np.abs(np.abs(reference) ** 2 - np.abs(rust) ** 2))
        )
        rust_norm_error = abs(float(np.vdot(rust, rust).real) - 1.0)
        if not all(map(math.isfinite, [max_error, max_probability_error, rust_norm_error])):
            raise AssertionError(f"numerical nonfinite result in {case_id}")
        if max_error > highest_component_difference:
            highest_component_difference = max_error
            worst_case = case_id
        highest_probability_difference = max(
            highest_probability_difference, max_probability_error
        )
        if max_error > TOLERANCE or rust_norm_error > TOLERANCE:
            raise AssertionError(
                f"{case_id}: amplitude_error={max_error:.5e} "
                f"probability_error={max_probability_error:.5e} "
                f"norm_error={rust_norm_error:.5e}, tolerance={TOLERANCE:.5e}"
            )
        if max_probability_error > 2.0 * TOLERANCE:
            raise AssertionError(f"{case_id}: Born probabilities differ")

        # Test oracle sensitivity with a *local* deliberately corrupted vector.
        deliberately_corrupted = rust.copy()
        deliberately_corrupted[0] += complex(0.125, -0.025)
        if float(np.max(np.abs(deliberately_corrupted - reference))) <= TOLERANCE:
            raise AssertionError(f"{case_id}: mutation-negative control not detected")
        total_amplitudes += count
        per_width[qubits] += 1

    if dict(per_width) != {1: 8, 2: 8, 3: 8, 4: 8}:
        raise AssertionError(f"incomplete width coverage: {dict(per_width)}")

    summary = {
        "result": "NUMERICAL_PARITY_PASS",
        "authority": "external-qiskit-statevector-numerical",
        "not_formal_proof": True,
        "precision": "IEEE-754 complex128",
        "qubit_order": "q0-lsb",
        "python_version": platform.python_version(),
        "packages": package_versions,
        "tolerance_amplitude_absolute": TOLERANCE,
        "cases": len(cases),
        "amplitudes_compared": total_amplitudes,
        "max_amplitude_difference": highest_component_difference,
        "max_born_probability_difference": highest_probability_difference,
        "worst_case": worst_case,
        "corrupted_oracle_negative_controls": len(cases),
        "qpu_hardware_invocations": 0,
    }
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print("Q28_PARITY " + json.dumps(summary, sort_keys=True), flush=True)
    return summary


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: check_qiskit_parity.py fixture.csv report.json")
    check_parity(Path(sys.argv[1]), Path(sys.argv[2]))
