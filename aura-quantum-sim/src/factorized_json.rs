//! Experimental offline JSON for the disconnected-product factor simulator.
//! Not MPS truncation, QIR, GPU, a quantum computer, nor a new physical
//! quantum model: original f64 Statevector is used inside each bounded factor.
use crate::circuit::{Instruction,TraceKind,ShotRecord};
use crate::factorized::{
    FactorCircuit,FactorError,FactorState,MAX_FACTOR_GLOBAL_QUBITS,
    MAX_FACTOR_INSTRUCTIONS,MAX_FACTOR_SHOTS,MAX_FACTOR_QUBITS,
    FACTORIZED_RESULT_SCHEMA,FACTORIZED_PROFILE,
};
use crate::{Gate,ENGINE_VERSION,DEFAULT_MEMORY_BUDGET,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;

pub const FACTOR_REQUEST_SCHEMA:&str="aura.quantum.factorized-request/1";
pub const FACTOR_ERROR_SCHEMA:&str="aura.quantum.factorized-error/1";
pub const MAX_REQUEST_BYTES:usize=256*1024;
pub const MAX_RESULT_BYTES:usize=16*1024*1024;
pub const MAX_AMP_INSPECTION_QUBITS:usize=8;

#[derive(Debug)]
pub enum FactorProtocolError {
    InputTooLarge,
    InvalidJson(String),
    UnsupportedSchema,
    ProfileLimit(&'static str),
    Factor(FactorError),
    Serialization(String),
}
impl FactorProtocolError {
    pub fn code(&self)->&'static str {
        match self {
            Self::InputTooLarge=>"input_too_large",
            Self::InvalidJson(_)=>"invalid_json",
            Self::UnsupportedSchema=>"unsupported_schema",
            Self::ProfileLimit(_)=>"profile_limit",
            Self::Factor(_)=>"circuit_rejected",
            Self::Serialization(_)=>"serialization_error",
        }
    }
}
impl std::fmt::Display for FactorProtocolError {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {write!(f,"{self:?}")}
}
impl std::error::Error for FactorProtocolError {}
impl From<FactorError> for FactorProtocolError {
    fn from(v:FactorError)->Self {Self::Factor(v)}
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
    inspect_factors:bool,
    #[serde(default)]
    inspect_amplitudes:bool,
    #[serde(default)]
    inspect_basis:Option<Vec<bool>>,
    instructions:Vec<Operation>,
}
#[derive(Debug,Deserialize)]
#[serde(tag="op",rename_all="snake_case",deny_unknown_fields)]
enum Operation {
    Gate {gate:GateSpec},
    Measure {qubit:usize,into_bit:usize},
    Reset {qubit:usize},
    If {bit:usize,equals:bool,gate:GateSpec},
}
#[derive(Debug,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
enum GateSpec {
    H{target:usize},X{target:usize},Y{target:usize},
    Z{target:usize},S{target:usize},T{target:usize},
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
struct ShotOut {
    bits:Vec<Option<bool>>,
    trace:Vec<EventOut>,
}
#[derive(Debug,Serialize)]
#[serde(tag="kind",rename_all="snake_case")]
enum EventOut {
    Measure{instruction:usize,qubit:usize,into_bit:usize,value:bool},
    Reset{instruction:usize,qubit:usize,previous_value:bool},
}
fn record(r:&ShotRecord)->ShotOut {
    ShotOut {
        bits:r.bits.clone(),
        trace:r.trace.iter().map(|e|match e.kind {
            TraceKind::Measure{into_bit,value}=>EventOut::Measure {
                instruction:e.instruction,qubit:e.qubit,into_bit,value,
            },
            TraceKind::Reset{previous_value}=>EventOut::Reset {
                instruction:e.instruction,qubit:e.qubit,previous_value,
            },
        }).collect(),
    }
}
fn hist_key(bits:&[Option<bool>])->String {
    bits.iter().rev().map(|b|match b {
        Some(true)=>'1',
        Some(false)=>'0',
        None=>'?'
    }).collect()
}
#[derive(Debug,Serialize)]
struct FactorOut {
    wires:Vec<usize>,
    local_dimension:usize,
    logical_state_bytes:usize,
}
#[derive(Debug,Serialize)]
struct Response {
    schema:&'static str,
    status:&'static str,
    engine_version:&'static str,
    profile:&'static str,
    precision:&'static str,
    truncation:&'static str,
    truncation_error_bound:f64,
    norm_tolerance:f64,
    factor_width_limit:usize,
    qubits:usize,
    classical_bits:usize,
    seed:u64,
    shots:usize,
    histogram_encoding:&'static str,
    histogram:BTreeMap<String,usize>,
    results:Vec<ShotOut>,
    peak_logical_state_bytes:usize,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
    #[serde(skip_serializing_if="Option::is_none")]
    factors:Option<Vec<FactorOut>>,
    #[serde(skip_serializing_if="Option::is_none")]
    inspected_basis_probability:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")]
    basis_ordered_complex_pairs:Option<Vec<[f64;2]>>,
}

/// Enforces selected bounded factor profile; once admitted, all state and
/// classical operations use the ordinary CPU Statevector/seeded RNG physics.
pub fn execute_factorized_json(raw:&[u8])->Result<String,FactorProtocolError> {
    if raw.len()>MAX_REQUEST_BYTES {return Err(FactorProtocolError::InputTooLarge);}
    let probe:SchemaProbe=serde_json::from_slice(raw)
        .map_err(|e|FactorProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=FACTOR_REQUEST_SCHEMA {
        return Err(FactorProtocolError::UnsupportedSchema);
    }
    let req:Request=serde_json::from_slice(raw)
        .map_err(|e|FactorProtocolError::InvalidJson(e.to_string()))?;
    if req.schema!=FACTOR_REQUEST_SCHEMA {
        return Err(FactorProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_FACTOR_GLOBAL_QUBITS).contains(&req.qubits) {
        return Err(FactorProtocolError::ProfileLimit("qubits"));
    }
    if req.classical_bits>64 {return Err(FactorProtocolError::ProfileLimit("classical_bits"));}
    if !(1..=MAX_FACTOR_SHOTS).contains(&req.shots) {
        return Err(FactorProtocolError::ProfileLimit("shots"));
    }
    if req.instructions.len()>MAX_FACTOR_INSTRUCTIONS {
        return Err(FactorProtocolError::ProfileLimit("instructions"));
    }
    if req.max_memory_bytes==0||req.max_memory_bytes>DEFAULT_MEMORY_BUDGET {
        return Err(FactorProtocolError::ProfileLimit("max_memory_bytes"));
    }
    if (req.inspect_factors||req.inspect_amplitudes||req.inspect_basis.is_some())
        &&req.shots!=1
    {
        return Err(FactorProtocolError::ProfileLimit("inspection_requires_one_shot"));
    }
    if req.inspect_amplitudes&&req.qubits>MAX_AMP_INSPECTION_QUBITS {
        return Err(FactorProtocolError::ProfileLimit("amplitude_inspection_width"));
    }
    if let Some(bits)=&req.inspect_basis {
        if bits.len()!=req.qubits {
            return Err(FactorProtocolError::ProfileLimit("inspect_basis_width"));
        }
    }
    let mut circuit=FactorCircuit::new(req.qubits,req.classical_bits);
    for op in &req.instructions {
        let instr=match op {
            Operation::Gate{gate}=>Instruction::Gate(gate.gate()),
            Operation::Measure{qubit,into_bit}=>Instruction::Measure{
                qubit:*qubit,into_bit:*into_bit,
            },
            Operation::Reset{qubit}=>Instruction::Reset{qubit:*qubit},
            Operation::If{bit,equals,gate}=>Instruction::If {
                bit:*bit,equals:*equals,gate:gate.gate(),
            },
        };
        circuit.push(instr);
    }
    let batch=circuit.execute_shots(req.shots,req.seed,req.max_memory_bytes)?;
    let mut histogram=BTreeMap::new();
    for record in &batch.records {
        *histogram.entry(hist_key(&record.bits)).or_insert(0usize)+=1;
    }
    let inspected=batch.one_shot_state.as_ref();
    let factors=if req.inspect_factors {
        let state=inspected.ok_or(FactorProtocolError::ProfileLimit("no_inspection_state"))?;
        Some(state.factor_wires().into_iter().map(|wires|{
            let dimension=1usize<<wires.len();
            FactorOut {
                local_dimension:dimension,
                logical_state_bytes:dimension*std::mem::size_of::<crate::Complex>(),
                wires,
            }
        }).collect())
    } else {None};
    let inspected_basis_probability=if let Some(bits)=&req.inspect_basis {
        Some(inspected.ok_or(FactorProtocolError::ProfileLimit("no_inspection_state"))?
            .basis_probability(bits)?)
    } else {None};
    let basis_ordered_complex_pairs=if req.inspect_amplitudes {
        let state: &FactorState=inspected
            .ok_or(FactorProtocolError::ProfileLimit("no_inspection_state"))?;
        let mut values=Vec::new();
        values.try_reserve_exact(1usize<<req.qubits)
            .map_err(|_|FactorProtocolError::ProfileLimit("allocation_failed"))?;
        for i in 0..(1usize<<req.qubits) {
            let bits=(0..req.qubits).map(|q|(i>>q)&1!=0).collect::<Vec<_>>();
            let amp=state.amplitude_bits(&bits)?;
            values.push([amp.re,amp.im]);
        }
        Some(values)
    } else {None};
    let response=Response{
        schema:FACTORIZED_RESULT_SCHEMA,
        status:"simulated",
        engine_version:ENGINE_VERSION,
        profile:FACTORIZED_PROFILE,
        precision:"IEEE-754-complex-f64",
        truncation:"none;factor-width-hard-limit",
        truncation_error_bound:0.0, // ONLY tensor truncation, not floating error.
        norm_tolerance:NORM_TOLERANCE,
        factor_width_limit:MAX_FACTOR_QUBITS,
        qubits:req.qubits,classical_bits:req.classical_bits,
        seed:req.seed,shots:req.shots,
        histogram_encoding:"msb-classical-bit-first;?=unwritten",
        histogram,
        results:batch.records.iter().map(record).collect(),
        peak_logical_state_bytes:batch.peak_logical_state_bytes,
        evidence_class:"simulation",
        qpu_hardware_jobs:0,
        factors,
        inspected_basis_probability,
        basis_ordered_complex_pairs,
    };
    let json=serde_json::to_string(&response)
        .map_err(|e|FactorProtocolError::Serialization(e.to_string()))?;
    if json.len()>MAX_RESULT_BYTES {
        return Err(FactorProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    Ok(json)
}
