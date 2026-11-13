//! Standalone real CLI / bounded JSON protocol acceptance for 65k counts.
use aura_quantum_sim::factorized_counts_json::{
    execute_factorized_counts_json,FACTOR_COUNTS_REQUEST_SCHEMA,
};
use aura_quantum_sim::factorized_json::FactorProtocolError;
use serde_json::{json,Value};
use std::io::Write;
use std::process::{Command,Output,Stdio};

fn fixture()->Value {
    serde_json::from_slice(include_bytes!("../fixtures/factorized_counts_hadamard_v1.json")).unwrap()
}
fn run(raw:&[u8],args:&[&str])->Output {
    let mut p=Command::new(env!("CARGO_BIN_EXE_aura-quantum-counts-run"))
        .args(args).stdin(Stdio::piped()).stdout(Stdio::piped())
        .stderr(Stdio::piped()).spawn().unwrap();
    let sent=p.stdin.take().unwrap().write_all(raw);
    if args.is_empty() {sent.unwrap();}
    else if let Err(e)=sent {assert_eq!(e.kind(),std::io::ErrorKind::BrokenPipe);}
    p.wait_with_output().unwrap()
}
#[test]
fn real_65536_shot_offline_cli_returns_compact_bounded_histogram() {
    let raw=include_bytes!("../fixtures/factorized_counts_hadamard_v1.json");
    let proc=run(raw,&[]);
    assert_eq!(proc.status.code(),Some(0),"{}",String::from_utf8_lossy(&proc.stderr));
    assert!(proc.stderr.is_empty());
    let v:Value=serde_json::from_slice(&proc.stdout).unwrap();
    assert_eq!(v["schema"],"aura.quantum.factorized-counts-result/1");
    assert_eq!(v["shots"],65536);
    assert_eq!(v["counts_only"],true);
    assert_eq!(v["retained_shot_records"],0);
    assert_eq!(v["qpu_hardware_jobs"],0);
    assert_eq!(v["evidence_class"],"simulation");
    assert_eq!(v["max_histogram_bins"],4096);
    assert!(v.get("results").is_none());
    assert!(v.get("trace").is_none());
    assert_eq!(v["histogram"]["0"].as_u64().unwrap()
              +v["histogram"]["1"].as_u64().unwrap(),65536);
    assert!(proc.stdout.len()<4096);
}
#[test]
fn same_input_seed_replays_byte_identical_json_and_no_hardware() {
    let mut v=fixture();
    v["shots"]=json!(3072);
    let raw=serde_json::to_vec(&v).unwrap();
    let a=run(&raw,&[]);
    let b=run(&raw,&[]);
    assert_eq!(a.status.code(),Some(0));
    assert_eq!(a.stdout,b.stdout);
    assert_eq!(a.stderr,b.stderr);
}
#[test]
fn empty_and_unwritten_classical_bits_preserve_tri_state_encoding() {
    let mut v=fixture();
    v["shots"]=json!(16);
    v["classical_bits"]=json!(3);
    v["instructions"][1]["into_bit"]=json!(1);
    let result=execute_factorized_counts_json(&serde_json::to_vec(&v).unwrap()).unwrap();
    let s:Value=serde_json::from_str(&result).unwrap();
    assert_eq!(s["histogram"].as_object().unwrap().len(),2);
    assert!(s["histogram"].get("?0?").is_some());
    assert!(s["histogram"].get("?1?").is_some());
}
#[test]
fn different_schema_old_detailed_cli_request_is_not_interpreted_as_counts() {
    let mut v=fixture();
    v["schema"]=json!("aura.quantum.factorized-request/1");
    assert!(matches!(
        execute_factorized_counts_json(&serde_json::to_vec(&v).unwrap()),
        Err(FactorProtocolError::UnsupportedSchema)
    ));
    v["schema"]=json!(FACTOR_COUNTS_REQUEST_SCHEMA);
    v["inspect_amplitudes"]=json!(true);
    assert_eq!(execute_factorized_counts_json(&serde_json::to_vec(&v).unwrap())
        .unwrap_err().code(),"invalid_json");
}
#[test]
fn invalid_early_flags_hardware_fields_and_memory_bounds_fail_with_zero_stdout() {
    let v=fixture();
    let mut hw=v.clone();
    hw["qpu_submit"]=json!(true);
    let bad=run(&serde_json::to_vec(&hw).unwrap(),&[]);
    assert_eq!(bad.status.code(),Some(2));
    assert!(bad.stdout.is_empty());
    let err:Value=serde_json::from_slice(&bad.stderr).unwrap();
    assert_eq!(err["schema"],"aura.quantum.factorized-counts-error/1");
    assert_eq!(err["code"],"invalid_json");
    assert_eq!(err["qpu_hardware_jobs"],0);
    let flags=run(&serde_json::to_vec(&v).unwrap(),&["--submit"]);
    assert_eq!(flags.status.code(),Some(2));
    assert!(flags.stdout.is_empty());
    let mut invalid=v.clone();
    invalid["max_memory_bytes"]=json!(0);
    assert_eq!(execute_factorized_counts_json(&serde_json::to_vec(&invalid).unwrap())
        .unwrap_err().code(),"profile_limit");
    invalid["shots"]=json!(65537);
    assert_eq!(execute_factorized_counts_json(&serde_json::to_vec(&invalid).unwrap())
        .unwrap_err().code(),"profile_limit");
}
#[test]
fn histogram_limit_error_has_no_partial_results_on_stdout() {
    let mut v=fixture();
    v["qubits"]=json!(13);
    v["classical_bits"]=json!(13);
    v["shots"]=json!(8000);
    let mut ops=Vec::new();
    for q in 0..13 {
        ops.push(json!({"op":"gate","gate":{"kind":"h","target":q}}));
        ops.push(json!({"op":"measure","qubit":q,"into_bit":q}));
    }
    v["instructions"]=json!(ops);
    let raw=serde_json::to_vec(&v).unwrap();
    let p=run(&raw,&[]);
    assert_eq!(p.status.code(),Some(2));
    assert!(p.stdout.is_empty());
    let err:Value=serde_json::from_slice(&p.stderr).unwrap();
    assert_eq!(err["code"],"circuit_rejected");
    assert!(err["message"].as_str().unwrap().contains("HistogramBinLimit"));
}
