//! C-026: strict independently versioned noisy mixed-state Pauli energy.
//! No sampled shots or QPU/hardware/provider output; full channel is computed
//! via original C-023 and C-024 engine, not separate noise physics.
use crate::factorized::{
    PauliTerm,MAX_PAULI_TERMS,MAX_ABS_PAULI_COEFFICIENT,
};
use crate::noisy_ensemble::{
    NoisyCondition,MAX_ENSEMBLE_QUBITS,MAX_ENSEMBLE_CLASSICAL_BITS,
    MAX_ENSEMBLE_MEASUREMENTS,MAX_ENSEMBLE_BRANCHES,MAX_ENSEMBLE_INSTRUCTIONS,
};
use crate::noisy_hamiltonian::{
    analyze_noisy_hamiltonian,NoisyHamiltonianError,NOISY_HAMILTONIAN_PROFILE,
};
use crate::noisy_ensemble_json::convert;
use crate::noisy_circuit::NoisyCircuit;
use crate::noisy_json::{Operation,MAX_NOISY_REQUEST_BYTES,MAX_NOISY_RESULT_BYTES};
use crate::{ENGINE_VERSION,DEFAULT_MEMORY_BUDGET,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};

pub const NOISY_HAMILTONIAN_REQUEST_SCHEMA:&str=
    "aura.quantum.noisy-hamiltonian-request/1";
pub const NOISY_HAMILTONIAN_RESULT_SCHEMA:&str=
    "aura.quantum.noisy-hamiltonian-result/1";
pub const NOISY_HAMILTONIAN_ERROR_SCHEMA:&str=
    "aura.quantum.noisy-hamiltonian-error/1";
