//! C-021 strict, standalone postselected ideal-channel local tomography/1.
//! Only selected final-register conditions are echoed, not other histories.
use crate::channel_tomography::{
    execute_conditioned_tomography,ClassicalCondition,MIN_CONDITION_PROBABILITY,
    MAX_TOMOGRAPHY_PAIRS,TOMOGRAPHY_PROFILE,
};
use crate::channel_moments::{ChannelError,MAX_CHANNEL_QUBITS,
    MAX_CHANNEL_INSTRUMENTS,MAX_CHANNEL_BRANCHES};
use crate::circuit::{Circuit,Instruction};
use crate::factorized_json::{Operation,MAX_REQUEST_BYTES,MAX_RESULT_BYTES};
use crate::{Complex,DEFAULT_MEMORY_BUDGET,ENGINE_VERSION,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};
pub const CONDITIONED_TOMOGRAPHY_REQUEST_SCHEMA:&str=
    "aura.quantum.conditioned-tomography-request/1";
pub const CONDITIONED_TOMOGRAPHY_RESULT_SCHEMA:&str=
    "aura.quantum.conditioned-tomography-result/1";
pub const CONDITIONED_TOMOGRAPHY_ERROR_SCHEMA:&str=
    "aura.quantum.conditioned-tomography-error/1";
