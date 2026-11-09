//! C-023 offline executable protocol + real full-density output acceptance.
use aura_quantum_sim::noisy_ensemble_json::{
    execute_noisy_ensemble_json,NOISY_ENSEMBLE_REQUEST_SCHEMA,
};
use serde_json::{Value,json};
use std::io::Write;
use std::process::{Command,Output,Stdio};
fn sample()->Value{
    serde_json::from_slice(include_bytes!("../fixtures/noisy_ensemble_feedback_v1.json"))
        .unwrap()
}
fn run(raw:&[u8],args:&[&str])->Output{
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-noisy-ensemble-run"))
        .args(args).stdin(Stdio::piped())
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    p.stdin.take().unwrap().write_all(raw).unwrap();
    p.wait_with_output().unwrap()
}
fn close(x:&Value,y:f64){
    let z=x.as_f64().unwrap();
    assert!((z-y).abs()<1e-9,"got {z} expected {y}");
}
#[test]
fn actual_noisy_feedback_and_hidden_reset_cli_two_classical_bits(){
    let raw=include_bytes!("../fixtures/noisy_ensemble_feedback_v1.json");
    let out=run(raw,&[]);
    assert_eq!(out.status.code(),Some(0),"{}",String::from_utf8_lossy(&out.stderr));
    assert!(out.stderr.is_empty());
    let v:Value=serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["schema"],"aura.quantum.noisy-ensemble-result/1");
    assert_eq!(v["sampling"],"none;all_representable_explicit_z_outcomes");
    assert_eq!(v["shots"],0);assert_eq!(v["qpu_hardware_jobs"],0);
    assert_eq!(v["measurement_count"],2);assert_eq!(v["final_branch_count"],4);
    assert_eq!(v["final_density_dimension"],4);
    assert_eq!(v["final_density_row_major_complex_pairs"]
        .as_array().unwrap().len(),16);
    close(&v["probability_mass"],1.0);
    close(&v["trace"],1.0);
    close(&v["classical_probabilities"]["00"],0.25);
    close(&v["classical_probabilities"]["01"],0.25);
    close(&v["classical_probabilities"]["10"],0.25);
    close(&v["classical_probabilities"]["11"],0.25);
}
#[test]
fn no_seed_repeatable_bytewise_and_foreign_schema_rejected(){
    let raw=serde_json::to_vec(&sample()).unwrap();
    let a=run(&raw,&[]);let b=run(&raw,&[]);
    assert_eq!(a.status.code(),Some(0));
    assert_eq!(a.stdout,b.stdout);
    let mut req=sample();
    req["schema"]=json!("aura.quantum.noisy-request/1");
    assert_eq!(execute_noisy_ensemble_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"unsupported_schema");
    req["schema"]=json!(NOISY_ENSEMBLE_REQUEST_SCHEMA);
    req["seed"]=json!(3);
    assert_eq!(execute_noisy_ensemble_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"invalid_json");
}
#[test]
fn hardware_shots_qpu_flags_fail_closed_no_partial_stdout(){
    for (key,value) in [
        ("shots",json!(1000)),("hardware",json!(true)),
        ("qpu_backend",json!("provider")),("seed",json!(7)),
    ]{
        let mut req=sample();req[key]=value;
        let out=run(&serde_json::to_vec(&req).unwrap(),&[]);
        assert_eq!(out.status.code(),Some(2));
        assert!(out.stdout.is_empty());
        let e:Value=serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(e["schema"],"aura.quantum.noisy-ensemble-error/1");
        assert_eq!(e["code"],"invalid_json");
        assert_eq!(e["qpu_hardware_jobs"],0);
    }
    let out=run(&serde_json::to_vec(&sample()).unwrap(),&["--qpu"]);
    assert_eq!(out.status.code(),Some(2));
    assert!(out.stdout.is_empty());
}
#[test]
fn reset_observed_and_unwritable_classical_control_fail_closed(){
    let mut req=sample();
    req["instructions"]=json!([{"op":"reset_observed","qubit":0}]);
    assert_eq!(execute_noisy_ensemble_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    req["instructions"]=json!([{"op":"if","bit":1,"equals":true,
        "action":{"kind":"gate","gate":{"kind":"x","target":0}}}]);
    assert_eq!(execute_noisy_ensemble_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
}
#[test]
fn impossible_resource_and_unphysical_noise_rejected(){
    let mut req=sample();req["qubits"]=json!(5);
    assert_eq!(execute_noisy_ensemble_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"profile_limit");
    req["qubits"]=json!(2);req["max_memory_bytes"]=json!(1);
    assert_eq!(execute_noisy_ensemble_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    req["max_memory_bytes"]=json!(16777216);
    req["instructions"][2]["channel"]["gamma"]=json!(1.1);
    assert_eq!(execute_noisy_ensemble_json(&serde_json::to_vec(&req).unwrap())
        .unwrap_err().code(),"circuit_rejected");
}
