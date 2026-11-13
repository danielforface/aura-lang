//! C-024 versioned offline final-register postselected noisy ensemble/1.
//! Does not expose other histories or conditional internal Kraus outcomes.
use crate::noisy_ensemble::{
    execute_noisy_conditioned_ensemble,NoisyCondition,EnsembleError,
    NOISY_ENSEMBLE_PROFILE,MIN_NOISY_CONDITION_PROBABILITY,
    MIN_ENSEMBLE_BRANCH_WEIGHT,MAX_ENSEMBLE_QUBITS,
    MAX_ENSEMBLE_CLASSICAL_BITS,MAX_ENSEMBLE_MEASUREMENTS,
    MAX_ENSEMBLE_BRANCHES,MAX_ENSEMBLE_INSTRUCTIONS,
};
use crate::noisy_ensemble_json::convert;
use crate::noisy_circuit::NoisyCircuit;
use crate::noisy_json::{Operation,MAX_NOISY_REQUEST_BYTES,MAX_NOISY_RESULT_BYTES};
use crate::{DEFAULT_MEMORY_BUDGET,ENGINE_VERSION,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};
pub const NOISY_CONDITIONED_REQUEST_SCHEMA:&str=
    "aura.quantum.noisy-conditioned-request/1";
pub const NOISY_CONDITIONED_RESULT_SCHEMA:&str=
    "aura.quantum.noisy-conditioned-result/1";
pub const NOISY_CONDITIONED_ERROR_SCHEMA:&str=
    "aura.quantum.noisy-conditioned-error/1";
