#!/usr/bin/env python3
"""C-019 Qiskit density-matrix oracle for REAL Rust C-018 ideal channel CLI.

The oracle evolves UNNORMALIZED density matrices using Qiskit Operator gates,
projective Kraus maps and classical branch keys. This is structurally
independent of C-018's conditional *Statevector* branch engine. The test
compares all observable moments, Born weights, hidden reset branches and
law-of-total-variance decomposition on deterministic non-Clifford circuits.
No QPU/provider calls. Pinned Qiskit/NumPy are required, not optional.
"""
from __future__ import annotations

import json
import math
import platform
import random
import subprocess
import sys
from collections import defaultdict
from importlib.metadata import version
from pathlib import Path

import numpy as np
from qiskit import QuantumCircuit
from qiskit.quantum_info import DensityMatrix, Operator, Pauli

PKGS = {"qiskit": "2.6.1", "numpy": "2.3.4"}
TOL = 8e-11
# Qiskit density-matrix U rho U† accumulates roundoff at *known impossible*
# reset outcomes.  This is an oracle-only numerical-resolution floor, NOT
# a change to Rust: Rust independently rejects representable underflow.
# This corpus deliberately has no physical rare branch near this threshold;
# fail closed if the smallest admitted nonzero branch is too small.
ORACLE_GHOST_TOL = 1e-13
MIN_TEST_PHYSICAL_BRANCH = 1e-9
REQUEST = "aura.quantum.channel-moments-request/1"
RESULT = "aura.quantum.channel-moments-result/1"


def gate(kind: str, *wires: int) -> dict:
    if kind in ("cx", "cz"):
        return {"op": "gate", "gate": {
            "kind": kind, "control": wires[0], "target": wires[1]}}
    if kind == "swap":
        return {"op": "gate", "gate": {
            "kind": kind, "a": wires[0], "b": wires[1]}}
    return {"op": "gate", "gate": {"kind": kind, "target": wires[0]}}


def request(n: int, bits: int, ops: list[dict], terms: list[dict]) -> dict:
    return {"schema": REQUEST, "qubits": n, "classical_bits": bits,
            "max_memory_bytes": 16 * 1024 * 1024,
            "instructions": ops, "terms": terms}


def random_case(n: int, case: int) -> dict:
    rng = random.Random(20261108 + n * 301 + case * 1009)
    ops = [
        gate("h", 0), gate("t", n - 1), gate("cx", 0, n - 1),
        gate("s", 1), gate("h", 0),
    ]
    for _ in range(3):
        kind = rng.choice(("h", "s", "t", "x", "y", "z"))
        ops.append(gate(kind, rng.randrange(n)))
    if case % 2 == 0:
        ops.extend([
            {"op": "measure", "qubit": 0, "into_bit": 0},
            {"op": "if", "bit": 0, "equals": True,
             "gate": {"kind": "x", "target": n - 1}},
        ])
    if case % 3 == 0:
        ops.append({"op": "reset", "qubit": n - 1})
    if case % 4 == 0:
        ops.extend([
            gate("h", n - 1),
            {"op": "measure", "qubit": n - 1, "into_bit": 1},
            {"op": "if", "bit": 1, "equals": False,
             "gate": {"kind": "z", "target": 0}},
        ])
    for _ in range(3):
        kind = rng.choice(("h", "s", "t", "x", "y", "z"))
        ops.append(gate(kind, rng.randrange(n)))
    words = ["I" * n, "Y" + "I" * (n - 1), "Z" * n,
             "X" + "I" * (n - 2) + "Y",
             "Y" + "I" * (n - 2) + "X",
             "".join(rng.choice("IXYZ") for _ in range(n))]
    coeff = [0.375, 0.8, -1.25, 0.3125, -0.9, 0.4]
    terms = [dict(paulis=w, coefficient=c) for w, c in zip(words, coeff)]
    return request(n, 2, ops, terms)


