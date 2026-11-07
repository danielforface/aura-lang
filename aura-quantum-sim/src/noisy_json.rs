//! Distinct versioned offline JSON input/output for *mixed-state noisy* dynamic
//! CPU simulation. The original pure "circuit-request/1" is unchanged.
//!
//! This schema and admission policy are EXPERIMENTAL, not QIR, Aura syntax,
//! or an external hardware-provider job protocol.
use crate::density::{DensityMatrix, NoiseChannel, MAX_DENSITY_QUBITS};
use crate::noisy_circuit::{
    NoisyAction, NoisyCircuit, NoisyCircuitError, NoisyEventKind, NoisyInstruction,
    NoisyMetadata, NoisyShot,
};
use crate::{Complex, Gate, DEFAULT_MEMORY_BUDGET};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const NOISY_REQUEST_SCHEMA: &str = "aura.quantum.noisy-request/1";
pub const NOISY_ERROR_SCHEMA: &str = "aura.quantum.noisy-error/1";
pub const MAX_NOISY_REQUEST_BYTES: usize = 256 * 1024;
pub const MAX_NOISY_RESULT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_NOISY_WORK_UNITS: usize = 4 * 1024 * 1024;
pub const MAX_NOISY_OPERATIONS: usize = 192;
pub const MAX_NOISY_SHOTS: usize = 1024;
pub const MAX_NOISY_QUBITS: usize = 6;
pub const MAX_NOISY_CLASSICAL_BITS: usize = 32;
pub const MAX_TOMOGRAPHY_QUBITS: usize = 4;

