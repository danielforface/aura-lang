//! C-022 real noisy-request/1 existing CLI acceptance for additive CPTP channels.
use aura_quantum_sim::noisy_json::execute_noisy_json_bytes;
use serde_json::{Value,json};
use std::process::{Command,Output,Stdio};
use std::io::Write;
fn request()->Value{
    serde_json::from_slice(include_bytes!("../fixtures/noisy_cptp_channels_v1.json")).unwrap()
}
fn run(doc:&Value,args:&[&str])->Output{
    let mut child=Command::new(env!("CARGO_BIN_EXE_aura-quantum-noisy-run"))
        .args(args).stdin(Stdio::piped()).stdout(Stdio::piped())
        .stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(
        &serde_json::to_vec(doc).unwrap()).unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn real_cli_full_two_qubit_complex_cptp_result_is_finite_and_normalized(){
    let p=run(&request(),&[]);
    assert_eq!(p.status.code(),Some(0),"{}",String::from_utf8_lossy(&p.stderr));
    assert!(p.stderr.is_empty());
    let v:Value=serde_json::from_slice(&p.stdout).unwrap();
    assert_eq!(v["schema"],"aura.quantum.noisy-result/1");
    assert_eq!(v["qpu_hardware_jobs"],0);
    assert_eq!(v["shots"],1);
    let density=&v["final_density"];
    assert_eq!(density["dimension"],4);
    assert_eq!(density["row_major_complex_pairs"].as_array().unwrap().len(),16);
    assert!((density["trace"].as_f64().unwrap()-1.0).abs()<1e-10);
    assert!(density["purity"].as_f64().unwrap()<1.0);
    assert!(density["purity"].as_f64().unwrap()>=0.25-1e-10);
}
#[test]
fn real_cli_new_noise_backward_compatible_seeded_replay(){
    let a=run(&request(),&[]);let b=run(&request(),&[]);
    assert_eq!(a.status.code(),Some(0));assert_eq!(a.stdout,b.stdout);
    let mut old=request();
    old["instructions"]=json!([
        {"op":"gate","gate":{"kind":"h","target":0}},
        {"op":"noise","channel":{
            "kind":"phase_flip","qubit":0,"probability":0.2}},
        {"op":"noise","channel":{
            "kind":"amplitude_damping","qubit":1,"gamma":0.4}}
    ]);
    let r=run(&old,&[]);
    assert_eq!(r.status.code(),Some(0),"{}",String::from_utf8_lossy(&r.stderr));
}
#[test]
fn strict_json_fails_impossible_pauli_mix_and_unknown_nested_parameters(){
    for variant in [
        json!({"kind":"pauli_mixture","qubit":0,"p_x":0.8,"p_y":0.6,"p_z":0.0}),
        json!({"kind":"pauli_mixture","qubit":0,"p_x":-0.1,"p_y":0.0,"p_z":0.0}),
        json!({"kind":"phase_damping","qubit":0,"gamma":1.1}),
        json!({"kind":"depolarizing","qubit":0,"probability":-0.2}),
    ]{
        let mut doc=request();
        doc["instructions"][2]["channel"]=variant;
        let out=run(&doc,&[]);
        assert_eq!(out.status.code(),Some(2));
        assert!(out.stdout.is_empty());
        let e:Value=serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(e["code"],"circuit_rejected");
        assert_eq!(e["qpu_hardware_jobs"],0);
    }
    let mut doc=request();
    doc["instructions"][2]["channel"]["backend"]=json!("QPU");
    assert_eq!(execute_noisy_json_bytes(
        &serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),"invalid_json");
}
#[test]
fn invalid_unreachable_if_noise_rejected_before_any_shot_and_no_partial_output(){
    let mut doc=request();
    doc["classical_bits"]=json!(1);
    doc["instructions"]=json!([
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"if","bit":0,"equals":true,"action":{
            "kind":"noise","channel":{"kind":"pauli_mixture",
              "qubit":1,"p_x":0.7,"p_y":0.7,"p_z":0.0}
        }}
    ]);
    let r=run(&doc,&[]);
    assert_eq!(r.status.code(),Some(2));
    assert!(r.stdout.is_empty());
}
#[test]
fn flags_hardware_fields_and_noisy_schema_isolation_fail_closed(){
    let out=run(&request(),&["--qpu"]);
    assert_eq!(out.status.code(),Some(2));assert!(out.stdout.is_empty());
    let mut doc=request();doc["hardware"]=json!(true);
    assert_eq!(execute_noisy_json_bytes(
        &serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),"invalid_json");
    doc.as_object_mut().unwrap().remove("hardware");
    doc["schema"]=json!("aura.quantum.channel-moments-request/1");
    assert_eq!(execute_noisy_json_bytes(
        &serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),"unsupported_schema");
}
