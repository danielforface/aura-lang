//! Q28 C-011: new public routing boundary across four existing offline engines.
//! Verify real external process, nested result equality and fail-closed modes.
use aura_quantum_sim::dispatch_json::{
    execute_dispatch_json, REQUEST_SCHEMA, MAX_INPUT_BYTES,
};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn fixture(name:&str)->Value {
    let bytes=match name {
        "clifford"=>include_bytes!("../fixtures/dispatch_clifford_v1.json").as_slice(),
        "nonclifford"=>include_bytes!("../fixtures/dispatch_nonclifford_v1.json").as_slice(),
        "noise"=>include_bytes!("../fixtures/dispatch_noise_v1.json").as_slice(),
        "exact"=>include_bytes!("../fixtures/dispatch_exact_v1.json").as_slice(),
        _=>panic!("unknown static test fixture"),
    };
    serde_json::from_slice(bytes).unwrap()
}
fn run_auto(input:&[u8])->Output {
    let mut process=Command::new(env!("CARGO_BIN_EXE_aura-quantum-auto-run"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().unwrap();
    process.stdin.take().unwrap().write_all(input).unwrap();
    process.wait_with_output().unwrap()
}
fn dispatch(v:&Value)->Value {
    let encoded=serde_json::to_vec(v).unwrap();
    serde_json::from_str(&execute_dispatch_json(&encoded).unwrap()).unwrap()
}
fn direct(v:&Value, schema:&str)->Value {
    let mut request=v.clone();
    let ob=request.as_object_mut().unwrap();
    ob.remove("mode");
    ob.remove("inspect_generators");
    ob.remove("inspect_probabilities");
    ob.remove("inspect_final_density");
    ob.insert("schema".to_owned(),json!(schema));
    let input=serde_json::to_vec(&request).unwrap();
    let raw=match schema {
        "aura.quantum.circuit-request/1"=>
            aura_quantum_sim::json_protocol::execute_json_bytes(&input).unwrap(),
        "aura.quantum.stabilizer-request/1"=>
            aura_quantum_sim::stabilizer_json::execute_stabilizer_json(&input).unwrap(),
        "aura.quantum.noisy-request/1"=>
            aura_quantum_sim::noisy_json::execute_noisy_json_bytes(&input).unwrap(),
        "aura.quantum.symbolic-request/1"=>
            aura_quantum_sim::symbolic_json::execute_symbolic_json(&input).unwrap(),
        _=>panic!("invalid test schema"),
    };
    serde_json::from_str(&raw).unwrap()
}
#[test]
fn auto_clifford_executes_real_polynomial_tableau_and_preserves_exact_seed(){
    let req=fixture("clifford");
    let a=dispatch(&req);
    assert_eq!(a["schema"],"aura.quantum.dispatch-result/1");
    assert_eq!(a["dispatch_policy"],"capability-first-no-silent-fallback/1");
    assert_eq!(a["selected_backend"],"clifford_stabilizer");
    assert_eq!(a["engine_result"]["schema"],"aura.quantum.stabilizer-result/1");
    assert_eq!(a["engine_result"]["shots"],256);
    assert_eq!(a["qpu_hardware_jobs"],0);
    assert_eq!(a["engine_result"],direct(&req,"aura.quantum.stabilizer-request/1"));
    assert_eq!(a,dispatch(&req));
    for shot in a["engine_result"]["results"].as_array().unwrap() {
        assert_eq!(shot["bits"][1],false);
        assert_eq!(shot["trace"].as_array().unwrap().len(),3);
    }
}
#[test]
fn sampled_nonclifford_t_routes_to_numerical_pure_statevector_without_rewrite(){
    let req=fixture("nonclifford");
    let a=dispatch(&req);
    assert_eq!(a["selected_backend"],"pure_statevector");
    assert_eq!(a["engine_result"]["schema"],"aura.quantum.circuit-result/1");
    assert_eq!(a["engine_result"],direct(&req,"aura.quantum.circuit-request/1"));
    assert_eq!(a["engine_result"]["results"].as_array().unwrap().len(),256);
}
#[test]
fn auto_noise_and_conditional_density_actions_preserve_real_kraus_result(){
    let req=fixture("noise");
    let a=dispatch(&req);
    assert_eq!(a["selected_backend"],"noisy_density");
    assert_eq!(a["engine_result"]["schema"],"aura.quantum.noisy-result/1");
    assert_eq!(a["engine_result"],direct(&req,"aura.quantum.noisy-request/1"));
    assert_eq!(a["engine_result"]["shots"],256);
    for shot in a["engine_result"]["results"].as_array().unwrap() {
        assert_eq!(shot["bits"][1],false);
    }
}
#[test]
fn exact_clifford_t_postselected_route_never_claims_sampling_or_drops_radicals(){
    let req=fixture("exact");
    let a=dispatch(&req);
    assert_eq!(a["selected_backend"],"exact_symbolic");
    assert_eq!(a["engine_result"]["schema"],"aura.quantum.symbolic-result/1");
    assert_eq!(a["engine_result"]["sampling"],"none;forced-outcomes-only");
    assert_eq!(a["engine_result"],direct(&req,"aura.quantum.symbolic-request/1"));
    assert_eq!(a["engine_result"]["final_joint_mass"]["numerator"]["rational"],"2");
    assert_eq!(a["engine_result"]["final_joint_mass"]["numerator"]["radical"],"-1");
    assert_eq!(a["engine_result"]["final_joint_mass"]["denominator_power_of_two"],2);
    assert_eq!(a["engine_result"]["trace"].as_array().unwrap().len(),1);
    assert_eq!(a["qpu_hardware_jobs"],0);
}
#[test]
fn subprocess_accepts_all_four_routed_public_profiles_and_is_deterministic(){
    for name in ["clifford","nonclifford","noise","exact"] {
        let request=serde_json::to_vec(&fixture(name)).unwrap();
        let first=run_auto(&request);
        let again=run_auto(&request);
        assert!(first.status.success(),"{}: {}",name,String::from_utf8_lossy(&first.stderr));
        assert!(first.stderr.is_empty());
        assert_eq!(first.stdout,again.stdout);
        let a:Value=serde_json::from_slice(&first.stdout).unwrap();
        assert_eq!(a,dispatch(&fixture(name)));
        assert_eq!(a["engine_result"]["evidence_class"],"simulation");
        assert_eq!(a["engine_result"]["qpu_hardware_jobs"],0);
    }
}
#[test]
fn auto_clifford_outside_tableau_shot_count_selects_pure_or_rejects_without_loss(){
    let mut req=fixture("clifford");
    req["qubits"]=json!(1);
    req["classical_bits"]=json!(0);
    req["instructions"]=json!([{"op":"gate","gate":{"kind":"h","target":0}}]);
    req["shots"]=json!(1025);
    let a=dispatch(&req);
    assert_eq!(a["selected_backend"],"pure_statevector");
    assert_eq!(a["engine_result"],direct(&req,"aura.quantum.circuit-request/1"));
    req["qubits"]=json!(16);
    // 16 qubits cannot enter bounded dense statevector, even though
    // no approximation is required for this Clifford circuit.
    req["shots"]=json!(1);
    let b=dispatch(&req);
    assert_eq!(b["selected_backend"],"clifford_stabilizer");
    assert_eq!(b["engine_result"]["qubits"],16);
    req["shots"]=json!(1025);
    let err=execute_dispatch_json(&serde_json::to_vec(&req).unwrap()).unwrap_err();
    assert_eq!(err.code(),"backend_rejected");
    assert!(err.to_string().contains("pure_statevector"));
}
#[test]
fn explicit_statevector_and_stabilizer_allow_comparison_without_hidden_fallback(){
    let mut req=fixture("clifford");
    req["mode"]=json!("statevector");
    let pure=dispatch(&req);
    assert_eq!(pure["selected_backend"],"pure_statevector");
    assert_eq!(pure["engine_result"],direct(&req,"aura.quantum.circuit-request/1"));
    req["mode"]=json!("stabilizer");
    let stab=dispatch(&req);
    assert_eq!(stab["selected_backend"],"clifford_stabilizer");
    assert_eq!(stab["engine_result"],direct(&req,"aura.quantum.stabilizer-request/1"));
    req=fixture("nonclifford");
    req["mode"]=json!("stabilizer");
    let err=execute_dispatch_json(&serde_json::to_vec(&req).unwrap()).unwrap_err();
    assert_eq!(err.code(),"incompatible_intent");
    assert!(err.to_string().contains("non_clifford_rejected_by_stabilizer"));
}
#[test]
fn explicit_density_without_noise_supports_bounded_density_tomography(){
    let mut req=fixture("noise");
    req["mode"]=json!("density");
    req["shots"]=json!(1);
    req["inspect_final_density"]=json!(true);
    req["instructions"]=json!([
        {"op":"gate","gate":{"kind":"h","target":0}},
        {"op":"noise","channel":{"kind":"amplitude_damping","qubit":0,"gamma":0.3}}
    ]);
    let a=dispatch(&req);
    assert_eq!(a["selected_backend"],"noisy_density");
    assert!(a["engine_result"]["final_density"].is_object());
    let mut raw=req.clone();
    raw.as_object_mut().unwrap().remove("mode");
    raw.as_object_mut().unwrap().remove("inspect_generators");
    raw.as_object_mut().unwrap().remove("inspect_probabilities");
    raw["schema"]=json!("aura.quantum.noisy-request/1");
    let result=aura_quantum_sim::noisy_json::execute_noisy_json_bytes(
        &serde_json::to_vec(&raw).unwrap()
    ).unwrap();
    assert_eq!(a["engine_result"],serde_json::from_str::<Value>(&result).unwrap());
}
#[test]
fn explicit_density_mode_is_not_selected_on_demand_by_unsupported_sampled_reset(){
    let mut req=fixture("clifford");
    req["instructions"]=json!([{"op":"reset_observed","qubit":0}]);
    let a=dispatch(&req);
    assert_eq!(a["selected_backend"],"noisy_density");
    assert_eq!(a["engine_result"]["schema"],"aura.quantum.noisy-result/1");
    req["mode"]=json!("statevector");
    let err=execute_dispatch_json(&serde_json::to_vec(&req).unwrap()).unwrap_err();
    assert_eq!(err.code(),"incompatible_intent");
}
#[test]
fn illegal_unknown_and_ambiguous_operation_fields_cannot_trigger_hidden_fallback(){
    let mut req=fixture("clifford");
    req["instructions"][0]["gate"]["theta"]=json!(1.5);
    let err=execute_dispatch_json(&serde_json::to_vec(&req).unwrap()).unwrap_err();
    assert_eq!(err.code(),"backend_rejected");
    req=fixture("clifford");
    req["instructions"][0]["gate"]["kind"]=json!("rx");
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_instruction");
    req=fixture("clifford");
    req["instructions"]=json!([{"op":"if","bit":0,"equals":true,
      "gate":{"kind":"x","target":0},
      "action":{"kind":"gate","gate":{"kind":"x","target":0}}}]);
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_instruction");
    req=fixture("clifford");
    req["instructions"]=json!([{"op":"teleport_hardware","qubit":0}]);
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_instruction");
}
#[test]
fn sampled_and_forced_measurements_have_distinct_nonoverlapping_semantics(){
    let mut req=fixture("clifford");
    req["instructions"]=json!([{"op":"measure","qubit":0,"into_bit":0,"outcome":true}]);
    let err=execute_dispatch_json(&serde_json::to_vec(&req).unwrap()).unwrap_err();
    assert_eq!(err.code(),"incompatible_intent");
    req=fixture("exact");
    req["seed"]=json!(7);
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"incompatible_intent");
    req=fixture("exact");
    req["instructions"].as_array_mut().unwrap()
        .push(json!({"op":"reset","qubit":0}));
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"incompatible_intent");
}
#[test]
fn unknown_schema_fields_and_qpu_controls_are_rejected_fail_closed(){
    let mut req=fixture("clifford");
    req["submit_to_qpu"]=json!(true);
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_json");
    req.as_object_mut().unwrap().remove("submit_to_qpu");
    req["schema"]=json!("aura.quantum.dispatch-request/2");
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"unsupported_schema");
    req["schema"]=json!(REQUEST_SCHEMA);
    req["mode"]=json!("gpu");
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_json");
    assert_eq!(execute_dispatch_json(&vec![b' ';MAX_INPUT_BYTES+1]).unwrap_err().code(),
        "input_too_large");
}
#[test]
fn returned_subprocess_error_is_structured_and_stdout_is_empty(){
    let mut req=fixture("nonclifford");
    req["mode"]=json!("stabilizer");
    let output=run_auto(&serde_json::to_vec(&req).unwrap());
    assert_eq!(output.status.code(),Some(2));
    assert!(output.stdout.is_empty());
    let error:Value=serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["schema"],"aura.quantum.dispatch-error/1");
    assert_eq!(error["code"],"incompatible_intent");
    assert_eq!(error["qpu_hardware_jobs"],0);
}
#[test]
fn rejected_density_noise_parameter_retains_backend_error_instead_of_fallback(){
    let mut req=fixture("noise");
    req["instructions"][2]["channel"]["probability"]=json!(2.0);
    let err=execute_dispatch_json(&serde_json::to_vec(&req).unwrap()).unwrap_err();
    assert_eq!(err.code(),"backend_rejected");
    assert!(err.to_string().contains("noisy_density"));
}
#[test]
fn too_large_resources_and_no_supported_numerical_backend_fail_closed(){
    let mut req=fixture("clifford");
    req["qubits"]=json!(129);
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"profile_limit");
    req=fixture("clifford");
    req["max_memory_bytes"]=json!(1);
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"backend_rejected");
    req=fixture("nonclifford");
    req["qubits"]=json!(16);
    assert_eq!(execute_dispatch_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"backend_rejected");
}
