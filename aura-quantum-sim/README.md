# Aura Quantum reference simulator — callable Rust profile

> **Status:** experimental CPU-only f64 statevector. The Q28 package is on an
> unmerged Draft PR; neither Aura compiler integration nor QPU execution is
> implemented or qualified. This is not a scientific equivalence proof.

This isolated crate is its **own Cargo workspace**, and can be tested without
changing the root workspace or Cargo.lock.

## Run the actual dynamic circuit API

```sh
cargo test --manifest-path aura-quantum-sim/Cargo.toml --all-targets
cargo clippy --manifest-path aura-quantum-sim/Cargo.toml --all-targets -- -D warnings
cargo run --manifest-path aura-quantum-sim/Cargo.toml --example feed_forward
```

The executable example prepares a Bell pair, measures q0 into a classical bit,
conditionally applies X to q1 based on that measurement, resets q0 using an
ideal Z-measure-and-correct operation, verifies the final state is `|00>`,
then runs 512 fresh, seeded shots and prints simulation-only metadata.

Here is the public API in use (this is Rust, *not yet Aura source syntax*):

```rust
use aura_quantum_sim::circuit::{Circuit, Instruction};
use aura_quantum_sim::{Gate, DEFAULT_MEMORY_BUDGET};

let mut circuit = Circuit::new(2, 1);
circuit.push(Instruction::Gate(Gate::H(0)));
circuit.push(Instruction::Gate(Gate::Cnot { control: 0, target: 1 }));
circuit.push(Instruction::Measure { qubit: 0, into_bit: 0 });
circuit.push(Instruction::If { bit: 0, equals: true, gate: Gate::X(1) });
circuit.push(Instruction::Reset { qubit: 0 });

let run = circuit.execute_once(42, DEFAULT_MEMORY_BUDGET)?;
assert_eq!(run.final_state.basis_probability(0)?, 1.0);
let shots = circuit.execute_shots(512, 42, DEFAULT_MEMORY_BUDGET)?;
assert_eq!(shots.metadata.shots, 512);
// Each shot has classical bits Vec<Option<bool>> and ordered trace entries.
```

## Versioned supported profile and semantics

- **Result schema:** `aura.quantum.circuit-result/1`
- **Execution profile:** `local-pure-statevector-f64-dynamic-z/1`
- **Engine ID:** `aura-quantum-sim/0.1.0`
- **Numerical normalization tolerance:** `1e-10`. This is a
  runtime invariant, **not** a certified end-to-end error bound.
- **Indexing:** q0 is the least significant computational-basis bit,
  ket notation `|q[n-1] ... q0>`. Only 1+ quantum wires supported
  by the dynamic circuit API (the lower-level statevector has separate semantics).
- **Input operations:** H, X, Y, Z, S, T, CX, CZ, SWAP, `Measure`,
  `Reset`, and classical bit equality guard on a gate.
  All gates are checked **before any shot executes**, including a gate on a
  branch that would not be taken during this run.
- **Z measurement:** ideal nondestructive projective measurement; each explicit
  measurement writes a classical bit and appends an ordered event.
- **Reset:** ideal *Z measurement followed by X when the outcome was 1*.
  Its internal previous state bit appears in the ordered simulation trace,
  but does not overwrite the classical register.
- **Control:** classical bits start as `None`, become defined only
  by explicit prior `Measure` on that bit; reading an unwritten bit
  returns `ReadBeforeWrite`. A measurement can overwrite an earlier
  bit; the latest value controls subsequent instructions.
- **RNG:** one deterministic SplitMix64 stream per invocation, seeded by caller;
  shots each restart the quantum and classical registers but share one
  sequential RNG stream. Every measurement/reset consumes one draw.
- **Results:** `execute_once` returns a final statevector, per-shot
  classical bits, ordered trace and metadata; `execute_shots` returns
  bounded per-shot records without duplicating full statevectors.
- **Logical memory budget:** includes one live statevector, classical output
  registers and the declared maximum event/output footprint across all shots.
  Inputs/operation storage, allocator capacity/padding, toolchain overhead and
  process RSS are not included. Allocation is fallible and integer operations
  are checked; explicit errors distinguish unsupported sizes and budget
  pressure. Use real OS telemetry for physical RSS.
- **Unsupported:** parallel execution, quantum channels/noise/QEC, arbitrary
  classical arithmetic/conditionals, mid-circuit IR import, OpenQASM, QIR,
  Aer parity certification, error-corrected logical qubits, real-device
  calibration and hardware submissions. In particular, successful Rust
  simulation is *not* independent Q23 formal proof.

## Acceptance fixtures

`tests/circuit_execution.rs` exercises non-Clifford teleportation
with classical X/Z correction and reset through the **public API**, including
four measurement branches, seed replay, Bell distributions, invalid gates even
inside untaken branches, classical read-before-write rejection, zero-shot and
allocation budget guards, ordered event provenance and final-state verification.

`examples/export_qiskit_vectors.rs` and the separately pinned Qiskit
CI verifier remain a numerical oracle for bounded *static* circuits. They do
not independently establish correctness for every dynamic instrument sequence;
a separate peer review of quantum operation ordering and adaptive semantics
is required.

**Integration:** The branch is a testable experimental library surface, not
a root Aura compiler feature. An integration steward and independent science
review must approve API/IR interfaces and a safe merge plan. No autonomous
merge, global QUALIFIED status or QPU spending is authorized.


## Offline JSON command-line execution (experimental schema)

The built binary `aura-quantum-run` accepts a **single bounded JSON request on
stdin** and emits one JSON result on stdout. Errors are JSON on **stderr** with
a nonzero exit code (2) and no partial success result.

