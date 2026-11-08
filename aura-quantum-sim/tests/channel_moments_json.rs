//! C-018 actual Rust offline channel-moments CLI/process contract tests.
use aura_quantum_sim::channel_moments_json::{
    execute_channel_moments_json,CHANNEL_REQUEST_SCHEMA,
};
use serde_json::{json,Value};
use std::process::{Command,Output,Stdio};
use std::io::Write;
fn fixture()->Value{
    serde_json::from_slice(
        include_bytes!("../fixtures/channel_moments_feedforward_v1.json")
    ).unwrap()
}
fn run(raw:&[u8],flags:&[&str])->Output{
    let mut process=Command::new(
        env!("CARGO_BIN_EXE_aura-quantum-channel-moments-run")
    ).args(flags).stdin(Stdio::piped())
    .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let write=process.stdin.take().unwrap().write_all(raw);
    if flags.is_empty(){write.unwrap();}
    process.wait_with_output().unwrap()
}
#[test]
fn full_dynamic_bell_and_hidden_reset_cli_has_true_ensemble_variance(){
    let input=include_bytes!("../fixtures/channel_moments_feedforward_v1.json");
    let result=run(input,&[]);
    assert_eq!(result.status.code(),Some(0),
        "{}",String::from_utf8_lossy(&result.stderr));
    assert!(result.stderr.is_empty());
    let j:Value=serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(j["schema"],"aura.quantum.channel-moments-result/1");
    assert_eq!(j["qubits"],3);
    assert_eq!(j["shots"],0);
    assert_eq!(j["qpu_hardware_jobs"],0);
    assert_eq!(j["final_branch_count"],4);
    assert_eq!(j["instrument_count"],2);
    assert_eq!(j["sampling"],"none;all_representable_instrument_branches");
    assert_eq!(j["energy_scope"],"unconditioned_ideal_instrument_channel");
    assert!(j["weighted_sum"].as_f64().unwrap().abs()<1e-10);
    assert!((j["weighted_second_moment"].as_f64().unwrap()-1.0).abs()<1e-10);
    assert!((j["weighted_variance"].as_f64().unwrap()-1.0).abs()<1e-10);
    assert!(j["within_branch_variance"].as_f64().unwrap().abs()<1e-10);
    assert!((j["between_branch_variance"].as_f64().unwrap()-1.0).abs()<1e-10);
    assert!((j["classical_probabilities"]["0"].as_f64().unwrap()-0.5).abs()<1e-10);
    assert!((j["classical_probabilities"]["1"].as_f64().unwrap()-0.5).abs()<1e-10);
    assert!((j["total_branch_probability"].as_f64().unwrap()-1.0).abs()<1e-10);
}
#[test]
fn reproducible_byte_identical_without_seed_and_old_schema_denial(){
    let raw=include_bytes!("../fixtures/channel_moments_feedforward_v1.json");
    let a=run(raw,&[]);let b=run(raw,&[]);
    assert_eq!(a.status.code(),Some(0));
    assert_eq!(a.stdout,b.stdout);
    let mut old=fixture();
    old["schema"]=json!("aura.quantum.moments-request/1");
    assert_eq!(execute_channel_moments_json(
        &serde_json::to_vec(&old).unwrap()
    ).unwrap_err().code(),"unsupported_schema");
    old["schema"]=json!(CHANNEL_REQUEST_SCHEMA);
    old["seed"]=json!(123);
    assert_eq!(execute_channel_moments_json(
        &serde_json::to_vec(&old).unwrap()
    ).unwrap_err().code(),"invalid_json");
}
#[test]
fn spoofed_qpu_provider_shots_and_cli_flags_rejected_atomically(){
    for (field,value) in [
        ("qpu_backend",json!("real-qpu")),
        ("shots",json!(1024)),
        ("hardware",json!(true)),
        ("device",json!("qpu")),
    ]{
        let mut request=fixture();request[field]=value;
        let out=run(&serde_json::to_vec(&request).unwrap(),&[]);
        assert_eq!(out.status.code(),Some(2));
        assert!(out.stdout.is_empty());
        let e:Value=serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(e["code"],"invalid_json");
        assert_eq!(e["schema"],"aura.quantum.channel-moments-error/1");
        assert_eq!(e["qpu_hardware_jobs"],0);
    }
    let p=run(&serde_json::to_vec(&fixture()).unwrap(),&["--qpu"]);
    assert_eq!(p.status.code(),Some(2));
    assert!(p.stdout.is_empty());
}
#[test]
fn oversized_width_invalid_pauli_and_budget_reject_before_result(){
    let mut input=fixture();
    input["qubits"]=json!(9);
    assert_eq!(execute_channel_moments_json(
        &serde_json::to_vec(&input).unwrap()
    ).unwrap_err().code(),"profile_limit");
    input["qubits"]=json!(3);
    input["terms"][0]["paulis"]=json!("QII");
    assert_eq!(execute_channel_moments_json(
        &serde_json::to_vec(&input).unwrap()
    ).unwrap_err().code(),"circuit_rejected");
    input["terms"][0]["paulis"]=json!("ZII");
    input["max_memory_bytes"]=json!(1);
    assert_eq!(execute_channel_moments_json(
        &serde_json::to_vec(&input).unwrap()
    ).unwrap_err().code(),"circuit_rejected");
    input["max_memory_bytes"]=json!(16777216);
    input["terms"]=json!([]);
    assert_eq!(execute_channel_moments_json(
        &serde_json::to_vec(&input).unwrap()
    ).unwrap_err().code(),"circuit_rejected");
}
