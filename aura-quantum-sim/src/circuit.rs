//! Bounded dynamic circuit execution for the independent Q28 CPU statevector.
//!
//! This API is callable from Rust. It is NOT Aura syntax, a Quantum IR/QIR
//! implementation, a physical hardware runner, a noise model or a proof engine.
//! Measurements are ideal nondestructive computational-basis instruments.
//! Reset is modeled as measurement followed by an X gate on outcome 1.
//! Classical controls may read only bits written by earlier explicit measures.
//! A seeded SplitMix64 stream is shared across all operations and shots.
use crate::{
    Complex, Gate, SeededRng, SimError, Statevector, ENGINE_VERSION, NORM_TOLERANCE,
};
use std::mem::size_of;

pub const CIRCUIT_SCHEMA_VERSION: &str = "aura.quantum.circuit-result/1";
pub const CIRCUIT_PROFILE: &str = "local-pure-statevector-f64-dynamic-z/1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitError {
    EmptyQuantumRegister,
    ZeroShots,
    Simulator(SimError),
    InvalidClassicalBit { bit: usize, bits: usize },
    ReadBeforeWrite { bit: usize, instruction: usize },
    ArithmeticOverflow,
    BudgetExceeded { required_bytes: usize, budget_bytes: usize },
    AllocationFailed,
}

impl From<SimError> for CircuitError {
    fn from(error: SimError) -> Self {
        Self::Simulator(error)
    }
}

impl std::fmt::Display for CircuitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CircuitError {}

/// Operations accepted by the v1 bounded circuit profile. Gate operations
/// obey the same qubit numbering as Statevector: q0 is the basis LSB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Gate(Gate),
    Measure { qubit: usize, into_bit: usize },
    Reset { qubit: usize },
    If {
        bit: usize,
        equals: bool,
        gate: Gate,
    },
}

