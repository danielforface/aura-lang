//! C-025 REAL binary invocation for strict NPT/mixture protocol.
use aura_quantum_sim::noisy_entanglement_json::{
    execute_noisy_pair_entanglement_json,ENTANGLEMENT_REQUEST_SCHEMA,
};
use serde_json::{Value,json};
use std::io::Write;
use std::process::{Command,Output,Stdio};
fn fixture()->Value{
    serde_json::from_slice(include_bytes!(
        "../fixtures/noisy_pair_entanglement_v1.json")).unwrap()
}
fn invoke(input:&Value,args:&[&str])->Output{
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-noisy-pair-entanglement-run"))
        .args(args).stdin(Stdio::piped()).stdout(Stdio::piped())
        .stderr(Stdio::piped()).spawn().unwrap();
    p.stdin.take().unwrap().write_all(
        &serde_json::to_vec(input).unwrap()).unwrap();
    p.wait_with_output().unwrap()
}
fn close(actual:&Value,expected:f64){
    let a=actual.as_f64().unwrap();
    assert!((a-expected).abs()<1e-9,"{a} vs {expected}");
}
fn no_other_events(v:&Value){
    for key in ["classical_probabilities","histogram","results",
                "instrument_trace","shot_trace","unselected_probability",
                "branch_weights","seed","full_density_row_major_complex_pairs"]{
        assert!(v.get(key).is_none(),"unexpected event/history field {key}");
    }
}
#[test]
fn real_selected_bell_is_entangled_conditional_with_no_leaked_other_ancilla_event(){
    let out=invoke(&fixture(),&[]);
    assert_eq!(out.status.code(),Some(0),"{}",String::from_utf8_lossy(&out.stderr));
    assert!(out.stderr.is_empty());
    let v:Value=serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["schema"],"aura.quantum.noisy-pair-entanglement-result/1");
    assert_eq!(v["shots"],0);assert_eq!(v["qpu_hardware_jobs"],0);
    assert_eq!(v["selected_pair"],json!([0,1]));
    assert_eq!(v["selected_conditions"],json!([{"bit":0,"equals":false}]));
    assert_eq!(v["selected_branch_count"],1);
    close(&v["condition_probability"],0.5);
    close(&v["negativity"],0.5);
    close(&v["logarithmic_negativity_bits"],1.0);
    close(&v["mutual_information_bits"],2.0);
    assert_eq!(v["npt_entanglement_witness_detected"],true);
    assert_eq!(v["reduced_density_row_major_complex_pairs"].as_array().unwrap().len(),16);
    no_other_events(&v);
}
#[test]
fn real_untreated_ancilla_mixes_entangled_and_separable_states(){
    let mut req=fixture();
    req.as_object_mut().unwrap().remove("conditions");
    let out=invoke(&req,&[]);
    assert_eq!(out.status.code(),Some(0));
    let v:Value=serde_json::from_slice(&out.stdout).unwrap();
    assert!(v.get("selected_conditions").is_none());
    assert!(v.get("condition_probability").is_none());
    assert!(v["negativity"].as_f64().unwrap()>0.0);
    assert!(v["negativity"].as_f64().unwrap()<0.5);
    no_other_events(&v);
}
#[test]
fn deterministic_replay_and_old_v1_schema_rejection(){
    let a=invoke(&fixture(),&[]);let b=invoke(&fixture(),&[]);
    assert_eq!(a.status.code(),Some(0));assert_eq!(a.stdout,b.stdout);
    let mut req=fixture();
    req["schema"]=json!("aura.quantum.noisy-conditioned-request/1");
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"unsupported_schema");
    req["schema"]=json!(ENTANGLEMENT_REQUEST_SCHEMA);
    req["seed"]=json!(42);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_json");
}
#[test]
fn flags_shots_hardware_and_other_provider_fields_are_rejected(){
    for (k,value) in [
        ("shots",json!(64)),("hardware",json!(true)),
        ("qpu_backend",json!("provider")),("seed",json!(7))
    ]{
        let mut req=fixture();req[k]=value;
        let p=invoke(&req,&[]);
        assert_eq!(p.status.code(),Some(2));
        assert!(p.stdout.is_empty());
        let e:Value=serde_json::from_slice(&p.stderr).unwrap();
        assert_eq!(e["schema"],"aura.quantum.noisy-pair-entanglement-error/1");
        assert_eq!(e["qpu_hardware_jobs"],0);
        assert_eq!(e["code"],"invalid_json");
    }
    let p=invoke(&fixture(),&["--qpu"]);
    assert_eq!(p.status.code(),Some(2));
    assert!(p.stdout.is_empty());
}
#[test]
fn invalid_pair_condition_rare_and_unphysical_noise_rejected(){
    let mut req=fixture();
    req["selected_pair"]=json!([1,0]);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    req["selected_pair"]=json!([0,1]);
    req["conditions"]=json!([]);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"profile_limit");
    req["conditions"]=json!([{"bit":0,"equals":false},{"bit":0,"equals":true}]);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    req["conditions"]=json!([{"bit":1,"equals":true}]);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    req=fixture();
    req["instructions"][4]["action"]["channel"]["gamma"]=json!(1.5);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
}
#[test]
fn width_budget_observed_reset_and_malformed_nested_data_fail(){
    let mut req=fixture();
    req["qubits"]=json!(5);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"profile_limit");
    req["qubits"]=json!(3);req["max_memory_bytes"]=json!(1);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    req["max_memory_bytes"]=json!(16777216);
    req["instructions"]=json!([{"op":"reset_observed","qubit":0}]);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    req["instructions"]=fixture()["instructions"].clone();
    req["selected_pair"]=json!([0,1,2]);
    assert_eq!(execute_noisy_pair_entanglement_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_json");
}
