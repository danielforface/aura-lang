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


## Noisy dynamic circuits and offline JSON CLI (Agent C, experimental v1)

**BEFORE:** density-matrix Kraus noise was callable only as separate matrix
operations, and the prior `aura-quantum-run` JSON v1 handled pure-state
dynamic circuits without noise.

**AFTER:** the additive `noisy_circuit` Rust API executes actual mixtures,
ideal Z instruments, classical correction gates and conditional noise.
A completely separate binary, `aura-quantum-noisy-run`, accepts the **new**
experimental schema `aura.quantum.noisy-request/1` and emits
`aura.quantum.noisy-result/1`. It **does not change** pure
`aura.quantum.circuit-request/1` or the preexisting `aura-quantum-run`.

Runnable independent examples:

```sh
cargo run --manifest-path aura-quantum-sim/Cargo.toml --example noisy_feedback
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml --bin aura-quantum-noisy-run \
  < aura-quantum-sim/fixtures/noisy_bell_feedforward_v1.json
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml --bin aura-quantum-noisy-run \
  < aura-quantum-sim/fixtures/noisy_tomography_v1.json
```

Noisy instructions: `{"op":"gate","gate":{"kind":"h","target":0}}`,
`{"op":"noise","channel":{"kind":"amplitude_damping","qubit":0,"gamma":0.36}}`,
`{"op":"noise","channel":{"kind":"phase_flip","qubit":0,"probability":0.25}}`,
`{"op":"measure","qubit":0,"into_bit":0}`,
`{"op":"reset","qubit":0}` (nonselective CPTP; no random draw),
`{"op":"reset_observed","qubit":0}` (sampled instrument + conditional X;
internal prior outcome **exposed**), and
`{"op":"if","bit":0,"equals":true,"action":{"kind":"noise","channel":{...}}}`
(with `action.kind=gate` for feedback unitaries). Invalid, unknowable
classical bits and even invalid **unreachable** branches are rejected before
execution. `Reset` and `ResetObserved` are explicitly different operations
in this profile; neither is silently treated as ordinary pure-state gate.

Result fields: seed, engine, profile, density_profile, norm_tolerance,
`evidence_class="simulation"`, `qpu_hardware_jobs=0`, per-shot
nullable classical bit arrays, ordered reset/measurement event traces and
`histogram` keyed by classical bit strings **MSB-first**, where `?`
denotes an unwritten classical bit. `inspect_final_density=true` is
supported **only for one shot and at most four qubits**; it includes complete
row-major complex pairs `[re,im]`, Born diagonal, trace and purity from
the **actual final mixed state after conditional instruments**, not just a
state-label or approximate visual mock.

Offline admission: maximum 256 KiB request, six qubits, 32 classical bits,
1024 shots, 192 operations, 4,194,304 matrix-work units
(`shots * operations * 4^qubits`, treating zero operations as one),
16 MiB logical budget and serialized output. The underlying density
matrix itself supports up to 8 qubits; the CLI intentionally uses a stricter
six-qubit budget. This guard is not a physical RSS or CPU time deadline;
full interpreter/compiler validation and per-target resource proof remain
separate responsibilities.

Ideal channels: amplitude damping `gamma` and phase flip `p`, both
finite [0,1] local parameters; **not calibrated noise levels**. Conditional
measurement on mixed states uses postselected density matrices, not a
fabricated pure trajectory. Because storage uses f64, extreme tiny physical
weights may underflow to zero, and numeric positive-semidefiniteness is
checked only by experimental fixtures, not independently certified under
arbitrary sequences. `Trace(\u03c1)`, Hermiticity and diagonal sanity
are required by the engine; full mathematical positivity is not formally
proven.

**Scientific evidence:** a separate independent Qiskit 2.6.1 /
NumPy 2.3.4 Python 3.11 harness compares 24 deterministic 1–3 qubit
noisy circuits, native Kraus operators, Qiskit `DensityMatrix.evolve`,
and Qiskit's nonselective reset against the Rust **full complex density
matrix** (not merely probabilities). No QPU, cloud or hardware credentials.
Independent Q23/Q37 scientific review, producer-consumer approval for
future Quantum IR / language integration, and global Governance Issue #100
remain distinct **NOT YET QUALIFIED** gates.

Source API: `src/noisy_circuit.rs`; JSON adapter `src/noisy_json.rs`;
real binary `src/bin/aura-quantum-noisy-run.rs`; acceptance
`tests/noisy_circuit.rs` and `tests/noisy_json.rs`;
external oracle `tools/check_qiskit_noisy.py`.


## Clifford stabilizer tableau backend — AURA-Q28-006