#[derive(Debug)]
pub enum NoisyHamiltonianProtocolError{
    InputTooLarge,InvalidJson(String),UnsupportedSchema,
    ProfileLimit(&'static str),Rejected(NoisyHamiltonianError),
    Serialization(String),
}
impl NoisyHamiltonianProtocolError{
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
impl std::fmt::Display for NoisyHamiltonianProtocolError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for NoisyHamiltonianProtocolError{}
impl From<NoisyHamiltonianError> for NoisyHamiltonianProtocolError{
    fn from(e:NoisyHamiltonianError)->Self{Self::Rejected(e)}
}
#[derive(Deserialize)]
struct Probe{schema:String}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConditionIn{bit:usize,equals:bool}
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
    #[serde(default)]
    conditions:Option<Vec<ConditionIn>>,
}
#[derive(Serialize)]
struct TermOut{
    paulis:String,coefficient:f64,expectation:f64,contribution:f64,
}
#[derive(Serialize)]
struct ConditionOut{bit:usize,equals:bool}
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
    max_classical_bits:usize,
    max_measurements:usize,
    max_branches:usize,
    max_instructions:usize,
    max_pauli_terms:usize,
    max_abs_coefficient:f64,
    pauli_word_order:&'static str,
    expectation_mode:&'static str,
    variance_definition:&'static str,
    sampling:&'static str,
    shots:usize,
    terms:Vec<TermOut>,
    term_count:usize,
    weighted_sum:f64,
    weighted_second_moment:f64,
    weighted_variance:f64,
    roundoff_clamped:bool,
    #[serde(skip_serializing_if="Option::is_none")]
    selected_conditions:Option<Vec<ConditionOut>>,
    #[serde(skip_serializing_if="Option::is_none")]
    condition_probability:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")]
    selected_branch_count:Option<usize>,
    reserved_peak_logical_bytes:usize,
    charged_work_units:usize,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
}
pub fn execute_noisy_hamiltonian_json(raw:&[u8])
    ->Result<String,NoisyHamiltonianProtocolError>{
    if raw.len()>MAX_NOISY_REQUEST_BYTES{
        return Err(NoisyHamiltonianProtocolError::InputTooLarge);
    }
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|NoisyHamiltonianProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=NOISY_HAMILTONIAN_REQUEST_SCHEMA{
        return Err(NoisyHamiltonianProtocolError::UnsupportedSchema);
    }
    let req:Request=serde_json::from_slice(raw)
        .map_err(|e|NoisyHamiltonianProtocolError::InvalidJson(e.to_string()))?;
    if req.schema!=NOISY_HAMILTONIAN_REQUEST_SCHEMA{
        return Err(NoisyHamiltonianProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_ENSEMBLE_QUBITS).contains(&req.qubits){
        return Err(NoisyHamiltonianProtocolError::ProfileLimit("qubits"));
    }
    if req.classical_bits>MAX_ENSEMBLE_CLASSICAL_BITS{
        return Err(NoisyHamiltonianProtocolError::ProfileLimit("classical_bits"));
    }
    if req.instructions.len()>MAX_ENSEMBLE_INSTRUCTIONS{
        return Err(NoisyHamiltonianProtocolError::ProfileLimit("instructions"));
    }
    if req.terms.is_empty()||req.terms.len()>MAX_PAULI_TERMS{
        return Err(NoisyHamiltonianProtocolError::ProfileLimit("terms"));
    }
    if req.conditions.as_ref().is_some_and(|bits|bits.is_empty()
        ||bits.len()>MAX_ENSEMBLE_CLASSICAL_BITS){
        return Err(NoisyHamiltonianProtocolError::ProfileLimit("conditions"));
    }
    if req.max_memory_bytes==0||req.max_memory_bytes>DEFAULT_MEMORY_BUDGET{
        return Err(NoisyHamiltonianProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=NoisyCircuit::new(req.qubits,req.classical_bits);
    for op in &req.instructions{circuit.push(convert(op));}
    let terms=req.terms.into_iter().map(|t|PauliTerm{
        paulis:t.paulis,coefficient:t.coefficient,
    }).collect::<Vec<_>>();
    let conditions=req.conditions.as_ref().map(|v|v.iter().map(|c|
        NoisyCondition{bit:c.bit,equals:c.equals}).collect::<Vec<_>>());
    let out=analyze_noisy_hamiltonian(
        &circuit,&terms,conditions.as_deref(),req.max_memory_bytes)?;
    let result=Response{
        schema:NOISY_HAMILTONIAN_RESULT_SCHEMA,status:"simulated",
        engine_version:ENGINE_VERSION,profile:NOISY_HAMILTONIAN_PROFILE,
        precision:"IEEE-754-complex-f64",norm_tolerance:NORM_TOLERANCE,
        qubits:req.qubits,classical_bits:req.classical_bits,
        max_qubits:MAX_ENSEMBLE_QUBITS,
        max_classical_bits:MAX_ENSEMBLE_CLASSICAL_BITS,
        max_measurements:MAX_ENSEMBLE_MEASUREMENTS,
        max_branches:MAX_ENSEMBLE_BRANCHES,
        max_instructions:MAX_ENSEMBLE_INSTRUCTIONS,
        max_pauli_terms:MAX_PAULI_TERMS,
        max_abs_coefficient:MAX_ABS_PAULI_COEFFICIENT,
        pauli_word_order:"ASCII_q0_first;basis_q0_lsb",
        expectation_mode:"analytic_noisy_mixed_channel_final",
        variance_definition:"Tr(rho H^2)-Tr(rho H)^2;not_shot_variance",
        sampling:"none;full_representable_Born_channel",
        shots:0,term_count:out.terms.len(),
        terms:out.terms.into_iter().map(|t|TermOut{
            paulis:t.paulis,coefficient:t.coefficient,
            expectation:t.expectation,contribution:t.contribution,
        }).collect(),
        weighted_sum:out.weighted_sum,
        weighted_second_moment:out.weighted_second_moment,
        weighted_variance:out.weighted_variance,
        roundoff_clamped:out.roundoff_clamped,
        selected_conditions:out.conditions.map(|v|v.into_iter().map(|c|
            ConditionOut{bit:c.bit,equals:c.equals}).collect()),
        condition_probability:out.condition_probability,
        selected_branch_count:out.selected_branch_count,
        reserved_peak_logical_bytes:out.reserved_peak_logical_bytes,
        charged_work_units:out.charged_work_units,
        evidence_class:"simulation",qpu_hardware_jobs:0,
    };
    let serialized=serde_json::to_string(&result)
        .map_err(|e|NoisyHamiltonianProtocolError::Serialization(e.to_string()))?;
    if serialized.len()>MAX_NOISY_RESULT_BYTES{
        return Err(NoisyHamiltonianProtocolError::ProfileLimit("serialized_result_bytes"));
    }
    Ok(serialized)
}
