//! Versioned, strictly bounded offline JSON request/response bridge.
//! No cloud provider, QPU submission, Aura syntax or QIR claims are implied.
use crate::circuit::{
    Circuit, CircuitError, Instruction, ShotRecord, TraceKind, CIRCUIT_PROFILE,
    CIRCUIT_SCHEMA_VERSION,
};
use crate::{Gate, DEFAULT_MEMORY_BUDGET, ENGINE_VERSION, NORM_TOLERANCE};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const REQUEST_SCHEMA: &str = "aura.quantum.circuit-request/1";
pub const ERROR_SCHEMA: &str = "aura.quantum.circuit-error/1";
pub const MAX_REQUEST_BYTES: usize = 256 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_WORK_UNITS: usize = 8 * 1024 * 1024;
pub const MAX_OPERATIONS: usize = 256;
pub const MAX_SHOTS: usize = 2048;
pub const MAX_QUBITS: usize = 12;
pub const MAX_CLASSICAL_BITS: usize = 64;

#[derive(Debug)]
pub enum ProtocolError {
    InputTooLarge,
    InvalidJson(String),
    UnsupportedSchema,
    ProfileLimit(&'static str),
    ArithmeticOverflow,
    Circuit(CircuitError),
    Serialization(String),
}

impl ProtocolError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InputTooLarge => "input_too_large",
            Self::InvalidJson(_) => "invalid_json",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::ProfileLimit(_) => "profile_limit",
            Self::ArithmeticOverflow => "arithmetic_overflow",
            Self::Circuit(_) => "circuit_rejected",
            Self::Serialization(_) => "serialization_error",
        }
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge => write!(f, "request exceeds {MAX_REQUEST_BYTES} bytes"),
            Self::InvalidJson(err) => write!(f, "invalid JSON request: {err}"),
            Self::UnsupportedSchema => write!(f, "unsupported circuit request schema"),
            Self::ProfileLimit(limit) => write!(f, "bounded local profile limit: {limit}"),
            Self::ArithmeticOverflow => write!(f, "request work estimate overflow"),
            Self::Circuit(err) => write!(f, "circuit rejected: {err}"),
            Self::Serialization(err) => write!(f, "unable to serialize result: {err}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<CircuitError> for ProtocolError {
    fn from(value: CircuitError) -> Self {
        Self::Circuit(value)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    qubits: usize,
    classical_bits: usize,
    seed: u64,
    shots: usize,
    max_memory_bytes: usize,
    instructions: Vec<Operation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Gate { gate: GateSpec },
    Measure { qubit: usize, into_bit: usize },
    Reset { qubit: usize },
    If { bit: usize, equals: bool, gate: GateSpec },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum GateSpec {
    H { target: usize },
    X { target: usize },
    Y { target: usize },
    Z { target: usize },
    S { target: usize },
    T { target: usize },
    Cx { control: usize, target: usize },
    Cz { control: usize, target: usize },
    Swap { a: usize, b: usize },
}

impl GateSpec {
    fn into_gate(self) -> Gate {
        match self {
            Self::H { target } => Gate::H(target),
            Self::X { target } => Gate::X(target),
            Self::Y { target } => Gate::Y(target),
            Self::Z { target } => Gate::Z(target),
            Self::S { target } => Gate::S(target),
            Self::T { target } => Gate::T(target),
            Self::Cx { control, target } => Gate::Cnot { control, target },
            Self::Cz { control, target } => Gate::Cz { control, target },
            Self::Swap { a, b } => Gate::Swap { a, b },
        }
    }
}

#[derive(Serialize)]
struct Response<'a> {
    schema: &'static str,
    status: &'static str,
    engine_version: &'static str,
    profile: &'static str,
    seed: u64,
    qubits: usize,
    classical_bits: usize,
    shots: usize,
    norm_tolerance: f64,
    evidence_class: &'static str,
    qpu_hardware_jobs: u8,
    results: Vec<ShotOut<'a>>,
}

#[derive(Serialize)]
struct ShotOut<'a> {
    bits: &'a [Option<bool>],
    trace: Vec<TraceOut>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TraceOut {
    Measure { instruction: usize, qubit: usize, into_bit: usize, value: bool },
    Reset { instruction: usize, qubit: usize, previous_value: bool },
}

impl<'a> From<&'a ShotRecord> for ShotOut<'a> {
    fn from(record: &'a ShotRecord) -> Self {
        let trace = record
            .trace
            .iter()
            .map(|event| match event.kind {
                TraceKind::Measure { into_bit, value } => TraceOut::Measure {
                    instruction: event.instruction,
                    qubit: event.qubit,
                    into_bit,
                    value,
                },
                TraceKind::Reset { previous_value } => TraceOut::Reset {
                    instruction: event.instruction,
                    qubit: event.qubit,
                    previous_value,
                },
            })
            .collect();
        Self { bits: &record.bits, trace }
    }
}