The additive `aura_quantum_sim::stabilizer` module implements a
**separate binary symplectic signed-Pauli 2n-row tableau**, not a
statevector wrapper. Stabilizer states and Clifford operations use
integer bit operations. H/X/Y/Z/S/CX/CZ/SWAP, ideal projective Z
measurement, observed reset, classical conditional gates, explicit
postselection and repeatable seeded shot batches are supported. T
and non-Clifford operations, Kraus noise/mixed-state dynamics, arbitrary
unitary rotations, hardware jobs and equivalence proofs are **not**.

```sh
cargo run --manifest-path aura-quantum-sim/Cargo.toml --example stabilizer_ghz
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml \
  --bin aura-quantum-stabilizer-run < aura-quantum-sim/fixtures/stabilizer_bell_feedback_v1.json
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml \
  --bin aura-quantum-stabilizer-run < aura-quantum-sim/fixtures/stabilizer_bell_inspection_v1.json
```


The standalone *new* JSON schema `aura.quantum.stabilizer-request/1`
returns `aura.quantum.stabilizer-result/1`, with exact
signed Pauli generators (q[n-1]..q0), classical bit histograms,
measurement/reset traces and simulated-only provenance.
`inspect_probabilities=true` returns a complete ordered
basis-probability array only for **one shot and at most six qubits**;
no imaginary exponential-speed claim. The Rust tableau supports up to
256 qubits, whereas this CLI admits at most 128, 1,024 shots,
512 operations, 64 classical bits, 256 KiB input, 16 MiB logical budget
and encoded output, and 8,388,608 operation×shot×qubit work units.
The logical estimate covers the 2n rows plus one scratch row and declared
shot results, **not** allocator padding/OS RSS, user-provided circuit
storage, caller clones, or CPU-time proof. Runtime is polynomial in n
for supported Clifford operations, but not constant-time.

Scientific boundaries: a 96-qubit GHZ **single Tableau** is tested
without materializing 2^96 complex amplitudes, and small widths are
tested against the independently implemented statevector engine.
Additional Qiskit 2.6.1 / NumPy 2.3.4 parity compares full basis Born
probabilities for 48 deterministic 1..6-qubit Clifford circuits
(1,008 basis values), including mutated negative controls. This
does **not** certify arbitrary adaptive stabilizer traces formally;
separate exact seed-by-seed measurements are tested in Rust.
The existing pure/noisy JSON v1 schemas are **unchanged** and no
Q22/Q23/Q27 consumer may treat the new schema as a frozen IR without
a reviewed producer/consumer contract.


## Exact small symbolic Clifford+T phase oracle — AURA-Q28-010

The **distinct** `aura_quantum_sim::symbolic` Rust engine represents each
amplitude *exactly* as an element of
`(a + b√2 + i(c + d√2)) / 2^k` with checked **signed i128**
integer coefficients and an explicitly bounded shared power-of-two
denominator. It executes the existing nine H/X/Y/Z/S/T/CX/CZ/SWAP
gate families with no floating-point arithmetic in the symbolic update,
including non-Clifford T phases. It returns exact complex coherences,
Born joint weights `(a+b√2)/2^k` and exact ray equivalence
via cross-multiplication (including global-phase invariance). Any
integer overflow fails closed, not modulo wrapping.

Projective Z measurements are **forced branches**, not stochastic
sampling or normalized postselection. This distinction is essential:
the normalized amplitude after selecting a general Clifford+T
outcome need not belong to the same elementary coefficient ring.
Instead the engine preserves an *unnormalized* conditional vector and
returns **exact joint weight before/after** each projection. The exact
conditional probability is the **ratio** of those two algebraic
weights. A numerical approximate ratio is separately labeled and is
**never used to evolve the symbolic state**.

```sh
cargo run --manifest-path aura-quantum-sim/Cargo.toml --example symbolic_phase
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml \
  --bin aura-quantum-symbolic-run < aura-quantum-sim/fixtures/symbolic_hth_v1.json
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml \
  --bin aura-quantum-symbolic-run < aura-quantum-sim/fixtures/symbolic_bell_forced_v1.json
```


The new bounded offline CLI uses experimental strict
`aura.quantum.symbolic-request/1` →
`aura.quantum.symbolic-result/1`, totally disjoint
from the existing pure/noisy/stabilizer schemas. **Exact signed i128
coefficients are encoded as decimal strings**, avoiding JavaScript's
53-bit integer precision limit. A `float` approximation exists
solely for display, testing and independent numerical-oracle comparison.
No QPU submission, network, seed or shots are accepted. Limits:
1–4 qubits, at most 128 instructions, at most 48 H/T operations,
16 classical bits, 64 KiB request, 1 MiB encoded response and
16 MiB logical memory admission (excluding runtime/OS RSS).

