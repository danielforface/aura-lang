//! C-021 real offline strict postselection CLI acceptance.
use aura_quantum_sim::conditioned_tomography_json::{
    execute_conditioned_tomography_json,CONDITIONED_TOMOGRAPHY_REQUEST_SCHEMA,
};
use serde_json::{json,Value};
use std::process::{Command,Output,Stdio};
use std::io::Write;
fn request()->Value{
    serde_json::from_slice(include_bytes!(
        "../fixtures/conditioned_tomography_bell_feedback_v1.json")).unwrap()
}
fn run(raw:&[u8],args:&[&str])->Output{
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-conditioned-tomography-run"))
        .args(args).stdin(Stdio::piped())
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    p.stdin.take().unwrap().write_all(raw).unwrap();
    p.wait_with_output().unwrap()
}
fn near(v:&Value,e:f64){
    let x=v.as_f64().unwrap();
    assert!((x-e).abs()<1e-9,"{x} vs {e}");
}
fn assert_no_other_histories(v:&Value){
    for forbidden in [
        "classical_probabilities","classical_register",
        "histogram","trace","branches","results",
        "total_branch_probability","unselected_probability",
    ]{
        assert!(v.get(forbidden).is_none(),"unexpected field: {forbidden}");
    }
}
#[test]
fn real_3q_bell_feedback_reset_conditioned_cli(){
    let raw=include_bytes!("../fixtures/conditioned_tomography_bell_feedback_v1.json");
    let out=run(raw,&[]);
    assert_eq!(out.status.code(),Some(0),"{}",String::from_utf8_lossy(&out.stderr));
    assert!(out.stderr.is_empty());
    let j:Value=serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(j["schema"],"aura.quantum.conditioned-tomography-result/1");
    assert_eq!(j["status"],"simulated");
    assert_eq!(j["shots"],0);
    assert_eq!(j["qpu_hardware_jobs"],0);
    assert_eq!(j["selected_conditions"],json!([{"bit":0,"equals":true}]));
    assert_eq!(j["selected_branch_count"],2); // hidden Reset still 2
    assert_eq!(j["instrument_count"],2);
    near(&j["condition_probability"],0.5);
    near(&j["single_wire_reductions"][0]["bloch_xyz"][2],-1.0);
    near(&j["single_wire_reductions"][1]["bloch_xyz"][2],1.0);
    near(&j["single_wire_reductions"][2]["bloch_xyz"][2],1.0);
    near(&j["selected_pair_reductions"][0]["purity"],1.0);
    near(&j["single_wire_reductions"][0]["purity"],1.0);
    near(&j["single_wire_reductions"][0]["branch_mixing_gap"],0.0);
    assert_eq!(j["selected_pair_reductions"][0]["wires"],json!([0,1]));
    assert_eq!(j["selected_pair_reductions"][0]
        ["rho_row_major_complex_pairs"].as_array().unwrap().len(),16);
    assert_no_other_histories(&j);
}
#[test]
fn deterministic_replay_and_separate_v1_schema(){
    let raw=serde_json::to_vec(&request()).unwrap();
    let a=run(&raw,&[]);let b=run(&raw,&[]);
    assert_eq!(a.status.code(),Some(0));
    assert_eq!(a.stdout,b.stdout);
    let mut req=request();
    req["schema"]=json!("aura.quantum.channel-tomography-request/1");
    assert_eq!(
        execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
            .unwrap_err().code(),"unsupported_schema");
    req["schema"]=json!(CONDITIONED_TOMOGRAPHY_REQUEST_SCHEMA);
    req["seed"]=json!(42);
    assert_eq!(
        execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
            .unwrap_err().code(),"invalid_json");
}
#[test]
fn flags_hardware_shots_and_external_provider_rejected_without_stdout(){
    for (key,value) in [
        ("shots",json!(4096)),("hardware",json!(true)),
        ("qpu_backend",json!("hardware")),("seed",json!(7))
    ]{
        let mut req=request();req[key]=value;
        let out=run(&serde_json::to_vec(&req).unwrap(),&[]);
        assert_eq!(out.status.code(),Some(2));
        assert!(out.stdout.is_empty());
        let err:Value=serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(err["schema"],"aura.quantum.conditioned-tomography-error/1");
        assert_eq!(err["code"],"invalid_json");
        assert_eq!(err["qpu_hardware_jobs"],0);
    }
    let out=run(&serde_json::to_vec(&request()).unwrap(),&["--qpu"]);
    assert_eq!(out.status.code(),Some(2));
    assert!(out.stdout.is_empty());
}
#[test]
fn impossible_event_reset_unwritten_and_invalid_predicates_fail_closed(){
    let mut req=request();
    req["conditions"]=json!([{"bit":0,"equals":false},{"bit":0,"equals":true}]);
    assert_eq!(
        execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
            .unwrap_err().code(),"circuit_rejected");
    req["conditions"]=json!([]);
    assert_eq!(
        execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
            .unwrap_err().code(),"profile_limit");
    req["conditions"]=json!([{"bit":1,"equals":true}]);
    assert_eq!(
        execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
            .unwrap_err().code(),"circuit_rejected");
    req["conditions"]=json!([{"bit":0,"equals":true}]);
    req["instructions"]=json!([{"op":"measure","qubit":0,"into_bit":0}]);
    assert_eq!(
        execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
            .unwrap_err().code(),"circuit_rejected");
}
#[test]
fn invalid_width_budget_and_pairs_rejected_without_result(){
    let mut req=request();req["qubits"]=json!(9);
    assert_eq!(execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"profile_limit");
    req["qubits"]=json!(3);req["max_memory_bytes"]=json!(1);
    assert_eq!(execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    req["max_memory_bytes"]=json!(16777216);
    req["pairs"]=json!([[1,0]]);
    assert_eq!(execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    req["pairs"]=json!([[0,1]]);
    req["conditions"]=json!([{"bit":0,"equals":true,"hardware":true}]);
    assert_eq!(execute_conditioned_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_json");
}
#[test]
fn partial_filter_has_no_unselected_register_distribution(){
    let mut req=request();
    req["qubits"]=json!(2);req["classical_bits"]=json!(2);
    req["instructions"]=json!([
        {"op":"gate","gate":{"kind":"h","target":0}},
        {"op":"gate","gate":{"kind":"h","target":1}},
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"measure","qubit":1,"into_bit":1}
    ]);
    req["pairs"]=json!([[0,1]]);
    req["conditions"]=json!([{"bit":0,"equals":true}]);
    let out=run(&serde_json::to_vec(&req).unwrap(),&[]);
    assert_eq!(out.status.code(),Some(0));
    let j:Value=serde_json::from_slice(&out.stdout).unwrap();
    near(&j["condition_probability"],0.5);
    assert_eq!(j["selected_branch_count"],2);
    near(&j["single_wire_reductions"][1]["purity"],0.5);
    near(&j["single_wire_reductions"][1]["branch_mixing_gap"],0.5);
    assert_no_other_histories(&j);
}
