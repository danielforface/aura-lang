//! Full external subprocess + in-memory JSON integration of the new mixed-state
//! simulator, with seed/contract and negative permission/resource controls.
use aura_quantum_sim::noisy_json::{
    execute_noisy_json_bytes, NoisyProtocolError, NOISY_REQUEST_SCHEMA,
    MAX_NOISY_REQUEST_BYTES,
};
use aura_quantum_sim::noisy_circuit::{
    NoisyCircuit, NoisyAction, NoisyInstruction,
};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
use serde_json::{json,Value};
use std::io::Write;
use std::process::{Command,Stdio,Output};

fn noise_request(shots:usize,inspect:bool)->Value {
    json!({
        "schema":NOISY_REQUEST_SCHEMA,
        "qubits":1,
        "classical_bits":0,
        "seed":99,
        "shots":shots,
        "inspect_final_density":inspect,
        "max_memory_bytes":DEFAULT_MEMORY_BUDGET,
        "instructions":[
            {"op":"gate","gate":{"kind":"h","target":0}},
            {"op":"noise","channel":{"kind":"amplitude_damping","qubit":0,"gamma":0.36}},
            {"op":"noise","channel":{"kind":"phase_flip","qubit":0,"probability":0.25}}
        ]
    })
}
fn invoke(raw:&[u8],args:&[&str])->Output {
    let mut child=Command::new(env!("CARGO_BIN_EXE_aura-quantum-noisy-run"))
        .args(args)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    // A rejected --submit flag exits before stdin is consumed. BrokenPipe
    // is an acceptable early-exit result for this *negative test* only.
    let sent=child.stdin.take().unwrap().write_all(raw);
    if args.is_empty() { sent.unwrap(); }
    else if let Err(err)=sent {
        assert_eq!(err.kind(),std::io::ErrorKind::BrokenPipe);
    }
    child.wait_with_output().unwrap()
}

#[test]
fn analytical_tomography_exposes_complete_mixed_matrix_and_provenance() {
    let serialized=serde_json::to_vec(&noise_request(1,true)).unwrap();
    let out=execute_noisy_json_bytes(&serialized).unwrap();
    let result:Value=serde_json::from_str(&out).unwrap();
    assert_eq!(result["schema"],"aura.quantum.noisy-result/1");
    assert_eq!(result["profile"],"local-density-matrix-f64-dynamic-cptp/1");
    assert_eq!(result["status"],"simulated");
    assert_eq!(result["evidence_class"],"simulation");
    assert_eq!(result["qpu_hardware_jobs"],0);
    assert_eq!(result["final_density"]["dimension"],2);
    let r=result["final_density"]["row_major_complex_pairs"].as_array().unwrap();
    let expected=[0.68,0.20,0.20,0.32];
    for (actual,want) in r.iter().zip(expected) {
        assert!((actual[0].as_f64().unwrap()-want).abs()<1e-12);
        assert!(actual[1].as_f64().unwrap().abs()<1e-12);
    }
    assert!((result["final_density"]["purity"].as_f64().unwrap()-0.6448).abs()<1e-12);
    assert!((result["final_density"]["trace"].as_f64().unwrap()-1.0).abs()<1e-12);
    assert_eq!(result["histogram"][""],1);
    assert_eq!(result["results"][0]["bits"],json!([]));
    assert!(result["results"][0]["trace"].as_array().unwrap().is_empty());
}

#[test]
fn actual_external_process_reads_json_and_reports_machine_parseable_result() {
    let fixture=include_bytes!("../fixtures/noisy_bell_feedforward_v1.json");
    let a=invoke(fixture,&[]);
    let b=invoke(fixture,&[]);
    assert!(a.status.success(),"{}",String::from_utf8_lossy(&a.stderr));
    assert!(b.status.success());
    assert_eq!(a.stdout,b.stdout);
    assert!(a.stderr.is_empty());
    let parsed:Value=serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(parsed["shots"],256);
    assert_eq!(parsed["results"].as_array().unwrap().len(),256);
    assert_eq!(parsed["evidence_class"],"simulation");
    assert_eq!(parsed["qpu_hardware_jobs"],0);
    assert!(parsed.get("final_density").is_none());
    let hist=parsed["histogram"].as_object().unwrap();
    let count0=hist["00"].as_u64().unwrap();
    let count1=hist["01"].as_u64().unwrap();
    assert_eq!(count0+count1,256);
    assert!(count0>75 && count1>75);
    for shot in parsed["results"].as_array().unwrap() {
        assert_eq!(shot["bits"][1],false);
        assert_eq!(shot["trace"][0]["kind"],"measure");
        assert_eq!(shot["trace"][1]["kind"],"reset");
        assert_eq!(shot["trace"][2]["kind"],"measure");
    }
}

