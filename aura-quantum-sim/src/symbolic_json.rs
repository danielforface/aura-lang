//! Offline versioned JSON bridge for exact small Clifford+T phase oracles.
//!
//! This is explicitly NOT the sampled pure/noisy/stabilizer JSON ABI, QIR or
//! QPU hardware. There is no seed or shots: measurement is a user-selected
//! subnormalized branch. i128 coefficients are *decimal strings* in JSON to
//! preserve their mathematical exactness in JavaScript consumers.
use crate::symbolic::{
    ExactAmplitude, ExactMeasureEvent, ExactProbability, Root2, Root2Complex,
    SymbolicCircuit, SymbolicError, SymbolicInstruction, SymbolicMetadata,
    MAX_SYMBOLIC_CLASSICAL_BITS, MAX_SYMBOLIC_OPERATIONS, MAX_SYMBOLIC_QUBITS,
};
use crate::{Gate, DEFAULT_MEMORY_BUDGET};
use serde::{Deserialize, Serialize};

pub const SYMBOLIC_REQUEST_SCHEMA: &str = "aura.quantum.symbolic-request/1";
pub const SYMBOLIC_ERROR_SCHEMA: &str = "aura.quantum.symbolic-error/1";
pub const MAX_SYMBOLIC_INPUT_BYTES: usize = 64 * 1024;
pub const MAX_SYMBOLIC_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub enum SymbolicProtocolError {
    InputTooLarge,
    InvalidJson(String),
    UnsupportedSchema,
    ProfileLimit(&'static str),
    Circuit(SymbolicError),
    Serialization(String),
}
impl SymbolicProtocolError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InputTooLarge => "input_too_large",
            Self::InvalidJson(_) => "invalid_json",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::ProfileLimit(_) => "profile_limit",
            Self::Circuit(_) => "circuit_rejected",
            Self::Serialization(_) => "serialization_error",
        }
    }
}
impl std::fmt::Display for SymbolicProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SymbolicProtocolError {}
impl From<SymbolicError> for SymbolicProtocolError {
    fn from(value: SymbolicError) -> Self { Self::Circuit(value) }
}

#[derive(Debug,Deserialize)]
struct SchemaProbe { schema: String }
#[derive(Debug,Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    qubits: usize,
    classical_bits: usize,
    max_memory_bytes: usize,
    instructions: Vec<Operation>,
}
#[derive(Debug,Deserialize)]
#[serde(tag="op",rename_all="snake_case",deny_unknown_fields)]
enum Operation {
    Gate { gate: GateSpec },
    Measure { qubit:usize, into_bit:usize, outcome:bool },
    If { bit:usize, equals:bool, gate:GateSpec },
}
#[derive(Debug,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
enum GateSpec {
    H{target:usize}, X{target:usize}, Y{target:usize},
    Z{target:usize}, S{target:usize}, T{target:usize},
    Cx{control:usize,target:usize},
    Cz{control:usize,target:usize},
    Swap{a:usize,b:usize},
}
impl GateSpec {
    fn gate(&self)->Gate {
        match self {
            Self::H{target}=>Gate::H(*target),
            Self::X{target}=>Gate::X(*target),
            Self::Y{target}=>Gate::Y(*target),
            Self::Z{target}=>Gate::Z(*target),
            Self::S{target}=>Gate::S(*target),
            Self::T{target}=>Gate::T(*target),
            Self::Cx{control,target}=>Gate::Cnot{control:*control,target:*target},
            Self::Cz{control,target}=>Gate::Cz{control:*control,target:*target},
            Self::Swap{a,b}=>Gate::Swap{a:*a,b:*b},
        }
    }
}

