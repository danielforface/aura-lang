//! Experimental, versioned, offline CLI JSON boundary for Clifford tableau.
//! It is distinct from the existing pure and noisy JSON v1 profiles.
//! This is not Aura source, OpenQASM, QIR, a proof or a hardware submission.
use crate::stabilizer::{
    StabilizerCircuit,StabilizerError,StabilizerEventKind,
    StabilizerInstruction,StabilizerMetadata,StabilizerShot,
};
use crate::{Gate, DEFAULT_MEMORY_BUDGET};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;

pub const REQUEST_SCHEMA: &str = "aura.quantum.stabilizer-request/1";
pub const ERROR_SCHEMA: &str = "aura.quantum.stabilizer-error/1";
pub const MAX_INPUT_BYTES: usize = 256 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CLI_QUBITS: usize = 128;
pub const MAX_CLI_SHOTS: usize = 1024;
pub const MAX_CLI_OPERATIONS: usize = 512;
pub const MAX_CLI_CLASSICAL_BITS: usize = 64;
pub const MAX_CLI_WORK_UNITS: usize = 8 * 1024 * 1024;
pub const MAX_PROBABILITY_INSPECTION_QUBITS: usize = 6;

#[derive(Debug)]
pub enum StabilizerProtocolError {
    InputTooLarge,
    InvalidJson(String),
    UnsupportedSchema,
    ProfileLimit(&'static str),
    ArithmeticOverflow,
    Circuit(StabilizerError),
    Serialization(String),
}
impl StabilizerProtocolError {
    pub fn code(&self)->&'static str {
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
impl std::fmt::Display for StabilizerProtocolError {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        write!(f,"{self:?}")
    }
}
impl std::error::Error for StabilizerProtocolError {}
impl From<StabilizerError> for StabilizerProtocolError {
    fn from(e:StabilizerError)->Self {Self::Circuit(e)}
}

#[derive(Debug,Deserialize)]
struct SchemaProbe {schema:String}
#[derive(Debug,Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema:String,
    qubits:usize,
    classical_bits:usize,
    seed:u64,
    shots:usize,
    max_memory_bytes:usize,
    #[serde(default)]
    inspect_generators:bool,
    #[serde(default)]
    inspect_probabilities:bool,
    instructions:Vec<Operation>,
}

