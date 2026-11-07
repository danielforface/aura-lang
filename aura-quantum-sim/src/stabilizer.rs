//! Independent binary-symplectic Clifford tableau simulator (AURA-Q28-006).
//!
//! This is a polynomial-memory CPU simulator for STABILIZER states only.
//! It is *not* implemented by running the statevector or density engine.
//! It rejects T (non-Clifford), arbitrary phase rotations, hardware jobs,
//! general Kraus noise and mixed-state channels. Pauli signs and projective
//! Z measurement follow the stabilizer tableau algorithm.
//!
//! q0 is the least-significant basis bit, and generator labels print q[n-1]..q0.
//! A seeded SplitMix64 draw is consumed by every explicit measurement/reset,
//! including a deterministic result, matching the existing pure circuit API.
//! This numerical implementation uses exact bit operations for Clifford rows;
//! the resulting finite-shot Born samples remain statistical observations.
use crate::{Gate, SeededRng, ENGINE_VERSION};
use std::mem::size_of;

pub const MAX_STABILIZER_QUBITS: usize = 256;
pub const MAX_STABILIZER_OPERATIONS: usize = 4096;
pub const MAX_STABILIZER_SHOTS: usize = 4096;
pub const MAX_STABILIZER_WORK_UNITS: usize = 64 * 1024 * 1024;
pub const STABILIZER_PROFILE: &str = "local-clifford-tableau-binary-symplectic/1";
pub const STABILIZER_RESULT_SCHEMA: &str = "aura.quantum.stabilizer-result/1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StabilizerError {
    UnsupportedWidth { qubits: usize, max: usize },
    UnsupportedNonClifford,
    InvalidQubit { qubit: usize, qubits: usize },
    DuplicateQubit(usize),
    InvalidBasisWidth { expected: usize, got: usize },
    ImpossibleOutcome,
    ZeroShots,
    TooManyShots,
    TooManyInstructions,
    ArithmeticOverflow,
    BudgetExceeded { required_bytes: usize, budget_bytes: usize },
    AllocationFailed,
    InvalidClassicalBit { bit: usize, bits: usize },
    ReadBeforeWrite { bit: usize, instruction: usize },
    WorkLimit,
}

impl std::fmt::Display for StabilizerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StabilizerError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PauliRow {
    x: Vec<u64>,
    z: Vec<u64>,
    sign: bool,
}
fn words(qubits: usize) -> usize { qubits.div_ceil(64) }
fn bit(data: &[u64], q: usize) -> bool {
    (data[q / 64] >> (q % 64)) & 1 != 0
}
fn assign(data: &mut [u64], q: usize, value: bool) {
    let mask = 1u64 << (q % 64);
    if value { data[q / 64] |= mask; } else { data[q / 64] &= !mask; }
}
impl PauliRow {
    fn empty(word_count: usize) -> Result<Self, StabilizerError> {
        fn zero_words(n: usize) -> Result<Vec<u64>, StabilizerError> {
            let mut v = Vec::new();
            v.try_reserve_exact(n).map_err(|_| StabilizerError::AllocationFailed)?;
            v.resize(n, 0);
            Ok(v)
        }
        Ok(Self {
            x: zero_words(word_count)?,
            z: zero_words(word_count)?,
            sign: false,
        })
    }
    fn x(&self, q: usize) -> bool { bit(&self.x, q) }
    fn z(&self, q: usize) -> bool { bit(&self.z, q) }
    fn set_x(&mut self, q: usize, value: bool) { assign(&mut self.x, q, value); }
    fn set_z(&mut self, q: usize, value: bool) { assign(&mut self.z, q, value); }