#[derive(Debug)]
pub enum NoisyProtocolError {
    InputTooLarge,
    InvalidJson(String),
    UnsupportedSchema,
    ProfileLimit(&'static str),
    ArithmeticOverflow,
    Circuit(NoisyCircuitError),
    Serialization(String),
}
impl NoisyProtocolError {
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
impl std::fmt::Display for NoisyProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for NoisyProtocolError {}
impl From<NoisyCircuitError> for NoisyProtocolError {
    fn from(value: NoisyCircuitError) -> Self { Self::Circuit(value) }
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
    #[serde(default)]
    inspect_final_density: bool,
    instructions: Vec<Operation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    Gate { gate: GateSpec },
    Noise { channel: ChannelSpec },
    Measure { qubit: usize, into_bit: usize },
    Reset { qubit: usize },
    ResetObserved { qubit: usize },
    If { bit: usize, equals: bool, action: Conditioned },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Conditioned {
    Gate { gate: GateSpec },
    Noise { channel: ChannelSpec },
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
    fn gate(self) -> Gate {
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

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ChannelSpec {
    PhaseFlip { qubit: usize, probability: f64 },
    AmplitudeDamping { qubit: usize, gamma: f64 },
}
impl ChannelSpec {
    fn channel(self) -> NoiseChannel {
        match self {
            Self::PhaseFlip { qubit, probability } =>
                NoiseChannel::PhaseFlip { qubit, probability },
            Self::AmplitudeDamping { qubit, gamma } =>
                NoiseChannel::AmplitudeDamping { qubit, gamma },
        }
    }
}

#[derive(Debug, Serialize)]
struct ResultOut {
    schema: &'static str,
    status: &'static str,
    engine_version: &'static str,
    profile: &'static str,
    density_profile: &'static str,
    precision: &'static str,
    norm_tolerance: f64,
    seed: u64,
    shots: usize,
    qubits: usize,
    classical_bits: usize,
    evidence_class: &'static str,
    qpu_hardware_jobs: u8,
    histogram_encoding: &'static str,
    histogram: BTreeMap<String, usize>,
    results: Vec<ShotOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    final_density: Option<FinalDensityOut>,
}

#[derive(Debug, Serialize)]
struct ShotOut {
    bits: Vec<Option<bool>>,
    trace: Vec<TraceOut>,
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TraceOut {
    Measure {
        instruction: usize, qubit: usize, into_bit: usize, value: bool,
    },
    Reset {
        instruction: usize, qubit: usize,
    },
    ResetObserved {
        instruction: usize, qubit: usize, value: bool,
    },
}
#[derive(Debug, Serialize)]
struct FinalDensityOut {
    dimension: usize,
    row_major_complex_pairs: Vec<[f64; 2]>,
    basis_probabilities: Vec<f64>,
    trace: f64,
    purity: f64,
}

fn shot_out(record: &NoisyShot) -> ShotOut {
    ShotOut {
        bits: record.bits.clone(),
        trace: record.trace.iter().map(|entry| {
            match entry.kind {
                NoisyEventKind::Measure { into_bit, value } => TraceOut::Measure {
                    instruction: entry.instruction,
                    qubit: entry.qubit,
                    into_bit,
                    value,
                },
                NoisyEventKind::ResetUnobserved => TraceOut::Reset {
                    instruction: entry.instruction, qubit: entry.qubit,
                },
                NoisyEventKind::ResetObserved { value } => TraceOut::ResetObserved {
                    instruction: entry.instruction, qubit: entry.qubit, value,
                },
            }
        }).collect(),
    }
}

fn shot_key(bits: &[Option<bool>]) -> String {
    if bits.is_empty() {
        return String::from("");
    }
    bits.iter().rev().map(|bit| match bit {
        Some(true) => '1',
        Some(false) => '0',
        None => '?',
    }).collect()
}

fn full_density(matrix: &DensityMatrix) -> Result<FinalDensityOut, NoisyProtocolError> {
    let dim = matrix.dimension();
    let mut row_major_complex_pairs = Vec::new();
    row_major_complex_pairs.try_reserve_exact(dim * dim)
        .map_err(|_| NoisyProtocolError::ProfileLimit("allocation_failed"))?;
    let mut basis_probabilities = Vec::new();
    basis_probabilities.try_reserve_exact(dim)
        .map_err(|_| NoisyProtocolError::ProfileLimit("allocation_failed"))?;
    for row in 0..dim {
        basis_probabilities.push(
            matrix.basis_probability(row).map_err(|err|
                NoisyProtocolError::Circuit(NoisyCircuitError::Density(err))
            )?
        );
        for col in 0..dim {
            let Complex { re, im } = matrix.element(row,col).map_err(|err|
                NoisyProtocolError::Circuit(NoisyCircuitError::Density(err))
            )?;
            row_major_complex_pairs.push([re,im]);
        }
    }
    Ok(FinalDensityOut {
        dimension: dim,
        row_major_complex_pairs,
        basis_probabilities,
        trace: matrix.trace(),
        purity: matrix.purity().map_err(|err|
            NoisyProtocolError::Circuit(NoisyCircuitError::Density(err))
        )?,
    })
}

fn response(
    request: &Request,
    metadata: &NoisyMetadata,
    records: &[NoisyShot],
    final_density: Option<FinalDensityOut>,
) -> Result<String, NoisyProtocolError> {
    let mut histogram = BTreeMap::<String,usize>::new();
    for record in records {
        *histogram.entry(shot_key(&record.bits)).or_insert(0) += 1;
    }
    let output=ResultOut {
        schema: metadata.schema_version,
        status: "simulated",
        engine_version: metadata.engine_version,
        profile: metadata.profile,
        density_profile: metadata.density_profile,
        precision: metadata.precision,
        norm_tolerance: metadata.norm_tolerance,
        seed: metadata.seed,
        shots: metadata.shots,
        qubits: request.qubits,
        classical_bits: request.classical_bits,
        evidence_class: metadata.evidence_class,
        qpu_hardware_jobs: metadata.qpu_hardware_jobs,
        histogram_encoding: "msb-classical-bit-first;?=unwritten",
        histogram,
        results: records.iter().map(shot_out).collect(),
        final_density,
    };
    let encoded = serde_json::to_string(&output)
        .map_err(|err| NoisyProtocolError::Serialization(err.to_string()))?;
    if encoded.len() > MAX_NOISY_RESULT_BYTES {
        return Err(NoisyProtocolError::ProfileLimit("serialized_result_bytes"));
    }
    Ok(encoded)
}

/// Fully in-memory and bounded local JSON execution; no filesystem access or
/// hardware/network use. On invalid requests no partial JSON is returned.
pub fn execute_noisy_json_bytes(raw: &[u8]) -> Result<String, NoisyProtocolError> {
    if raw.len() > MAX_NOISY_REQUEST_BYTES {
        return Err(NoisyProtocolError::InputTooLarge);
    }
    let request: Request = serde_json::from_slice(raw)
        .map_err(|err| NoisyProtocolError::InvalidJson(err.to_string()))?;
    if request.schema != NOISY_REQUEST_SCHEMA {
        return Err(NoisyProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_NOISY_QUBITS).contains(&request.qubits) {
        return Err(NoisyProtocolError::ProfileLimit("qubits"));
    }
    if request.qubits > MAX_DENSITY_QUBITS {
        return Err(NoisyProtocolError::ProfileLimit("density_width"));
    }
    if request.classical_bits > MAX_NOISY_CLASSICAL_BITS {
        return Err(NoisyProtocolError::ProfileLimit("classical_bits"));
    }
    if !(1..=MAX_NOISY_SHOTS).contains(&request.shots) {
        return Err(NoisyProtocolError::ProfileLimit("shots"));
    }
    if request.instructions.len() > MAX_NOISY_OPERATIONS {
        return Err(NoisyProtocolError::ProfileLimit("instructions"));
    }
    if request.max_memory_bytes == 0 ||
        request.max_memory_bytes > DEFAULT_MEMORY_BUDGET {
        return Err(NoisyProtocolError::ProfileLimit("max_memory_bytes"));
    }
    if request.inspect_final_density &&
        (request.shots != 1 || request.qubits > MAX_TOMOGRAPHY_QUBITS) {
        return Err(NoisyProtocolError::ProfileLimit("final_density_inspection"));
    }
    // Density operations walk the full 4^n matrix; cap predictable work
    // in addition to allocation and encoded-result bytes.
    let dim = 1usize << request.qubits;
    let work = dim.checked_mul(dim)
        .and_then(|x| x.checked_mul(request.instructions.len().max(1)))
        .and_then(|x| x.checked_mul(request.shots))
        .ok_or(NoisyProtocolError::ArithmeticOverflow)?;
    if work > MAX_NOISY_WORK_UNITS {
        return Err(NoisyProtocolError::ProfileLimit("matrix_work_units"));
    }
    let mut program=NoisyCircuit::new(request.qubits,request.classical_bits);
    for op in &request.instructions {
        let instruction = match op {
            Operation::Gate{gate} => NoisyInstruction::Gate(gate.clone_gate()),
            Operation::Noise{channel} => NoisyInstruction::Noise(channel.clone_channel()),
            Operation::Measure{qubit,into_bit} =>
                NoisyInstruction::Measure{qubit:*qubit,into_bit:*into_bit},
            Operation::Reset{qubit} => NoisyInstruction::Reset{qubit:*qubit},
            Operation::ResetObserved{qubit} =>
                NoisyInstruction::ResetObserved{qubit:*qubit},
            Operation::If{bit,equals,action} => NoisyInstruction::If{
                bit:*bit,equals:*equals,
                action:match action {
                    Conditioned::Gate{gate} => NoisyAction::Gate(gate.clone_gate()),
                    Conditioned::Noise{channel} => NoisyAction::Noise(channel.clone_channel()),
                }
            },
        };
        program.push(instruction);
    }

    if request.inspect_final_density {
        let run = program.execute_once(request.seed,request.max_memory_bytes)?;
        let last = full_density(&run.final_density)?;
        response(&request,&run.metadata,&[run.shot],Some(last))
    } else {
        let batch=program.execute_shots(
            request.shots,request.seed,request.max_memory_bytes
        )?;
        response(&request,&batch.metadata,&batch.records,None)
    }
}

// Avoid exposing potentially unstable parser internals as Rust public API.
// The request schema is explicitly experimental and distinct from v1.
impl GateSpec {
    fn clone_gate(&self) -> Gate {
        match self {
            Self::H{target} => Gate::H(*target),
            Self::X{target} => Gate::X(*target),
            Self::Y{target} => Gate::Y(*target),
            Self::Z{target} => Gate::Z(*target),
            Self::S{target} => Gate::S(*target),
            Self::T{target} => Gate::T(*target),
            Self::Cx{control,target} =>
                Gate::Cnot{control:*control,target:*target},
            Self::Cz{control,target} =>
                Gate::Cz{control:*control,target:*target},
            Self::Swap{a,b} => Gate::Swap{a:*a,b:*b},
        }
    }
}
impl ChannelSpec {
    fn clone_channel(&self) -> NoiseChannel {
        match self {
            Self::PhaseFlip{qubit,probability} => NoiseChannel::PhaseFlip {
                qubit:*qubit,probability:*probability,
            },
            Self::AmplitudeDamping{qubit,gamma} =>
                NoiseChannel::AmplitudeDamping{qubit:*qubit,gamma:*gamma},
        }
    }
}