#[test]
fn pure_v1_json_cli_unchanged_by_new_noisy_schema() {
    let fixture=include_bytes!("../fixtures/bell_feedforward_v1.json");
    let result=aura_quantum_sim::json_protocol::execute_json_bytes(fixture).unwrap();
    let value:Value=serde_json::from_str(&result).unwrap();
    assert_eq!(value["schema"],"aura.quantum.circuit-result/1");
    assert_eq!(value["profile"],"local-pure-statevector-f64-dynamic-z/1");
    let rejected=aura_quantum_sim::json_protocol::execute_json_bytes(
        &serde_json::to_vec(&noise_request(1,true)).unwrap());
    assert!(rejected.is_err());
    assert!(matches!(
        execute_noisy_json_bytes(fixture),
        Err(NoisyProtocolError::UnsupportedSchema)
    ));
}

#[test]
fn full_json_trace_matches_direct_public_rust_noisy_circuit() {
    let doc:Value=serde_json::from_slice(
        include_bytes!("../fixtures/noisy_bell_feedforward_v1.json")
    ).unwrap();
    let output:Value=serde_json::from_str(
        &execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap()
    ).unwrap();
    let mut c=NoisyCircuit::new(2,2);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    c.push(NoisyInstruction::Noise(NoiseChannel::PhaseFlip{qubit:1,probability:0.5}));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    c.push(NoisyInstruction::If{bit:0,equals:true,
        action:NoisyAction::Gate(Gate::X(1))});
    c.push(NoisyInstruction::Reset{qubit:0});
    c.push(NoisyInstruction::Measure{qubit:1,into_bit:1});
    let seed=doc["seed"].as_u64().unwrap();
    let direct=c.execute_shots(256,seed,DEFAULT_MEMORY_BUDGET).unwrap();
    for (source,expected) in output["results"].as_array().unwrap()
        .iter().zip(&direct.records) {
        assert_eq!(source["bits"],json!(expected.bits));
        assert_eq!(source["trace"].as_array().unwrap().len(),expected.trace.len());
        assert_eq!(source["trace"][0]["value"],expected.bits[0].unwrap());
    }
}

#[test]
fn conditional_noise_is_supported_and_affects_correct_branch() {
    let mut doc=noise_request(1,true);
    doc["qubits"]=json!(2);
    doc["classical_bits"]=json!(1);
    doc["instructions"]=json!([
        {"op":"gate","gate":{"kind":"h","target":0}},
        {"op":"gate","gate":{"kind":"h","target":1}},
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"if","bit":0,"equals":true,
         "action":{"kind":"noise","channel":{
             "kind":"amplitude_damping","qubit":1,"gamma":1.0}}}
    ]);
    let mut seen=[false;2];
    for seed in 0..96u64 {
        doc["seed"]=json!(seed);
        let output:Value=serde_json::from_str(
            &execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap()
        ).unwrap();
        let branch=output["results"][0]["bits"][0].as_bool().unwrap();
        seen[branch as usize]=true;
        let probs=output["final_density"]["basis_probabilities"].as_array().unwrap();
        let one=probs[2].as_f64().unwrap()+probs[3].as_f64().unwrap();
        if branch { assert!(one.abs()<1e-12); }
        else { assert!((one-0.5).abs()<1e-12); }
    }
    assert!(seen.into_iter().all(|value| value));
}

#[test]
fn parser_rejects_unknown_fields_disallowed_gates_and_hardware_flags() {
    let mut doc=noise_request(1,true);
    doc["hardware_submit"]=json!(true);
    assert!(matches!(
        execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()),
        Err(NoisyProtocolError::InvalidJson(_))
    ));
    doc.as_object_mut().unwrap().remove("hardware_submit");
    doc["instructions"][0]["gate"]["nonlinear_parameter"]=json!(true);
    assert!(matches!(
        execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()),
        Err(NoisyProtocolError::InvalidJson(_))
    ));
    doc["instructions"][0]=json!({"op":"gate","gate":{"kind":"rz","target":0,"angle":0.2}});
    assert!(matches!(
        execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()),
        Err(NoisyProtocolError::InvalidJson(_))
    ));
}