    /// Multiply two commuting signed tensor-Paulis, preserving the Hermitian
    /// phase. Each local factor is I/X/Y/Z, Y=iXZ. This is distinct from a
    /// floating-point matrix oracle and never guesses a global phase.
    fn multiply(&mut self, rhs: &Self, qubits: usize) {
        let mut phase = 2 * i32::from(self.sign) + 2 * i32::from(rhs.sign);
        for q in 0..qubits {
            let x1 = self.x(q);
            let z1 = self.z(q);
            let x2 = rhs.x(q);
            let z2 = rhs.z(q);
            phase += match (x1, z1) {
                (false, false) => 0,
                (true, true) => i32::from(z2) - i32::from(x2),
                (true, false) => i32::from(z2) * (2 * i32::from(x2) - 1),
                (false, true) => i32::from(x2) * (1 - 2 * i32::from(z2)),
            };
        }
        // Multiplication is used only for commuting stabilizer rows.
        debug_assert_eq!(phase.rem_euclid(2), 0);
        self.sign = phase.rem_euclid(4) == 2;
        for (a, b) in self.x.iter_mut().zip(&rhs.x) { *a ^= b; }
        for (a, b) in self.z.iter_mut().zip(&rhs.z) { *a ^= b; }
    }

    fn label(&self, qubits: usize) -> String {
        let mut result = String::with_capacity(qubits + 1);
        result.push(if self.sign { '-' } else { '+' });
        for q in (0..qubits).rev() {
            result.push(match (self.x(q), self.z(q)) {
                (false, false) => 'I',
                (true, false) => 'X',
                (false, true) => 'Z',
                (true, true) => 'Y',
            });
        }
        result
    }
}

fn tableau_logical_bytes(qubits: usize) -> Result<usize, StabilizerError> {
    if !(1..=MAX_STABILIZER_QUBITS).contains(&qubits) {
        return Err(StabilizerError::UnsupportedWidth {
            qubits, max: MAX_STABILIZER_QUBITS,
        });
    }
    // The tableau stores 2n rows; one extra scratch/pivot row is reserved
    // in the logical estimate. Allocator padding and process RSS excluded.
    let row_bytes = words(qubits)
        .checked_mul(2 * size_of::<u64>())
        .and_then(|b| b.checked_add(size_of::<PauliRow>()))
        .ok_or(StabilizerError::ArithmeticOverflow)?;
    (2 * qubits + 1).checked_mul(row_bytes)
        .ok_or(StabilizerError::ArithmeticOverflow)
}

/// 2n-row Aaronson-Gottesman-style tableau: first n destabilizers
/// (initial X_q), last n stabilizers (initial Z_q).
#[derive(Debug, Clone)]
pub struct StabilizerTableau {
    qubits: usize,
    rows: Vec<PauliRow>,
    logical_bytes: usize,
}