#[derive(Debug,Deserialize)]
#[serde(tag="op",rename_all="snake_case",deny_unknown_fields)]
enum Operation {
    Gate{gate:GateSpec},
    Measure{qubit:usize,into_bit:usize},
    Reset{qubit:usize},
    If{bit:usize,equals:bool,gate:GateSpec},
}
#[derive(Debug,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
enum GateSpec {
    H{target:usize},
    X{target:usize},
    Y{target:usize},
    Z{target:usize},
    S{target:usize},
    T{target:usize},
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
struct ResultOut {
    schema:&'static str,
    status:&'static str,
    profile:&'static str,
    engine_version:&'static str,
    precision:&'static str,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
    qubits:usize,
    classical_bits:usize,
    seed:u64,
    shots:usize,
    state_representation:&'static str,
    logical_state_bytes:usize,
    histogram_encoding:&'static str,
    histogram:BTreeMap<String,usize>,
    results:Vec<ShotOut>,
    #[serde(skip_serializing_if="Option::is_none")]
    stabilizer_generators:Option<Vec<String>>,
    #[serde(skip_serializing_if="Option::is_none")]
    basis_probabilities:Option<Vec<f64>>,
}
#[derive(Debug,Serialize)]
struct ShotOut {
    bits:Vec<Option<bool>>,
    trace:Vec<TraceOut>,
}
#[derive(Debug,Serialize)]
#[serde(tag="kind",rename_all="snake_case")]
enum TraceOut {
    Measure{instruction:usize,qubit:usize,into_bit:usize,value:bool},
    Reset{instruction:usize,qubit:usize,previous_value:bool},
}

fn shot_key(bits:&[Option<bool>])->String {
    bits.iter().rev().map(|b|match b {
        Some(true)=>'1',Some(false)=>'0',None=>'?'
    }).collect()
}
fn shot_out(s:&StabilizerShot)->ShotOut {
    ShotOut {
        bits:s.bits.clone(),
        trace:s.trace.iter().map(|event|match event.kind {
            StabilizerEventKind::Measure{into_bit,value}=>TraceOut::Measure{
                instruction:event.instruction,qubit:event.qubit,into_bit,value,
            },
            StabilizerEventKind::Reset{previous_value}=>TraceOut::Reset{
                instruction:event.instruction,qubit:event.qubit,previous_value,
            },
        }).collect(),
    }
}
fn encode(
    request:&Request,
    metadata:&StabilizerMetadata,
    records:&[StabilizerShot],
    state_bytes:usize,
    stabilizer_generators:Option<Vec<String>>,
    basis_probabilities:Option<Vec<f64>>,
)->Result<String,StabilizerProtocolError> {
    let mut histogram=BTreeMap::<String,usize>::new();
    for record in records {
        *histogram.entry(shot_key(&record.bits)).or_insert(0)+=1;
    }
    let output=ResultOut {
        schema:metadata.schema_version,
        status:"simulated",
        profile:metadata.profile,
        engine_version:metadata.engine_version,
        precision:metadata.precision,
        evidence_class:metadata.evidence_class,
        qpu_hardware_jobs:metadata.qpu_hardware_jobs,
        qubits:request.qubits,
        classical_bits:request.classical_bits,
        seed:metadata.seed,
        shots:metadata.shots,
        state_representation:"2n-signed-pauli-tableau",
        logical_state_bytes:state_bytes,
        histogram_encoding:"msb-classical-bit-first;?=unwritten",
        histogram,
        results:records.iter().map(shot_out).collect(),
        stabilizer_generators,
        basis_probabilities,
    };
    let encoded=serde_json::to_string(&output)
        .map_err(|err|StabilizerProtocolError::Serialization(err.to_string()))?;
    if encoded.len()>MAX_OUTPUT_BYTES {
        return Err(StabilizerProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    Ok(encoded)
}

/// One versioned, bounded offline request -> complete JSON result.
/// All unsupported operations are rejected before *any* shots execute.
pub fn execute_stabilizer_json(raw:&[u8])->Result<String,StabilizerProtocolError> {
    if raw.len()>MAX_INPUT_BYTES {
        return Err(StabilizerProtocolError::InputTooLarge);
    }
    let probe:SchemaProbe=serde_json::from_slice(raw)
        .map_err(|err|StabilizerProtocolError::InvalidJson(err.to_string()))?;
    if probe.schema!=REQUEST_SCHEMA {
        return Err(StabilizerProtocolError::UnsupportedSchema);
    }
    let request:Request=serde_json::from_slice(raw)
        .map_err(|err|StabilizerProtocolError::InvalidJson(err.to_string()))?;
    if request.schema!=REQUEST_SCHEMA {
        return Err(StabilizerProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_CLI_QUBITS).contains(&request.qubits) {
        return Err(StabilizerProtocolError::ProfileLimit("qubits"));
    }
    if request.classical_bits>MAX_CLI_CLASSICAL_BITS {
        return Err(StabilizerProtocolError::ProfileLimit("classical_bits"));
    }
    if !(1..=MAX_CLI_SHOTS).contains(&request.shots) {
        return Err(StabilizerProtocolError::ProfileLimit("shots"));
    }
    if request.instructions.len()>MAX_CLI_OPERATIONS {
        return Err(StabilizerProtocolError::ProfileLimit("operations"));
    }
    if request.max_memory_bytes==0||request.max_memory_bytes>DEFAULT_MEMORY_BUDGET {
        return Err(StabilizerProtocolError::ProfileLimit("max_memory_bytes"));
    }
    if (request.inspect_generators||request.inspect_probabilities)
        &&request.shots!=1 {
        return Err(StabilizerProtocolError::ProfileLimit("inspection_requires_one_shot"));
    }
    if request.inspect_probabilities&&request.qubits>MAX_PROBABILITY_INSPECTION_QUBITS {
        return Err(StabilizerProtocolError::ProfileLimit("basis_inspection_width"));
    }
    let work=request.qubits.checked_mul(request.instructions.len().max(1))
        .and_then(|x|x.checked_mul(request.shots))
        .ok_or(StabilizerProtocolError::ArithmeticOverflow)?;
    if work>MAX_CLI_WORK_UNITS {
        return Err(StabilizerProtocolError::ProfileLimit("tableau_work_units"));
    }
    let mut circuit=StabilizerCircuit::new(request.qubits,request.classical_bits);
    for instruction in &request.instructions {
        circuit.push(match instruction {
            Operation::Gate{gate}=>StabilizerInstruction::Gate(gate.gate()),
            Operation::Measure{qubit,into_bit}=>StabilizerInstruction::Measure{
                qubit:*qubit,into_bit:*into_bit
            },
            Operation::Reset{qubit}=>StabilizerInstruction::Reset{qubit:*qubit},
            Operation::If{bit,equals,gate}=>StabilizerInstruction::If{
                bit:*bit,equals:*equals,gate:gate.gate(),
            },
        });
    }
    if request.shots==1 {
        let outcome=circuit.execute_once(request.seed,request.max_memory_bytes)?;
        let labels=if request.inspect_generators {
            Some(outcome.final_tableau.stabilizer_generators())
        } else {None};
        let probabilities=if request.inspect_probabilities {
            let n=request.qubits;
            let mut values=Vec::new();
            values.try_reserve_exact(1usize<<n)
                .map_err(|_|StabilizerProtocolError::ProfileLimit("allocation_failed"))?;
            for basis in 0..(1usize<<n) {
                let bits=(0..n).map(|q|(basis>>q)&1!=0).collect::<Vec<_>>();
                values.push(outcome.final_tableau.basis_probability(&bits)?);
            }
            Some(values)
        } else {None};
        encode(
            &request,&outcome.metadata,&[outcome.shot],
            outcome.final_tableau.logical_bytes(),labels,probabilities,
        )
    } else {
        let batch=circuit.execute_shots(request.shots,request.seed,request.max_memory_bytes)?;
        // A deterministic allocation-size expression, not physical RSS.
        let single=circuit.execute_once(request.seed,request.max_memory_bytes)?;
        encode(&request,&batch.metadata,&batch.records,
               single.final_tableau.logical_bytes(),None,None)
    }
}