#[derive(Debug,Serialize)]
struct Root2Out {
    rational: String,
    radical: String,
}
impl From<Root2> for Root2Out {
    fn from(r:Root2)->Self {
        Self { rational:r.rational.to_string(), radical:r.radical.to_string() }
    }
}
#[derive(Debug,Serialize)]
struct ComplexOut {
    real:Root2Out,
    imag:Root2Out,
}
impl From<Root2Complex> for ComplexOut {
    fn from(z:Root2Complex)->Self {
        Self { real:z.real.into(), imag:z.imag.into() }
    }
}
#[derive(Debug,Serialize)]
struct AmplitudeOut {
    numerator:ComplexOut,
    denominator_power_of_two:u32,
    approximate_real_imag:[f64;2],
}
impl From<ExactAmplitude> for AmplitudeOut {
    fn from(a:ExactAmplitude)->Self {
        let (re,im)=a.approximate();
        Self {
            numerator:a.numerator.into(),
            denominator_power_of_two:a.denominator_power_of_two,
            approximate_real_imag:[re,im],
        }
    }
}
#[derive(Debug,Serialize)]
struct ProbabilityOut {
    numerator:Root2Out,
    denominator_power_of_two:u32,
    approximate_f64:f64,
}
impl From<ExactProbability> for ProbabilityOut {
    fn from(p:ExactProbability)->Self {
        Self {
            numerator:p.numerator.into(),
            denominator_power_of_two:p.denominator_power_of_two,
            approximate_f64:p.approximate(),
        }
    }
}
#[derive(Debug,Serialize)]
struct MeasureOut {
    instruction:usize,
    qubit:usize,
    into_bit:usize,
    outcome:bool,
    joint_weight_before:ProbabilityOut,
    joint_weight_after:ProbabilityOut,
    /// This is only a convenient numerical *ratio*; the exact conditional
    /// expression is joint_weight_after / joint_weight_before.
    conditional_probability_approx_f64:f64,
}
impl From<&ExactMeasureEvent> for MeasureOut {
    fn from(e:&ExactMeasureEvent)->Self {
        Self {
            instruction:e.instruction,qubit:e.qubit,
            into_bit:e.into_bit,outcome:e.outcome,
            joint_weight_before:e.before_mass.into(),
            joint_weight_after:e.after_mass.into(),
            conditional_probability_approx_f64:
                e.after_mass.approximate()/e.before_mass.approximate(),
        }
    }
}
#[derive(Debug,Serialize)]
struct ResultOut {
    schema:&'static str,
    status:&'static str,
    profile:&'static str,
    engine_version:&'static str,
    exact_ring:&'static str,
    measurement_model:&'static str,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
    qubits:usize,
    classical_bits:usize,
    sampling:&'static str,
    logical_state_bytes:usize,
    coefficient_encoding:&'static str,
    basis_order:&'static str,
    classical_register:Vec<Option<bool>>,
    trace:Vec<MeasureOut>,
    final_joint_mass:ProbabilityOut,
    amplitudes:Vec<AmplitudeOut>,
    basis_joint_weights:Vec<ProbabilityOut>,
}
fn convert(request:&Request,metadata:&SymbolicMetadata,
           result:&crate::symbolic::SymbolicResult)
    ->Result<String,SymbolicProtocolError>
{
    let state=&result.final_state;
    let mut amplitudes=Vec::new();
    let mut probabilities=Vec::new();
    amplitudes.try_reserve_exact(state.dimension())
        .map_err(|_|SymbolicProtocolError::ProfileLimit("allocation_failed"))?;
    probabilities.try_reserve_exact(state.dimension())
        .map_err(|_|SymbolicProtocolError::ProfileLimit("allocation_failed"))?;
    for i in 0..state.dimension() {
        amplitudes.push(state.amplitude(i)?.into());
        probabilities.push(state.basis_weight(i)?.into());
    }
    let output=ResultOut {
        schema:metadata.schema,status:"simulated",
        profile:metadata.profile,engine_version:metadata.engine_version,
        exact_ring:metadata.exact_ring,
        measurement_model:metadata.measurement_model,
        evidence_class:metadata.evidence_class,
        qpu_hardware_jobs:metadata.qpu_hardware_jobs,
        qubits:request.qubits,classical_bits:request.classical_bits,
        sampling:"none;forced-outcomes-only",
        logical_state_bytes:state.logical_bytes(),
        coefficient_encoding:"signed-i128-decimal-strings",
        basis_order:"index-lsb-is-q0",
        classical_register:result.classical_bits.clone(),
        trace:result.trace.iter().map(MeasureOut::from).collect(),
        final_joint_mass:state.total_mass()?.into(),
        amplitudes,basis_joint_weights:probabilities,
    };
    let bytes=serde_json::to_string(&output)
        .map_err(|e|SymbolicProtocolError::Serialization(e.to_string()))?;
    if bytes.len()>MAX_SYMBOLIC_OUTPUT_BYTES {
        return Err(SymbolicProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    Ok(bytes)
}

/// Strict independent contract: no seed, sampling, QIR, source syntax or
/// hardware submission. Unknown fields fail before any program executes.
pub fn execute_symbolic_json(raw:&[u8])->Result<String,SymbolicProtocolError> {
    if raw.len()>MAX_SYMBOLIC_INPUT_BYTES {
        return Err(SymbolicProtocolError::InputTooLarge);
    }
    let probe:SchemaProbe=serde_json::from_slice(raw)
        .map_err(|e|SymbolicProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=SYMBOLIC_REQUEST_SCHEMA {
        return Err(SymbolicProtocolError::UnsupportedSchema);
    }
    let req:Request=serde_json::from_slice(raw)
        .map_err(|e|SymbolicProtocolError::InvalidJson(e.to_string()))?;
    if req.schema!=SYMBOLIC_REQUEST_SCHEMA {
        return Err(SymbolicProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_SYMBOLIC_QUBITS).contains(&req.qubits) {
        return Err(SymbolicProtocolError::ProfileLimit("qubits"));
    }
    if req.classical_bits>MAX_SYMBOLIC_CLASSICAL_BITS {
        return Err(SymbolicProtocolError::ProfileLimit("classical_bits"));
    }
    if req.instructions.len()>MAX_SYMBOLIC_OPERATIONS {
        return Err(SymbolicProtocolError::ProfileLimit("instructions"));
    }
    if req.max_memory_bytes==0||req.max_memory_bytes>DEFAULT_MEMORY_BUDGET {
        return Err(SymbolicProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=SymbolicCircuit::new(req.qubits,req.classical_bits);
    for op in &req.instructions {
        circuit.push(match op {
            Operation::Gate{gate}=>SymbolicInstruction::Gate(gate.gate()),
            Operation::Measure{qubit,into_bit,outcome}=>
                SymbolicInstruction::Measure {
                    qubit:*qubit,into_bit:*into_bit,outcome:*outcome,
                },
            Operation::If{bit,equals,gate}=>SymbolicInstruction::If {
                bit:*bit,equals:*equals,gate:gate.gate(),
            },
        });
    }
    let result=circuit.execute(req.max_memory_bytes)?;
    convert(&req,&result.metadata,&result)
}
