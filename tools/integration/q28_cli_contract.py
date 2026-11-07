#!/usr/bin/env python3
"""X-owned black-box acceptance of Agent C C-009 v1 CLI consumers.

Runs compiled subprocesses, not Rust unit helpers. The scientific bounds here
are small analytic checks; this is NOT Q23 proof or QPU certification.
"""
import argparse
import copy
import json
import subprocess
from pathlib import Path

MIB = 1024 * 1024
SCHEMAS = {
    "pure": ("aura.quantum.circuit-request/1", "aura.quantum.circuit-result/1", "aura.quantum.circuit-error/1", "aura-quantum-run"),
    "noisy": ("aura.quantum.noisy-request/1", "aura.quantum.noisy-result/1", "aura.quantum.noisy-error/1", "aura-quantum-noisy-run"),
    "stabilizer": ("aura.quantum.stabilizer-request/1", "aura.quantum.stabilizer-result/1", "aura.quantum.stabilizer-error/1", "aura-quantum-stabilizer-run"),
}


def ensure(condition, label):
    if not condition:
        raise AssertionError(label)


def call(binary, payload, extra=()):
    raw = payload if isinstance(payload, bytes) else json.dumps(payload, separators=(",", ":")).encode()
    return subprocess.run([str(binary), *extra], input=raw, capture_output=True, timeout=20, check=False)


def success(binary, kind, payload, label):
    p = call(binary, payload)
    ensure(p.returncode == 0, f"{label}: rc={p.returncode} stderr={p.stderr[:500]!r}")
    ensure(not p.stderr, f"{label}: unexpected stderr {p.stderr[:500]!r}")
    result = json.loads(p.stdout)
    ensure(result["schema"] == SCHEMAS[kind][1], f"{label}: schema")
    ensure(result["status"] == "simulated", f"{label}: status")
    ensure(result["evidence_class"] == "simulation", f"{label}: evidence boundary")
    ensure(result["qpu_hardware_jobs"] == 0, f"{label}: hardware boundary")
    ensure(result["seed"] == payload["seed"], f"{label}: seed")
    ensure(result["shots"] == payload["shots"], f"{label}: shots")
    ensure(len(result["results"]) == payload["shots"], f"{label}: shot records")
    return p.stdout, result


def rejection(binary, kind, payload, expected, label, extra=()):
    p = call(binary, payload, extra=extra)
    ensure(p.returncode == 2, f"{label}: expected rc=2, got {p.returncode}")
    ensure(p.stdout == b"", f"{label}: partial success leaked on stdout")
    error = json.loads(p.stderr)
    ensure(error["schema"] == SCHEMAS[kind][2], f"{label}: error schema")
    ensure(error["status"] == "error" and error["code"] == expected, f"{label}: error code {error}")
    ensure(error["qpu_hardware_jobs"] == 0, f"{label}: hardware boundary")
    ensure(isinstance(error["message"], str) and error["message"], f"{label}: missing diagnostic")


def base_request(kind):
    return {
        "schema": SCHEMAS[kind][0], "qubits": 2, "classical_bits": 1,
        "seed": 731, "shots": 3, "max_memory_bytes": 16 * MIB,
        "instructions": [],
    }


def gate(kind, name, **kwargs):
    return {"op": "gate", "gate": {"kind": name, **kwargs}}


