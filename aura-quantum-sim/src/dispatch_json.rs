//! Q28 C-011: capability-safe offline dispatch across four *existing* CPU engines.
//!
//! A new standalone request profile, NOT Aura source, Quantum IR, QPU access,
//! formal equivalence proof, or a change to existing legacy JSON contracts.
//! No numerical downgrade is hidden: auto sampled Clifford -> tableau,
//! sampled T -> f64 statevector, sampled noise -> density, explicit forced
//! exact branch -> symbolic algebraic Clifford+T. No fallback after failure.
//! All underlying engines validate each instruction and resource budget.
//! Unknown/conflicting features fail before a quantum engine is invoked.
use crate::json_protocol::{execute_json_bytes, ProtocolError};
use crate::noisy_json::{execute_noisy_json_bytes, NoisyProtocolError};
use crate::stabilizer_json::{execute_stabilizer_json, StabilizerProtocolError};
use crate::symbolic_json::{execute_symbolic_json, SymbolicProtocolError};
use crate::{DEFAULT_MEMORY_BUDGET, ENGINE_VERSION};
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub const REQUEST_SCHEMA: &str = "aura.quantum.dispatch-request/1";
pub const RESULT_SCHEMA: &str = "aura.quantum.dispatch-result/1";
pub const ERROR_SCHEMA: &str = "aura.quantum.dispatch-error/1";
pub const DISPATCH_POLICY: &str = "capability-first-no-silent-fallback/1";
pub const MAX_INPUT_BYTES: usize = 256 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_OPERATIONS: usize = 512;
pub const MAX_CLI_QUBITS: usize = 128;
pub const MAX_CLASSICAL_BITS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestedMode {
    Auto,
    Statevector,
    Stabilizer,
    Density,
    ExactBranch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Statevector,
    Stabilizer,
    Density,
    Symbolic,
}

impl Backend {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Statevector => "pure_statevector",
            Self::Stabilizer => "clifford_stabilizer",
            Self::Density => "noisy_density",
            Self::Symbolic => "exact_symbolic",
        }
    }
    const fn schema(self) -> &'static str {
        match self {
            Self::Statevector => "aura.quantum.circuit-request/1",
            Self::Stabilizer => "aura.quantum.stabilizer-request/1",
            Self::Density => "aura.quantum.noisy-request/1",
            Self::Symbolic => "aura.quantum.symbolic-request/1",
        }
    }
    const fn result_schema(self) -> &'static str {
        match self {
            Self::Statevector => "aura.quantum.circuit-result/1",
            Self::Stabilizer => "aura.quantum.stabilizer-result/1",
            Self::Density => "aura.quantum.noisy-result/1",
            Self::Symbolic => "aura.quantum.symbolic-result/1",
        }
    }
}

#[derive(Debug)]
pub enum DispatchError {
    InputTooLarge,
    InvalidJson(String),
    UnsupportedSchema,
    InvalidInstruction(&'static str),
    IncompatibleIntent(&'static str),
    ProfileLimit(&'static str),
    EngineRejected {
        backend: &'static str,
        code: &'static str,
        detail: String,
    },
    Serialization(String),
}
impl DispatchError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InputTooLarge => "input_too_large",
            Self::InvalidJson(_) => "invalid_json",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::InvalidInstruction(_) => "invalid_instruction",
            Self::IncompatibleIntent(_) => "incompatible_intent",
            Self::ProfileLimit(_) => "profile_limit",
            Self::EngineRejected { .. } => "backend_rejected",
            Self::Serialization(_) => "serialization_error",
        }
    }
}
impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for DispatchError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    mode: RequestedMode,
    qubits: usize,
    classical_bits: usize,
    max_memory_bytes: usize,
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    shots: Option<usize>,
    #[serde(default)]
    inspect_generators: bool,
    #[serde(default)]
    inspect_probabilities: bool,
    #[serde(default)]
    inspect_final_density: bool,
    instructions: Vec<Value>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchDecision {
    pub backend: Backend,
    pub reason: &'static str,
}

