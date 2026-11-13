//! C-024 real strict JSON CPU postselection CLI subprocess checks.
use aura_quantum_sim::noisy_conditioned_json::{
    execute_noisy_conditioned_json,NOISY_CONDITIONED_REQUEST_SCHEMA,
};
use serde_json::{Value,json};
use std::process::{Command,Output,Stdio};
use std::io::Write;
fn fixture()->Value{
    serde_json::from_slice(include_bytes!(
        "../fixtures/noisy_conditioned_feedback_v1.json")).unwrap()
}
fn run(raw:&[u8],args:&[&str])->Output{
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-noisy-conditioned-run"))
        .args(args).stdin(Stdio::piped())
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    p.stdin.take().unwrap().write_all(raw).unwrap();
    p.wait_with_output().unwrap()
}
fn expect_no_other_histories(v:&Value){
    for key in ["classical_probabilities","histogram","results","trace_events",
                "instrument_trace","unselected_probability","branch_weights",
                "shots_samples","seed"]{
        assert!(v.get(key).is_none(),"exposed {key}");
    }
}
#[test]
fn real_cli_conditional_noisy_feedback_reset_joint_event(){
    let raw=include_bytes!("../fixtures/noisy_conditioned_feedback_v1.json");
    let p=run(raw,&[]);
    assert_eq!(p.status.code(),Some(0),"{}",String::from_utf8_lossy(&p.stderr));
    assert!(p.stderr.is_empty());
    let v:Value=serde_json::from_slice(&p.stdout).unwrap();
    assert_eq!(v["schema"],"aura.quantum.noisy-conditioned-result/1");
    assert_eq!(v["shots"],0);
    assert_eq!(v["qpu_hardware_jobs"],0);
    assert_eq!(v["selected_conditions"],
        json!([{"bit":0,"equals":true},{"bit":1,"equals":false}]));
    assert_eq!(v["selected_branch_count"],1);
    assert_eq!(v["measurement_count"],2);
    assert_eq!(v["final_density_dimension"],4);
    assert_eq!(v["final_density_row_major_complex_pairs"]
        .as_array().unwrap().len(),16);
    assert!((v["condition_probability"].as_f64().unwrap()-0.25).abs()<1e-9);
    assert!((v["trace"].as_f64().unwrap()-1.0).abs()<1e-9);
    assert!((v["purity"].as_f64().unwrap()-0.5).abs()<1e-9);
    expect_no_other_histories(&v);
}
#[test]
fn seeded_replay_not_needed_and_deterministic_output_no_rng(){
    let raw=serde_json::to_vec(&fixture()).unwrap();
    let a=run(&raw,&[]);let b=run(&raw,&[]);
    assert_eq!(a.status.code(),Some(0));
    assert_eq!(a.stdout,b.stdout);
    let mut input=fixture();
    input["schema"]=json!("aura.quantum.noisy-ensemble-request/1");
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"unsupported_schema");
    input["schema"]=json!(NOISY_CONDITIONED_REQUEST_SCHEMA);
    input["seed"]=json!(123);
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"invalid_json");
}
#[test]
fn hardware_shots_and_flags_are_fail_closed_no_stdout(){
    for (key,value) in [
        ("shots",json!(256)),("hardware",json!(true)),
        ("qpu_backend",json!("provider")),("seed",json!(23))
    ]{
        let mut req=fixture();req[key]=value;
        let p=run(&serde_json::to_vec(&req).unwrap(),&[]);
        assert_eq!(p.status.code(),Some(2));
        assert!(p.stdout.is_empty());
        let e:Value=serde_json::from_slice(&p.stderr).unwrap();
        assert_eq!(e["schema"],"aura.quantum.noisy-conditioned-error/1");
        assert_eq!(e["code"],"invalid_json");
        assert_eq!(e["qpu_hardware_jobs"],0);
    }
    let p=run(&serde_json::to_vec(&fixture()).unwrap(),&["--qpu"]);
    assert_eq!(p.status.code(),Some(2));
    assert!(p.stdout.is_empty());
}
#[test]
fn impossible_condition_duplicate_and_unwritten_bit_rejected(){
    let mut q=fixture();
    q["conditions"]=json!([]);
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&q).unwrap())
        .unwrap_err().code(),"profile_limit");
    q["conditions"]=json!([{"bit":0,"equals":true},{"bit":0,"equals":false}]);
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&q).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    q["conditions"]=json!([{"bit":0,"equals":true,"hardware":true}]);
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&q).unwrap())
        .unwrap_err().code(),"invalid_json");
    q["conditions"]=json!([{"bit":1,"equals":true}]);
    q["instructions"]=json!([{"op":"measure","qubit":0,"into_bit":0}]);
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&q).unwrap())
        .unwrap_err().code(),"circuit_rejected");
}
#[test]
fn invalid_kraus_budgets_width_and_reset_observed_rejected(){
    let mut q=fixture();q["qubits"]=json!(5);
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&q).unwrap())
        .unwrap_err().code(),"profile_limit");
    q["qubits"]=json!(2);q["max_memory_bytes"]=json!(1);
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&q).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    q["max_memory_bytes"]=json!(16777216);
    q["instructions"][2]["channel"]["gamma"]=json!(1.5);
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&q).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    q=fixture();q["instructions"]=json!([
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"reset_observed","qubit":0}
    ]);
    assert_eq!(execute_noisy_conditioned_json(&serde_json::to_vec(&q).unwrap())
        .unwrap_err().code(),"circuit_rejected");
}
#[test]
fn partial_condition_does_not_expose_other_bit_distribution(){
    let mut q=fixture();
    q["instructions"]=json!([
        {"op":"gate","gate":{"kind":"h","target":0}},
        {"op":"gate","gate":{"kind":"h","target":1}},
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"measure","qubit":1,"into_bit":1}
    ]);
    q["conditions"]=json!([{"bit":0,"equals":true}]);
    let p=run(&serde_json::to_vec(&q).unwrap(),&[]);
    assert_eq!(p.status.code(),Some(0));
    let x:Value=serde_json::from_slice(&p.stdout).unwrap();
    assert_eq!(x["selected_branch_count"],2);
    assert!((x["condition_probability"].as_f64().unwrap()-0.5).abs()<1e-9);
    assert!((x["purity"].as_f64().unwrap()-0.5).abs()<1e-9);
    expect_no_other_histories(&x);
}