```sh
cargo run --manifest-path aura-quantum-sim/Cargo.toml --bin aura-quantum-run \
  < aura-quantum-sim/fixtures/bell_feedforward_v1.json
```

The versioned request schema `aura.quantum.circuit-request/1` is a local
experimental integration adapter, **not** Aura source syntax, QIR, OpenQASM
or a frozen semantic interface. The fixture demonstrates Bell measurement,
classical feed-forward and reset from an external process without linking Rust.

Top-level fields: `schema`, `qubits`, `classical_bits`, `seed`,
`shots`, `max_memory_bytes`, and `instructions`.
Operations are tagged `{"op":"gate","gate":{"kind":"h","target":0}}`,
`{"op":"measure","qubit":0,"into_bit":0}`,
`{"op":"reset","qubit":0}`, or
`{"op":"if","bit":0,"equals":true,"gate":{"kind":"x","target":1}}`.
Gate kinds: `h,x,y,z,s,t,cx,cz,swap`; two-wire gates use
`control,target` or `a,b`. Unrecognized fields, unsupported gates,
uninitialized classical control, duplicate two-wire indexes and impossible
resource budgets fail closed.

The response has `schema="aura.quantum.circuit-result/1"`,
`status="simulated"`, `engine_version`, `profile`, `seed`, `shots`,
`qubits`, `classical_bits`, `norm_tolerance`, and
`evidence_class="simulation"`, `qpu_hardware_jobs=0`. Each item in
`results` contains nullable classical `bits` and ordered measurement/reset
`trace`, not a fabricated hardware receipt.

**CLI admission caps:** input <= 256 KiB, qubits 1..12, classical bits <=64,
shots 1..2048, operations <=256, total `shots * operations * 2^qubits`
<= 8,388,608 (using at least one operation for the work estimate),
requested logical memory budget 1..16 MiB and encoded response <=16 MiB.
These limits constrain local work and output; OS RSS, JSON parse
allocations and the caller's input are **not** covered by the underlying
`Circuit` logical state/output accounting. The output consists of a full
ordered shot trace and is not streamed. This is a local CPU-only process:
no network, hardware, credentials, provider calls or billable submission.

The JSON adapter adds exact direct dependencies `serde=1.0.229` and
`serde_json=1.0.154` to the isolated crate only. Their transitive versions
are **not** locked to a committed `Cargo.lock` in this increment; full
hash-locked cross-environment reproducibility is pending integration review.


## Bounded noise simulation — AURA-Q28-007 (independent Rust API)

The optional `aura_quantum_sim::density` module provides a physically
specified **idealized density-matrix** reference for single-qubit Kraus noise,
rather than pretending that a pure statevector directly models mixed noise.

```sh
cargo run --manifest-path aura-quantum-sim/Cargo.toml --example density_noise
```

```rust
use aura_quantum_sim::density::{DensityMatrix, NoiseChannel};
use aura_quantum_sim::{Gate, DEFAULT_MEMORY_BUDGET};

let mut rho = DensityMatrix::zero(1, DEFAULT_MEMORY_BUDGET)?;
rho.apply_gate(Gate::H(0))?;
rho.apply_noise(NoiseChannel::AmplitudeDamping { qubit: 0, gamma: 0.36 })?;
rho.apply_noise(NoiseChannel::PhaseFlip { qubit: 0, probability: 0.25 })?;
assert!((rho.basis_probability(0)? - 0.68).abs() < 1e-10);
assert!((rho.element(0, 1)?.re - 0.20).abs() < 1e-10);
assert!((rho.purity()? - 0.6448).abs() < 1e-10);
```

Scientific meaning: `PhaseFlip(p)` is the CPTP map
`(1-p)rho + p Z rho Z`. `AmplitudeDamping(gamma)` has Kraus
`K0=|0><0|+sqrt(1-gamma)|1><1|` and
`K1=sqrt(gamma)|0><1|`. Both parameters must be finite in [0,1].
The method preserves trace and complete positivity **in exact arithmetic**.
`DensityMatrix::from_statevector` constructs a pure outer product for
external numerical comparison. The new API supports the existing H/X/Y/Z/S/T,
CX/CZ/SWAP families through `rho → U rho U†`. It exposes diagonal Born
probabilities, row/column coherence, single-qubit partial traces and
`Tr(rho²)` purity. This is a **mixed-state numerical model**, not
a seeded stochastic trajectory or a hardware-calibrated noise measurement.

The profile is `local-density-matrix-f64-cptp/1`. Width is explicitly
1..8 qubits and logical matrix allocation is `16·4^n` bytes under a
caller-supplied budget. The implementation performs in-place local channel
maps and gate conjugation, but physical process RSS, proof of complete
positivity under finite rounding and a universal error bound are **not**
certified. A necessary numerical trace/Hermiticity check is implemented;
a full positive-eigenvalue certificate is **not** claimed. Invalid
probabilities, aliases, indices, insufficient memory and unsupported widths
fail closed before mutation.

**Contract boundary:** This is an additive, library-only Agent C research
feature. It intentionally does NOT extend `aura.quantum.circuit-request/1`,
the pure-state `Circuit` runner, QIR/Aura compiler semantics, OpenQASM,
provider targets, QPU jobs or calibration snapshots. Connecting this noise
model to the shared dynamic JSON protocol is **D2** and requires producer/
consumer coordination and versioned schema acceptance before it occurs.
Independent Q23/Q37 scientific review and real integration to main remain
pending. Tests: `tests/density_noise.rs` and `examples/density_noise.rs`.
