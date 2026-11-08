//! C-016 real offline observable CLI scientific and contract tests.
use aura_quantum_sim::observable_json::{
    execute_observable_json,OBSERVABLE_REQUEST_SCHEMA,
};
use aura_quantum_sim::factorized_json::FactorProtocolError;
use serde_json::{json,Value};
use std::io::Write;
use std::process::{Command,Output,Stdio};
fn fixture()->Value {
    serde_json::from_slice(include_bytes!("../fixtures/observable_bell_t_128_v1.json")).unwrap()
}
fn run(raw:&[u8],flags:&[&str])->Output {
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-observable-run"))
        .args(flags).stdin(Stdio::piped()).stdout(Stdio::piped())
        .stderr(Stdio::piped()).spawn().unwrap();
    let written=p.stdin.take().unwrap().write_all(raw);
    if flags.is_empty(){written.unwrap();}
    let result=p.wait_with_output().unwrap();
    result
}
#[test]
fn full_128_wire_vqe_style_hamiltonian_real_process_is_finite_and_bounded(){
    let raw=include_bytes!("../fixtures/observable_bell_t_128_v1.json");
    let out=run(raw,&[]);
    assert_eq!(out.status.code(),Some(0),"{}",String::from_utf8_lossy(&out.stderr));
    assert!(out.stderr.is_empty());
    let d:Value=serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(d["schema"],"aura.quantum.observable-result/1");
    assert_eq!(d["qubits"],128);
    assert_eq!(d["shots"],1);
    assert_eq!(d["term_count"],5);
    assert_eq!(d["pauli_word_order"],"q0-first;position-0-is-least-significant-wire");
    assert_eq!(d["expectation_mode"],"analytic_pure_state_final");
    assert_eq!(d["qpu_hardware_jobs"],0);
    let want=1.25+std::f64::consts::FRAC_1_SQRT_2;
    assert!((d["weighted_sum"].as_f64().unwrap()-want).abs()<1e-10);
    assert!(d["peak_logical_state_bytes"].as_u64().unwrap()<8192);
    assert!(d["trace"].as_array().unwrap().is_empty());
    assert_eq!(d["terms"][2]["paulis"].as_str().unwrap().bytes().filter(|&c|c==b'Y').count(),2);
}
#[test]
fn seed_replay_is_byte_identical_for_sampled_conditional_branch(){
    let raw=include_bytes!("../fixtures/observable_dynamic_v1.json");
    let a=run(raw,&[]);let b=run(raw,&[]);
    assert_eq!(a.status.code(),Some(0));
    assert_eq!(a.stdout,b.stdout);
    assert_eq!(a.stderr,b.stderr);
    let d:Value=serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(d["trace"].as_array().unwrap().len(),2);
    assert_eq!(d["classical_register"].as_array().unwrap().len(),1);
    assert_eq!(d["conditioned_on_observed_instruments"],true);
    assert_eq!(d["qpu_hardware_jobs"],0);
}
#[test]
fn rejects_old_factor_schema_and_all_unrecognized_hardware_fields(){
    let mut request=fixture();
    request["schema"]=json!("aura.quantum.factorized-request/1");
    let input=serde_json::to_vec(&request).unwrap();
    assert!(matches!(execute_observable_json(&input),
        Err(FactorProtocolError::UnsupportedSchema)));
    request["schema"]=json!(OBSERVABLE_REQUEST_SCHEMA);
    request["qpu_backend"]=json!("fake-hardware");
    let p=run(&serde_json::to_vec(&request).unwrap(),&[]);
    assert_eq!(p.status.code(),Some(2));
    assert!(p.stdout.is_empty());
    let e:Value=serde_json::from_slice(&p.stderr).unwrap();
    assert_eq!(e["schema"],"aura.quantum.observable-error/1");
    assert_eq!(e["code"],"invalid_json");
    assert_eq!(e["qpu_hardware_jobs"],0);
}
#[test]
fn rejects_invalid_pauli_letter_wrong_width_and_nonfinite_json(){
    let mut request=fixture();
    request["terms"][0]["paulis"]=json!("Q".repeat(128));
    assert_eq!(execute_observable_json(&serde_json::to_vec(&request).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    request["terms"][0]["paulis"]=json!("X".repeat(127));
    assert_eq!(execute_observable_json(&serde_json::to_vec(&request).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    request["terms"][0]["paulis"]=json!("I".repeat(128));
    request["terms"][0]["coefficient"]=json!(10001.0);
    assert_eq!(execute_observable_json(&serde_json::to_vec(&request).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    let raw=br#"{"schema":"aura.quantum.observable-request/1","qubits":2,
        "classical_bits":0,"seed":1,"max_memory_bytes":16777216,
        "instructions":[],"terms":[{"paulis":"II","coefficient":NaN}]}"#;
    assert_eq!(execute_observable_json(raw).unwrap_err().code(),"invalid_json");
}
#[test]
fn zero_or_65_terms_and_intrusive_flags_fail_without_partial_stdout(){
    let mut input=fixture();
    input["terms"]=json!([]);
    assert_eq!(execute_observable_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"profile_limit");
    input["terms"]=json!(vec![json!({"paulis":"I".repeat(128),"coefficient":1.0});65]);
    assert_eq!(execute_observable_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"profile_limit");
    let out=run(&serde_json::to_vec(&fixture()).unwrap(),&["--qpu"]);
    assert_eq!(out.status.code(),Some(2));
    assert!(out.stdout.is_empty());
}
#[test]
fn invalid_unreachable_branch_and_read_before_write_reject_atomically(){
    let mut input:Value=serde_json::from_slice(
        include_bytes!("../fixtures/observable_dynamic_v1.json")).unwrap();
    input["instructions"][0]=json!({"op":"if","bit":0,"equals":false,
        "gate":{"kind":"x","target":0}});
    let out=run(&serde_json::to_vec(&input).unwrap(),&[]);
    assert_eq!(out.status.code(),Some(2));
    assert!(out.stdout.is_empty());
    let err:Value=serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["code"],"circuit_rejected");
    input["instructions"][0]=json!({"op":"gate","gate":{"kind":"h","target":0}});
    input["instructions"].as_array_mut().unwrap().push(json!(
        {"op":"if","bit":0,"equals":false,
         "gate":{"kind":"cz","control":1,"target":1}}
    ));
    assert_eq!(execute_observable_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"circuit_rejected");
}
