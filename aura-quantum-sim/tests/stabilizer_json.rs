//! End-to-end external subprocess and fail-closed Clifford v1 JSON contract.
use aura_quantum_sim::stabilizer_json::{
    execute_stabilizer_json,StabilizerProtocolError,REQUEST_SCHEMA,MAX_INPUT_BYTES,
};
use aura_quantum_sim::stabilizer::{StabilizerError,StabilizerTableau};
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
use serde_json::{json,Value};
use std::io::Write;
use std::process::{Command,Stdio,Output};

fn request()->Value {
    json!({
      "schema":REQUEST_SCHEMA,"qubits":2,"classical_bits":0,
      "seed":19,"shots":1,"max_memory_bytes":DEFAULT_MEMORY_BUDGET,
      "inspect_generators":true,"inspect_probabilities":true,
      "instructions":[
        {"op":"gate","gate":{"kind":"h","target":0}},
        {"op":"gate","gate":{"kind":"cx","control":0,"target":1}}
      ]
    })
}
fn invoke(bytes:&[u8],args:&[&str])->Output {
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-stabilizer-run"))
      .args(args).stdin(Stdio::piped())
      .stdout(Stdio::piped()).stderr(Stdio::piped())
      .spawn().unwrap();
    p.stdin.take().unwrap().write_all(bytes).unwrap();
    p.wait_with_output().unwrap()
}
#[test]
fn pure_clifford_bell_generators_and_full_basis_probabilities(){
    let out=execute_stabilizer_json(&serde_json::to_vec(&request()).unwrap()).unwrap();
    let r:Value=serde_json::from_str(&out).unwrap();
    assert_eq!(r["schema"],"aura.quantum.stabilizer-result/1");
    assert_eq!(r["profile"],"local-clifford-tableau-binary-symplectic/1");
    assert_eq!(r["precision"],"exact-binary-Clifford-tableau");
    assert_eq!(r["evidence_class"],"simulation");
    assert_eq!(r["qpu_hardware_jobs"],0);
    assert_eq!(r["state_representation"],"2n-signed-pauli-tableau");
    assert_eq!(r["stabilizer_generators"],json!(["+XX","+ZZ"]));
    assert_eq!(r["basis_probabilities"],json!([0.5,0.0,0.0,0.5]));
    assert_eq!(r["histogram"][""],1);
    assert!(r["logical_state_bytes"].as_u64().unwrap() < 65536);
}
#[test]
fn actual_subprocess_bell_feedback_is_seed_reproducible_and_recorded(){
    let input=include_bytes!("../fixtures/stabilizer_bell_feedback_v1.json");
    let a=invoke(input,&[]);
    let b=invoke(input,&[]);
    assert!(a.status.success(),"{}",String::from_utf8_lossy(&a.stderr));
    assert_eq!(a.stdout,b.stdout);
    assert!(a.stderr.is_empty());
    let r:Value=serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(r["shots"],256);
    assert_eq!(r["results"].as_array().unwrap().len(),256);
    assert_eq!(r["histogram"]["00"].as_u64().unwrap()+
        r["histogram"]["01"].as_u64().unwrap(),256);
    for shot in r["results"].as_array().unwrap() {
        assert_eq!(shot["bits"][1],false);
        assert_eq!(shot["trace"].as_array().unwrap().len(),3);
        assert_eq!(shot["trace"][0]["kind"],"measure");
        assert_eq!(shot["trace"][1]["kind"],"reset");
        assert_eq!(shot["trace"][2]["kind"],"measure");
    }
}
#[test]
fn old_pure_and_noisy_json_schemas_remain_unchanged(){
    let pure=include_bytes!("../fixtures/bell_feedforward_v1.json");
    let noisy=include_bytes!("../fixtures/noisy_tomography_v1.json");
    assert!(aura_quantum_sim::json_protocol::execute_json_bytes(pure).is_ok());
    assert!(aura_quantum_sim::noisy_json::execute_noisy_json_bytes(noisy).is_ok());
    assert!(matches!(execute_stabilizer_json(pure),
        Err(StabilizerProtocolError::UnsupportedSchema)));
    assert!(matches!(execute_stabilizer_json(noisy),
        Err(StabilizerProtocolError::UnsupportedSchema)));
}
#[test]
fn non_clifford_t_and_invalid_unreachable_gates_are_denied(){
    let mut d=request();
    d["instructions"][0]["gate"]=json!({"kind":"t","target":0});
    let err=execute_stabilizer_json(&serde_json::to_vec(&d).unwrap()).unwrap_err();
    assert!(matches!(err,StabilizerProtocolError::Circuit(
        StabilizerError::UnsupportedNonClifford)));
    d=request();
    d["classical_bits"]=json!(1);
    d["instructions"]=json!([
       {"op":"measure","qubit":0,"into_bit":0},
       {"op":"if","bit":0,"equals":true,
        "gate":{"kind":"t","target":0}}
    ]);
    assert_eq!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    d["instructions"][1]["gate"]=json!({"kind":"cx","control":0,"target":0});
    assert_eq!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap())
        .unwrap_err().code(),"circuit_rejected");
}
#[test]
fn unknown_fields_hardware_flags_and_invalid_schema_fail_closed(){
    let mut d=request();
    d["hardware_submit"]=json!(true);
    assert!(matches!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap()),
        Err(StabilizerProtocolError::InvalidJson(_))));
    d=request();
    d["instructions"][0]["gate"]["theta"]=json!(0.7);
    assert!(matches!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap()),
        Err(StabilizerProtocolError::InvalidJson(_))));
    d=request();
    d["schema"]=json!("aura.quantum.stabilizer-request/unknown");
    let a=invoke(&serde_json::to_vec(&d).unwrap(),&[]);
    assert_eq!(a.status.code(),Some(2));
    assert!(a.stdout.is_empty());
    let error:Value=serde_json::from_slice(&a.stderr).unwrap();
    assert_eq!(error["schema"],"aura.quantum.stabilizer-error/1");
    assert_eq!(error["code"],"unsupported_schema");
    assert_eq!(error["qpu_hardware_jobs"],0);
    let bad=invoke(b"{}",&["--submit"]);
    assert_eq!(bad.status.code(),Some(2));
    assert!(bad.stdout.is_empty());
}
#[test]
fn uninitialized_classical_control_is_rejected_and_noise_is_not_clifford(){
    let mut d=request();
    d["classical_bits"]=json!(1);
    d["instructions"]=json!([
       {"op":"if","bit":0,"equals":false,
        "gate":{"kind":"x","target":0}}
    ]);
    let err=execute_stabilizer_json(&serde_json::to_vec(&d).unwrap()).unwrap_err();
    assert!(err.to_string().contains("ReadBeforeWrite"));
    d["instructions"]=json!([
       {"op":"noise","channel":{"kind":"phase_flip","qubit":0,"probability":0.2}}
    ]);
    assert!(matches!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap()),
        Err(StabilizerProtocolError::InvalidJson(_))));
}
#[test]
fn explicit_inspection_resource_and_width_caps_are_enforced(){
    let mut d=request();
    d["qubits"]=json!(129);
    assert_eq!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap())
        .unwrap_err().code(),"profile_limit");
    d=request();
    d["shots"]=json!(1025);
    assert_eq!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap())
        .unwrap_err().code(),"profile_limit");
    d=request();
    d["shots"]=json!(2);
    assert_eq!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap())
        .unwrap_err().code(),"profile_limit");
    d=request();
    d["qubits"]=json!(7);
    assert_eq!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap())
        .unwrap_err().code(),"profile_limit");
    d=request();
    d["max_memory_bytes"]=json!(1);
    assert_eq!(execute_stabilizer_json(&serde_json::to_vec(&d).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    assert!(matches!(execute_stabilizer_json(&vec![b' ';MAX_INPUT_BYTES+1]),
        Err(StabilizerProtocolError::InputTooLarge)));
}
#[test]
fn high_width_cli_reports_polynomial_logical_memory_and_generators(){
    let mut d=request();
    d["qubits"]=json!(96);
    d["inspect_probabilities"]=json!(false);
    let result:Value=serde_json::from_str(
        &execute_stabilizer_json(&serde_json::to_vec(&d).unwrap()).unwrap()
    ).unwrap();
    assert_eq!(result["stabilizer_generators"].as_array().unwrap().len(),96);
    assert!(result["logical_state_bytes"].as_u64().unwrap()<65536);
    assert_eq!(result["qpu_hardware_jobs"],0);
}
#[test]
fn induced_clifford_state_probabilities_are_consistent_without_complex_amplitudes(){
    let mut t=StabilizerTableau::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    t.apply(Gate::H(1)).unwrap();
    t.apply(Gate::S(1)).unwrap();
    t.apply(Gate::Cz{control:0,target:1}).unwrap();
    assert!((t.basis_probability(&[false,false]).unwrap()-0.5).abs()<1e-12);
    assert!((t.basis_probability(&[false,true]).unwrap()-0.5).abs()<1e-12);
}
