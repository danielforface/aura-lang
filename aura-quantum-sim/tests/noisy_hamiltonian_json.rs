//! C-026 actual offline executable protocol acceptance, no fake calls.
use aura_quantum_sim::noisy_hamiltonian_json::{
    execute_noisy_hamiltonian_json,NOISY_HAMILTONIAN_REQUEST_SCHEMA,
};
use serde_json::{Value,json};
use std::process::{Command,Output,Stdio};
use std::io::Write;
fn fixture()->Value{
    serde_json::from_slice(include_bytes!(
        "../fixtures/noisy_hamiltonian_v1.json")).unwrap()
}
fn invoke(req:&Value,args:&[&str])->Output{
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-noisy-hamiltonian-run"))
        .args(args).stdin(Stdio::piped()).stdout(Stdio::piped())
        .stderr(Stdio::piped()).spawn().unwrap();
    p.stdin.take().unwrap().write_all(
        &serde_json::to_vec(req).unwrap()).unwrap();
    p.wait_with_output().unwrap()
}
fn close(x:&Value,y:f64){
    let v=x.as_f64().unwrap();
    assert!((v-y).abs()<1e-9,"value {v} expected {y}");
}
fn no_other_events(v:&Value){
    for key in [
        "histogram","classical_probabilities","instrument_trace",
        "shot_trace","results","seed","branch_weights",
        "full_density_row_major_complex_pairs",
    ]{
        assert!(v.get(key).is_none(),"unexpected field {key}");
    }
}
#[test]
fn real_compiled_cli_mixed_noisy_h2_and_v1_result(){
    let p=invoke(&fixture(),&[]);
    assert_eq!(p.status.code(),Some(0),"{}",String::from_utf8_lossy(&p.stderr));
    assert!(p.stderr.is_empty());
    let v:Value=serde_json::from_slice(&p.stdout).unwrap();
    assert_eq!(v["schema"],"aura.quantum.noisy-hamiltonian-result/1");
    assert_eq!(v["status"],"simulated");
    assert_eq!(v["sampling"],"none;full_representable_Born_channel");
    assert_eq!(v["shots"],0);
    assert_eq!(v["qpu_hardware_jobs"],0);
    assert_eq!(v["term_count"],5);
    assert_eq!(v["terms"].as_array().unwrap().len(),5);
    assert_eq!(v["variance_definition"],
        "Tr(rho H^2)-Tr(rho H)^2;not_shot_variance");
    assert!(v["weighted_sum"].as_f64().unwrap().is_finite());
    assert!(v["weighted_second_moment"].as_f64().unwrap()
        >=v["weighted_sum"].as_f64().unwrap().powi(2)-1e-9);
    assert!(v["weighted_variance"].as_f64().unwrap()>=0.0);
    no_other_events(&v);
}
#[test]
fn real_selected_event_preserves_probability_and_hides_other_outcomes(){
    let mut doc=fixture();
    doc["conditions"]=json!([{"bit":0,"equals":true}]);
    let p=invoke(&doc,&[]);
    assert_eq!(p.status.code(),Some(0),
        "{}",String::from_utf8_lossy(&p.stderr));
    let v:Value=serde_json::from_slice(&p.stdout).unwrap();
    assert_eq!(v["selected_conditions"],
        json!([{"bit":0,"equals":true}]));
    assert_eq!(v["selected_branch_count"],1);
    close(&v["condition_probability"],0.5);
    no_other_events(&v);
}
#[test]
fn deterministic_replay_and_strict_schema_isolation(){
    let a=invoke(&fixture(),&[]);
    let b=invoke(&fixture(),&[]);
    assert_eq!(a.status.code(),Some(0));
    assert_eq!(a.stdout,b.stdout);
    let mut doc=fixture();
    doc["schema"]=json!("aura.quantum.moments-request/1");
    assert_eq!(execute_noisy_hamiltonian_json(
        &serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "unsupported_schema");
    doc["schema"]=json!(NOISY_HAMILTONIAN_REQUEST_SCHEMA);
    doc["seed"]=json!(5);
    assert_eq!(execute_noisy_hamiltonian_json(
        &serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "invalid_json");
}
#[test]
fn qpu_seed_shots_and_extra_provider_flags_fail_closed_no_success_stdout(){
    for (key,value) in [
        ("shots",json!(256)),("hardware",json!(true)),
        ("qpu_backend",json!("provider")),("seed",json!(10)),
    ]{
        let mut doc=fixture();
        doc[key]=value;
        let p=invoke(&doc,&[]);
        assert_eq!(p.status.code(),Some(2));
        assert!(p.stdout.is_empty());
        let err:Value=serde_json::from_slice(&p.stderr).unwrap();
        assert_eq!(err["schema"],"aura.quantum.noisy-hamiltonian-error/1");
        assert_eq!(err["code"],"invalid_json");
        assert_eq!(err["qpu_hardware_jobs"],0);
    }
    let p=invoke(&fixture(),&["--qpu"]);
    assert_eq!(p.status.code(),Some(2));
    assert!(p.stdout.is_empty());
}
#[test]
fn invalid_pauli_coefficient_count_and_condition_rejected(){
    for change in ["symbol","width","coefficient","no_terms","duplicate_conditions"]{
        let mut doc=fixture();
        match change{
            "symbol"=>doc["terms"][0]["paulis"]=json!("QQ"),
            "width"=>doc["terms"][0]["paulis"]=json!("Z"),
            "coefficient"=>doc["terms"][0]["coefficient"]=json!(10001.0),
            "no_terms"=>doc["terms"]=json!([]),
            "duplicate_conditions"=>doc["conditions"]=json!([
                {"bit":0,"equals":true},{"bit":0,"equals":false}
            ]),
            _=>unreachable!(),
        }
        let p=invoke(&doc,&[]);
        assert_eq!(p.status.code(),Some(2));
        assert!(p.stdout.is_empty());
        let v:Value=serde_json::from_slice(&p.stderr).unwrap();
        assert_eq!(v["qpu_hardware_jobs"],0);
        if change=="no_terms"{
            assert_eq!(v["code"],"profile_limit");
        }else{
            assert_eq!(v["code"],"circuit_rejected");
        }
    }
}
#[test]
fn bad_budget_observed_reset_and_extra_nested_term_metadata_rejected(){
    let mut doc=fixture();
    doc["max_memory_bytes"]=json!(1);
    assert_eq!(execute_noisy_hamiltonian_json(
        &serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "circuit_rejected");
    doc=fixture();
    doc["instructions"]=json!([{"op":"reset_observed","qubit":0}]);
    assert_eq!(execute_noisy_hamiltonian_json(
        &serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "circuit_rejected");
    doc=fixture();
    doc["terms"][0]["hardware"]=json!("QPU");
    assert_eq!(execute_noisy_hamiltonian_json(
        &serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "invalid_json");
}
