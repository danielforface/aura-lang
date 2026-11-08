//! C-017 real strict CLI acceptance, QPU denial and schema isolation.
use aura_quantum_sim::moments_json::{
    execute_moments_json,MOMENTS_REQUEST_SCHEMA,
};
use aura_quantum_sim::observable_json::execute_observable_json;
use aura_quantum_sim::factorized_json::FactorProtocolError;
use serde_json::{json,Value};
use std::io::Write;
use std::process::{Command,Output,Stdio};
fn fixture()->Value {
    let mut v:Value=serde_json::from_slice(
        include_bytes!("../fixtures/observable_bell_t_128_v1.json")
    ).unwrap();
    v["schema"]=json!(MOMENTS_REQUEST_SCHEMA);
    v
}
fn run(raw:&[u8],flags:&[&str])->Output {
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-moments-run"))
        .args(flags).stdin(Stdio::piped()).stdout(Stdio::piped())
        .stderr(Stdio::piped()).spawn().unwrap();
    let written=p.stdin.take().unwrap().write_all(raw);
    if flags.is_empty(){written.unwrap();}
    p.wait_with_output().unwrap()
}
#[test]
fn full_128q_offline_process_reports_finite_conditional_variance(){
    let raw=serde_json::to_vec(&fixture()).unwrap();
    let out=run(&raw,&[]);
    assert_eq!(out.status.code(),Some(0),
        "{}",String::from_utf8_lossy(&out.stderr));
    assert!(out.stderr.is_empty());
    let d:Value=serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(d["schema"],"aura.quantum.moments-result/1");
    assert_eq!(d["qubits"],128);
    assert_eq!(d["term_count"],5);
    assert_eq!(d["shots"],1);
    assert_eq!(d["qpu_hardware_jobs"],0);
    assert_eq!(d["conditioned_on_observed_instruments"],true);
    let mean=d["weighted_sum"].as_f64().unwrap();
    let second=d["weighted_second_moment"].as_f64().unwrap();
    let variance=d["weighted_variance"].as_f64().unwrap();
    assert!(variance>=0.0);
    assert!((second-mean*mean-variance).abs()<1e-7);
    assert!(d["estimated_moment_work_units"].as_u64().unwrap()<16_777_216);
    assert!(d["peak_logical_state_bytes"].as_u64().unwrap()<8192);
}
#[test]
fn original_observable_v1_remains_separate_and_seeded_trace_matches(){
    let original=include_bytes!("../fixtures/observable_dynamic_v1.json");
    let old:Value=serde_json::from_str(&execute_observable_json(original).unwrap()).unwrap();
    let mut v:Value=serde_json::from_slice(original).unwrap();
    v["schema"]=json!(MOMENTS_REQUEST_SCHEMA);
    let raw=serde_json::to_vec(&v).unwrap();
    let a=run(&raw,&[]);
    let b=run(&raw,&[]);
    assert_eq!(a.status.code(),Some(0));
    assert_eq!(a.stdout,b.stdout);
    let d:Value=serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(d["trace"],old["trace"]);
    assert_eq!(d["classical_register"],old["classical_register"]);
    assert_eq!(d["terms"],old["terms"]);
    assert!((d["weighted_sum"].as_f64().unwrap()-
        old["weighted_sum"].as_f64().unwrap()).abs()<1e-10);
    assert!(old.get("weighted_second_moment").is_none());
}
#[test]
fn malformed_hardware_claim_unknown_field_and_flags_fail_no_stdout(){
    let mut f=fixture();
    f["provider"]=json!("physical-qpu");
    let p=run(&serde_json::to_vec(&f).unwrap(),&[]);
    assert_eq!(p.status.code(),Some(2));
    assert!(p.stdout.is_empty());
    let err:Value=serde_json::from_slice(&p.stderr).unwrap();
    assert_eq!(err["code"],"invalid_json");
    assert_eq!(err["schema"],"aura.quantum.moments-error/1");
    assert_eq!(err["qpu_hardware_jobs"],0);
    let p=run(&serde_json::to_vec(&fixture()).unwrap(),&["--qpu"]);
    assert_eq!(p.status.code(),Some(2));
    assert!(p.stdout.is_empty());
}
#[test]
fn wrong_schema_word_coefficient_budget_and_large_term_list_reject(){
    let mut input=fixture();
    input["schema"]=json!("aura.quantum.observable-request/1");
    assert!(matches!(execute_moments_json(&serde_json::to_vec(&input).unwrap()),
        Err(FactorProtocolError::UnsupportedSchema)));
    input["schema"]=json!(MOMENTS_REQUEST_SCHEMA);
    input["terms"][0]["paulis"]=json!("Q".repeat(128));
    assert_eq!(execute_moments_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    input["terms"][0]["paulis"]=json!("I".repeat(128));
    input["terms"][0]["coefficient"]=json!(10001.0);
    assert_eq!(execute_moments_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    input["terms"][0]["coefficient"]=json!(1.0);
    input["max_memory_bytes"]=json!(1);
    assert_eq!(execute_moments_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"profile_limit");
    input["max_memory_bytes"]=json!(16777216);
    input["terms"]=json!(vec![json!({"paulis":"I".repeat(128),"coefficient":1.0});65]);
    assert_eq!(execute_moments_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"profile_limit");
}
