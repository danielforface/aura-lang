//! C-012 real offline factorized JSON and subprocess acceptance.
use aura_quantum_sim::factorized_json::{
    execute_factorized_json,FactorProtocolError,FACTOR_REQUEST_SCHEMA,MAX_REQUEST_BYTES,
};
use serde_json::{json,Value};
use std::io::Write;
use std::process::{Command,Stdio,Output};

fn small()->Value {
    serde_json::from_slice(include_bytes!("../fixtures/factorized_small_phase_v1.json")).unwrap()
}
fn exec(raw:&[u8],args:&[&str])->Output {
    let mut child=Command::new(env!("CARGO_BIN_EXE_aura-quantum-factorized-run"))
        .args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    let sent=child.stdin.take().unwrap().write_all(raw);
    if args.is_empty(){sent.unwrap();}
    else if let Err(e)=sent {assert_eq!(e.kind(),std::io::ErrorKind::BrokenPipe);}
    child.wait_with_output().unwrap()
}
#[test]
fn real_128_qubit_nonclifford_bell_pairs_keep_64_factors_and_no_global_vector(){
    let raw=include_bytes!("../fixtures/factorized_128_bell_t_v1.json");
    let a=exec(raw,&[]);
    assert!(a.status.success(),"{}",String::from_utf8_lossy(&a.stderr));
    assert!(a.stderr.is_empty());
    let v:Value=serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(v["schema"],"aura.quantum.factorized-result/1");
    assert_eq!(v["qubits"],128);
    assert_eq!(v["shots"],1);
    assert_eq!(v["factor_width_limit"],10);
    assert_eq!(v["truncation"],"none;factor-width-hard-limit");
    assert_eq!(v["truncation_error_bound"],0.0);
    assert_eq!(v["qpu_hardware_jobs"],0);
    assert_eq!(v["evidence_class"],"simulation");
    assert_eq!(v["factors"].as_array().unwrap().len(),64);
    for f in v["factors"].as_array().unwrap() {
        assert_eq!(f["local_dimension"],4);
        assert_eq!(f["logical_state_bytes"],64);
    }
    assert!(v["peak_logical_state_bytes"].as_u64().unwrap()<8192);
    let expected=2f64.powi(-64);
    let got=v["inspected_basis_probability"].as_f64().unwrap();
    assert!((got-expected).abs()<1e-25);
}
#[test]
fn public_small_amplitudes_have_direct_q0_lsb_phase_conventions(){
    let v:Value=serde_json::from_str(
        &execute_factorized_json(&serde_json::to_vec(&small()).unwrap()).unwrap()
    ).unwrap();
    assert_eq!(v["profile"],"local-disconnected-factor-statevector-f64-untruncated/1");
    assert_eq!(v["basis_ordered_complex_pairs"].as_array().unwrap().len(),16);
    assert_eq!(v["factors"].as_array().unwrap().len(),2);
    assert_eq!(v["histogram"][""],1);
    assert_eq!(v["results"][0]["trace"].as_array().unwrap().len(),0);
}
#[test]
fn same_cli_bytes_same_seed_replays_exactly_with_no_implicit_qpu(){
    let raw=serde_json::to_vec(&small()).unwrap();
    let first=exec(&raw,&[]);
    let next=exec(&raw,&[]);
    assert_eq!(first.status.code(),Some(0));
    assert_eq!(first.stdout,next.stdout);
    assert!(first.stderr.is_empty());
}
#[test]
fn sampled_bell_measurements_with_reset_and_feedforward_are_seed_stable(){
    let mut r=small();
    r["inspect_factors"]=json!(false);
    r["inspect_amplitudes"]=json!(false);
    r["qubits"]=json!(2);
    r["classical_bits"]=json!(2);
    r["shots"]=json!(64);
    r["instructions"]=json!([
        {"op":"gate","gate":{"kind":"h","target":0}},
        {"op":"gate","gate":{"kind":"cx","control":0,"target":1}},
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"if","bit":0,"equals":true,"gate":{"kind":"x","target":1}},
        {"op":"reset","qubit":0},
        {"op":"measure","qubit":1,"into_bit":1}
    ]);
    let raw=serde_json::to_vec(&r).unwrap();
    let a=execute_factorized_json(&raw).unwrap();
    let b=execute_factorized_json(&raw).unwrap();
    assert_eq!(a,b);
    let v:Value=serde_json::from_str(&a).unwrap();
    assert_eq!(v["results"].as_array().unwrap().len(),64);
    assert!(v["results"].as_array().unwrap()
        .iter().all(|r|r["bits"][1]==false&&r["trace"].as_array().unwrap().len()==3));
}
#[test]
fn impossible_11_qubit_entanglement_merge_exits_without_stdout(){
    let mut r=small();
    r["qubits"]=json!(11);
    r["inspect_amplitudes"]=json!(false);
    r["inspect_factors"]=json!(false);
    let mut op=vec![json!({"op":"gate","gate":{"kind":"h","target":0}})];
    for q in 0..10 {
        op.push(json!({"op":"gate","gate":{"kind":"cx","control":q,"target":q+1}}));
    }
    r["instructions"]=json!(op);
    let raw=serde_json::to_vec(&r).unwrap();
    let err=execute_factorized_json(&raw).unwrap_err();
    assert_eq!(err.code(),"circuit_rejected");
    assert!(err.to_string().contains("FactorWidthExceeded"));
    let cli=exec(&raw,&[]);
    assert_eq!(cli.status.code(),Some(2));
    assert!(cli.stdout.is_empty());
    let e:Value=serde_json::from_slice(&cli.stderr).unwrap();
    assert_eq!(e["schema"],"aura.quantum.factorized-error/1");
    assert_eq!(e["code"],"circuit_rejected");
    assert_eq!(e["qpu_hardware_jobs"],0);
}
#[test]
fn invalid_unreachable_gate_and_uninitialized_bit_fail_before_shots(){
    let mut r=small();
    r["classical_bits"]=json!(1);
    r["instructions"]=json!([
        {"op":"if","bit":0,"equals":false,"gate":{"kind":"x","target":0}}
    ]);
    assert_eq!(execute_factorized_json(&serde_json::to_vec(&r).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    r["instructions"]=json!([
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"if","bit":0,"equals":true,
            "gate":{"kind":"cx","control":0,"target":0}}
    ]);
    assert_eq!(execute_factorized_json(&serde_json::to_vec(&r).unwrap())
        .unwrap_err().code(),"circuit_rejected");
}
#[test]
fn inspection_rejects_multiple_shots_too_many_wires_or_bad_basis_bits(){
    let mut r=small();
    r["shots"]=json!(2);
    assert_eq!(execute_factorized_json(&serde_json::to_vec(&r).unwrap())
        .unwrap_err().code(),"profile_limit");
    r["shots"]=json!(1);
    r["qubits"]=json!(9);
    assert_eq!(execute_factorized_json(&serde_json::to_vec(&r).unwrap())
        .unwrap_err().code(),"profile_limit");
    r["qubits"]=json!(4);
    r["inspect_amplitudes"]=json!(false);
    r["inspect_basis"]=json!([false,true]);
    assert_eq!(execute_factorized_json(&serde_json::to_vec(&r).unwrap())
        .unwrap_err().code(),"profile_limit");
}
#[test]
fn unknown_fields_hardware_qpu_and_old_schema_are_rejected_strictly(){
    let mut r=small();
    r["qpu_submit"]=json!(true);
    assert!(matches!(execute_factorized_json(&serde_json::to_vec(&r).unwrap()),
        Err(FactorProtocolError::InvalidJson(_))));
    r.as_object_mut().unwrap().remove("qpu_submit");
    r["schema"]=json!("aura.quantum.circuit-request/1");
    assert!(matches!(execute_factorized_json(&serde_json::to_vec(&r).unwrap()),
        Err(FactorProtocolError::UnsupportedSchema)));
    r["schema"]=json!(FACTOR_REQUEST_SCHEMA);
    r["instructions"][0]["gate"]["theta"]=json!(0.5);
    assert_eq!(execute_factorized_json(&serde_json::to_vec(&r).unwrap())
        .unwrap_err().code(),"invalid_json");
    assert_eq!(execute_factorized_json(&vec![b' ';MAX_REQUEST_BYTES+1])
        .unwrap_err().code(),"input_too_large");
    let args=exec(b"{}",&["--submit"]);
    assert_eq!(args.status.code(),Some(2));
    assert!(args.stdout.is_empty());
}
#[test]
fn strict_zero_resource_and_129_wire_limits_have_no_partial_output(){
    let mut r=small();
    r["max_memory_bytes"]=json!(0);
    assert_eq!(execute_factorized_json(&serde_json::to_vec(&r).unwrap())
        .unwrap_err().code(),"profile_limit");
    r["max_memory_bytes"]=json!(16777216);
    r["qubits"]=json!(129);
    assert_eq!(execute_factorized_json(&serde_json::to_vec(&r).unwrap())
        .unwrap_err().code(),"profile_limit");
}
