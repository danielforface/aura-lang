//! Dynamic mixed-state circuits: actual CPTP noise + ideal measurement and
//! seeded classical feed-forward on bounded CPU density matrices.
//!
//! This is an additive experimental *Rust* contract, not Aura QIR,
//! hardware/provider execution or a formal proof. One matrix state per shot
//! is prepared fresh, while all instruments share a versioned RNG stream.
//!
//! Difference between reset operations matters:
//! - Reset: deterministic nonselective CPTP reset; no sampled bit or RNG draw.
//! - ResetObserved: sampled Z measurement + X; reveals internal branch in
//!   simulation trace, just as the pre-existing pure statevector runner does.
use crate::density::{
    DensityError, DensityMatrix, NoiseChannel, DENSITY_PROFILE, MAX_DENSITY_QUBITS,
};
use crate::{Gate, SeededRng, ENGINE_VERSION, NORM_TOLERANCE};
use std::mem::size_of;

pub const NOISY_RESULT_SCHEMA: &str = "aura.quantum.noisy-result/1";
pub const NOISY_DYNAMIC_PROFILE: &str = "local-density-matrix-f64-dynamic-cptp/1";

#[derive(Debug, Clone, PartialEq)]
pub enum NoisyAction {
    Gate(Gate),
    Noise(NoiseChannel),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NoisyInstruction {
    Gate(Gate),
    Noise(NoiseChannel),
    Measure { qubit: usize, into_bit: usize },
    Reset { qubit: usize },
    ResetObserved { qubit: usize },
    If { bit: usize, equals: bool, action: NoisyAction },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoisyEventKind {
    Measure { into_bit: usize, value: bool },
    ResetUnobserved,
    ResetObserved { value: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoisyEvent {
    pub instruction: usize,
    pub qubit: usize,
    pub kind: NoisyEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoisyShot {
    pub bits: Vec<Option<bool>>,
    pub trace: Vec<NoisyEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoisyMetadata {
    pub schema_version: &'static str,
    pub engine_version: &'static str,
    pub profile: &'static str,
    pub density_profile: &'static str,
    pub precision: &'static str,
    pub norm_tolerance: f64,
    pub seed: u64,
    pub shots: usize,
    pub evidence_class: &'static str,
    pub qpu_hardware_jobs: u8,
}

impl NoisyMetadata {
    fn new(shots: usize, seed: u64) -> Self {
        Self {
            schema_version: NOISY_RESULT_SCHEMA,
            engine_version: ENGINE_VERSION,
            profile: NOISY_DYNAMIC_PROFILE,
            density_profile: DENSITY_PROFILE,
            precision: "f64-complex",
            norm_tolerance: NORM_TOLERANCE,
            seed,
            shots,
            evidence_class: "simulation",
            qpu_hardware_jobs: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NoisySingleShot {
    pub metadata: NoisyMetadata,
    pub shot: NoisyShot,
    pub final_density: DensityMatrix,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoisyShotBatch {
    pub metadata: NoisyMetadata,
    pub records: Vec<NoisyShot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoisyCircuitError {
    ZeroShots,
    InvalidClassicalBit { bit: usize, bits: usize },
    ReadBeforeWrite { bit: usize, instruction: usize },
    InvalidWidth { qubits: usize, max: usize },
    ArithmeticOverflow,
    BudgetExceeded { required_bytes: usize, budget_bytes: usize },
    AllocationFailed,
    Density(DensityError),
}

impl From<DensityError> for NoisyCircuitError {
    fn from(err: DensityError) -> Self {
        Self::Density(err)
    }
}

impl std::fmt::Display for NoisyCircuitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for NoisyCircuitError {}

#[derive(Debug, Clone)]
pub struct NoisyCircuit {
    qubits: usize,
    classical_bits: usize,
    instructions: Vec<NoisyInstruction>,
}

impl NoisyCircuit {
    pub fn new(qubits: usize, classical_bits: usize) -> Self {
        Self { qubits, classical_bits, instructions: Vec::new() }
    }

    pub fn qubits(&self) -> usize { self.qubits }
    pub fn classical_bits(&self) -> usize { self.classical_bits }
    pub fn instructions(&self) -> &[NoisyInstruction] { &self.instructions }
    pub fn push(&mut self, instruction: NoisyInstruction) {
        self.instructions.push(instruction);
    }

    fn check_qubit(&self, qubit: usize) -> Result<(), NoisyCircuitError> {
        if qubit >= self.qubits {
            return Err(DensityError::InvalidQubit {
                index: qubit, qubits: self.qubits,
            }.into());
        }
        Ok(())
    }

    fn check_classical(&self, bit: usize) -> Result<(), NoisyCircuitError> {
        if bit >= self.classical_bits {
            return Err(NoisyCircuitError::InvalidClassicalBit {
                bit, bits: self.classical_bits,
            });
        }
        Ok(())
    }

    fn check_gate(&self, gate: &Gate) -> Result<(), NoisyCircuitError> {
        match gate {
            Gate::H(q) | Gate::X(q) | Gate::Y(q)
            | Gate::Z(q) | Gate::S(q) | Gate::T(q) => self.check_qubit(*q),
            Gate::Cnot { control, target } | Gate::Cz { control, target } => {
                self.check_qubit(*control)?;
                self.check_qubit(*target)?;
                if control == target {
                    return Err(DensityError::DuplicateQubit(*control).into());
                }
                Ok(())
            }
            Gate::Swap { a, b } => {
                self.check_qubit(*a)?;
                self.check_qubit(*b)?;
                if a == b { return Err(DensityError::DuplicateQubit(*a).into()); }
                Ok(())
            }
        }
    }

    fn check_noise(&self, channel: &NoiseChannel) -> Result<(), NoisyCircuitError> {
        // The exact same CPTP parameter contract is applied before any shot
        // and on direct DensityMatrix operation, including untaken If actions.
        channel.validated_qubit(self.qubits)?;
        Ok(())
    }

    fn check_action(&self, action: &NoisyAction) -> Result<(), NoisyCircuitError> {
        match action {
            NoisyAction::Gate(g) => self.check_gate(g),
            NoisyAction::Noise(n) => self.check_noise(n),
        }
    }

    /// Validate the complete program before executing *any* shot, even
    /// unreachable classical branches. Cap logical live density matrix +
    /// complete batch classical trace output, using checked integer math.
    /// Caller-owned instruction bytes and process RSS are not included.
    fn preflight(
        &self, shots: usize, max_bytes: usize,
    ) -> Result<usize, NoisyCircuitError> {
        if shots == 0 { return Err(NoisyCircuitError::ZeroShots); }
        if !(1..=MAX_DENSITY_QUBITS).contains(&self.qubits) {
            return Err(NoisyCircuitError::InvalidWidth {
                qubits: self.qubits, max: MAX_DENSITY_QUBITS,
            });
        }
        let dim = 1usize.checked_shl(self.qubits as u32)
            .ok_or(NoisyCircuitError::ArithmeticOverflow)?;
        let state_bytes = dim.checked_mul(dim)
            .and_then(|v| v.checked_mul(size_of::<crate::Complex>()))
            .ok_or(NoisyCircuitError::ArithmeticOverflow)?;
        let min_shot = self.classical_bits
            .checked_mul(size_of::<Option<bool>>())
            .and_then(|v| v.checked_add(size_of::<NoisyShot>()))
            .ok_or(NoisyCircuitError::ArithmeticOverflow)?;
        let minimum = min_shot.checked_mul(shots)
            .and_then(|v| v.checked_add(state_bytes))
            .ok_or(NoisyCircuitError::ArithmeticOverflow)?;
        if minimum > max_bytes {
            return Err(NoisyCircuitError::BudgetExceeded {
                required_bytes: minimum, budget_bytes: max_bytes,
            });
        }

        // A fallible initialized-bit tracker, never usize-MAX allocation.
        let mut written = Vec::<bool>::new();
        written.try_reserve_exact(self.classical_bits)
            .map_err(|_| NoisyCircuitError::AllocationFailed)?;
        written.resize(self.classical_bits, false);
        let mut traces = 0usize;
        for (index, instruction) in self.instructions.iter().enumerate() {
            match instruction {
                NoisyInstruction::Gate(g) => self.check_gate(g)?,
                NoisyInstruction::Noise(n) => self.check_noise(n)?,
                NoisyInstruction::Measure { qubit, into_bit } => {
                    self.check_qubit(*qubit)?;
                    self.check_classical(*into_bit)?;
                    written[*into_bit] = true;
                    traces = traces.checked_add(1)
                        .ok_or(NoisyCircuitError::ArithmeticOverflow)?;
                }
                NoisyInstruction::Reset { qubit }
                | NoisyInstruction::ResetObserved { qubit } => {
                    self.check_qubit(*qubit)?;
                    traces = traces.checked_add(1)
                        .ok_or(NoisyCircuitError::ArithmeticOverflow)?;
                }
                NoisyInstruction::If { bit, action, .. } => {
                    self.check_classical(*bit)?;
                    self.check_action(action)?;
                    if !written[*bit] {
                        return Err(NoisyCircuitError::ReadBeforeWrite {
                            bit: *bit, instruction: index,
                        });
                    }
                }
            }
        }
        let per_shot = traces
            .checked_mul(size_of::<NoisyEvent>())
            .and_then(|bytes| min_shot.checked_add(bytes))
            .ok_or(NoisyCircuitError::ArithmeticOverflow)?;
        let total = per_shot.checked_mul(shots)
            .and_then(|bytes| bytes.checked_add(state_bytes))
            .ok_or(NoisyCircuitError::ArithmeticOverflow)?;
        if total > max_bytes {
            return Err(NoisyCircuitError::BudgetExceeded {
                required_bytes: total, budget_bytes: max_bytes,
            });
        }
        Ok(traces)
    }

    fn execute_one(
        &self, rng: &mut SeededRng, budget: usize, trace_cap: usize,
    ) -> Result<(NoisyShot, DensityMatrix), NoisyCircuitError> {
        let mut state = DensityMatrix::zero(self.qubits, budget)?;
        let mut bits = Vec::new();
        bits.try_reserve_exact(self.classical_bits)
            .map_err(|_| NoisyCircuitError::AllocationFailed)?;
        bits.resize(self.classical_bits, None);
        let mut trace = Vec::new();
        trace.try_reserve_exact(trace_cap)
            .map_err(|_| NoisyCircuitError::AllocationFailed)?;

        for (instruction, operation) in self.instructions.iter().enumerate() {
            match operation {
                NoisyInstruction::Gate(g) => state.apply_gate(g.clone())?,
                NoisyInstruction::Noise(n) => state.apply_noise(*n)?,
                NoisyInstruction::Measure { qubit, into_bit } => {
                    let value = state.measure(*qubit, rng)?;
                    bits[*into_bit] = Some(value);
                    trace.push(NoisyEvent {
                        instruction, qubit: *qubit,
                        kind: NoisyEventKind::Measure { into_bit: *into_bit, value },
                    });
                }
                NoisyInstruction::Reset { qubit } => {
                    state.reset_unobserved(*qubit)?;
                    trace.push(NoisyEvent {
                        instruction, qubit: *qubit,
                        kind: NoisyEventKind::ResetUnobserved,
                    });
                }
                NoisyInstruction::ResetObserved { qubit } => {
                    let value = state.measure(*qubit, rng)?;
                    if value { state.apply_gate(Gate::X(*qubit))?; }
                    trace.push(NoisyEvent {
                        instruction, qubit: *qubit,
                        kind: NoisyEventKind::ResetObserved { value },
                    });
                }
                NoisyInstruction::If { bit, equals, action } => {
                    let value = bits[*bit].ok_or(
                        NoisyCircuitError::ReadBeforeWrite {
                            bit: *bit, instruction,
                        },
                    )?;
                    if value == *equals {
                        match action {
                            NoisyAction::Gate(g) => state.apply_gate(g.clone())?,
                            NoisyAction::Noise(n) => state.apply_noise(*n)?,
                        }
                    }
                }
            }
        }

        Ok((NoisyShot { bits, trace }, state))
    }

    /// Single seeded shot with a fully inspectable final mixed state.
    pub fn execute_once(
        &self, seed: u64, max_bytes: usize,
    ) -> Result<NoisySingleShot, NoisyCircuitError> {
        let trace_cap = self.preflight(1, max_bytes)?;
        let mut rng = SeededRng::new(seed);
        let (shot, final_density) = self.execute_one(&mut rng, max_bytes, trace_cap)?;
        Ok(NoisySingleShot {
            metadata: NoisyMetadata::new(1, seed),
            shot,
            final_density,
        })
    }

    /// Fresh prepared density state per shot, one shared versioned RNG stream.
    /// Does not store or average full states for the batch. A batch is a set
    /// of observed trajectories, not one unconditional density matrix.
    pub fn execute_shots(
        &self, shots: usize, seed: u64, max_bytes: usize,
    ) -> Result<NoisyShotBatch, NoisyCircuitError> {
        let trace_cap = self.preflight(shots, max_bytes)?;
        let mut rng = SeededRng::new(seed);
        let mut records = Vec::new();
        records.try_reserve_exact(shots)
            .map_err(|_| NoisyCircuitError::AllocationFailed)?;
        for _ in 0..shots {
            let (shot, _) = self.execute_one(&mut rng, max_bytes, trace_cap)?;
            records.push(shot);
        }
        Ok(NoisyShotBatch {
            metadata: NoisyMetadata::new(shots, seed),
            records,
        })
    }
}