Acceptance: analytic `H T H |0>` produces exact Born values
`(2+√2)/4` and `(2−√2)/4`; a forced Bell
branch preserves exact joint probability 1/2 with a classically
controlled correction. Small deterministic mixed Clifford+T
circuits are cross-checked against the existing numerical
Statevector, and an independent Qiskit 2.6.1 / NumPy 2.3.4 suite
compares *full complex amplitudes* and exact transport for
64 unitary + 16 projected cases (1–4 qubits). The latter is
only **empirical parity**, not formal Q23 equivalence or a universal
numeric bound. This is an experimental finite-width CPU oracle,
NOT a universal high-performance Quantum runtime or a released
Aura/QIR feature.


## C-011 — Capability-first CPU quantum dispatcher

A fifth executable, aura-quantum-auto-run, is a standalone *router*, not a new physical simulator. It chooses among the four already-tested Rust engines through a strict new experimental JSON schema: aura.quantum.dispatch-request/1 → aura.quantum.dispatch-result/1.

- mode=auto + sampled Clifford gates → polynomial-memory stabilizer when the declared shot/width profile admits it.
- mode=auto + T/non-Clifford sampled gates → bounded numerical pure Statevector.
- mode=auto + CPTP noise / conditional noise / observed reset / tomography → bounded density matrix.
- mode=exact_branch → exact signed-i128 Clifford+T symbolic oracle. Outcome must be explicitly selected; sampling/seed are forbidden.
- mode=statevector, mode=stabilizer or mode=density → forced engine for direct comparisons.

The dispatcher returns the selected backend, selection reason and complete unchanged engine_result JSON, not a new interpretation of quantum physics. Every engine validates the request and resource limits independently. No fallback after backend execution failure; no silent conversion between exact and approximate, stochastic and forced outcomes, or pure and noisy models. Unknown operations, impossible gates, inconsistent classical conditions, over-budget work, unauthorized hardware flags and incompatible profiles fail closed.

Try: cargo run --manifest-path aura-quantum-sim/Cargo.toml --example dispatch_four_profiles
Or pipe each fixtures/dispatch_*_v1.json to cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml --bin aura-quantum-auto-run.

Tests: tests/dispatch_json.rs checks real CLI, all four modes, seed replay, nested result parity, invalid fields, fail-closed edge cases and no QPU use. The tools/check_dispatch_equivalence.py script invokes the actual dispatcher AND selected old engine binaries on 100 deterministic positive cases, comparing complete JSON results, and exercises 17 negative cases. No QIR, Aura source syntax, hardware, GPU or formal theorem claim. Independent existing Qiskit oracles and CPU scaling checks stay enabled. Limits remain those of the selected engine; envelope <=128 qubits, <=64 classical bits, <=512 instructions, <=256 KiB request, <=16 MiB response.



## C-012 / Q28-008 — Untruncated disconnected-factor quantum simulator

`aura-quantum-factorized-run` is a **new real CPU simulator** for a
global *pure* state that factors into multiple small complex vectors.
It is NOT a generic MPS/SVD network, a state-of-the-art tensor
compression algorithm, an approximate rank-truncation scheme or
support for arbitrary globally entangled 128-qubit states. It uses
the existing validated Statevector gate engine *inside each factor*.
Cross-factor CNOT/CZ atomically join factors; a SWAP between
disconnected components exchanges their wire labels without
needlessly merging them. Any attempted merge beyond **10 qubits
in one factor** fails closed with no truncation.

The supported pure gate set is H/X/Y/Z/S/T/CX/CZ/SWAP, including
non-Clifford T; seeded computational Z measurement, observed
reset and classical feedforward are also supported. Up to **128
total qubits** may be represented if no connected factor exceeds
10. Example: 64 independent Bell pairs with T gates on each
pair have 128 total qubits but only 64 components of two qubits:
their **logical stored amplitudes occupy 4,096 bytes**, not
(2^{128}) complex entries. Temporary factor joins and shot
output budgets are separately checked; actual process RSS,
memory allocator overhead and input bytes are excluded from this
logical estimate.

The numerical arithmetic inside each factor remains **IEEE-754
complex f64**, with the old `NORM_TOLERANCE`. Tensor
**truncation error is exactly zero** because no truncation is
performed; this is *not* a guarantee of zero floating-point
rounding error. No dense-state approximation or implicit
conditioning happens when a merge exceeds the explicit width.
After a projective Z measurement, the measured coordinate is known
to be separable by the exact measurement postulate. C-013 now
**releases that wire as a one-qubit factor** (including its full
postselected complex phase), provided the old and replacement
vectors fit the checked temporary logical-memory budget. The rest of
the factor remains together even when further separation might be
possible: this is *not* tolerance-based Schmidt/SVD splitting.
If memory prevents the optional topology change, the already-collapsed
original factor remains valid; that deferred optimization is visible
in the result and later over-width joins still reject.

Run real apps and JSON:

