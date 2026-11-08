//! C-018 independent exact ideal quantum channel/1 offline contract.
//! No seed, shots, noise-model claims, hardware dispatch, or partial stdout.
use crate::channel_moments::{
    execute_channel_moments,ChannelError,CHANNEL_PROFILE,MAX_CHANNEL_QUBITS,
    MAX_CHANNEL_INSTRUMENTS,MAX_CHANNEL_BRANCHES,
};
use crate::circuit::{Circuit,Instruction};
use crate::factorized::PauliTerm;
use crate::factorized_json::{Operation,MAX_REQUEST_BYTES,MAX_RESULT_BYTES};
use crate::{DEFAULT_MEMORY_BUDGET,ENGINE_VERSION,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
pub const CHANNEL_REQUEST_SCHEMA:&str="aura.quantum.channel-moments-request/1";
pub const CHANNEL_RESULT_SCHEMA:&str="aura.quantum.channel-moments-result/1";
pub const CHANNEL_ERROR_SCHEMA:&str="aura.quantum.channel-moments-error/1";
#[derive(Debug)]
pub enum ChannelProtocolError{
    InputTooLarge,InvalidJson(String),UnsupportedSchema,
    ProfileLimit(&'static str),Rejected(ChannelError),Serialization(String),
}
impl ChannelProtocolError{
    pub fn code(&self)->&'static str{
        match self{
            Self::InputTooLarge=>"input_too_large",
            Self::InvalidJson(_)=>"invalid_json",
            Self::UnsupportedSchema=>"unsupported_schema",
            Self::ProfileLimit(_)=>"profile_limit",
            Self::Rejected(_)=>"circuit_rejected",
            Self::Serialization(_)=>"serialization_error",
        }
    }
}
impl From<ChannelError> for ChannelProtocolError{
    fn from(e:ChannelError)->Self{Self::Rejected(e)}
}
impl std::fmt::Display for ChannelProtocolError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for ChannelProtocolError{}
#[derive(Deserialize)]
struct Probe{schema:String}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TermIn{paulis:String,coefficient:f64}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request{
    schema:String,
    qubits:usize,
    classical_bits:usize,
    max_memory_bytes:usize,
    instructions:Vec<Operation>,
    terms:Vec<TermIn>,
}
#[derive(Serialize)]
struct Response{
    schema:&'static str,
    status:&'static str,
    engine_version:&'static str,
    profile:&'static str,
    precision:&'static str,
    norm_tolerance:f64,
    qubits:usize,
    classical_bits:usize,
    max_qubits:usize,
    max_instruments:usize,
    max_branches:usize,
    shots:usize,
    sampling:&'static str,
    energy_scope:&'static str,
    pauli_word_order:&'static str,
    instrument_count:usize,
    final_branch_count:usize,
    total_branch_probability:f64,
    classical_probabilities:BTreeMap<String,f64>,
    weighted_sum:f64,
    weighted_second_moment:f64,
    weighted_variance:f64,
    within_branch_variance:f64,
    between_branch_variance:f64,
    roundoff_clamped:bool,
    estimated_work_units:usize,
    reserved_peak_logical_bytes:usize,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
}
fn operation(op:&Operation)->Instruction{
    match op{
        Operation::Gate{gate}=>Instruction::Gate(gate.gate()),
        Operation::Measure{qubit,into_bit}=>Instruction::Measure{
            qubit:*qubit,into_bit:*into_bit,
        },
        Operation::Reset{qubit}=>Instruction::Reset{qubit:*qubit},
        Operation::If{bit,equals,gate}=>Instruction::If{
            bit:*bit,equals:*equals,gate:gate.gate(),
        },
    }
}
/// Strict typed result: all represented ideal instrument outcomes or failure.
/// The old observable and seeded moments schemas are not accepted or updated.
pub fn execute_channel_moments_json(raw:&[u8])
    ->Result<String,ChannelProtocolError>{
    if raw.len()>MAX_REQUEST_BYTES{return Err(ChannelProtocolError::InputTooLarge);}
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|ChannelProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=CHANNEL_REQUEST_SCHEMA{
        return Err(ChannelProtocolError::UnsupportedSchema);
    }
    let input:Request=serde_json::from_slice(raw)
        .map_err(|e|ChannelProtocolError::InvalidJson(e.to_string()))?;
    if input.schema!=CHANNEL_REQUEST_SCHEMA{
        return Err(ChannelProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_CHANNEL_QUBITS).contains(&input.qubits){
        return Err(ChannelProtocolError::ProfileLimit("qubits"));
    }
    if input.classical_bits>64{
        return Err(ChannelProtocolError::ProfileLimit("classical_bits"));
    }
    if input.max_memory_bytes==0||input.max_memory_bytes>DEFAULT_MEMORY_BUDGET{
        return Err(ChannelProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=Circuit::new(input.qubits,input.classical_bits);
    for op in &input.instructions{circuit.push(operation(op));}
    let terms=input.terms.into_iter().map(|t|PauliTerm{
        paulis:t.paulis,coefficient:t.coefficient,
    }).collect::<Vec<_>>();
    let m=execute_channel_moments(&circuit,&terms,input.max_memory_bytes)?;
    let out=Response{
        schema:CHANNEL_RESULT_SCHEMA,status:"simulated",
        engine_version:ENGINE_VERSION,profile:CHANNEL_PROFILE,
        precision:"IEEE-754-complex-f64",norm_tolerance:NORM_TOLERANCE,
        qubits:input.qubits,classical_bits:input.classical_bits,
        max_qubits:MAX_CHANNEL_QUBITS,max_instruments:MAX_CHANNEL_INSTRUMENTS,
        max_branches:MAX_CHANNEL_BRANCHES,shots:0,
        sampling:"none;all_representable_instrument_branches",
        energy_scope:"unconditioned_ideal_instrument_channel",
        pauli_word_order:"q0-first;position-0-is-least-significant-wire",
        instrument_count:m.instrument_count,final_branch_count:m.branch_count,
        total_branch_probability:m.probability_mass,
        classical_probabilities:m.classical_probabilities,
        weighted_sum:m.mean,weighted_second_moment:m.second_moment,
        weighted_variance:m.variance,
        within_branch_variance:m.within_branch_variance,
        between_branch_variance:m.between_branch_variance,
        roundoff_clamped:m.roundoff_clamped,
        estimated_work_units:m.work_units,
        reserved_peak_logical_bytes:m.reserved_peak_logical_bytes,
        evidence_class:"simulation",qpu_hardware_jobs:0,
    };
    let serialized=serde_json::to_string(&out)
        .map_err(|e|ChannelProtocolError::Serialization(e.to_string()))?;
    if serialized.len()>MAX_RESULT_BYTES{
        return Err(ChannelProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    Ok(serialized)
}
