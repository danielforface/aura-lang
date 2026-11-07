//! Bounded offline histogram-only CPU sampling without retaining shot traces.
//! Independent strict schema: does not change factorized-request/1 semantics.
use crate::factorized::{
    FactorCircuit,FactorCounts,
    MAX_FACTOR_GLOBAL_QUBITS,MAX_FACTOR_INSTRUCTIONS,
    MAX_FACTOR_COUNT_SHOTS,MAX_FACTOR_COUNT_WORK_UNITS,
    MAX_FACTOR_QUBITS,FACTORIZED_COUNTS_RESULT_SCHEMA,FACTORIZED_PROFILE,
};
use crate::factorized_json::{
    Operation,FactorProtocolError,MAX_REQUEST_BYTES,MAX_RESULT_BYTES,
};
use crate::circuit::Instruction;
use crate::{DEFAULT_MEMORY_BUDGET,ENGINE_VERSION,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;

pub const FACTOR_COUNTS_REQUEST_SCHEMA:&str="aura.quantum.factorized-counts-request/1";
pub const FACTOR_COUNTS_ERROR_SCHEMA:&str="aura.quantum.factorized-counts-error/1";

#[derive(Debug,Deserialize)]
struct Probe {schema:String}
#[derive(Debug,Deserialize)]
#[serde(deny_unknown_fields)]
struct CountsRequest {
    schema:String,
    qubits:usize,
    classical_bits:usize,
    seed:u64,
    shots:usize,
    max_memory_bytes:usize,
    instructions:Vec<Operation>,
}
#[derive(Serialize)]
struct CountsResponse<'a> {
    schema:&'static str,
    status:&'static str,
    engine_version:&'static str,
    profile:&'static str,
    precision:&'static str,
    truncation:&'static str,
    norm_tolerance:f64,
    qubits:usize,
    classical_bits:usize,
    seed:u64,
    shots:usize,
    counts_only:bool,
    retained_shot_records:usize,
    max_histogram_bins:usize,
    histogram_encoding:&'static str,
    histogram:&'a BTreeMap<String,usize>,
    logical_histogram_reserved_bytes:usize,
    peak_logical_state_bytes:usize,
    factor_width_limit:usize,
    max_cpu_work_units:usize,
    projective_wire_splits:usize,
    projective_split_deferred:usize,
    product_preserving_two_wire_gates:usize,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
}
fn convert(op:&Operation)->Instruction {
    match op {
        Operation::Gate{gate}=>Instruction::Gate(gate.gate()),
        Operation::Measure{qubit,into_bit}=>Instruction::Measure{
            qubit:*qubit,into_bit:*into_bit
        },
        Operation::Reset{qubit}=>Instruction::Reset{qubit:*qubit},
        Operation::If{bit,equals,gate}=>Instruction::If{
            bit:*bit,equals:*equals,gate:gate.gate()
        },
    }
}
fn wrap<'a>(r:&'a FactorCounts,qubits:usize,bits:usize)->CountsResponse<'a> {
    CountsResponse {
        schema:FACTORIZED_COUNTS_RESULT_SCHEMA,
        status:"simulated",
        engine_version:ENGINE_VERSION,
        profile:FACTORIZED_PROFILE,
        precision:"IEEE-754-complex-f64",
        truncation:"none;factor-width-hard-limit",
        norm_tolerance:NORM_TOLERANCE,
        qubits,classical_bits:bits,
        seed:r.metadata.seed,shots:r.metadata.shots,
        counts_only:true,retained_shot_records:r.retained_shot_records,
        max_histogram_bins:r.max_histogram_bins,
        histogram_encoding:"msb-classical-bit-first;?=unwritten",
        histogram:&r.histogram,
        logical_histogram_reserved_bytes:r.logical_histogram_reserved_bytes,
        peak_logical_state_bytes:r.peak_logical_state_bytes,
        factor_width_limit:MAX_FACTOR_QUBITS,
        max_cpu_work_units:MAX_FACTOR_COUNT_WORK_UNITS,
        projective_wire_splits:r.projective_wire_splits,
        projective_split_deferred:r.projective_split_deferred,
        product_preserving_two_wire_gates:r.product_preserving_two_wire_gates,
        evidence_class:"simulation",qpu_hardware_jobs:0,
    }
}
/// Real streamed shot execution; no shots/trace array is serialized or retained.
/// The sample stream uses exactly the same seeded RNG as detailed shots.
pub fn execute_factorized_counts_json(raw:&[u8])
    ->Result<String,FactorProtocolError>
{
    if raw.len()>MAX_REQUEST_BYTES {return Err(FactorProtocolError::InputTooLarge);}
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|FactorProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=FACTOR_COUNTS_REQUEST_SCHEMA {
        return Err(FactorProtocolError::UnsupportedSchema);
    }
    let req:CountsRequest=serde_json::from_slice(raw)
        .map_err(|e|FactorProtocolError::InvalidJson(e.to_string()))?;
    if req.schema!=FACTOR_COUNTS_REQUEST_SCHEMA {
        return Err(FactorProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_FACTOR_GLOBAL_QUBITS).contains(&req.qubits) {
        return Err(FactorProtocolError::ProfileLimit("qubits"));
    }
    if req.classical_bits>64 {
        return Err(FactorProtocolError::ProfileLimit("classical_bits"));
    }
    if !(1..=MAX_FACTOR_COUNT_SHOTS).contains(&req.shots) {
        return Err(FactorProtocolError::ProfileLimit("shots"));
    }
    if req.instructions.len()>MAX_FACTOR_INSTRUCTIONS {
        return Err(FactorProtocolError::ProfileLimit("instructions"));
    }
    if req.max_memory_bytes==0||req.max_memory_bytes>DEFAULT_MEMORY_BUDGET {
        return Err(FactorProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=FactorCircuit::new(req.qubits,req.classical_bits);
    for op in &req.instructions {
        circuit.push(convert(op));
    }
    let answer=circuit.execute_counts(req.shots,req.seed,req.max_memory_bytes)
        .map_err(FactorProtocolError::from)?;
    let response=wrap(&answer,req.qubits,req.classical_bits);
    let result=serde_json::to_string(&response)
        .map_err(|e|FactorProtocolError::Serialization(e.to_string()))?;
    if result.len()>MAX_RESULT_BYTES {
        return Err(FactorProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    // Output is atomic: all shots and the strict histogram limit are
    // checked before any success JSON reaches stdout.
    Ok(result)
}
