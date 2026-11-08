//! Versioned offline *analytic* Pauli observable service for factor-product
//! pure CPU states. Results after midcircuit instrument operations are
//! conditional on the one actually sampled trajectory, NOT branch averaged.
use crate::circuit::{Instruction,TraceEntry,TraceKind};
use crate::factorized::{
    FactorCircuit,FactorError,PauliTerm,ObservableBatch,
    MAX_FACTOR_GLOBAL_QUBITS,MAX_FACTOR_INSTRUCTIONS,MAX_FACTOR_QUBITS,
    MAX_PAULI_TERMS,OBSERVABLE_RESULT_SCHEMA,FACTORIZED_PROFILE,
};
use crate::factorized_json::{
    Operation,FactorProtocolError,MAX_REQUEST_BYTES,MAX_RESULT_BYTES,
};
use crate::{DEFAULT_MEMORY_BUDGET,ENGINE_VERSION,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};

pub const OBSERVABLE_REQUEST_SCHEMA:&str="aura.quantum.observable-request/1";
pub const OBSERVABLE_ERROR_SCHEMA:&str="aura.quantum.observable-error/1";

#[derive(Debug,Deserialize)]
struct Probe{schema:String}
#[derive(Debug,Deserialize)]
#[serde(deny_unknown_fields)]
struct TermIn{
    paulis:String,
    coefficient:f64,
}
#[derive(Debug,Deserialize)]
#[serde(deny_unknown_fields)]
struct Request{
    schema:String,
    qubits:usize,
    classical_bits:usize,
    seed:u64,
    max_memory_bytes:usize,
    instructions:Vec<Operation>,
    terms:Vec<TermIn>,
}
#[derive(Debug,Serialize)]
#[serde(tag="kind",rename_all="snake_case")]
enum EventOut {
    Measure{instruction:usize,qubit:usize,into_bit:usize,value:bool},
    Reset{instruction:usize,qubit:usize,previous_value:bool},
}
fn event(x:&TraceEntry)->EventOut {
    match x.kind {
        TraceKind::Measure{into_bit,value}=>EventOut::Measure{
            instruction:x.instruction,qubit:x.qubit,into_bit,value,
        },
        TraceKind::Reset{previous_value}=>EventOut::Reset{
            instruction:x.instruction,qubit:x.qubit,previous_value,
        },
    }
}
#[derive(Serialize)]
struct TermOut {
    paulis:String,
    coefficient:f64,
    expectation:f64,
    contribution:f64,
}
#[derive(Serialize)]
struct Response {
    schema:&'static str,
    status:&'static str,
    engine_version:&'static str,
    profile:&'static str,
    precision:&'static str,
    truncation:&'static str,
    truncation_error_bound:f64,
    norm_tolerance:f64,
    qubits:usize,
    classical_bits:usize,
    factor_width_limit:usize,
    seed:u64,
    shots:usize,
    expectation_mode:&'static str,
    pauli_word_order:&'static str,
    conditioned_on_observed_instruments:bool,
    classical_register:Vec<Option<bool>>,
    trace:Vec<EventOut>,
    terms:Vec<TermOut>,
    weighted_sum:f64,
    term_count:usize,
    observable_logical_reserved_bytes:usize,
    peak_logical_state_bytes:usize,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
}
fn convert(op:&Operation)->Instruction {
    match op {
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
fn response(batch:ObservableBatch,qubits:usize,bits:usize)->Response {
    let term_count=batch.estimates.len();
    Response{
        schema:OBSERVABLE_RESULT_SCHEMA,
        status:"simulated",
        engine_version:ENGINE_VERSION,
        profile:FACTORIZED_PROFILE,
        precision:"IEEE-754-complex-f64",
        truncation:"none;factor-width-hard-limit",
        truncation_error_bound:0.0, // tensor truncation only, not f64 error
        norm_tolerance:NORM_TOLERANCE,
        qubits,classical_bits:bits,
        factor_width_limit:MAX_FACTOR_QUBITS,
        seed:batch.metadata.seed,shots:batch.metadata.shots,
        expectation_mode:"analytic_pure_state_final",
        pauli_word_order:"q0-first;position-0-is-least-significant-wire",
        conditioned_on_observed_instruments:true,
        classical_register:batch.shot.bits,
        trace:batch.shot.trace.iter().map(event).collect(),
        terms:batch.estimates.into_iter().map(|x|TermOut{
            paulis:x.paulis,coefficient:x.coefficient,
            expectation:x.expectation,contribution:x.contribution,
        }).collect(),
        weighted_sum:batch.weighted_sum,
        term_count,
        observable_logical_reserved_bytes:batch.observable_logical_reserved_bytes,
        peak_logical_state_bytes:batch.peak_logical_state_bytes,
        evidence_class:"simulation",
        qpu_hardware_jobs:0,
    }
}
/// Strict one-seeded-final-state CPU service. Reuses the real bounded
/// dynamic factorized circuit and enforces every unreachable branch guard.
pub fn execute_observable_json(raw:&[u8])
    ->Result<String,FactorProtocolError>
{
    if raw.len()>MAX_REQUEST_BYTES {return Err(FactorProtocolError::InputTooLarge);}
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|FactorProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=OBSERVABLE_REQUEST_SCHEMA {
        return Err(FactorProtocolError::UnsupportedSchema);
    }
    let req:Request=serde_json::from_slice(raw)
        .map_err(|e|FactorProtocolError::InvalidJson(e.to_string()))?;
    if req.schema!=OBSERVABLE_REQUEST_SCHEMA {
        return Err(FactorProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_FACTOR_GLOBAL_QUBITS).contains(&req.qubits){
        return Err(FactorProtocolError::ProfileLimit("qubits"));
    }
    if req.classical_bits>64 {
        return Err(FactorProtocolError::ProfileLimit("classical_bits"));
    }
    if req.instructions.len()>MAX_FACTOR_INSTRUCTIONS {
        return Err(FactorProtocolError::ProfileLimit("instructions"));
    }
    if req.terms.is_empty()||req.terms.len()>MAX_PAULI_TERMS {
        return Err(FactorProtocolError::ProfileLimit("terms"));
    }
    if req.max_memory_bytes==0||req.max_memory_bytes>DEFAULT_MEMORY_BUDGET {
        return Err(FactorProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=FactorCircuit::new(req.qubits,req.classical_bits);
    for op in &req.instructions {
        circuit.push(convert(op));
    }
    let terms=req.terms.into_iter().map(|t|PauliTerm{
        paulis:t.paulis,coefficient:t.coefficient
    }).collect::<Vec<_>>();
    let batch=circuit.execute_observables(&terms,req.seed,req.max_memory_bytes)
        .map_err(FactorProtocolError::from)?;
    let output=response(batch,req.qubits,req.classical_bits);
    let serialized=serde_json::to_string(&output)
        .map_err(|e|FactorProtocolError::Serialization(e.to_string()))?;
    if serialized.len()>MAX_RESULT_BYTES {
        return Err(FactorProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    // Never publish any partial term or partial result.
    Ok(serialized)
}
