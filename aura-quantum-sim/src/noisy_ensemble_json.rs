//! C-023 versioned offline JSON: unconditional noisy dynamic channel/1.
//! Reject unknown fields, seed, shots, hardware, QIR and ResetObserved.
use crate::noisy_ensemble::{
    execute_noisy_ensemble,EnsembleError,NOISY_ENSEMBLE_PROFILE,
    MAX_ENSEMBLE_QUBITS,MAX_ENSEMBLE_CLASSICAL_BITS,
    MAX_ENSEMBLE_MEASUREMENTS,MAX_ENSEMBLE_BRANCHES,
    MAX_ENSEMBLE_INSTRUCTIONS,MIN_ENSEMBLE_BRANCH_WEIGHT,
};
use crate::noisy_circuit::{NoisyAction,NoisyCircuit,NoisyInstruction};
use crate::noisy_json::{Operation,Conditioned,MAX_NOISY_REQUEST_BYTES,
    MAX_NOISY_RESULT_BYTES};
use crate::{DEFAULT_MEMORY_BUDGET,ENGINE_VERSION,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
pub const NOISY_ENSEMBLE_REQUEST_SCHEMA:&str="aura.quantum.noisy-ensemble-request/1";
pub const NOISY_ENSEMBLE_RESULT_SCHEMA:&str="aura.quantum.noisy-ensemble-result/1";
pub const NOISY_ENSEMBLE_ERROR_SCHEMA:&str="aura.quantum.noisy-ensemble-error/1";
#[derive(Debug)]
pub enum NoisyEnsembleProtocolError{
    InputTooLarge,InvalidJson(String),UnsupportedSchema,
    ProfileLimit(&'static str),Rejected(EnsembleError),Serialization(String),
}
impl NoisyEnsembleProtocolError{
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
impl std::fmt::Display for NoisyEnsembleProtocolError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for NoisyEnsembleProtocolError{}
impl From<EnsembleError> for NoisyEnsembleProtocolError{
    fn from(x:EnsembleError)->Self{Self::Rejected(x)}
}
#[derive(Deserialize)]
struct Probe{schema:String}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request{
    schema:String,
    qubits:usize,
    classical_bits:usize,
    max_memory_bytes:usize,
    instructions:Vec<Operation>,
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
    sampling:&'static str,
    shots:usize,
    noise_semantics:&'static str,
    reset_policy:&'static str,
    basis_order:&'static str,
    classical_histogram_encoding:&'static str,
    final_branch_count:usize,
    measurement_count:usize,
    probability_mass:f64,
    classical_probabilities:BTreeMap<String,f64>,
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
pub(crate) fn convert(op:&Operation)->NoisyInstruction{
    match op{
        Operation::Gate{gate}=>NoisyInstruction::Gate(gate.clone_gate()),
        Operation::Noise{channel}=>NoisyInstruction::Noise(channel.clone_channel()),
        Operation::Measure{qubit,into_bit}=>NoisyInstruction::Measure{
            qubit:*qubit,into_bit:*into_bit,
        },
        Operation::Reset{qubit}=>NoisyInstruction::Reset{qubit:*qubit},
        Operation::ResetObserved{qubit}=>NoisyInstruction::ResetObserved{
            qubit:*qubit,
        },
        Operation::If{bit,equals,action}=>NoisyInstruction::If{
            bit:*bit,equals:*equals,action:match action{
                Conditioned::Gate{gate}=>NoisyAction::Gate(gate.clone_gate()),
                Conditioned::Noise{channel}=>NoisyAction::Noise(channel.clone_channel()),
            },
        },
    }
}
/// Result is assembled only after entire bounded channel passes numeric checks.
pub fn execute_noisy_ensemble_json(raw:&[u8])
    ->Result<String,NoisyEnsembleProtocolError>{
    if raw.len()>MAX_NOISY_REQUEST_BYTES{
        return Err(NoisyEnsembleProtocolError::InputTooLarge);
    }
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|NoisyEnsembleProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=NOISY_ENSEMBLE_REQUEST_SCHEMA{
        return Err(NoisyEnsembleProtocolError::UnsupportedSchema);
    }
    let req:Request=serde_json::from_slice(raw)
        .map_err(|e|NoisyEnsembleProtocolError::InvalidJson(e.to_string()))?;
    if req.schema!=NOISY_ENSEMBLE_REQUEST_SCHEMA{
        return Err(NoisyEnsembleProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_ENSEMBLE_QUBITS).contains(&req.qubits){
        return Err(NoisyEnsembleProtocolError::ProfileLimit("qubits"));
    }
    if req.classical_bits>MAX_ENSEMBLE_CLASSICAL_BITS{
        return Err(NoisyEnsembleProtocolError::ProfileLimit("classical_bits"));
    }
    if req.max_memory_bytes==0||req.max_memory_bytes>DEFAULT_MEMORY_BUDGET{
        return Err(NoisyEnsembleProtocolError::ProfileLimit("max_memory_bytes"));
    }
    if req.instructions.len()>MAX_ENSEMBLE_INSTRUCTIONS{
        return Err(NoisyEnsembleProtocolError::ProfileLimit("instructions"));
    }
    let mut c=NoisyCircuit::new(req.qubits,req.classical_bits);
    for op in &req.instructions{c.push(convert(op));}
    let result=execute_noisy_ensemble(&c,req.max_memory_bytes)?;
    let density=result.final_density.iter().map(|x|[x.re,x.im]).collect();
    let basis=(0..result.dimension)
        .map(|i|result.final_density[i*result.dimension+i].re)
        .collect();
    let out=Response{
        schema:NOISY_ENSEMBLE_RESULT_SCHEMA,status:"simulated",
        engine_version:ENGINE_VERSION,profile:NOISY_ENSEMBLE_PROFILE,
        precision:"IEEE-754-complex-f64",norm_tolerance:NORM_TOLERANCE,
        qubits:req.qubits,classical_bits:req.classical_bits,
        max_qubits:MAX_ENSEMBLE_QUBITS,
        max_classical_bits:MAX_ENSEMBLE_CLASSICAL_BITS,
        max_measurements:MAX_ENSEMBLE_MEASUREMENTS,
        max_branches:MAX_ENSEMBLE_BRANCHES,
        max_instructions:MAX_ENSEMBLE_INSTRUCTIONS,
        min_represented_branch_weight:MIN_ENSEMBLE_BRANCH_WEIGHT,
        sampling:"none;all_representable_explicit_z_outcomes",
        shots:0,noise_semantics:"idealized_single_wire_CPTP_Kraus",
        reset_policy:"nonselective;reset_observed_rejected",
        basis_order:"q0_lsb;row_major_complex_pairs",
        classical_histogram_encoding:"msb_classical_first;?=unwritten",
        final_branch_count:result.final_branch_count,
        measurement_count:result.measurement_count,
        probability_mass:result.probability_mass,
        classical_probabilities:result.classical_probabilities,
        final_density_dimension:result.dimension,
        final_density_row_major_complex_pairs:density,
        final_basis_probabilities:basis,trace:result.trace,
        purity:result.purity,
        charged_work_units:result.charged_work_units,
        reserved_peak_logical_bytes:result.reserved_peak_logical_bytes,
        evidence_class:"simulation",qpu_hardware_jobs:0,
    };
    let serialized=serde_json::to_string(&out)
        .map_err(|e|NoisyEnsembleProtocolError::Serialization(e.to_string()))?;
    if serialized.len()>MAX_NOISY_RESULT_BYTES{
        return Err(NoisyEnsembleProtocolError::ProfileLimit("serialized_result_bytes"));
    }
    Ok(serialized)
}
