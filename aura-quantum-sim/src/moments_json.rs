//! Separate strict offline Hamiltonian moments/1 protocol, CPU-only.
//! Original observable-request/1 and its response remain byte-compatible.
use crate::circuit::{Instruction,TraceEntry,TraceKind};
use crate::factorized::{
    FactorCircuit,PauliTerm,MAX_FACTOR_GLOBAL_QUBITS,MAX_FACTOR_INSTRUCTIONS,
    MAX_FACTOR_QUBITS,PAULI_TERM_LOGICAL_OVERHEAD,FACTORIZED_PROFILE,
};
use crate::factorized_json::{
    Operation,FactorProtocolError,MAX_REQUEST_BYTES,MAX_RESULT_BYTES,
};
use crate::moments::{evaluate_hamiltonian_moments,MOMENTS_RESULT_SCHEMA};
use crate::{DEFAULT_MEMORY_BUDGET,ENGINE_VERSION,NORM_TOLERANCE};
use serde::{Deserialize,Serialize};

pub const MOMENTS_REQUEST_SCHEMA:&str="aura.quantum.moments-request/1";
pub const MOMENTS_ERROR_SCHEMA:&str="aura.quantum.moments-error/1";

#[derive(Deserialize)]
struct Probe {schema:String}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TermIn {paulis:String,coefficient:f64}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema:String,
    qubits:usize,
    classical_bits:usize,
    seed:u64,
    max_memory_bytes:usize,
    instructions:Vec<Operation>,
    terms:Vec<TermIn>,
}
#[derive(Serialize)]
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
    variance_definition:&'static str,
    pauli_word_order:&'static str,
    conditioned_on_observed_instruments:bool,
    classical_register:Vec<Option<bool>>,
    trace:Vec<EventOut>,
    terms:Vec<TermOut>,
    term_count:usize,
    weighted_sum:f64,
    weighted_second_moment:f64,
    weighted_variance:f64,
    commuting_pairs:usize,
    anticommuting_pairs:usize,
    estimated_moment_work_units:usize,
    roundoff_clamped:bool,
    observable_logical_reserved_bytes:usize,
    peak_logical_state_bytes:usize,
    evidence_class:&'static str,
    qpu_hardware_jobs:u8,
}
/// Version 1 produces only a full completed, finite CPU simulation result;
/// unsupported hardware/provider keys, unknown operations and unbounded
/// requests are rejected. No partial stdout is produced by the CLI.
pub fn execute_moments_json(raw:&[u8])
    ->Result<String,FactorProtocolError>
{
    if raw.len()>MAX_REQUEST_BYTES {return Err(FactorProtocolError::InputTooLarge);}
    let probe:Probe=serde_json::from_slice(raw)
        .map_err(|e|FactorProtocolError::InvalidJson(e.to_string()))?;
    if probe.schema!=MOMENTS_REQUEST_SCHEMA {
        return Err(FactorProtocolError::UnsupportedSchema);
    }
    let req:Request=serde_json::from_slice(raw)
        .map_err(|e|FactorProtocolError::InvalidJson(e.to_string()))?;
    if req.schema!=MOMENTS_REQUEST_SCHEMA {
        return Err(FactorProtocolError::UnsupportedSchema);
    }
    if !(1..=MAX_FACTOR_GLOBAL_QUBITS).contains(&req.qubits) {
        return Err(FactorProtocolError::ProfileLimit("qubits"));
    }
    if req.classical_bits>64 {
        return Err(FactorProtocolError::ProfileLimit("classical_bits"));
    }
    if req.instructions.len()>MAX_FACTOR_INSTRUCTIONS {
        return Err(FactorProtocolError::ProfileLimit("instructions"));
    }
    if req.terms.is_empty()||req.terms.len()>64 {
        return Err(FactorProtocolError::ProfileLimit("terms"));
    }
    if req.max_memory_bytes==0||req.max_memory_bytes>DEFAULT_MEMORY_BUDGET {
        return Err(FactorProtocolError::ProfileLimit("max_memory_bytes"));
    }
    let mut circuit=FactorCircuit::new(req.qubits,req.classical_bits);
    for op in &req.instructions {circuit.push(convert(op));}
    let terms=req.terms.into_iter().map(|x|PauliTerm{
        paulis:x.paulis,coefficient:x.coefficient,
    }).collect::<Vec<_>>();
    // Reserve bounded output strings and moments scratch at admission;
    // logical accounting only, never an allocator RSS guarantee.
    let per_term=req.qubits.checked_add(PAULI_TERM_LOGICAL_OVERHEAD)
        .ok_or(FactorProtocolError::ProfileLimit("arithmetic_overflow"))?;
    let reserved=terms.len().checked_mul(per_term)
        .and_then(|x|x.checked_add(512))
        .ok_or(FactorProtocolError::ProfileLimit("arithmetic_overflow"))?;
    let state_budget=req.max_memory_bytes.checked_sub(reserved)
        .ok_or(FactorProtocolError::ProfileLimit("max_memory_bytes"))?;
    let mut batch=circuit.execute_shots(1,req.seed,state_budget)
        .map_err(FactorProtocolError::from)?;
    let state=batch.one_shot_state.as_ref()
        .ok_or(FactorProtocolError::ProfileLimit("one_shot_state"))?;
    let m=evaluate_hamiltonian_moments(state,&terms)
        .map_err(FactorProtocolError::from)?;
    let mut estimates=Vec::new();
    estimates.try_reserve_exact(terms.len())
        .map_err(|_|FactorProtocolError::ProfileLimit("allocation"))?;
    for t in &terms {
        let expectation=state.expectation_pauli(&t.paulis)
            .map_err(FactorProtocolError::from)?;
        estimates.push(TermOut{
            paulis:t.paulis.clone(),coefficient:t.coefficient,
            expectation,contribution:t.coefficient*expectation,
        });
    }
    let shot=batch.records.remove(0);
    let response=Response{
        schema:MOMENTS_RESULT_SCHEMA,
        status:"simulated",
        engine_version:ENGINE_VERSION,
        profile:FACTORIZED_PROFILE,
        precision:"IEEE-754-complex-f64",
        truncation:"none;factor-width-hard-limit",
        truncation_error_bound:0.0,
        norm_tolerance:NORM_TOLERANCE,
        qubits:req.qubits,classical_bits:req.classical_bits,
        factor_width_limit:MAX_FACTOR_QUBITS,
        seed:req.seed,shots:1,
        expectation_mode:"analytic_pure_state_final",
        variance_definition:"H_squared_expectation_minus_mean_squared",
        pauli_word_order:"q0-first;position-0-is-least-significant-wire",
        conditioned_on_observed_instruments:true,
        classical_register:shot.bits,
        trace:shot.trace.iter().map(event).collect(),
        term_count:estimates.len(),terms:estimates,
        weighted_sum:m.mean,
        weighted_second_moment:m.second_moment,
        weighted_variance:m.variance,
        commuting_pairs:m.commuting_pairs,
        anticommuting_pairs:m.anticommuting_pairs,
        estimated_moment_work_units:m.estimated_work_units,
        roundoff_clamped:m.roundoff_clamped,
        observable_logical_reserved_bytes:reserved,
        peak_logical_state_bytes:batch.peak_logical_state_bytes,
        evidence_class:"simulation",
        qpu_hardware_jobs:0,
    };
    let serialized=serde_json::to_string(&response)
        .map_err(|e|FactorProtocolError::Serialization(e.to_string()))?;
    if serialized.len()>MAX_RESULT_BYTES {
        return Err(FactorProtocolError::ProfileLimit("serialized_output_bytes"));
    }
    Ok(serialized)
}