#[derive(Default)]
struct Features {
    contains_t: bool,
    needs_density: bool,
    forced_measurement: bool,
    has_reset: bool,
}

fn gate_kind(gate: Option<&Value>) -> Result<&str, DispatchError> {
    let kind = gate.and_then(|x| x.get("kind")).and_then(Value::as_str)
        .ok_or(DispatchError::InvalidInstruction("gate_kind_missing"))?;
    match kind {
        "h"|"x"|"y"|"z"|"s"|"t"|"cx"|"cz"|"swap" => Ok(kind),
        _ => Err(DispatchError::InvalidInstruction("unsupported_gate_kind")),
    }
}

fn analyze(request: &Request) -> Result<Features, DispatchError> {
    let mut features=Features::default();
    for op in &request.instructions {
        let name = op.get("op").and_then(Value::as_str)
            .ok_or(DispatchError::InvalidInstruction("op_missing"))?;
        match name {
            "gate" => {
                features.contains_t |= gate_kind(op.get("gate"))? == "t";
            }
            "if" => {
                match (op.get("gate"), op.get("action")) {
                    (Some(gate), None) => {
                        features.contains_t |= gate_kind(Some(gate))? == "t";
                    }
                    (None, Some(action)) => {
                        features.needs_density=true;
                        match action.get("kind").and_then(Value::as_str) {
                            Some("gate") => {
                                features.contains_t |= gate_kind(action.get("gate"))? == "t";
                            }
                            Some("noise") => {
                                if action.get("channel").is_none() {
                                    return Err(DispatchError::InvalidInstruction(
                                        "conditioned_noise_channel_missing"));
                                }
                            }
                            _ => return Err(DispatchError::InvalidInstruction(
                                "unsupported_conditioned_action")),
                        }
                    }
                    _ => return Err(DispatchError::InvalidInstruction(
                        "ambiguous_conditional_gate_or_action")),
                }
            }
            "measure" => {
                features.forced_measurement |= op.get("outcome").is_some();
            }
            "reset" => { features.has_reset=true; }
            "reset_observed" => {
                features.has_reset=true;
                features.needs_density=true;
            }
            "noise" => {
                if op.get("channel").is_none() {
                    return Err(DispatchError::InvalidInstruction("noise_channel_missing"));
                }
                features.needs_density=true;
            }
            _ => return Err(DispatchError::InvalidInstruction("unsupported_operation")),
        }
    }
    Ok(features)
}