def corpus(fixtures: Path) -> list[tuple[str, dict]]:
    result = [("C018_real_Bell_feedback", json.loads(
        (fixtures / "channel_moments_feedforward_v1.json").read_text()))]
    result.extend([
        ("T_nonuniform", request(1, 1,
            [gate("h", 0), gate("t", 0), gate("h", 0),
             {"op": "measure", "qubit": 0, "into_bit": 0}],
            [dict(paulis="Z", coefficient=1.0)])),
        ("Y_dephasing", request(1, 1,
            [gate("h", 0), gate("s", 0),
             {"op": "measure", "qubit": 0, "into_bit": 0}],
            [dict(paulis="Y", coefficient=1.0)])),
        ("hidden_reset", request(1, 0,
            [gate("h", 0), {"op": "reset", "qubit": 0}],
            [dict(paulis="Z", coefficient=1.0)])),
        ("correlated_remeasure", request(1, 2,
            [gate("h", 0),
             {"op": "measure", "qubit": 0, "into_bit": 0},
             {"op": "measure", "qubit": 0, "into_bit": 1}],
            [dict(paulis="Z", coefficient=1.0)])),
        ("five_hidden_instruments", request(5, 0,
            [x for q in range(5) for x in
             (gate("h", q), {"op": "reset", "qubit": q})],
            [dict(paulis="ZIIII", coefficient=1.0)])),
    ])
    for n in range(2, 7):
        for i in range(8):
            result.append((f"nonclifford_{n}q_{i}", random_case(n, i)))
    # Dense >6q Qiskit density-matrix witnesses are costlier; use two each.
    for n in (7, 8):
        for i in (0, 2):
            result.append((f"dense_{n}q_{i}", random_case(n, i)))
    return result


def unitary(n: int, spec: dict) -> np.ndarray:
    qc = QuantumCircuit(n)
    k = spec["kind"]
    if k in ("cx", "cz"):
        getattr(qc, k)(spec["control"], spec["target"])
    elif k == "swap":
        qc.swap(spec["a"], spec["b"])
    else:
        getattr(qc, k)(spec["target"])
    return np.asarray(Operator(qc).data, dtype=np.complex128)


def reference(case: dict) -> dict:
    n = case["qubits"]
    dim = 1 << n
    branches: list[tuple[np.ndarray, list[bool | None]]] = [
        (np.asarray(DensityMatrix.from_int(0, 2**n).data,
                    dtype=np.complex128), [None] * case["classical_bits"])
    ]
    gates: dict[str, np.ndarray] = {}
    masks = np.arange(dim, dtype=np.int64)
    discarded_numeric_ghosts = 0
    maximum_numeric_ghost_mass = 0.0
    minimum_admitted_branch_probability = 1.0

    def gate_operator(g: dict) -> np.ndarray:
        name = json.dumps(g, sort_keys=True)
        if name not in gates:
            gates[name] = unitary(n, g)
        return gates[name]

    for op in case["instructions"]:
        kind = op["op"]
        next_branches = []
        for rho, bits in branches:
            if kind == "gate":
                u = gate_operator(op["gate"])
                next_branches.append((u @ rho @ u.conj().T, bits))
            elif kind == "if":
                if bits[op["bit"]] == op["equals"]:
                    u = gate_operator(op["gate"])
                    rho = u @ rho @ u.conj().T
                next_branches.append((rho, bits))
            elif kind in ("measure", "reset"):
                q = op["qubit"]
                for bit in (False, True):
                    # P_b rho P_b; no conditional-statevector path is used.
                    selected = ((masks >> q) & 1) == int(bit)
                    conditional = rho * np.outer(selected, selected)
                    weight = float(np.trace(conditional).real)
                    if weight < -ORACLE_GHOST_TOL:
                        raise AssertionError("invalid negative density probability")
                    if weight <= ORACLE_GHOST_TOL:
                        # A 1e-16 unitary roundoff residue in an otherwise
                        # exactly impossible Z instrument outcome is NOT a
                        # new physical classical branch. Track, bound and
                        # report discarded numerical ghosts explicitly.
                        if weight != 0.0:
                            discarded_numeric_ghosts += 1
                            maximum_numeric_ghost_mass = max(
                                maximum_numeric_ghost_mass, abs(weight))
                        continue
                    parent_mass = float(np.trace(rho).real)
                    conditional_probability = weight / parent_mass
                    if conditional_probability < MIN_TEST_PHYSICAL_BRANCH:
                        raise AssertionError(
                            "independent density oracle cannot resolve a "
                            "genuinely rare conditional outcome at this precision")
                    minimum_admitted_branch_probability = min(
                        minimum_admitted_branch_probability, conditional_probability)
                    new_bits = list(bits)
                    if kind == "measure":
                        new_bits[op["into_bit"]] = bit
                    elif bit:
                        u = gate_operator({"kind": "x", "target": q})
                        conditional = u @ conditional @ u.conj().T
                    next_branches.append((conditional, new_bits))
            else:
                raise AssertionError(f"unsupported oracle operation {kind}")
        branches = next_branches
        if not branches or len(branches) > 256:
            raise AssertionError("oracle branch count out of profile")

    h = np.zeros((dim, dim), dtype=np.complex128)
    for term in case["terms"]:
        # Aura input positions are [q0,q1,..], Qiskit Pauli is high-to-low.
        h += term["coefficient"] * Pauli(term["paulis"][::-1]).to_matrix()
    h2 = h @ h
    sums = defaultdict(float)
    histogram: dict[str, float] = defaultdict(float)
    conditional_moments = []
    mass = 0.0
    for rho, bits in branches:
        weight = float(np.trace(rho).real)
        mass += weight
        e = np.trace(rho @ h)
        e2 = np.trace(rho @ h2)
        if abs(e.imag) > TOL or abs(e2.imag) > TOL:
            raise AssertionError("non-Hermitian oracle expectation")
        mu = float(e.real) / weight
        m2 = float(e2.real) / weight
        var = m2 - mu * mu
        if var < -TOL:
            raise AssertionError("negative oracle variance")
        conditional_moments.append((weight, mu))
        sums["weighted_sum"] += float(e.real)
        sums["weighted_second_moment"] += float(e2.real)
        sums["within_branch_variance"] += weight * max(0.0, var)
        key = "".join(
            "?" if bit is None else str(int(bit)) for bit in reversed(bits))
        histogram[key] += weight
    mean = sums["weighted_sum"]
    second = sums["weighted_second_moment"]
    sums["weighted_variance"] = second - mean**2
    sums["between_branch_variance"] = sum(
        w * (m - mean)**2 for w, m in conditional_moments)
    sums["total_branch_probability"] = mass
    if abs(mass - 1) > TOL or abs(
        sums["within_branch_variance"] +
        sums["between_branch_variance"] -
        sums["weighted_variance"]) > TOL:
        raise AssertionError("Qiskit density channel norm or variance law failed")
    return {
        "numbers": dict(sums), "classical_probabilities": dict(histogram),
        "final_branch_count": len(branches),
        "instrument_count": sum(x["op"] in ("measure", "reset")
                                for x in case["instructions"]),
        "discarded_numeric_ghosts": discarded_numeric_ghosts,
        "maximum_numeric_ghost_mass": maximum_numeric_ghost_mass,
        "minimum_admitted_branch_probability":
            minimum_admitted_branch_probability,
    }