/// Execute one bounded JSON document and return one complete JSON result.
/// Invalid input has no partial stdout or partially executed physical effect.
/// This is an *experimental local profile*, not the frozen Quantum IR ABI.
pub fn execute_json_bytes(raw: &[u8]) -> Result<String, ProtocolError> {
    if raw.len() > MAX_REQUEST_BYTES {
        return Err(ProtocolError::InputTooLarge);
    }
    let request: Request =
        serde_json::from_slice(raw).map_err(|err| ProtocolError::InvalidJson(err.to_string()))?;
    if request.schema != REQUEST_SCHEMA {
        return Err(ProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_QUBITS).contains(&request.qubits) {
        return Err(ProtocolError::ProfileLimit("qubits"));
    }
    if request.classical_bits > MAX_CLASSICAL_BITS {
        return Err(ProtocolError::ProfileLimit("classical_bits"));
    }
    if !(1..=MAX_SHOTS).contains(&request.shots) {
        return Err(ProtocolError::ProfileLimit("shots"));
    }
    if request.max_memory_bytes == 0 || request.max_memory_bytes > DEFAULT_MEMORY_BUDGET {
        return Err(ProtocolError::ProfileLimit("max_memory_bytes"));
    }
    if request.instructions.len() > MAX_OPERATIONS {
        return Err(ProtocolError::ProfileLimit("instructions"));
    }

    // One state dimension is traversed per operation for norm checks.
    // This is a conservative *work admission count*, not a timing guarantee.
    let work = (1usize << request.qubits)
        .checked_mul(request.instructions.len().max(1))
        .and_then(|items| items.checked_mul(request.shots))
        .ok_or(ProtocolError::ArithmeticOverflow)?;
    if work > MAX_WORK_UNITS {
        return Err(ProtocolError::ProfileLimit("work_units"));
    }

    let mut circuit = Circuit::new(request.qubits, request.classical_bits);
    for operation in request.instructions {
        let instruction = match operation {
            Operation::Gate { gate } => Instruction::Gate(gate.into_gate()),
            Operation::Measure { qubit, into_bit } => Instruction::Measure { qubit, into_bit },
            Operation::Reset { qubit } => Instruction::Reset { qubit },
            Operation::If { bit, equals, gate } => Instruction::If {
                bit,
                equals,
                gate: gate.into_gate(),
            },
        };
        circuit.push(instruction);
    }

    // Execute only the local simulator. The circuit preflight validates all
    // branches, even those that are not taken by this seed.
    let batch = circuit.execute_shots(
        request.shots, request.seed, request.max_memory_bytes,
    )?;
    let response = Response {
        schema: CIRCUIT_SCHEMA_VERSION,
        status: "simulated",
        engine_version: ENGINE_VERSION,
        profile: CIRCUIT_PROFILE,
        seed: request.seed,
        qubits: request.qubits,
        classical_bits: request.classical_bits,
        shots: request.shots,
        norm_tolerance: NORM_TOLERANCE,
        evidence_class: "simulation",
        qpu_hardware_jobs: 0,
        results: batch.shots.iter().map(ShotOut::from).collect(),
    };
    let encoded =
        serde_json::to_string(&response).map_err(|err| ProtocolError::Serialization(err.to_string()))?;
    if encoded.len() > MAX_RESPONSE_BYTES {
        return Err(ProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    Ok(encoded)
}
