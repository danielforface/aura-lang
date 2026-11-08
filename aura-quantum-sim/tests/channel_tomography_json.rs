//! C-020 real offline CLI positive and fail-closed process contracts.
use aura_quantum_sim::channel_tomography_json::{
    execute_channel_tomography_json,TOMOGRAPHY_REQUEST_SCHEMA,
};
use serde_json::{json,Value};
use std::process::{Command,Output,Stdio};
use std::io::Write;
fn input()->Value{
    serde_json::from_slice(include_bytes!(
        "../fixtures/channel_tomography_bell_feedback_v1.json")).unwrap()
}
fn run(raw:&[u8],flags:&[&str])->Output{
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-channel-tomography-run"))
        .args(flags).stdin(Stdio::piped())
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    p.stdin.take().unwrap().write_all(raw).unwrap();
    p.wait_with_output().unwrap()
}
fn close(x:&Value,expected:f64){
    let v=x.as_f64().unwrap();
    assert!((v-expected).abs()<1e-9,"got {v} expected {expected}");
}
#[test]
fn real_bell_feedback_and_reset_pair_tomography_cli(){
    let raw=include_bytes!("../fixtures/channel_tomography_bell_feedback_v1.json");
    let p=run(raw,&[]);
    assert_eq!(p.status.code(),Some(0),"{}",String::from_utf8_lossy(&p.stderr));
    assert!(p.stderr.is_empty());
    let v:Value=serde_json::from_slice(&p.stdout).unwrap();
    assert_eq!(v["schema"],"aura.quantum.channel-tomography-result/1");
    assert_eq!(v["shots"],0);
    assert_eq!(v["qpu_hardware_jobs"],0);
    assert_eq!(v["final_branch_count"],4);
    assert_eq!(v["instrument_count"],2);
    assert_eq!(v["single_wire_reductions"].as_array().unwrap().len(),3);
    assert_eq!(v["selected_pair_reductions"].as_array().unwrap().len(),2);
    close(&v["total_branch_probability"],1.0);
    close(&v["classical_probabilities"]["0"],0.5);
    close(&v["classical_probabilities"]["1"],0.5);
    close(&v["single_wire_reductions"][0]["bloch_xyz"][2],0.0);
    close(&v["single_wire_reductions"][0]["purity"],0.5);
    close(&v["single_wire_reductions"][0]["branch_mixing_gap"],0.5);
    close(&v["single_wire_reductions"][1]["bloch_xyz"][2],1.0);
    close(&v["single_wire_reductions"][1]["purity"],1.0);
    close(&v["single_wire_reductions"][2]["bloch_xyz"][2],1.0);
    close(&v["selected_pair_reductions"][0]["purity"],0.5);
    close(&v["selected_pair_reductions"][0]["branch_mixing_gap"],0.5);
    assert_eq!(v["selected_pair_reductions"][0]["wires"],json!([0,1]));
    assert_eq!(v["selected_pair_reductions"][0]
        ["rho_row_major_complex_pairs"].as_array().unwrap().len(),16);
    assert_eq!(v["single_wire_reductions"][0]
        ["rho_row_major_complex_pairs"].as_array().unwrap().len(),4);
}
#[test]
fn deterministic_no_seed_byte_exact_replay_and_isolated_old_contract(){
    let raw=serde_json::to_vec(&input()).unwrap();
    let a=run(&raw,&[]);let b=run(&raw,&[]);
    assert_eq!(a.status.code(),Some(0));
    assert_eq!(a.stdout,b.stdout);
    let mut req=input();
    req["schema"]=json!("aura.quantum.channel-moments-request/1");
    assert_eq!(execute_channel_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"unsupported_schema");
    req["schema"]=json!(TOMOGRAPHY_REQUEST_SCHEMA);
    req["seed"]=json!(123);
    assert_eq!(execute_channel_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_json");
}
#[test]
fn hardware_shots_and_cli_flags_are_rejected_without_partial_stdout(){
    for (key,value) in [
        ("shots",json!(1024)),("hardware",json!(true)),
        ("qpu_backend",json!("device")),("noise_model",json!("depolarizing"))
    ]{
        let mut req=input();req[key]=value;
        let p=run(&serde_json::to_vec(&req).unwrap(),&[]);
        assert_eq!(p.status.code(),Some(2));
        assert!(p.stdout.is_empty());
        let err:Value=serde_json::from_slice(&p.stderr).unwrap();
        assert_eq!(err["schema"],"aura.quantum.channel-tomography-error/1");
        assert_eq!(err["code"],"invalid_json");
        assert_eq!(err["qpu_hardware_jobs"],0);
    }
    let p=run(&serde_json::to_vec(&input()).unwrap(),&["--qpu"]);
    assert_eq!(p.status.code(),Some(2));
    assert!(p.stdout.is_empty());
}
#[test]
fn invalid_pairs_width_budget_and_order_fail_closed(){
    let mut req=input();
    for invalid in [json!([[0,1],[0,1]]),json!([[1,0]]),json!([[0,0]]),
        json!([[0,3]]),json!([[0,1],[0,2],[0,3]])]{
        req["pairs"]=invalid;
        assert_eq!(execute_channel_tomography_json(&serde_json::to_vec(&req).unwrap())
            .unwrap_err().code(),"circuit_rejected");
    }
    req["pairs"]=json!([[0,1]]);
    req["qubits"]=json!(9);
    assert_eq!(execute_channel_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"profile_limit");
    req["qubits"]=json!(3);req["max_memory_bytes"]=json!(1);
    assert_eq!(execute_channel_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
}
#[test]
fn missing_pair_list_and_unknown_nested_fields_rejected(){
    let mut req=input();
    req.as_object_mut().unwrap().remove("pairs");
    assert_eq!(execute_channel_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_json");
    let mut req=input();
    req["instructions"][0]["gate"]["provider"]=json!("device");
    assert_eq!(execute_channel_tomography_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_json");
}