def run_actual(binary: Path, request_body: dict) -> tuple[bytes, dict]:
    raw = json.dumps(request_body, sort_keys=True).encode()
    p = subprocess.run([str(binary.resolve())], input=raw, capture_output=True,
                       timeout=45, check=False)
    if p.returncode != 0 or p.stderr:
        raise AssertionError(
            f"real Rust channel-moments CLI failed: {p.returncode} {p.stderr[:700]}")
    result = json.loads(p.stdout)
    if (result["schema"] != RESULT or result["status"] != "simulated"
        or result["energy_scope"] != "unconditioned_ideal_instrument_channel"
        or result["qpu_hardware_jobs"] != 0 or result["shots"] != 0
        or result["sampling"] != "none;all_representable_instrument_branches"
        or result["max_qubits"] != 8 or result["max_branches"] != 256
        or result["pauli_word_order"] !=
            "q0-first;position-0-is-least-significant-wire"):
        raise AssertionError("real Rust channel semantics/provenance mismatch")
    return p.stdout, result


def compare(result: dict, expected: dict, name: str) -> float:
    if (result["instrument_count"] != expected["instrument_count"]
        or result["final_branch_count"] != expected["final_branch_count"]
        or set(result["classical_probabilities"]) !=
            set(expected["classical_probabilities"])):
        raise AssertionError(
            f"{name}: branch count or classical keys disagree: "
            f"rust={result['final_branch_count']}/"
            f"{result['classical_probabilities']} "
            f"qiskit={expected['final_branch_count']}/"
            f"{expected['classical_probabilities']}")
    max_diff = 0.0
    for field, value in expected["numbers"].items():
        diff = abs(float(result[field]) - value)
        if not math.isfinite(diff) or diff > TOL * max(1.0, abs(value)):
            raise AssertionError(f"{name}: {field} diff={diff}, expected={value}")
        max_diff = max(max_diff, diff)
    for key, value in expected["classical_probabilities"].items():
        diff = abs(float(result["classical_probabilities"][key]) - value)
        if not math.isfinite(diff) or diff > TOL:
            raise AssertionError(f"{name}: histogram[{key}] diff={diff}")
        max_diff = max(max_diff, diff)
    return max_diff