#[derive(Debug)]
pub enum NoisyConditionedProtocolError{
    InputTooLarge,InvalidJson(String),UnsupportedSchema,
    ProfileLimit(&'static str),Rejected(EnsembleError),Serialization(String),
}
impl NoisyConditionedProtocolError{
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
impl std::fmt::Display for NoisyConditionedProtocolError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for NoisyConditionedProtocolError{}
impl From<EnsembleError> for NoisyConditionedProtocolError{
    fn from(e:EnsembleError)->Self{Self::Rejected(e)}
}
#[derive(Deserialize)]
struct Probe{schema:String}
#[derive(Clone,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
struct ConditionIn{bit:usize,equals:bool}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request{
    schema:String,
    qubits:usize,
    classical_bits:usize,
    max_memory_bytes:usize,
    instructions:Vec<Operation>,
    conditions:Vec<ConditionIn>,
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
    max_classical_bits:usize,
    max_measurements:usize,
    max_branches:usize,
    max_instructions:usize,
    min_represented_branch_weight:f64,
    min_condition_probability:f64,
    sampling:&'static str,
    shots:usize,
    noise_semantics:&'static str,
    reset_policy:&'static str,
    condition_scope:&'static str,
    disclosure_policy:&'static str,
    basis_order:&'static str,
    selected_conditions:Vec<ConditionIn>,
    condition_probability:f64,
    selected_branch_count:usize,
    measurement_count:usize,
    final_density_dimension:usize,
    final_density_row_major_complex_pairs:Vec<[f64;2]>,
    final_basis_probabilities:Vec<f64>,
    trace:f64,
    purity:f64,
    charged_work_units:usize,
    reserved_peak_logical_bytes:usize,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
}
pub fn execute_noisy_conditioned_json(raw:&[u8])
    ->Result<String,NoisyConditionedProtocolError>{
    if raw.len()>MAX_NOISY_REQUEST_BYTES{
        return Err(NoisyConditionedProtocolError::InputTooLarge);
    }
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|NoisyConditionedProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=NOISY_CONDITIONED_REQUEST_SCHEMA{
        return Err(NoisyConditionedProtocolError::UnsupportedSchema);
    }
    let req:Request=serde_json::from_slice(raw)
        .map_err(|e|NoisyConditionedProtocolError::InvalidJson(e.to_string()))?;
    if req.schema!=NOISY_CONDITIONED_REQUEST_SCHEMA{
        return Err(NoisyConditionedProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_ENSEMBLE_QUBITS).contains(&req.qubits){
        return Err(NoisyConditionedProtocolError::ProfileLimit("qubits"));
    }
    if req.classical_bits>MAX_ENSEMBLE_CLASSICAL_BITS{
        return Err(NoisyConditionedProtocolError::ProfileLimit("classical_bits"));
    }
    if req.instructions.len()>MAX_ENSEMBLE_INSTRUCTIONS{
        return Err(NoisyConditionedProtocolError::ProfileLimit("instructions"));
    }
    if req.conditions.is_empty()
        ||req.conditions.len()>MAX_ENSEMBLE_CLASSICAL_BITS{
        return Err(NoisyConditionedProtocolError::ProfileLimit("conditions"));
    }
    if req.max_memory_bytes==0||req.max_memory_bytes>DEFAULT_MEMORY_BUDGET{
        return Err(NoisyConditionedProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=NoisyCircuit::new(req.qubits,req.classical_bits);
    for op in &req.instructions{circuit.push(convert(op));}
    let conditions=req.conditions.iter().map(|c|NoisyCondition{
        bit:c.bit,equals:c.equals,
    }).collect::<Vec<_>>();
    let result=execute_noisy_conditioned_ensemble(
        &circuit,&conditions,req.max_memory_bytes)?;
    let density=result.final_density.iter().map(|z|[z.re,z.im]).collect();
    let basis=(0..result.dimension)
        .map(|i|result.final_density[i*result.dimension+i].re).collect();
    let out=Response{
        schema:NOISY_CONDITIONED_RESULT_SCHEMA,status:"simulated",
        engine_version:ENGINE_VERSION,profile:NOISY_ENSEMBLE_PROFILE,
        precision:"IEEE-754-complex-f64",norm_tolerance:NORM_TOLERANCE,
        qubits:req.qubits,classical_bits:req.classical_bits,
        max_qubits:MAX_ENSEMBLE_QUBITS,
        max_classical_bits:MAX_ENSEMBLE_CLASSICAL_BITS,
        max_measurements:MAX_ENSEMBLE_MEASUREMENTS,
        max_branches:MAX_ENSEMBLE_BRANCHES,
        max_instructions:MAX_ENSEMBLE_INSTRUCTIONS,
        min_represented_branch_weight:MIN_ENSEMBLE_BRANCH_WEIGHT,
        min_condition_probability:MIN_NOISY_CONDITION_PROBABILITY,
        sampling:"none;postselected_born_ensemble",
        shots:0,noise_semantics:"idealized_single_wire_CPTP_Kraus",
        reset_policy:"nonselective;reset_observed_rejected",
        condition_scope:"final_explicitly_measured_classical_register",
        disclosure_policy:"selected_event_only;no_unselected_histogram_or_trace",
        basis_order:"q0_lsb;row_major_complex_pairs",
        selected_conditions:req.conditions,
        condition_probability:result.condition_probability,
        selected_branch_count:result.selected_branch_count,
        measurement_count:result.measurement_count,
        final_density_dimension:result.dimension,
        final_density_row_major_complex_pairs:density,
        final_basis_probabilities:basis,trace:result.trace,
        purity:result.purity,
        charged_work_units:result.charged_work_units,
        reserved_peak_logical_bytes:result.reserved_peak_logical_bytes,
        evidence_class:"simulation",qpu_hardware_jobs:0,
    };
    let serialized=serde_json::to_string(&out)
        .map_err(|e|NoisyConditionedProtocolError::Serialization(e.to_string()))?;
    if serialized.len()>MAX_NOISY_RESULT_BYTES{
        return Err(NoisyConditionedProtocolError::ProfileLimit("serialized_result_bytes"));
    }
    Ok(serialized)
}