def read_fixture(path):
    return json.loads(path.read_text(encoding="utf-8"))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--fixture-dir", type=Path, default=Path("aura-quantum-sim/fixtures"))
    args = parser.parse_args()
    binary = {kind: args.bin_dir / schema[3] for kind, schema in SCHEMAS.items()}
    for kind, path in binary.items():
        ensure(path.is_file(), f"{kind}: expected built binary at {path}")

    pure = read_fixture(args.fixture_dir / "bell_feedforward_v1.json")
    noisy = read_fixture(args.fixture_dir / "noisy_tomography_v1.json")
    stabilizer = read_fixture(args.fixture_dir / "stabilizer_bell_inspection_v1.json")
    for kind, request in (("pure", pure), ("noisy", noisy), ("stabilizer", stabilizer)):
        left, result = success(binary[kind], kind, request, f"{kind} fixture")
        right, _ = success(binary[kind], kind, request, f"{kind} seeded replay")
        ensure(left == right, f"{kind}: same-seed subprocess replay not byte-identical")

    _, p = success(binary["pure"], "pure", pure, "dynamic Bell feedback")
    ensure(all(len(s["trace"]) == 2 and len(s["bits"]) == 1 for s in p["results"]), "pure: feedback trace")

    _, n = success(binary["noisy"], "noisy", noisy, "analytic density")
    rho = n["final_density"]
    ensure(rho["dimension"] == 2, "noise: density dimension")
    # H|0>, amplitude damping gamma=0.36, phase flip p=0.25:
    # rho = [[0.68,0.20],[0.20,0.32]], Tr(rho^2)=0.6448.
    expected = ((0.68, 0), (0.20, 0), (0.20, 0), (0.32, 0))
    for i, (actual, (re, im)) in enumerate(zip(rho["row_major_complex_pairs"], expected)):
        ensure(abs(actual[0] - re) < 1e-10 and abs(actual[1] - im) < 1e-10, f"noise: analytic rho[{i}]")
    ensure(len(rho["row_major_complex_pairs"]) == 4, "noise: matrix size")
    ensure(abs(rho["trace"] - 1) < 1e-10, "noise: trace")
    ensure(abs(rho["purity"] - .6448) < 1e-10, "noise: analytic purity")

    _, s = success(binary["stabilizer"], "stabilizer", stabilizer, "Clifford Bell")
    ensure(s["basis_probabilities"] == [0.5, 0.0, 0.0, 0.5], "stabilizer: analytic Bell probabilities")
    ensure(s["stabilizer_generators"] == ["+XX", "+ZZ"], "stabilizer: Bell generators")

    # Independent process-level cross-engine invariant: Bell Z results are
    # always correlated; do not compare independent RNG stream internals.
    for kind in ("pure", "stabilizer"):
        request = base_request(kind)
        request.update(classical_bits=2, shots=64)
        request["instructions"] = [
            gate(kind, "h", target=0), gate(kind, "cx", control=0, target=1),
            {"op": "measure", "qubit": 0, "into_bit": 0},
            {"op": "measure", "qubit": 1, "into_bit": 1},
        ]
        _, outcome = success(binary[kind], kind, request, f"{kind} measured Bell")
        ensure(all(type(x["bits"][0]) is bool and x["bits"][0] == x["bits"][1]
                   and len(x["trace"]) == 2 for x in outcome["results"]), f"{kind}: Bell correlation")
        if kind == "stabilizer":
            ensure(sum(outcome["histogram"].values()) == 64, "stabilizer: histogram conservation")
            ensure(set(outcome["histogram"]).issubset({"00", "11"}), "stabilizer: histogram support")

    negative = 0
    for kind in SCHEMAS:
        exe = binary[kind]
        good = base_request(kind)
        rejection(exe, kind, b'{"schema":', "invalid_json", f"{kind} malformed")
        negative += 1
        rejection(exe, kind, b" " * (256 * 1024 + 1), "input_too_large", f"{kind} bounded input")
        negative += 1
        bad = copy.deepcopy(good)
        bad["schema"] = "aura.quantum.future-request/7"
        rejection(exe, kind, bad, "unsupported_schema", f"{kind} version isolation")
        negative += 1
        bad = copy.deepcopy(good)
        bad["surprise"] = True
        rejection(exe, kind, bad, "invalid_json", f"{kind} strict top-level")
        negative += 1
        bad = copy.deepcopy(good)
        bad["shots"] = 0
        rejection(exe, kind, bad, "profile_limit", f"{kind} zero shots")
        negative += 1
        bad = copy.deepcopy(good)
        bad["qubits"] = 999
        rejection(exe, kind, bad, "profile_limit", f"{kind} width limit")
        negative += 1
        bad = copy.deepcopy(good)
        bad["instructions"] = [gate(kind, "cx", control=0, target=0)]
        rejection(exe, kind, bad, "circuit_rejected", f"{kind} duplicate quantum wire")
        negative += 1
        bad = copy.deepcopy(good)
        if kind == "noisy":
            bad["instructions"] = [{"op": "if", "bit": 0, "equals": True, "action":
                                    {"kind": "gate", "gate": {"kind": "x", "target": 0}}}]
        else:
            bad["instructions"] = [{"op": "if", "bit": 0, "equals": True,
                                    "gate": {"kind": "x", "target": 0}}]
        rejection(exe, kind, bad, "circuit_rejected", f"{kind} unwritten classical bit")
        negative += 1
        rejection(exe, kind, good, "usage", f"{kind} forbidden CLI flag", extra=("--help",))
        negative += 1

    bad = base_request("noisy")
    bad["instructions"] = [{"op": "noise", "channel":
                             {"kind": "phase_flip", "qubit": 0, "probability": 1.2}}]
    rejection(binary["noisy"], "noisy", bad, "circuit_rejected", "noise: nonphysical probability")
    negative += 1
    bad = base_request("stabilizer")
    bad["instructions"] = [gate("stabilizer", "t", target=0)]
    rejection(binary["stabilizer"], "stabilizer", bad, "circuit_rejected", "stabilizer: non-Clifford T")
    negative += 1
    print(f"X002_Q28_EXTERNAL_CONSUMER_CONTRACT_PASS positives=8 negatives={negative} "
          "profiles=3 analytic_density=pass bell_correlation=pass seed_replay=pass")


if __name__ == "__main__":
    main()
