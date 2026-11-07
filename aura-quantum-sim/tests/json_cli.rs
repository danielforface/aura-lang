//! Full external-process acceptance: versioned JSON stdin -> JSON stdout.
use aura_quantum_sim::circuit::{Circuit, Instruction, TraceKind};
use aura_quantum_sim::json_protocol::{
    execute_json_bytes, ProtocolError, REQUEST_SCHEMA, MAX_REQUEST_BYTES,
};
use aura_quantum_sim::{Gate, DEFAULT_MEMORY_BUDGET};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn bell_request(seed: u64, shots: usize) -> Value {
    json!({
        "schema": REQUEST_SCHEMA,
        "qubits": 2,
        "classical_bits": 1,
        "seed": seed,
        "shots": shots,
        "max_memory_bytes": DEFAULT_MEMORY_BUDGET,
        "instructions": [
            {"op":"gate","gate":{"kind":"h","target":0}},
            {"op":"gate","gate":{"kind":"cx","control":0,"target":1}},
            {"op":"measure","qubit":0,"into_bit":0},
            {"op":"if","bit":0,"equals":true,"gate":{"kind":"x","target":1}},
            {"op":"reset","qubit":0}
        ]
    })
}

fn invoke(input: &[u8], args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aura-quantum-run"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // On invalid CLI flags the child can exit before reading stdin.
    // BrokenPipe is then expected for this negative test; actual status,
    // stdout and stderr are still asserted by the calling test.
    let sent=child.stdin.take().unwrap().write_all(input);
    if args.is_empty() { sent.unwrap(); }
    else if let Err(err)=sent {
        assert_eq!(err.kind(),std::io::ErrorKind::BrokenPipe);
    }
    child.wait_with_output().unwrap()
}

#[test]
fn public_json_api_matches_direct_rust_circuit_and_seed_replays() {
    let doc = bell_request(0x5151_2626, 128);
    let serialized = serde_json::to_vec(&doc).unwrap();
    let first = execute_json_bytes(&serialized).unwrap();
    let second = execute_json_bytes(&serialized).unwrap();
    assert_eq!(first, second);
    let output: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(output["status"], "simulated");
    assert_eq!(output["evidence_class"], "simulation");
    assert_eq!(output["qpu_hardware_jobs"], 0);
    assert_eq!(output["schema"], "aura.quantum.circuit-result/1");
    assert_eq!(output["shots"], 128);
    assert_eq!(output["profile"], "local-pure-statevector-f64-dynamic-z/1");
    assert_eq!(output["results"].as_array().unwrap().len(), 128);

    let mut original = Circuit::new(2, 1);
    original.push(Instruction::Gate(Gate::H(0)));
    original.push(Instruction::Gate(Gate::Cnot { control: 0, target: 1 }));
    original.push(Instruction::Measure { qubit: 0, into_bit: 0 });
    original.push(Instruction::If {
        bit: 0,
        equals: true,
        gate: Gate::X(1),
    });
    original.push(Instruction::Reset { qubit: 0 });
    let direct = original.execute_shots(128, 0x5151_2626, DEFAULT_MEMORY_BUDGET).unwrap();
    let mut both = [false; 2];
    for (serial, rust) in output["results"].as_array().unwrap().iter().zip(direct.shots) {
        let observed = rust.bits[0].unwrap();
        both[observed as usize] = true;
        assert_eq!(serial["bits"][0], observed);
        assert!(serial["bits"].as_array().unwrap().len() == 1);
        let trace = serial["trace"].as_array().unwrap();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0]["kind"], "measure");
        assert_eq!(trace[0]["value"], observed);
        assert_eq!(trace[0]["into_bit"], 0);
        assert_eq!(trace[0]["instruction"], 2);
        assert_eq!(trace[1]["kind"], "reset");
        assert_eq!(trace[1]["instruction"], 4);
        assert_eq!(trace[1]["previous_value"], observed);
        assert_eq!(
            rust.trace[1].kind,
            TraceKind::Reset { previous_value: observed }
        );
    }
    assert!(both.into_iter().all(|present| present));
}

#[test]
fn executable_consumes_stdin_and_emits_one_complete_simulation_result() {
    let bytes = serde_json::to_vec(&bell_request(91, 128)).unwrap();
    let a = invoke(&bytes, &[]);
    let b = invoke(&bytes, &[]);
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    assert!(b.status.success());
    assert!(a.stderr.is_empty());
    assert_eq!(a.stdout, b.stdout);
    let result: Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(result["status"], "simulated");
    assert_eq!(result["shots"], 128);
    let counts = result["results"].as_array().unwrap().iter().filter(
        |item| item["bits"][0] == true
    ).count();
    assert!(counts > 30 && counts < 98);
}