impl StabilizerTableau {
    pub fn zero(qubits: usize, budget: usize) -> Result<Self, StabilizerError> {
        let bytes = tableau_logical_bytes(qubits)?;
        if bytes > budget {
            return Err(StabilizerError::BudgetExceeded {
                required_bytes: bytes, budget_bytes: budget,
            });
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(2 * qubits)
            .map_err(|_| StabilizerError::AllocationFailed)?;
        for index in 0..2 * qubits {
            let mut row = PauliRow::empty(words(qubits))?;
            if index < qubits { row.set_x(index, true); }
            else { row.set_z(index - qubits, true); }
            rows.push(row);
        }
        Ok(Self { qubits, rows, logical_bytes: bytes })
    }

    pub fn qubits(&self) -> usize { self.qubits }
    pub fn logical_bytes(&self) -> usize { self.logical_bytes }
    pub fn stabilizer_generators(&self) -> Vec<String> {
        self.rows[self.qubits..].iter()
            .map(|row| row.label(self.qubits)).collect()
    }

    fn valid_qubit(&self, q: usize) -> Result<(), StabilizerError> {
        if q >= self.qubits {
            return Err(StabilizerError::InvalidQubit {
                qubit: q, qubits: self.qubits,
            });
        }
        Ok(())
    }
    fn valid_pair(&self, a: usize, b: usize) -> Result<(), StabilizerError> {
        self.valid_qubit(a)?;
        self.valid_qubit(b)?;
        if a == b { return Err(StabilizerError::DuplicateQubit(a)); }
        Ok(())
    }

    /// Gate validation can be reused by the circuit preflight without making
    /// a temporary tableau or touching any state.
    fn validate_gate(&self, gate: &Gate) -> Result<(), StabilizerError> {
        match gate {
            Gate::H(q) | Gate::X(q) | Gate::Y(q) | Gate::Z(q)
            | Gate::S(q) => self.valid_qubit(*q),
            Gate::T(_) => Err(StabilizerError::UnsupportedNonClifford),
            Gate::Cnot { control, target } | Gate::Cz { control, target } =>
                self.valid_pair(*control, *target),
            Gate::Swap { a, b } => self.valid_pair(*a, *b),
        }
    }

    fn hadamard(&mut self, q: usize) {
        for row in &mut self.rows {
            let x = row.x(q);
            let z = row.z(q);
            row.sign ^= x && z;
            row.set_x(q, z);
            row.set_z(q, x);
        }
    }
    fn phase(&mut self, q: usize) {
        for row in &mut self.rows {
            let x = row.x(q);
            let z = row.z(q);
            row.sign ^= x && z;
            row.set_z(q, z ^ x);
        }
    }
    fn controlled_not(&mut self, control: usize, target: usize) {
        for row in &mut self.rows {
            let xc = row.x(control);
            let zc = row.z(control);
            let xt = row.x(target);
            let zt = row.z(target);
            row.sign ^= xc && zt && (xt ^ zc ^ true);
            row.set_x(target, xt ^ xc);
            row.set_z(control, zc ^ zt);
        }
    }

    /// Clifford gates are exact in binary tableau arithmetic. T is rejected
    /// even though it is supported by the separate statevector engine.
    pub fn apply(&mut self, gate: Gate) -> Result<(), StabilizerError> {
        self.validate_gate(&gate)?;
        match gate {
            Gate::H(q) => self.hadamard(q),
            Gate::S(q) => self.phase(q),
            Gate::X(q) => for row in &mut self.rows { row.sign ^= row.z(q); },
            Gate::Y(q) => for row in &mut self.rows {
                row.sign ^= row.x(q) ^ row.z(q);
            },
            Gate::Z(q) => for row in &mut self.rows { row.sign ^= row.x(q); },
            Gate::Cnot { control, target } => self.controlled_not(control, target),
            Gate::Cz { control, target } => {
                self.hadamard(target);
                self.controlled_not(control, target);
                self.hadamard(target);
            }
            Gate::Swap { a, b } => {
                self.controlled_not(a, b);
                self.controlled_not(b, a);
                self.controlled_not(a, b);
            }
            Gate::T(_) => return Err(StabilizerError::UnsupportedNonClifford),
        }
        Ok(())
    }

    fn random_pivot(&self, q: usize) -> Option<usize> {
        (self.qubits..2 * self.qubits).find(|&row| self.rows[row].x(q))
    }

    fn deterministic_outcome(&self, q: usize) -> bool {
        let mut scratch = PauliRow::empty(words(self.qubits))
            .expect("one scratch tableau row has already been budgeted");
        for i in 0..self.qubits {
            if self.rows[i].x(q) {
                scratch.multiply(&self.rows[self.qubits + i], self.qubits);
            }
        }
        scratch.sign
    }

    fn collapse_random(&mut self, q: usize, pivot: usize, outcome: bool) {
        let source = self.rows[pivot].clone();
        for i in 0..2 * self.qubits {
            // The matching destabilizer anticommutes with its pivot and
            // will be overwritten: never multiply an anti-Hermitian row.
            if i != pivot && i != pivot - self.qubits && self.rows[i].x(q) {
                self.rows[i].multiply(&source, self.qubits);
            }
        }
        self.rows[pivot - self.qubits] = source;
        let mut measured = PauliRow::empty(words(self.qubits))
            .expect("one replacement row has already been budgeted");
        measured.set_z(q, true);
        measured.sign = outcome;
        self.rows[pivot] = measured;
    }

    /// Exact Born branch probability 0, 1/2 or 1 for a stabilizer state.
    pub fn wire_probability(&self, q: usize, outcome: bool)
        -> Result<f64, StabilizerError>
    {
        self.valid_qubit(q)?;
        if self.random_pivot(q).is_some() { Ok(0.5) }
        else { Ok(f64::from(self.deterministic_outcome(q) == outcome)) }
    }

    /// Force a projective Z branch. Impossible selections leave state intact.
    pub fn postselect(&mut self, q: usize, outcome: bool)
        -> Result<f64, StabilizerError>
    {
        self.valid_qubit(q)?;
        if let Some(pivot) = self.random_pivot(q) {
            self.collapse_random(q, pivot, outcome);
            Ok(0.5)
        } else if self.deterministic_outcome(q) == outcome {
            Ok(1.0)
        } else {
            Err(StabilizerError::ImpossibleOutcome)
        }
    }

    /// Draw once for every Z measurement, including deterministic outcomes.
    pub fn measure(&mut self, q: usize, rng: &mut SeededRng)
        -> Result<bool, StabilizerError>
    {
        self.valid_qubit(q)?;
        let random_choice = rng.next_f64() < 0.5;
        if let Some(pivot) = self.random_pivot(q) {
            self.collapse_random(q, pivot, random_choice);
            Ok(random_choice)
        } else {
            Ok(self.deterministic_outcome(q))
        }
    }

    /// Observed Z reset; unlike the density nonselective CPTP reset this
    /// samples an internal branch. Equivalent to pure-state measured reset.
    pub fn reset_observed(&mut self, q: usize, rng: &mut SeededRng)
        -> Result<bool, StabilizerError>
    {
        let outcome = self.measure(q, rng)?;
        if outcome { self.apply(Gate::X(q))?; }
        Ok(outcome)
    }

    /// Exact computational-basis probability, with bit order q0..q[n-1].
    /// The caller can inspect arbitrary Clifford-state marginals without
    /// materializing 2^n complex amplitudes. This clones a tableau, so the
    /// work memory of *the caller's clone* is not part of logical_bytes().
    pub fn basis_probability(&self, bits_lsb: &[bool])
        -> Result<f64, StabilizerError>
    {
        if bits_lsb.len() != self.qubits {
            return Err(StabilizerError::InvalidBasisWidth {
                expected: self.qubits, got: bits_lsb.len(),
            });
        }
        let mut copy = self.clone();
        let mut prob = 1.0;
        for (q, outcome) in bits_lsb.iter().enumerate() {
            match copy.postselect(q, *outcome) {
                Ok(p) => prob *= p,
                Err(StabilizerError::ImpossibleOutcome) => return Ok(0.0),
                Err(error) => return Err(error),
            }
        }
        Ok(prob)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StabilizerInstruction {
    Gate(Gate),
    Measure { qubit: usize, into_bit: usize },
    Reset { qubit: usize },
    If { bit: usize, equals: bool, gate: Gate },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StabilizerEventKind {
    Measure { into_bit: usize, value: bool },
    Reset { previous_value: bool },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilizerEvent {
    pub instruction: usize,
    pub qubit: usize,
    pub kind: StabilizerEventKind,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilizerShot {
    pub bits: Vec<Option<bool>>,
    pub trace: Vec<StabilizerEvent>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilizerMetadata {
    pub schema_version: &'static str,
    pub profile: &'static str,
    pub engine_version: &'static str,
    pub seed: u64,
    pub shots: usize,
    pub precision: &'static str,
    pub evidence_class: &'static str,
    pub qpu_hardware_jobs: u8,
}
impl StabilizerMetadata {
    fn new(seed: u64, shots: usize) -> Self {
        Self {
            schema_version: STABILIZER_RESULT_SCHEMA,
            profile: STABILIZER_PROFILE,
            engine_version: ENGINE_VERSION,
            seed, shots,
            precision: "exact-binary-Clifford-tableau",
            evidence_class: "simulation",
            qpu_hardware_jobs: 0,
        }
    }
}
#[derive(Debug, Clone)]
pub struct StabilizerSingleShot {
    pub metadata: StabilizerMetadata,
    pub shot: StabilizerShot,
    pub final_tableau: StabilizerTableau,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilizerBatch {
    pub metadata: StabilizerMetadata,
    pub records: Vec<StabilizerShot>,
}
#[derive(Debug, Clone)]
pub struct StabilizerCircuit {
    qubits: usize,
    classical_bits: usize,
    instructions: Vec<StabilizerInstruction>,
}
impl StabilizerCircuit {
    pub fn new(qubits: usize, classical_bits: usize) -> Self {
        Self { qubits, classical_bits, instructions: Vec::new() }
    }
    pub fn qubits(&self) -> usize { self.qubits }
    pub fn classical_bits(&self) -> usize { self.classical_bits }
    pub fn instructions(&self) -> &[StabilizerInstruction] { &self.instructions }
    pub fn push(&mut self, instruction: StabilizerInstruction) {
        self.instructions.push(instruction);
    }

    fn preflight(&self, shots: usize, budget: usize)
        -> Result<usize, StabilizerError>
    {
        if shots == 0 { return Err(StabilizerError::ZeroShots); }
        if shots > MAX_STABILIZER_SHOTS { return Err(StabilizerError::TooManyShots); }
        if self.instructions.len() > MAX_STABILIZER_OPERATIONS {
            return Err(StabilizerError::TooManyInstructions);
        }
        let state_bytes = tableau_logical_bytes(self.qubits)?;
        let work = self.instructions.len().max(1)
            .checked_mul(shots)
            .and_then(|v| v.checked_mul(self.qubits))
            .ok_or(StabilizerError::ArithmeticOverflow)?;
        if work > MAX_STABILIZER_WORK_UNITS { return Err(StabilizerError::WorkLimit); }

        let per_shot = self.classical_bits
            .checked_mul(size_of::<Option<bool>>())
            .and_then(|v| v.checked_add(size_of::<StabilizerShot>()))
            .ok_or(StabilizerError::ArithmeticOverflow)?;
        let minimum = per_shot.checked_mul(shots)
            .and_then(|v| v.checked_add(state_bytes))
            .ok_or(StabilizerError::ArithmeticOverflow)?;
        if minimum > budget {
            return Err(StabilizerError::BudgetExceeded {
                required_bytes: minimum, budget_bytes: budget,
            });
        }

        let mut written = Vec::new();
        written.try_reserve_exact(self.classical_bits)
            .map_err(|_| StabilizerError::AllocationFailed)?;
        written.resize(self.classical_bits, false);
        let mut trace_capacity = 0usize;
        // Validate without allocating a tableau; the verifier cannot depend
        // on which branches a particular seed happens to take.
        for (instruction, operation) in self.instructions.iter().enumerate() {
            let check_bit = |bit: usize| -> Result<(), StabilizerError> {
                if bit >= self.classical_bits {
                    Err(StabilizerError::InvalidClassicalBit {
                        bit, bits: self.classical_bits,
                    })
                } else { Ok(()) }
            };
            let check_qubit = |q: usize| -> Result<(), StabilizerError> {
                if q >= self.qubits {
                    Err(StabilizerError::InvalidQubit {
                        qubit: q, qubits: self.qubits,
                    })
                } else { Ok(()) }
            };
            let check_gate = |gate: &Gate| -> Result<(), StabilizerError> {
                match gate {
                    Gate::H(q) | Gate::X(q) | Gate::Y(q) | Gate::Z(q)
                    | Gate::S(q) => check_qubit(*q),
                    Gate::T(_) => Err(StabilizerError::UnsupportedNonClifford),
                    Gate::Cnot { control, target } | Gate::Cz { control, target } => {
                        check_qubit(*control)?;
                        check_qubit(*target)?;
                        if control == target {
                            Err(StabilizerError::DuplicateQubit(*control))
                        } else { Ok(()) }
                    }
                    Gate::Swap { a, b } => {
                        check_qubit(*a)?;
                        check_qubit(*b)?;
                        if a == b { Err(StabilizerError::DuplicateQubit(*a)) }
                        else { Ok(()) }
                    }
                }
            };
            match operation {
                StabilizerInstruction::Gate(gate) => check_gate(gate)?,
                StabilizerInstruction::Measure { qubit, into_bit } => {
                    check_qubit(*qubit)?;
                    check_bit(*into_bit)?;
                    written[*into_bit] = true;
                    trace_capacity = trace_capacity.checked_add(1)
                        .ok_or(StabilizerError::ArithmeticOverflow)?;
                }
                StabilizerInstruction::Reset { qubit } => {
                    check_qubit(*qubit)?;
                    trace_capacity = trace_capacity.checked_add(1)
                        .ok_or(StabilizerError::ArithmeticOverflow)?;
                }
                StabilizerInstruction::If { bit, gate, .. } => {
                    check_bit(*bit)?;
                    check_gate(gate)?;
                    if !written[*bit] {
                        return Err(StabilizerError::ReadBeforeWrite {
                            bit: *bit, instruction,
                        });
                    }
                }
            }
        }
        let per_shot_total = trace_capacity
            .checked_mul(size_of::<StabilizerEvent>())
            .and_then(|v| v.checked_add(per_shot))
            .ok_or(StabilizerError::ArithmeticOverflow)?;
        let total = per_shot_total.checked_mul(shots)
            .and_then(|v| v.checked_add(state_bytes))
            .ok_or(StabilizerError::ArithmeticOverflow)?;
        if total > budget {
            return Err(StabilizerError::BudgetExceeded {
                required_bytes: total, budget_bytes: budget,
            });
        }
        Ok(trace_capacity)
    }

    fn one(&self, rng: &mut SeededRng, budget: usize, cap: usize)
        -> Result<(StabilizerShot, StabilizerTableau), StabilizerError>
    {
        let mut tableau = StabilizerTableau::zero(self.qubits, budget)?;
        let mut bits = Vec::new();
        bits.try_reserve_exact(self.classical_bits)
            .map_err(|_| StabilizerError::AllocationFailed)?;
        bits.resize(self.classical_bits, None);
        let mut trace = Vec::new();
        trace.try_reserve_exact(cap)
            .map_err(|_| StabilizerError::AllocationFailed)?;
        for (instruction, op) in self.instructions.iter().enumerate() {
            match op {
                StabilizerInstruction::Gate(gate) => tableau.apply(gate.clone())?,
                StabilizerInstruction::Measure { qubit, into_bit } => {
                    let value = tableau.measure(*qubit, rng)?;
                    bits[*into_bit] = Some(value);
                    trace.push(StabilizerEvent {
                        instruction, qubit: *qubit,
                        kind: StabilizerEventKind::Measure {
                            into_bit: *into_bit, value,
                        },
                    });
                }
                StabilizerInstruction::Reset { qubit } => {
                    let previous_value = tableau.reset_observed(*qubit, rng)?;
                    trace.push(StabilizerEvent {
                        instruction, qubit: *qubit,
                        kind: StabilizerEventKind::Reset { previous_value },
                    });
                }
                StabilizerInstruction::If { bit, equals, gate } => {
                    let value = bits[*bit].ok_or(StabilizerError::ReadBeforeWrite {
                        bit: *bit, instruction,
                    })?;
                    if value == *equals { tableau.apply(gate.clone())?; }
                }
            }
        }
        Ok((StabilizerShot { bits, trace }, tableau))
    }

    pub fn execute_once(&self, seed: u64, budget: usize)
        -> Result<StabilizerSingleShot, StabilizerError>
    {
        let cap = self.preflight(1, budget)?;
        let mut rng = SeededRng::new(seed);
        let (shot, final_tableau) = self.one(&mut rng, budget, cap)?;
        Ok(StabilizerSingleShot {
            metadata: StabilizerMetadata::new(seed, 1),
            shot, final_tableau,
        })
    }
    pub fn execute_shots(&self, shots: usize, seed: u64, budget: usize)
        -> Result<StabilizerBatch, StabilizerError>
    {
        let cap = self.preflight(shots, budget)?;
        let mut records = Vec::new();
        records.try_reserve_exact(shots)
            .map_err(|_| StabilizerError::AllocationFailed)?;
        let mut rng = SeededRng::new(seed);
        for _ in 0..shots {
            let (shot, _) = self.one(&mut rng, budget, cap)?;
            records.push(shot);
        }
        Ok(StabilizerBatch {
            metadata: StabilizerMetadata::new(seed, shots),
            records,
        })
    }
}