```sh
cargo run --manifest-path aura-quantum-sim/Cargo.toml --example factorized_128_bell_t
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml \
  --bin aura-quantum-factorized-run < aura-quantum-sim/fixtures/factorized_128_bell_t_v1.json
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml \
  --bin aura-quantum-auto-run < aura-quantum-sim/fixtures/dispatch_factorized_128_v1.json
```

The additive strict `aura.quantum.factorized-request/1` →
`aura.quantum.factorized-result/1` API supports one-shot
`inspect_factors`, optional `inspect_basis` (128-bit Boolean
vector) and complete complex amplitude inspection at **≤8 wires**.
For sampled execution all inspections require exactly one shot,
never turning measurements into forced symbolic postselection.
Limits: ≤128 wires, ≤10 per factor, ≤512 instructions,
≤1024 shots, ≤64 classical bits, ≤256 KiB request,
≤16 MiB output, plus an explicit CPU-work admission limit.
All invalid hardware flags, unknown gates and unsupported
noise are rejected, and every result states QPU jobs = 0.

The pre-existing unified `aura-quantum-auto-run` now includes an
**additive fifth engine** (`selected_backend=factor_product`)
when an admitted sampled *non-Clifford* circuit exceeds the
ordinary pure Statevector 12-qubit width, or when explicit
`mode=factorized` or factor inspection is requested. It does
not silently downgrade to numerical approximations or retry
another backend when the factor-width limit is exceeded.
Previous four direct JSON request schemas are unchanged.
Verification uses full dense 1–8-wire Rust statevector phase
comparison, adaptive seed/trace equality, high-width Bell/T,
strict negative controls and external Qiskit full complex
amplitude parity (separately logged; not a formal proof).


## C-013 — Projective wire release and 128-wire mid-circuit reuse

A mid-circuit projective Z measurement on any nontrivial factor
**guarantees** mathematical separation
`|measured_bit> ⊗ |conditional_rest>`. The factor engine
now extracts the measured coordinate after the existing trusted seeded
measurement without a second random draw, retaining all complex
amplitudes of the remaining conditional state with their phases.
There is no Schmidt approximation or truncation; the usual floating
`f64` numerical limitations still apply.

The implementation creates the remaining and measured states
*before* committing a topology change. During this operation the
old vector and both new vectors count against a temporary logical
memory budget. If allocation or the budget prevents splitting,
the already-collapsed factor stays intact and simulations remain
physically consistent, albeit possibly unable to accept a later
join exceeding width 10. A structurally impossible or numerically
invalid state is not silently reinterpreted. Single-wire repeat
measurements do not invent additional releases or RNG changes.

Each factorized JSON result now includes additive, non-breaking
counters `projective_wire_splits` and
`projective_split_deferred` (totals over all
shots). A user can inspect the exact factor wire labels after a
single shot using `inspect_factors`; the default result
schema remains `aura.quantum.factorized-result/1`.
The unified `aura-quantum-auto-run` reports the
original complete factor result unchanged inside `engine_result`.

**Previously unsupported real program:** 128 qubits, repeatedly
apply H(0), CX(0,new), measure q0, then conditionally X(q0) to
reinitialize it, on **127 different fresh target qubits**.
With four non-Clifford T gates on independently measured targets,
that is the full **512 instruction admission limit**.
Without projective release, the old engine reaches an 11-wire
factor and rejects. With C-013, the program completes,
uses 4,096 current logical state bytes (plus bounded temporary
copies, classical trace and allocator overhead), has 128 separate
one-qubit factors, 127 genuine postselection-driven factor splits,
no deferred splits and no QPU jobs. It stores the stochastic Z
measurement trace and tests the final selected 128-wire basis
probability. **This is not a 128-way globally entangled state.**

```sh
cargo run --manifest-path aura-quantum-sim/Cargo.toml --example factorized_projective_reuse
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml \
  --bin aura-quantum-factorized-run < aura-quantum-sim/fixtures/factorized_projective_reuse_v1.json
cargo run --quiet --manifest-path aura-quantum-sim/Cargo.toml \
  --bin aura-quantum-auto-run < aura-quantum-sim/fixtures/dispatch_projective_reuse_v1.json
```


Validation: `tests/factorized_recycling.rs` compares complete
complex dense amplitudes (including phases) and seeded RNG events,
demonstrates a 10-wire entangled factor releasing a wire to connect
an 11th, and verifies tight-memory deferral is non-mutating for
factor topology. A separate pinned Qiskit dynamic oracle
`tools/check_qiskit_projective_recycling.py` independently
evolves each gate and conditions on the *actual* simulator
measurement/reset trace, comparing full final complex amplitudes
over 56 circuits of 2–8 wires, including T and cross-factor SWAP.
The prior five-backend Qiskit tests, resource controls and direct
CLI parity remain enabled. This is Q28-008/Q28-011 scoped simulation
assurance, **not QIR, hardware, a formal proof or Q28-wide release**.