#[test]
fn bad_schema_and_unrecognized_fields_are_denied_not_silently_ignored() {
    let mut wrong = bell_request(3, 1);
    wrong["schema"] = json!("aura.quantum.circuit-request/2");
    assert!(matches!(
        execute_json_bytes(&serde_json::to_vec(&wrong).unwrap()),
        Err(ProtocolError::UnsupportedSchema)
    ));

    let mut new_field = bell_request(3, 1);
    new_field["hardware_submit"] = json!(true);
    assert!(matches!(
        execute_json_bytes(&serde_json::to_vec(&new_field).unwrap()),
        Err(ProtocolError::InvalidJson(_))
    ));
    let mut nested = bell_request(3, 1);
    nested["instructions"][0]["gate"]["forbidden"] = json!(true);
    assert!(matches!(
        execute_json_bytes(&serde_json::to_vec(&nested).unwrap()),
        Err(ProtocolError::InvalidJson(_))
    ));
    let mut unsupported_gate = bell_request(3, 1);
    unsupported_gate["instructions"][0]["gate"]["kind"] = json!("rz");
    assert!(matches!(
        execute_json_bytes(&serde_json::to_vec(&unsupported_gate).unwrap()),
        Err(ProtocolError::InvalidJson(_))
    ));
}

#[test]
fn invalid_classical_flow_and_unreachable_bad_gates_fail_closed() {
    let mut no_measure = bell_request(0, 1);
    no_measure["instructions"] = json!([
        {"op":"if","bit":0,"equals":true,"gate":{"kind":"x","target":0}}
    ]);
    let rejected = execute_json_bytes(&serde_json::to_vec(&no_measure).unwrap())
        .unwrap_err();
    assert_eq!(rejected.code(), "circuit_rejected");
    assert!(rejected.to_string().contains("ReadBeforeWrite"));

    let mut bad_gate = bell_request(0, 1);
    bad_gate["instructions"] = json!([
        {"op":"measure","qubit":0,"into_bit":0},
        {"op":"if","bit":0,"equals":true,
         "gate":{"kind":"cx","control":0,"target":0}}
    ]);
    let rejected = execute_json_bytes(&serde_json::to_vec(&bad_gate).unwrap())
        .unwrap_err();
    assert_eq!(rejected.code(), "circuit_rejected");
    assert!(rejected.to_string().contains("DuplicateQubit"));
}

#[test]
fn cpu_work_input_size_and_shot_limits_are_enforced() {
    let mut large = bell_request(0, 2049);
    assert_eq!(
        execute_json_bytes(&serde_json::to_vec(&large).unwrap())
            .unwrap_err().code(),
        "profile_limit"
    );
    large["shots"] = json!(2048);
    large["qubits"] = json!(12);
    large["instructions"] = json!(
        (0..256).map(|_| json!({"op":"gate","gate":{"kind":"h","target":0}}))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        execute_json_bytes(&serde_json::to_vec(&large).unwrap())
            .unwrap_err().code(),
        "profile_limit"
    );
    let oversized = vec![b' '; MAX_REQUEST_BYTES + 1];
    assert!(matches!(
        execute_json_bytes(&oversized),
        Err(ProtocolError::InputTooLarge)
    ));
    let mut memory = bell_request(0, 16);
    memory["max_memory_bytes"] = json!(32);
    assert_eq!(
        execute_json_bytes(&serde_json::to_vec(&memory).unwrap())
            .unwrap_err().code(),
        "circuit_rejected"
    );
}

#[test]
fn invalid_cli_request_never_returns_partial_success_json() {
    let mut wrong = bell_request(0, 1);
    wrong["schema"] = json!("aura.quantum.circuit-request/unknown");
    let output = invoke(&serde_json::to_vec(&wrong).unwrap(), &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["status"], "error");
    assert_eq!(error["code"], "unsupported_schema");
    assert_eq!(error["qpu_hardware_jobs"], 0);
    assert_eq!(error["schema"], "aura.quantum.circuit-error/1");

    let bad_flags = invoke(b"{}", &["--submit"]);
    assert_eq!(bad_flags.status.code(), Some(2));
    assert!(bad_flags.stdout.is_empty());
    let error: Value = serde_json::from_slice(&bad_flags.stderr).unwrap();
    assert_eq!(error["code"], "usage");
}

#[test]
fn single_shot_without_measurement_has_no_phantom_classical_bits() {
    let mut request = bell_request(0, 1);
    request["instructions"] = json!([
        {"op":"gate","gate":{"kind":"h","target":0}},
        {"op":"gate","gate":{"kind":"z","target":1}},
        {"op":"gate","gate":{"kind":"s","target":0}},
        {"op":"gate","gate":{"kind":"t","target":0}},
        {"op":"gate","gate":{"kind":"y","target":0}},
        {"op":"gate","gate":{"kind":"swap","a":0,"b":1}},
        {"op":"gate","gate":{"kind":"cz","control":0,"target":1}}
    ]);
    request["classical_bits"] = json!(2);
    let result: Value = serde_json::from_str(&execute_json_bytes(
        &serde_json::to_vec(&request).unwrap()
    ).unwrap()).unwrap();
    assert_eq!(result["results"][0]["bits"], json!([null, null]));
    assert!(result["results"][0]["trace"].as_array().unwrap().is_empty());
}
