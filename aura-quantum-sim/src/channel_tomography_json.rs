//! Distinct strict offline C-020 local-channel tomography request/result/1.
//! No QPU, sampling, seed, hardware profile or quantum IR.
use crate::channel_tomography::{
    execute_channel_tomography,MAX_TOMOGRAPHY_PAIRS,TOMOGRAPHY_PROFILE,
};
use crate::channel_moments::{ChannelError,MAX_CHANNEL_QUBITS,
    MAX_CHANNEL_INSTRUMENTS,MAX_CHANNEL_BRANCHES};
use crate::circuit::{Circuit,Instruction};
use crate::factorized_json::{Operation,MAX_REQUEST_BYTES,MAX_RESULT_BYTES};
use crate::{Complex,DEFAULT_MEMORY_BUDGET,ENGINE_VERSION,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
pub const TOMOGRAPHY_REQUEST_SCHEMA:&str="aura.quantum.channel-tomography-request/1";
pub const TOMOGRAPHY_RESULT_SCHEMA:&str="aura.quantum.channel-tomography-result/1";
pub const TOMOGRAPHY_ERROR_SCHEMA:&str="aura.quantum.channel-tomography-error/1";
#[derive(Debug)]
pub enum TomographyProtocolError{
    InputTooLarge,InvalidJson(String),UnsupportedSchema,
    ProfileLimit(&'static str),Rejected(ChannelError),Serialization(String),
}
impl TomographyProtocolError{
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
impl From<ChannelError> for TomographyProtocolError{
    fn from(x:ChannelError)->Self{Self::Rejected(x)}
}
impl std::fmt::Display for TomographyProtocolError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for TomographyProtocolError{}
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
    pairs:Vec<[usize;2]>,
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
    shots:usize,
    sampling:&'static str,
    density_scope:&'static str,
    basis_order:&'static str,
    measurement_outcome_policy:&'static str,
    instrument_count:usize,
    final_branch_count:usize,
    total_branch_probability:f64,
    classical_probabilities:BTreeMap<String,f64>,
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
fn pairs<const N:usize>(matrix:&[[Complex;N];N])->Vec<[f64;2]>{
    matrix.iter().flat_map(|row|row.iter().map(|z|[z.re,z.im])).collect()
}
/// Independent v1 surface with no implicit default pairs or hardware.
pub fn execute_channel_tomography_json(raw:&[u8])
    ->Result<String,TomographyProtocolError>{
    if raw.len()>MAX_REQUEST_BYTES{return Err(TomographyProtocolError::InputTooLarge);}
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|TomographyProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=TOMOGRAPHY_REQUEST_SCHEMA{
        return Err(TomographyProtocolError::UnsupportedSchema);
    }
    let input:Request=serde_json::from_slice(raw)
        .map_err(|e|TomographyProtocolError::InvalidJson(e.to_string()))?;
    if input.schema!=TOMOGRAPHY_REQUEST_SCHEMA{
        return Err(TomographyProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_CHANNEL_QUBITS).contains(&input.qubits){
        return Err(TomographyProtocolError::ProfileLimit("qubits"));
    }
    if input.classical_bits>64{
        return Err(TomographyProtocolError::ProfileLimit("classical_bits"));
    }
    if input.pairs.len()>MAX_TOMOGRAPHY_PAIRS{
        return Err(TomographyProtocolError::ProfileLimit("pairs"));
    }
    if input.max_memory_bytes==0||input.max_memory_bytes>DEFAULT_MEMORY_BUDGET{
        return Err(TomographyProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=Circuit::new(input.qubits,input.classical_bits);
    for op in &input.instructions{circuit.push(convert(op));}
    let result=execute_channel_tomography(
        &circuit,&input.pairs,input.max_memory_bytes)?;
    let singles=result.wires.into_iter().map(|d|OneOut{
        wire:d.wire,
        rho_row_major_complex_pairs:pairs(&d.density),
        bloch_xyz:d.bloch,purity:d.purity,
        mean_branch_purity:d.mean_branch_purity,
        branch_mixing_gap:d.branch_mixing_gap,
    }).collect();
    let pair_reductions=result.pairs.into_iter().map(|d|TwoOut{
        wires:d.wires,rho_row_major_complex_pairs:pairs(&d.density),
        purity:d.purity,mean_branch_purity:d.mean_branch_purity,
        branch_mixing_gap:d.branch_mixing_gap,
    }).collect();
    let out=Response{
        schema:TOMOGRAPHY_RESULT_SCHEMA,status:"simulated",
        engine_version:ENGINE_VERSION,profile:TOMOGRAPHY_PROFILE,
        precision:"IEEE-754-complex-f64",norm_tolerance:NORM_TOLERANCE,
        qubits:input.qubits,classical_bits:input.classical_bits,
        max_qubits:MAX_CHANNEL_QUBITS,max_instruments:MAX_CHANNEL_INSTRUMENTS,
        max_branches:MAX_CHANNEL_BRANCHES,max_requested_pairs:MAX_TOMOGRAPHY_PAIRS,
        shots:0,sampling:"none;all_representable_instrument_branches",
        density_scope:"unconditioned_final_ideal_channel_reduced_subsystems",
        basis_order:"global_q0_lsb;pair_[a,b]_local_basis_bit_a+2*bit_b",
        measurement_outcome_policy:"reset_hidden;explicit_measure_writes_classical",
        instrument_count:result.instrument_count,
        final_branch_count:result.branch_count,
        total_branch_probability:result.probability_mass,
        classical_probabilities:result.classical_probabilities,
        single_wire_reductions:singles,selected_pair_reductions:pair_reductions,
        estimated_work_units:result.work_units,
        reserved_peak_logical_bytes:result.reserved_peak_logical_bytes,
        evidence_class:"simulation",qpu_hardware_jobs:0,
    };
    let serialized=serde_json::to_string(&out)
        .map_err(|e|TomographyProtocolError::Serialization(e.to_string()))?;
    if serialized.len()>MAX_RESULT_BYTES{
        return Err(TomographyProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    Ok(serialized)
}