/// Simulation trace events preserve program order, including the internal
/// measurement needed for reset. These are observations from a simulator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceKind {
    Measure { into_bit: usize, value: bool },
    Reset { previous_value: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    pub instruction: usize,
    pub qubit: usize,
    pub kind: TraceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShotRecord {
    /// None means never explicitly measured into this classical register bit.
    pub bits: Vec<Option<bool>>,
    /// Explicit Z-measurements and internal reset instrument results.
    pub trace: Vec<TraceEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunMetadata {
    pub schema_version: &'static str,
    pub engine_version: &'static str,
    pub profile: &'static str,
    pub norm_tolerance: f64,
    pub seed: u64,
    pub shots: usize,
}

impl RunMetadata {
    fn new(seed: u64, shots: usize) -> Self {
        Self {
            schema_version: CIRCUIT_SCHEMA_VERSION,
            engine_version: ENGINE_VERSION,
            profile: CIRCUIT_PROFILE,
            norm_tolerance: NORM_TOLERANCE,
            seed,
            shots,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShotBatch {
    pub metadata: RunMetadata,
    pub shots: Vec<ShotRecord>,
}

/// Single-shot execution also returns the actual, collapsed final quantum
/// state for scientific inspection without storing a state per batch shot.
#[derive(Debug, Clone)]
pub struct SingleShot {
    pub metadata: RunMetadata,
    pub shot: ShotRecord,
    pub final_state: Statevector,
}

#[derive(Debug, Clone, Default)]
pub struct Circuit {
    qubits: usize,
    classical_bits: usize,
    instructions: Vec<Instruction>,
}

impl Circuit {
    pub fn new(qubits: usize, classical_bits: usize) -> Self {
        Self {
            qubits,
            classical_bits,
            instructions: Vec::new(),
        }
    }

    pub fn qubits(&self) -> usize {
        self.qubits
    }

    pub fn classical_bits(&self) -> usize {
        self.classical_bits
    }

    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    pub fn push(&mut self, operation: Instruction) {
        self.instructions.push(operation);
    }

    fn check_qubit(&self, qubit: usize) -> Result<(), CircuitError> {
        if qubit >= self.qubits {
            return Err(CircuitError::Simulator(SimError::InvalidQubit {
                index: qubit,
                qubits: self.qubits,
            }));
        }
        Ok(())
    }

    fn check_classical_bit(&self, bit: usize) -> Result<(), CircuitError> {
        if bit >= self.classical_bits {
            return Err(CircuitError::InvalidClassicalBit {
                bit,
                bits: self.classical_bits,
            });
        }
        Ok(())
    }

    fn check_gate(&self, gate: &Gate) -> Result<(), CircuitError> {
        match gate {
            Gate::H(q) | Gate::X(q) | Gate::Y(q) | Gate::Z(q)
            | Gate::S(q) | Gate::T(q) => self.check_qubit(*q),
            Gate::Cnot { control, target } | Gate::Cz { control, target } => {
                self.check_qubit(*control)?;
                self.check_qubit(*target)?;
                if control == target {
                    return Err(CircuitError::Simulator(SimError::DuplicateQubit(*control)));
                }
                Ok(())
            }
            Gate::Swap { a, b } => {
                self.check_qubit(*a)?;
                self.check_qubit(*b)?;
                if a == b {
                    return Err(CircuitError::Simulator(SimError::DuplicateQubit(*a)));
                }
                Ok(())
            }
        }
    }

    /// Validate every operation before any shot executes, including branches
    /// whose classical conditions could evaluate false at runtime. A bounded
    /// *logical* byte budget covers one live state and all declared shot output.
    /// Input circuit storage, OS/runtime overhead and allocator padding are
    /// not part of this logical accounting (see documentation).
    fn preflight(&self, shots: usize, max_bytes: usize) -> Result<usize, CircuitError> {
        if shots == 0 {
            return Err(CircuitError::ZeroShots);
        }
        if self.qubits == 0 {
            return Err(CircuitError::EmptyQuantumRegister);
        }
        if self.qubits >= usize::BITS as usize {
            return Err(CircuitError::Simulator(SimError::DimensionOverflow));
        }
        let dimensions = 1usize
            .checked_shl(self.qubits as u32)
            .ok_or(CircuitError::ArithmeticOverflow)?;
        let state_bytes = dimensions
            .checked_mul(size_of::<Complex>())
            .ok_or(CircuitError::ArithmeticOverflow)?;

        // Reject unrepresentable or oversized output dimensions before trying
        // to allocate classical initialization tracking.
        let per_shot_bits = self.classical_bits
            .checked_mul(size_of::<Option<bool>>())
            .ok_or(CircuitError::ArithmeticOverflow)?;
        let minimum = size_of::<ShotRecord>()
            .checked_add(per_shot_bits)
            .ok_or(CircuitError::ArithmeticOverflow)?;
        let min_total = minimum
            .checked_mul(shots)
            .and_then(|bytes| bytes.checked_add(state_bytes))
            .ok_or(CircuitError::ArithmeticOverflow)?;
        if min_total > max_bytes {
            return Err(CircuitError::BudgetExceeded {
                required_bytes: min_total,
                budget_bytes: max_bytes,
            });
        }

        let mut known = Vec::new();
        known
            .try_reserve_exact(self.classical_bits)
            .map_err(|_| CircuitError::AllocationFailed)?;
        known.resize(self.classical_bits, false);

        let mut trace_capacity = 0usize;
        for (index, operation) in self.instructions.iter().enumerate() {
            match operation {
                Instruction::Gate(gate) => self.check_gate(gate)?,
                Instruction::Measure { qubit, into_bit } => {
                    self.check_qubit(*qubit)?;
                    self.check_classical_bit(*into_bit)?;
                    known[*into_bit] = true;
                    trace_capacity = trace_capacity
                        .checked_add(1)
                        .ok_or(CircuitError::ArithmeticOverflow)?;
                }
                Instruction::Reset { qubit } => {
                    self.check_qubit(*qubit)?;
                    trace_capacity = trace_capacity
                        .checked_add(1)
                        .ok_or(CircuitError::ArithmeticOverflow)?;
                }
                Instruction::If { bit, gate, .. } => {
                    self.check_classical_bit(*bit)?;
                    self.check_gate(gate)?;
                    if !known[*bit] {
                        return Err(CircuitError::ReadBeforeWrite {
                            bit: *bit,
                            instruction: index,
                        });
                    }
                }
            }
        }
        let per_shot = trace_capacity
            .checked_mul(size_of::<TraceEntry>())
            .and_then(|trace_bytes| minimum.checked_add(trace_bytes))
            .ok_or(CircuitError::ArithmeticOverflow)?;
        let total = per_shot
            .checked_mul(shots)
            .and_then(|output| output.checked_add(state_bytes))
            .ok_or(CircuitError::ArithmeticOverflow)?;
        if total > max_bytes {
            return Err(CircuitError::BudgetExceeded {
                required_bytes: total,
                budget_bytes: max_bytes,
            });
        }
        Ok(trace_capacity)
    }

    fn execute_one(
        &self,
        rng: &mut SeededRng,
        max_bytes: usize,
        trace_capacity: usize,
    ) -> Result<(ShotRecord, Statevector), CircuitError> {
        let mut state = Statevector::zero(self.qubits, max_bytes)?;
        let mut bits = Vec::new();
        bits.try_reserve_exact(self.classical_bits)
            .map_err(|_| CircuitError::AllocationFailed)?;
        bits.resize(self.classical_bits, None);
        let mut trace = Vec::new();
        trace
            .try_reserve_exact(trace_capacity)
            .map_err(|_| CircuitError::AllocationFailed)?;

        for (instruction, operation) in self.instructions.iter().enumerate() {
            match operation {
                Instruction::Gate(gate) => state.apply(gate.clone())?,
                Instruction::Measure { qubit, into_bit } => {
                    let value = state.measure(*qubit, rng)?;
                    bits[*into_bit] = Some(value);
                    trace.push(TraceEntry {
                        instruction,
                        qubit: *qubit,
                        kind: TraceKind::Measure {
                            into_bit: *into_bit,
                            value,
                        },
                    });
                }
                Instruction::Reset { qubit } => {
                    let previous_value = state.measure(*qubit, rng)?;
                    if previous_value {
                        state.apply(Gate::X(*qubit))?;
                    }
                    trace.push(TraceEntry {
                        instruction,
                        qubit: *qubit,
                        kind: TraceKind::Reset { previous_value },
                    });
                }
                Instruction::If { bit, equals, gate } => {
                    let value = bits[*bit].ok_or(CircuitError::ReadBeforeWrite {
                        bit: *bit,
                        instruction,
                    })?;
                    if value == *equals {
                        state.apply(gate.clone())?;
                    }
                }
            }
        }
        Ok((ShotRecord { bits, trace }, state))
    }

    /// Execute once from |0...0>. This returns the final pure quantum state,
    /// the ordered ideal instrument trace and any written classical bits.
    pub fn execute_once(&self, seed: u64, max_bytes: usize) -> Result<SingleShot, CircuitError> {
        let trace_capacity = self.preflight(1, max_bytes)?;
        let mut rng = SeededRng::new(seed);
        let (shot, final_state) = self.execute_one(&mut rng, max_bytes, trace_capacity)?;
        Ok(SingleShot {
            metadata: RunMetadata::new(seed, 1),
            shot,
            final_state,
        })
    }

    /// Reinitialize quantum/classical registers for every shot while advancing
    /// one deterministic RNG stream across the entire batch. The final quantum
    /// state is intentionally not duplicated per shot.
    pub fn execute_shots(
        &self,
        shots: usize,
        seed: u64,
        max_bytes: usize,
    ) -> Result<ShotBatch, CircuitError> {
        let trace_capacity = self.preflight(shots, max_bytes)?;
        let mut rng = SeededRng::new(seed);
        let mut records = Vec::new();
        records
            .try_reserve_exact(shots)
            .map_err(|_| CircuitError::AllocationFailed)?;
        for _ in 0..shots {
            let (shot, _) = self.execute_one(&mut rng, max_bytes, trace_capacity)?;
            records.push(shot);
        }
        Ok(ShotBatch {
            metadata: RunMetadata::new(seed, shots),
            shots: records,
        })
    }
}
