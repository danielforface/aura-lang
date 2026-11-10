//! C-025 strict offline mixed-state two-wire entanglement v1.
use crate::noisy_entanglement::{
    analyze_noisy_pair,EntanglementError,ENTANGLEMENT_PROFILE,
    EIGEN_ABS_TOLERANCE,PPT_WITNESS_TOLERANCE,
};
use crate::noisy_ensemble::{
    MAX_ENSEMBLE_QUBITS,MAX_ENSEMBLE_CLASSICAL_BITS,
    MAX_ENSEMBLE_MEASUREMENTS,MAX_ENSEMBLE_BRANCHES,
    MAX_ENSEMBLE_INSTRUCTIONS,NoisyCondition,
};
use crate::noisy_ensemble_json::convert;
use crate::noisy_circuit::NoisyCircuit;
use crate::noisy_json::{Operation,MAX_NOISY_REQUEST_BYTES,MAX_NOISY_RESULT_BYTES};
use crate::{DEFAULT_MEMORY_BUDGET,ENGINE_VERSION,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};
pub const ENTANGLEMENT_REQUEST_SCHEMA:&str="aura.quantum.noisy-pair-entanglement-request/1";
pub const ENTANGLEMENT_RESULT_SCHEMA:&str="aura.quantum.noisy-pair-entanglement-result/1";
pub const ENTANGLEMENT_ERROR_SCHEMA:&str="aura.quantum.noisy-pair-entanglement-error/1";
#[derive(Debug)]
pub enum EntanglementProtocolError{
    InputTooLarge,InvalidJson(String),UnsupportedSchema,
    ProfileLimit(&'static str),Rejected(EntanglementError),Serialization(String),
}
impl EntanglementProtocolError{
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
impl std::fmt::Display for EntanglementProtocolError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for EntanglementProtocolError{}
impl From<EntanglementError> for EntanglementProtocolError{
    fn from(e:EntanglementError)->Self{Self::Rejected(e)}
}
#[derive(Deserialize)]
struct Probe{schema:String}
#[derive(Clone,Serialize,Deserialize)]
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
    selected_pair:[usize;2],
    #[serde(default)]
    conditions:Option<Vec<ConditionIn>>,
}
#[derive(Serialize)]
struct Response{
    schema:&'static str,
    status:&'static str,
    evidence_class:&'static str,
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
    sampling:&'static str,
    shots:usize,
    entanglement_scope:&'static str,
    basis_order:&'static str,
    ppt_witness_numeric_threshold:f64,
    eigensolver_absolute_tolerance:f64,
    selected_pair:[usize;2],
    #[serde(skip_serializing_if="Option::is_none")]
    selected_conditions:Option<Vec<ConditionIn>>,
    #[serde(skip_serializing_if="Option::is_none")]
    condition_probability:Option<f64>,
    #[serde(skip_serializing_if="Option::is_none")]
    selected_branch_count:Option<usize>,
    reduced_density_row_major_complex_pairs:Vec<[f64;2]>,
    pair_purity:f64,
    single_wire_purities:[f64;2],
    mutual_information_bits:f64,
    partial_transpose_eigenvalues_ascending:[f64;4],
    negativity:f64,
    logarithmic_negativity_bits:f64,
    npt_entanglement_witness_detected:bool,
    near_zero_eigenvalues_clamped:bool,
    charged_work_units:usize,
    reserved_peak_logical_bytes:usize,
    qpu_hardware_jobs:u8,
}
/// Standalone strict protocol; no seed or sampled shots, no QPU/provider.
pub fn execute_noisy_pair_entanglement_json(raw:&[u8])
    ->Result<String,EntanglementProtocolError>{
    if raw.len()>MAX_NOISY_REQUEST_BYTES{
        return Err(EntanglementProtocolError::InputTooLarge);
    }
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|EntanglementProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=ENTANGLEMENT_REQUEST_SCHEMA{
        return Err(EntanglementProtocolError::UnsupportedSchema);
    }
    let req:Request=serde_json::from_slice(raw)
        .map_err(|e|EntanglementProtocolError::InvalidJson(e.to_string()))?;
    if req.schema!=ENTANGLEMENT_REQUEST_SCHEMA{
        return Err(EntanglementProtocolError::UnsupportedSchema);
    }
    if !(2..=MAX_ENSEMBLE_QUBITS).contains(&req.qubits){
        return Err(EntanglementProtocolError::ProfileLimit("qubits"));
    }
    if req.classical_bits>MAX_ENSEMBLE_CLASSICAL_BITS{
        return Err(EntanglementProtocolError::ProfileLimit("classical_bits"));
    }
    if req.instructions.len()>MAX_ENSEMBLE_INSTRUCTIONS{
        return Err(EntanglementProtocolError::ProfileLimit("instructions"));
    }
    if req.conditions.as_ref().is_some_and(|v|v.is_empty()
        ||v.len()>MAX_ENSEMBLE_CLASSICAL_BITS){
        return Err(EntanglementProtocolError::ProfileLimit("conditions"));
    }
    if req.max_memory_bytes==0||req.max_memory_bytes>DEFAULT_MEMORY_BUDGET{
        return Err(EntanglementProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=NoisyCircuit::new(req.qubits,req.classical_bits);
    for op in &req.instructions{circuit.push(convert(op));}
    let predicates=req.conditions.as_ref().map(|conditions|
        conditions.iter().map(|c|NoisyCondition{
            bit:c.bit,equals:c.equals,
        }).collect::<Vec<_>>()
    );
    let result=analyze_noisy_pair(&circuit,req.selected_pair,
        predicates.as_deref(),req.max_memory_bytes)?;
    let reduced=result.reduced_density.iter()
        .flat_map(|row|row.iter().map(|z|[z.re,z.im]))
        .collect::<Vec<_>>();
    let response=Response{
        schema:ENTANGLEMENT_RESULT_SCHEMA,status:"simulated",
        evidence_class:"simulation",engine_version:ENGINE_VERSION,
        profile:ENTANGLEMENT_PROFILE,precision:"IEEE-754-complex-f64",
        norm_tolerance:NORM_TOLERANCE,
        qubits:req.qubits,classical_bits:req.classical_bits,
        max_qubits:MAX_ENSEMBLE_QUBITS,max_classical_bits:MAX_ENSEMBLE_CLASSICAL_BITS,
        max_measurements:MAX_ENSEMBLE_MEASUREMENTS,
        max_branches:MAX_ENSEMBLE_BRANCHES,max_instructions:MAX_ENSEMBLE_INSTRUCTIONS,
        sampling:"none;exact_in_model_noisy_ensemble",
        shots:0,
        entanglement_scope:"two_wire_reduced_mixed_state_PPT_NPT_witness",
        basis_order:"global_q0_lsb;local_pair_a_plus_2b",
        ppt_witness_numeric_threshold:PPT_WITNESS_TOLERANCE,
        eigensolver_absolute_tolerance:EIGEN_ABS_TOLERANCE,
        selected_pair:result.selected_pair,
        selected_conditions:result.selected_conditions.as_ref().map(|x|
            x.iter().map(|c|ConditionIn{bit:c.bit,equals:c.equals}).collect()),
        condition_probability:result.condition_probability,
        selected_branch_count:result.selected_branch_count,
        reduced_density_row_major_complex_pairs:reduced,
        pair_purity:result.reduced_purity,
        single_wire_purities:result.single_wire_purities,
        mutual_information_bits:result.mutual_information_bits,
        partial_transpose_eigenvalues_ascending:result.partial_transpose_eigenvalues,
        negativity:result.negativity,
        logarithmic_negativity_bits:result.logarithmic_negativity_bits,
        npt_entanglement_witness_detected:result.npt_witness_detected,
        near_zero_eigenvalues_clamped:result.numerical_clamp_applied,
        charged_work_units:result.charged_work_units,
        reserved_peak_logical_bytes:result.reserved_peak_logical_bytes,
        qpu_hardware_jobs:0,
    };
    let serialized=serde_json::to_string(&response)
        .map_err(|e|EntanglementProtocolError::Serialization(e.to_string()))?;
    if serialized.len()>MAX_NOISY_RESULT_BYTES{
        return Err(EntanglementProtocolError::ProfileLimit("serialized_result_bytes"));
    }
    Ok(serialized)
}
