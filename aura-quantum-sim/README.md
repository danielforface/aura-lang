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