/// A pure preflight selector. Every routed program is still fully validated
/// by the selected existing backend's OWN fail-closed admission checks.
/// Never mask a backend error by trying another profile after execution.
fn select(request: &Request) -> Result<DispatchDecision, DispatchError> {
    if !(1..=MAX_CLI_QUBITS).contains(&request.qubits) {
        return Err(DispatchError::ProfileLimit("qubits"));
    }
    if request.classical_bits>MAX_CLASSICAL_BITS {
        return Err(DispatchError::ProfileLimit("classical_bits"));
    }
    if request.instructions.len()>MAX_OPERATIONS {
        return Err(DispatchError::ProfileLimit("instructions"));
    }
    if request.max_memory_bytes==0 || request.max_memory_bytes>DEFAULT_MEMORY_BUDGET {
        return Err(DispatchError::ProfileLimit("max_memory_bytes"));
    }
    let feature=analyze(request)?;
    let require_sample=|| -> Result<usize,DispatchError> {
        let shots=request.shots.ok_or(
            DispatchError::IncompatibleIntent("sampled_requires_shots"))?;
        if shots==0 {
            return Err(DispatchError::IncompatibleIntent("shots_must_be_positive"));
        }
        if request.seed.is_none() {
            return Err(DispatchError::IncompatibleIntent("sampled_requires_seed"));
        }
        if feature.forced_measurement {
            return Err(DispatchError::IncompatibleIntent(
                "forced_outcome_requires_exact_branch"));
        }
        Ok(shots)
    };
    let no_stabilizer_inspect=|| -> Result<(),DispatchError> {
        if request.inspect_generators || request.inspect_probabilities {
            return Err(DispatchError::IncompatibleIntent(
                "stabilizer_inspection_requires_stabilizer"));
        }
        Ok(())
    };
    let no_density_inspect=|| -> Result<(),DispatchError> {
        if request.inspect_final_density {
            return Err(DispatchError::IncompatibleIntent(
                "density_inspection_requires_density"));
        }
        Ok(())
    };
    match request.mode {
        RequestedMode::ExactBranch => {
            if request.seed.is_some() || request.shots.is_some() {
                return Err(DispatchError::IncompatibleIntent(
                    "exact_branch_has_no_seed_or_shots"));
            }
            if feature.needs_density || feature.has_reset {
                return Err(DispatchError::IncompatibleIntent(
                    "exact_branch_no_noise_or_reset"));
            }
            no_stabilizer_inspect()?;
            no_density_inspect()?;
            Ok(DispatchDecision {backend:Backend::Symbolic,
                reason:"explicit_exact_algebraic_branch"})
        }
        RequestedMode::Density => {
            require_sample()?;
            if feature.needs_density && feature.forced_measurement {
                return Err(DispatchError::IncompatibleIntent("noisy_forced_outcome_unsupported"));
            }
            no_stabilizer_inspect()?;
            Ok(DispatchDecision {backend:Backend::Density,
                reason:"explicit_density_profile"})
        }
        RequestedMode::Statevector => {
            require_sample()?;
            no_density_inspect()?;
            no_stabilizer_inspect()?;
            if feature.needs_density {
                return Err(DispatchError::IncompatibleIntent("statevector_no_noise_or_observed_reset"));
            }
            Ok(DispatchDecision {backend:Backend::Statevector,
                reason:"explicit_pure_statevector"})
        }
        RequestedMode::Stabilizer => {
            require_sample()?;
            no_density_inspect()?;
            if feature.needs_density {
                return Err(DispatchError::IncompatibleIntent("stabilizer_no_noise_or_observed_reset"));
            }
            if feature.contains_t {
                return Err(DispatchError::IncompatibleIntent("non_clifford_rejected_by_stabilizer"));
            }
            Ok(DispatchDecision {backend:Backend::Stabilizer,
                reason:"explicit_clifford_stabilizer"})
        }
        RequestedMode::Auto => {
            let shots=require_sample()?;
            if feature.needs_density || request.inspect_final_density {
                no_stabilizer_inspect()?;
                return Ok(DispatchDecision {
                    backend:Backend::Density,
                    reason:"noise_or_density_tomography_requires_density",
                });
            }
            if feature.contains_t {
                no_stabilizer_inspect()?;
                return Ok(DispatchDecision {
                    backend:Backend::Statevector,
                    reason:"non_clifford_requires_pure_statevector_for_sampling",
                });
            }
            // When caller specifically requests tableau inspection, never
            // silently discard it by falling back to the pure solver.
            let can_stabilizer=request.qubits<=crate::stabilizer_json::MAX_CLI_QUBITS
                && request.classical_bits<=crate::stabilizer_json::MAX_CLI_CLASSICAL_BITS
                && request.instructions.len()<=crate::stabilizer_json::MAX_CLI_OPERATIONS
                && shots<=crate::stabilizer_json::MAX_CLI_SHOTS;
            if can_stabilizer {
                Ok(DispatchDecision {
                    backend:Backend::Stabilizer,
                    reason:"clifford_within_polynomial_tableau_profile",
                })
            } else {
                no_stabilizer_inspect()?;
                Ok(DispatchDecision {
                    backend:Backend::Statevector,
                    reason:"tableau_admission_exceeded_pure_profile_fallback",
                })
            }
        }
    }
}