def rejected(binary: Path, req: dict, expected_code: str) -> None:
    p = subprocess.run([str(binary.resolve())], input=json.dumps(req),
                       text=True, capture_output=True, timeout=30)
    if p.returncode != 2 or p.stdout.strip():
        raise AssertionError("invalid channel input produced output or exit success")
    err = json.loads(p.stderr)
    if (err["schema"] != "aura.quantum.channel-moments-error/1"
        or err["code"] != expected_code or err["qpu_hardware_jobs"] != 0):
        raise AssertionError(f"not a typed fail-closed rejection: {err}")


def main(binary: Path, fixtures: Path, report_path: Path) -> dict:
    versions = {name: version(name) for name in PKGS}
    if versions != PKGS or sys.version_info[:2] != (3, 11):
        raise RuntimeError(f"wrong pinned scientific environment {versions}")
    results = corpus(fixtures)
    report = {
        "status": "Q28_CHANNEL_DENSITY_QISKIT_PASS",
        "cases": 0, "tested_instrument_histories": 0,
        "mutated_negative_controls": 0,
        "invalid_protocol_controls": 0,
        "max_component_delta": 0.0,
        "highest_qubit_width": 0,
        "oracle_roundoff_ghosts_bounded": 0,
        "max_oracle_ghost_mass": 0.0,
        "minimum_admitted_branch_probability": 1.0,
        "oracle_ghost_resolution_floor": ORACLE_GHOST_TOL,
        "qpu_hardware_jobs": 0,
        "measurement_scope": "all_represented_unconditioned_ideal_instrument_outcomes",
        "independent_reference": "Qiskit-DensityMatrix-Operator-Kraus-not-Statevector",
        "threshold": TOL, "packages": versions,
        "python": platform.python_version(),
    }
    for name, req in results:
        actual_bytes, actual = run_actual(binary, req)
        qiskit = reference(req)
        maximum = compare(actual, qiskit, name)
        report["max_component_delta"] = max(
            report["max_component_delta"], maximum)
        report["tested_instrument_histories"] += qiskit["final_branch_count"]
        report["highest_qubit_width"] = max(
            report["highest_qubit_width"], req["qubits"])
        report["oracle_roundoff_ghosts_bounded"] += qiskit[
            "discarded_numeric_ghosts"]
        report["max_oracle_ghost_mass"] = max(
            report["max_oracle_ghost_mass"], qiskit[
                "maximum_numeric_ghost_mass"])
        report["minimum_admitted_branch_probability"] = min(
            report["minimum_admitted_branch_probability"], qiskit[
                "minimum_admitted_branch_probability"])
        # Deliberately corrupt the *Rust result*, then require independent
        # density oracle detection. No self-oracle fabricated success.
        mutated = dict(actual)
        mutated["weighted_second_moment"] += 0.01
        try:
            compare(mutated, qiskit, name + "_mutated")
        except AssertionError:
            report["mutated_negative_controls"] += 1
        else:
            raise AssertionError("poisoned Rust moment was accepted")
        if name in ("T_nonuniform", "hidden_reset", "Y_dephasing"):
            again, _ = run_actual(binary, req)
            if actual_bytes != again:
                raise AssertionError("unseeded deterministic channel replay differs")
        if name == "T_nonuniform":
            target = math.sqrt(0.5)
            if abs(qiskit["numbers"]["weighted_sum"] - target) > TOL:
                raise AssertionError("non-uniform Born weights were averaged uniformly")
        report["cases"] += 1
    negative = results[0][1]
    for field, value in (
        ("hardware", True), ("seed", 1), ("shots", 128),
        ("qpu_backend", "provider"),
    ):
        rejected(binary, {**negative, field: value}, "invalid_json")
        report["invalid_protocol_controls"] += 1
    rejected(binary, {**negative, "schema": "aura.quantum.moments-request/1"},
             "unsupported_schema")
    report["invalid_protocol_controls"] += 1
    rejected(binary, {**negative, "qubits": 9}, "profile_limit")
    report["invalid_protocol_controls"] += 1
    if (report["cases"] != 50 or report["mutated_negative_controls"] != 50
        or report["invalid_protocol_controls"] != 6):
        raise AssertionError("partial C-019 density-matrix oracle corpus")
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    print("Q28_CHANNEL_DENSITY_QISKIT " +
          json.dumps(report, sort_keys=True), flush=True)
    return report


if __name__ == "__main__":
    if len(sys.argv) != 4:
        raise SystemExit(
            "usage: check_qiskit_channel_moments.py real_binary fixtures report")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]))