#[test]
fn invalid_unreachable_noise_and_classical_use_before_write_fail_closed() {
    let mut doc=noise_request(1,false);
    doc["classical_bits"]=json!(1);
    doc["instructions"]=json!([
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"if","bit":0,"equals":true,
         "action":{"kind":"noise","channel":{
            "kind":"phase_flip","qubit":0,"probability":1.01}}}
    ]);
    let rejection=execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap_err();
    assert_eq!(rejection.code(),"circuit_rejected");
    assert!(rejection.to_string().contains("InvalidProbability"));
    doc["instructions"]=json!([
        {"op":"if","bit":0,"equals":false,
         "action":{"kind":"gate","gate":{"kind":"x","target":0}}}
    ]);
    let rejection=execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap_err();
    assert_eq!(rejection.code(),"circuit_rejected");
    assert!(rejection.to_string().contains("ReadBeforeWrite"));
}

#[test]
fn requests_are_bounded_by_matrix_work_size_shots_and_inspection_profile() {
    let mut doc=noise_request(1025,false);
    assert_eq!(
        execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "profile_limit"
    );
    doc=noise_request(2,true);
    assert_eq!(
        execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "profile_limit"
    );
    doc=noise_request(1,true);
    doc["qubits"]=json!(5);
    assert_eq!(
        execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "profile_limit"
    );
    doc=noise_request(1024,false);
    doc["qubits"]=json!(6);
    doc["instructions"]=json!(
        (0..192).map(|_|json!({"op":"gate","gate":{"kind":"h","target":0}}))
        .collect::<Vec<_>>()
    );
    assert_eq!(
        execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "profile_limit"
    );
    assert!(matches!(
        execute_noisy_json_bytes(&vec![b' ';MAX_NOISY_REQUEST_BYTES+1]),
        Err(NoisyProtocolError::InputTooLarge)
    ));
    doc=noise_request(1,false);
    doc["max_memory_bytes"]=json!(15);
    assert_eq!(
        execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap_err().code(),
        "circuit_rejected"
    );
}

#[test]
fn invalid_cli_returns_no_stdout_and_structured_stderr_without_qpu_receipt() {
    let mut doc=noise_request(1,true);
    doc["schema"]=json!("aura.quantum.noisy-request/2");
    let bad=invoke(&serde_json::to_vec(&doc).unwrap(),&[]);
    assert_eq!(bad.status.code(),Some(2));
    assert!(bad.stdout.is_empty());
    let error:Value=serde_json::from_slice(&bad.stderr).unwrap();
    assert_eq!(error["schema"],"aura.quantum.noisy-error/1");
    assert_eq!(error["code"],"unsupported_schema");
    assert_eq!(error["qpu_hardware_jobs"],0);
    let flags=invoke(b"{}",&["--submit"]);
    assert_eq!(flags.status.code(),Some(2));
    assert!(flags.stdout.is_empty());
    let err:Value=serde_json::from_slice(&flags.stderr).unwrap();
    assert_eq!(err["code"],"usage");
}

#[test]
fn observed_reset_samples_branch_but_unobserved_reset_does_not_leak() {
    let mut doc=noise_request(1,true);
    doc["instructions"]=json!([
        {"op":"gate","gate":{"kind":"h","target":0}},
        {"op":"reset","qubit":0},
        {"op":"reset_observed","qubit":0}
    ]);
    let result:Value=serde_json::from_str(
        &execute_noisy_json_bytes(&serde_json::to_vec(&doc).unwrap()).unwrap()
    ).unwrap();
    assert_eq!(result["results"][0]["trace"][0]["kind"],"reset");
    assert!(result["results"][0]["trace"][0].get("value").is_none());
    assert_eq!(result["results"][0]["trace"][1]["kind"],"reset_observed");
    assert_eq!(result["results"][0]["trace"][1]["value"],false);
    assert!((result["final_density"]["purity"].as_f64().unwrap()-1.0).abs()<1e-12);
}
