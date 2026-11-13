//! C-012 acceptance: 128-wire *non-Clifford* factorized runtime is reachable
//! from the unified automatic CPU dispatcher without a silent f64 downgrade.
use aura_quantum_sim::dispatch_json::{execute_dispatch_json,DispatchError};
use aura_quantum_sim::factorized_json::execute_factorized_json;
use serde_json::{json,Value};

fn to_auto(src:&[u8])->Value {
    let mut input:Value=serde_json::from_slice(src).unwrap();
    input["schema"]=json!("aura.quantum.dispatch-request/1");
    input["mode"]=json!("auto");
    input
}
fn run(x:&Value)->Value {
    serde_json::from_str(&execute_dispatch_json(
        &serde_json::to_vec(x).unwrap()
    ).unwrap()).unwrap()
}
#[test]
fn factor_128_nonclifford_bell_pairs_auto_and_original_result_match(){
    let raw=include_bytes!("../fixtures/factorized_128_bell_t_v1.json");
    let request=to_auto(raw);
    let output=run(&request);
    assert_eq!(output["selected_backend"],"factor_product");
    assert_eq!(output["engine_result"]["schema"],"aura.quantum.factorized-result/1");
    assert_eq!(output["engine_result"]["qubits"],128);
    assert_eq!(output["engine_result"]["factors"].as_array().unwrap().len(),64);
    assert_eq!(output["engine_result"]["truncation"],"none;factor-width-hard-limit");
    assert_eq!(output["engine_result"]["qpu_hardware_jobs"],0);
    let direct:Value=serde_json::from_str(&execute_factorized_json(raw).unwrap()).unwrap();
    assert_eq!(output["engine_result"],direct);
}
#[test]
fn explicit_factorized_four_wire_phase_composes_with_selected_inspection(){
    let raw=include_bytes!("../fixtures/factorized_small_phase_v1.json");
    let mut request=to_auto(raw);
    request["mode"]=json!("factorized");
    let output=run(&request);
    assert_eq!(output["selected_backend"],"factor_product");
    assert_eq!(output["engine_result"]["basis_ordered_complex_pairs"].as_array().unwrap().len(),16);
    let direct:Value=serde_json::from_str(&execute_factorized_json(raw).unwrap()).unwrap();
    assert_eq!(output["engine_result"],direct);
}
#[test]
fn nontrivial_invalid_wide_nonclifford_connected_component_does_not_fallback(){
    let raw=include_bytes!("../fixtures/factorized_128_bell_t_v1.json");
    let mut request=to_auto(raw);
    let mut operations=vec![
        json!({"op":"gate","gate":{"kind":"h","target":0}}),
        json!({"op":"gate","gate":{"kind":"t","target":0}}),
    ];
    for q in 0..10 {
        operations.push(json!({"op":"gate","gate":{
            "kind":"cx","control":q,"target":q+1}}));
    }
    request["instructions"]=json!(operations);
    let err=execute_dispatch_json(&serde_json::to_vec(&request).unwrap()).unwrap_err();
    assert_eq!(err.code(),"backend_rejected");
    assert!(err.to_string().contains("factor_product"));
    assert!(err.to_string().contains("FactorWidthExceeded"));
}
#[test]
fn explicit_factorized_rejects_noise_instead_of_dropping_channel_or_switching(){
    let raw=include_bytes!("../fixtures/dispatch_noise_v1.json");
    let mut request:Value=serde_json::from_slice(raw).unwrap();
    request["mode"]=json!("factorized");
    let err=execute_dispatch_json(&serde_json::to_vec(&request).unwrap()).unwrap_err();
    assert_eq!(err.code(),"incompatible_intent");
    assert!(matches!(err,DispatchError::IncompatibleIntent(_)));
}