#[derive(Debug)]
pub enum ConditionedProtocolError{
    InputTooLarge,InvalidJson(String),UnsupportedSchema,
    ProfileLimit(&'static str),Rejected(ChannelError),Serialization(String),
}
impl ConditionedProtocolError{
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
impl std::fmt::Display for ConditionedProtocolError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for ConditionedProtocolError{}
impl From<ChannelError> for ConditionedProtocolError{
    fn from(e:ChannelError)->Self{Self::Rejected(e)}
}
#[derive(Deserialize)]
struct Probe{schema:String}
#[derive(Debug,Clone,Deserialize,Serialize)]
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
    pairs:Vec<[usize;2]>,
    conditions:Vec<ConditionIn>,
}
#[derive(Serialize)]
struct OneOut{
    wire:usize,
    rho_row_major_complex_pairs:Vec<[f64;2]>,
    bloch_xyz:[f64;3],
    purity:f64,
    mean_branch_purity:f64,
    branch_mixing_gap:f64,
}
#[derive(Serialize)]
struct TwoOut{
    wires:[usize;2],
    rho_row_major_complex_pairs:Vec<[f64;2]>,
    purity:f64,
    mean_branch_purity:f64,
    branch_mixing_gap:f64,
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
    max_requested_pairs:usize,
    min_condition_probability:f64,
    shots:usize,
    sampling:&'static str,
    density_scope:&'static str,
    basis_order:&'static str,
    condition_scope:&'static str,
    history_output_policy:&'static str,
    selected_conditions:Vec<ConditionIn>,
    condition_probability:f64,
    selected_branch_count:usize,
    instrument_count:usize,
    single_wire_reductions:Vec<OneOut>,
    selected_pair_reductions:Vec<TwoOut>,
    estimated_work_units:usize,
    reserved_peak_logical_bytes:usize,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
}
fn convert(op:&Operation)->Instruction{
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
fn complex_pairs<const N:usize>(matrix:&[[Complex;N];N])->Vec<[f64;2]>{
    matrix.iter().flat_map(|row|row.iter().map(|z|[z.re,z.im])).collect()
}
/// Returned only after complete bounded ideal-channel enumeration,
/// final-register postselection and numerical validation.
pub fn execute_conditioned_tomography_json(raw:&[u8])
    ->Result<String,ConditionedProtocolError>{
    if raw.len()>MAX_REQUEST_BYTES{
        return Err(ConditionedProtocolError::InputTooLarge);
    }
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|ConditionedProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=CONDITIONED_TOMOGRAPHY_REQUEST_SCHEMA{
        return Err(ConditionedProtocolError::UnsupportedSchema);
    }
    let input:Request=serde_json::from_slice(raw)
        .map_err(|e|ConditionedProtocolError::InvalidJson(e.to_string()))?;
    if input.schema!=CONDITIONED_TOMOGRAPHY_REQUEST_SCHEMA{
        return Err(ConditionedProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_CHANNEL_QUBITS).contains(&input.qubits){
        return Err(ConditionedProtocolError::ProfileLimit("qubits"));
    }
    if input.classical_bits>64{
        return Err(ConditionedProtocolError::ProfileLimit("classical_bits"));
    }
    if input.pairs.len()>MAX_TOMOGRAPHY_PAIRS{
        return Err(ConditionedProtocolError::ProfileLimit("pairs"));
    }
    if input.conditions.is_empty()||input.conditions.len()>64{
        return Err(ConditionedProtocolError::ProfileLimit("conditions"));
    }
    if input.max_memory_bytes==0||input.max_memory_bytes>DEFAULT_MEMORY_BUDGET{
        return Err(ConditionedProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=Circuit::new(input.qubits,input.classical_bits);
    for op in &input.instructions{circuit.push(convert(op));}
    let conditions=input.conditions.iter().map(|x|ClassicalCondition{
        bit:x.bit,equals:x.equals,
    }).collect::<Vec<_>>();
    let result=execute_conditioned_tomography(
        &circuit,&input.pairs,&conditions,input.max_memory_bytes)?;
    let singles=result.wires.into_iter().map(|d|OneOut{
        wire:d.wire,
        rho_row_major_complex_pairs:complex_pairs(&d.density),
        bloch_xyz:d.bloch,purity:d.purity,
        mean_branch_purity:d.mean_branch_purity,
        branch_mixing_gap:d.branch_mixing_gap,
    }).collect();
    let pairs=result.pairs.into_iter().map(|d|TwoOut{
        wires:d.wires,
        rho_row_major_complex_pairs:complex_pairs(&d.density),
        purity:d.purity,mean_branch_purity:d.mean_branch_purity,
        branch_mixing_gap:d.branch_mixing_gap,
    }).collect();
    let out=Response{
        schema:CONDITIONED_TOMOGRAPHY_RESULT_SCHEMA,
        status:"simulated",engine_version:ENGINE_VERSION,
        profile:TOMOGRAPHY_PROFILE,precision:"IEEE-754-complex-f64",
        norm_tolerance:NORM_TOLERANCE,
        qubits:input.qubits,classical_bits:input.classical_bits,
        max_qubits:MAX_CHANNEL_QUBITS,max_instruments:MAX_CHANNEL_INSTRUMENTS,
        max_branches:MAX_CHANNEL_BRANCHES,max_requested_pairs:MAX_TOMOGRAPHY_PAIRS,
        min_condition_probability:MIN_CONDITION_PROBABILITY,
        shots:0,sampling:"none;born_postselected_final_register",
        density_scope:"conditioned_final_ideal_channel_reduced_subsystems",
        basis_order:"global_q0_lsb;pair_[a,b]_local_basis_bit_a+2*bit_b",
        condition_scope:"final_explicitly_measured_classical_register",
        history_output_policy:"selected_only;no_unrequested_classical_histogram_or_trace",
        selected_conditions:input.conditions,
        condition_probability:result.condition_probability,
        selected_branch_count:result.selected_branch_count,
        instrument_count:result.instrument_count,
        single_wire_reductions:singles,
        selected_pair_reductions:pairs,
        estimated_work_units:result.work_units,
        reserved_peak_logical_bytes:result.reserved_peak_logical_bytes,
        evidence_class:"simulation",qpu_hardware_jobs:0,
    };
    let serialized=serde_json::to_string(&out)
        .map_err(|e|ConditionedProtocolError::Serialization(e.to_string()))?;
    if serialized.len()>MAX_RESULT_BYTES{
        return Err(ConditionedProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    Ok(serialized)
}