fn downstream(backend:Backend, encoded:&[u8]) -> Result<String, DispatchError> {
    let rejection=|code,detail:String| DispatchError::EngineRejected {
        backend:backend.label(), code, detail,
    };
    match backend {
        Backend::Statevector=>execute_json_bytes(encoded)
            .map_err(|e:ProtocolError|rejection(e.code(),e.to_string())),
        Backend::Stabilizer=>execute_stabilizer_json(encoded)
            .map_err(|e:StabilizerProtocolError|rejection(e.code(),e.to_string())),
        Backend::Density=>execute_noisy_json_bytes(encoded)
            .map_err(|e:NoisyProtocolError|rejection(e.code(),e.to_string())),
        Backend::Symbolic=>execute_symbolic_json(encoded)
            .map_err(|e:SymbolicProtocolError|rejection(e.code(),e.to_string())),
    }
}

fn forward_request(req:&Request, selected:Backend) -> Result<Vec<u8>,DispatchError> {
    let mut map=Map::new();
    map.insert("schema".to_owned(),json!(selected.schema()));
    map.insert("qubits".to_owned(),json!(req.qubits));
    map.insert("classical_bits".to_owned(),json!(req.classical_bits));
    map.insert("max_memory_bytes".to_owned(),json!(req.max_memory_bytes));
    map.insert("instructions".to_owned(),json!(req.instructions));
    if selected!=Backend::Symbolic {
        let seed=req.seed.ok_or(DispatchError::IncompatibleIntent("missing_seed"))?;
        let shots=req.shots.ok_or(DispatchError::IncompatibleIntent("missing_shots"))?;
        map.insert("seed".to_owned(),json!(seed));
        map.insert("shots".to_owned(),json!(shots));
    }
    match selected {
        Backend::Stabilizer=>{
            map.insert("inspect_generators".to_owned(),json!(req.inspect_generators));
            map.insert("inspect_probabilities".to_owned(),json!(req.inspect_probabilities));
        }
        Backend::Density=>{
            map.insert("inspect_final_density".to_owned(),json!(req.inspect_final_density));
        }
        Backend::Statevector|Backend::Symbolic=>{}
    }
    serde_json::to_vec(&Value::Object(map))
        .map_err(|e|DispatchError::Serialization(e.to_string()))
}

/// Public callable Rust API and JSON entry point. Selection metadata is
/// transparent, and original engine output is nested byte-preserving as
/// structured JSON (not transformed to a new quantum result interpretation).
pub fn execute_dispatch_json(raw:&[u8])->Result<String,DispatchError> {
    if raw.len()>MAX_INPUT_BYTES { return Err(DispatchError::InputTooLarge); }
    let request:Request=serde_json::from_slice(raw)
        .map_err(|e|DispatchError::InvalidJson(e.to_string()))?;
    if request.schema!=REQUEST_SCHEMA { return Err(DispatchError::UnsupportedSchema); }
    let decision=select(&request)?;
    let encoded=forward_request(&request,decision.backend)?;
    let text=downstream(decision.backend,&encoded)?;
    let engine_result:Value=serde_json::from_str(&text)
        .map_err(|e|DispatchError::Serialization(e.to_string()))?;
    if engine_result.get("schema").and_then(Value::as_str)
        !=Some(decision.backend.result_schema())
        ||engine_result.get("evidence_class").and_then(Value::as_str)
            !=Some("simulation")
        ||engine_result.get("qpu_hardware_jobs").and_then(Value::as_u64)
            !=Some(0)
    {
        return Err(DispatchError::Serialization(
            "backend_result_provenance_mismatch".to_owned()));
    }
    let value=json!({
        "schema":RESULT_SCHEMA,
        "status":"simulated",
        "engine_version":ENGINE_VERSION,
        "dispatch_policy":DISPATCH_POLICY,
        "selected_backend":decision.backend.label(),
        "selection_reason":decision.reason,
        "evidence_class":"simulation",
        "qpu_hardware_jobs":0,
        "engine_result":engine_result,
    });
    let out=serde_json::to_string(&value)
        .map_err(|e|DispatchError::Serialization(e.to_string()))?;
    if out.len()>MAX_OUTPUT_BYTES {
        return Err(DispatchError::ProfileLimit("dispatched_output_bytes"));
    }
    Ok(out)
}
