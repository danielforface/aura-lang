//! AURA-Q28-010 offline exact symbolic contract through public JSON and real
//! executable. Positive, negative and interop isolation acceptance.
use aura_quantum_sim::symbolic_json::{
    execute_symbolic_json, SymbolicProtocolError, SYMBOLIC_REQUEST_SCHEMA,
    MAX_SYMBOLIC_INPUT_BYTES,
};
use aura_quantum_sim::{DEFAULT_MEMORY_BUDGET, symbolic::SymbolicError};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn fixture() -> Value {
    serde_json::from_slice(include_bytes!("../fixtures/symbolic_hth_v1.json")).unwrap()
}
fn run_binary(raw: &[u8], args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aura-quantum-symbolic-run"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().unwrap();
    child.stdin.take().unwrap().write_all(raw).unwrap();
    child.wait_with_output().unwrap()
}
#[test]
fn exact_hth_output_coefficients_are_lossless_strings_and_phase_sensitive() {
    let output = execute_symbolic_json(&serde_json::to_vec(&fixture()).unwrap()).unwrap();
    let data: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(data["schema"], "aura.quantum.symbolic-result/1");
    assert_eq!(data["profile"], "local-exact-root2-gaussian-phase/1");
    assert_eq!(data["exact_ring"], "Z[sqrt(2),i]/2^k,checked-i128");
    assert_eq!(data["sampling"], "none;forced-outcomes-only");
    assert_eq!(data["coefficient_encoding"], "signed-i128-decimal-strings");
    assert_eq!(data["evidence_class"], "simulation");
    assert_eq!(data["qpu_hardware_jobs"], 0);
    assert_eq!(data["trace"].as_array().unwrap().len(), 0);
    assert_eq!(data["classical_register"], json!([]));
    let probabilities = data["basis_joint_weights"].as_array().unwrap();
    assert_eq!(probabilities.len(), 2);
    assert_eq!(probabilities[0]["numerator"]["rational"], "2");
    assert_eq!(probabilities[0]["numerator"]["radical"], "1");
    assert_eq!(probabilities[0]["denominator_power_of_two"], 2);
    assert_eq!(probabilities[1]["numerator"]["rational"], "2");
    assert_eq!(probabilities[1]["numerator"]["radical"], "-1");
    assert_eq!(probabilities[1]["denominator_power_of_two"], 2);
    assert_eq!(data["final_joint_mass"]["numerator"]["rational"], "1");
    assert_eq!(data["final_joint_mass"]["numerator"]["radical"], "0");
    assert_eq!(data["final_joint_mass"]["denominator_power_of_two"], 0);
    assert!(data["amplitudes"][0]["numerator"]["real"]["rational"].is_string());
    assert!(data["amplitudes"][0]["numerator"]["imag"]["radical"].is_string());
}

#[test]
fn externally_executed_cli_is_byte_deterministic_and_has_no_implicit_sampling() {
    let raw = include_bytes!("../fixtures/symbolic_hth_v1.json");
    let a = run_binary(raw, &[]);
    let b = run_binary(raw, &[]);
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    assert!(a.stderr.is_empty());
    assert_eq!(a.stdout, b.stdout);
    let data: Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(data["sampling"], "none;forced-outcomes-only");
    assert!(data.get("seed").is_none());
    assert!(data.get("shots").is_none());
    let direct = execute_symbolic_json(raw).unwrap();
    assert_eq!(String::from_utf8(a.stdout).unwrap().trim(), direct);
}

#[test]
fn forced_bell_feedback_event_ratio_is_explicit_and_joint_mass_stays_half() {
    let data: Value = serde_json::from_str(&execute_symbolic_json(
        include_bytes!("../fixtures/symbolic_bell_forced_v1.json")
    ).unwrap()).unwrap();
    assert_eq!(data["classical_register"], json!([true,false]));
    let trace = data["trace"].as_array().unwrap();
    assert_eq!(trace.len(), 2);
    assert_eq!(trace[0]["instruction"], 2);
    assert_eq!(trace[0]["outcome"], true);
    assert_eq!(trace[0]["joint_weight_before"]["numerator"]["rational"], "1");
    assert_eq!(trace[0]["joint_weight_after"]["numerator"]["rational"], "1");
    assert_eq!(trace[0]["joint_weight_after"]["denominator_power_of_two"], 1);
    assert_eq!(trace[1]["joint_weight_after"], trace[0]["joint_weight_after"]);
    assert_eq!(trace[1]["conditional_probability_approx_f64"], 1.0);
    assert_eq!(data["final_joint_mass"], trace[1]["joint_weight_after"]);
    let weights = data["basis_joint_weights"].as_array().unwrap();
    for (index,p) in weights.iter().enumerate() {
        if index == 1 {
            assert_eq!(p["numerator"]["rational"], "1");
            assert_eq!(p["denominator_power_of_two"], 1);
        } else {
            assert_eq!(p["numerator"]["rational"], "0");
        }
    }
}

#[test]
fn impossible_selected_outcome_is_rejected_without_false_success() {
    let mut input = fixture();
    input["instructions"] = json!([
        {"op":"measure","qubit":0,"into_bit":0,"outcome":true}
    ]);
    input["classical_bits"]=json!(1);
    let raw=serde_json::to_vec(&input).unwrap();
    let err=execute_symbolic_json(&raw).unwrap_err();
    assert!(matches!(err,SymbolicProtocolError::Circuit(SymbolicError::ImpossibleOutcome)));
    let cli=run_binary(&raw,&[]);
    assert_eq!(cli.status.code(),Some(2));
    assert!(cli.stdout.is_empty());
    let error:Value=serde_json::from_slice(&cli.stderr).unwrap();
    assert_eq!(error["schema"],"aura.quantum.symbolic-error/1");
    assert_eq!(error["code"],"circuit_rejected");
    assert_eq!(error["qpu_hardware_jobs"],0);
}

#[test]
fn version_mismatch_and_old_pure_noisy_stabilizer_schemas_are_isolated() {
    let mut input=fixture();
    input["schema"]=json!("aura.quantum.symbolic-request/2");
    assert!(matches!(
        execute_symbolic_json(&serde_json::to_vec(&input).unwrap()),
        Err(SymbolicProtocolError::UnsupportedSchema)
    ));
    for raw in [
        include_bytes!("../fixtures/bell_feedforward_v1.json").as_slice(),
        include_bytes!("../fixtures/noisy_tomography_v1.json").as_slice(),
        include_bytes!("../fixtures/stabilizer_bell_inspection_v1.json").as_slice(),
    ] {
        assert!(matches!(execute_symbolic_json(raw),
            Err(SymbolicProtocolError::UnsupportedSchema)));
    }
    let symbolic=include_bytes!("../fixtures/symbolic_hth_v1.json");
    assert!(aura_quantum_sim::json_protocol::execute_json_bytes(symbolic).is_err());
    assert!(aura_quantum_sim::noisy_json::execute_noisy_json_bytes(symbolic).is_err());
    assert!(aura_quantum_sim::stabilizer_json::execute_stabilizer_json(symbolic).is_err());
}

#[test]
fn request_parser_rejects_hardware_seed_shots_rotation_and_extra_gate_fields() {
    let mut input=fixture();
    for (key,value) in [
        ("hardware_submit",json!(true)),
        ("seed",json!(1)),
        ("shots",json!(100)),
    ] {
        input[key]=value;
        assert!(matches!(execute_symbolic_json(&serde_json::to_vec(&input).unwrap()),
            Err(SymbolicProtocolError::InvalidJson(_))));
        input.as_object_mut().unwrap().remove(key);
    }
    input["instructions"][0]["gate"]["theta"]=json!(0.3);
    assert!(matches!(execute_symbolic_json(&serde_json::to_vec(&input).unwrap()),
        Err(SymbolicProtocolError::InvalidJson(_))));
    input["instructions"][0]["gate"]=json!({"kind":"rz","target":0,"theta":0.3});
    assert!(matches!(execute_symbolic_json(&serde_json::to_vec(&input).unwrap()),
        Err(SymbolicProtocolError::InvalidJson(_))));
    let bad=run_binary(b"{}",&["--submit"]);
    assert_eq!(bad.status.code(),Some(2));
    assert!(bad.stdout.is_empty());
}

#[test]
fn invalid_unreachable_gate_and_classical_read_before_write_fail_preflight() {
    let mut input=fixture();
    input["classical_bits"]=json!(1);
    input["instructions"]=json!([
      {"op":"if","bit":0,"equals":false,"gate":{"kind":"x","target":0}}
    ]);
    let err=execute_symbolic_json(&serde_json::to_vec(&input).unwrap()).unwrap_err();
    assert_eq!(err.code(),"circuit_rejected");
    assert!(err.to_string().contains("ReadBeforeWrite"));
    input["instructions"]=json!([
      {"op":"measure","qubit":0,"into_bit":0,"outcome":false},
      {"op":"if","bit":0,"equals":true,"gate":{"kind":"cx","control":0,"target":0}}
    ]);
    let err=execute_symbolic_json(&serde_json::to_vec(&input).unwrap()).unwrap_err();
    assert!(err.to_string().contains("DuplicateQubit"));
}

#[test]
fn input_limits_memory_budget_and_symbolic_denominator_depth_are_enforced() {
    let mut input=fixture();
    input["qubits"]=json!(5);
    assert_eq!(execute_symbolic_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"profile_limit");
    input=fixture();
    input["max_memory_bytes"]=json!(16);
    assert_eq!(execute_symbolic_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    input=fixture();
    input["max_memory_bytes"]=json!(DEFAULT_MEMORY_BUDGET+1);
    assert_eq!(execute_symbolic_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"profile_limit");
    input=fixture();
    input["instructions"]=json!(
        (0..49).map(|_|json!({"op":"gate","gate":{"kind":"h","target":0}}))
            .collect::<Vec<_>>()
    );
    assert_eq!(execute_symbolic_json(&serde_json::to_vec(&input).unwrap())
        .unwrap_err().code(),"circuit_rejected");
    assert!(matches!(
        execute_symbolic_json(&vec![b' ';MAX_SYMBOLIC_INPUT_BYTES+1]),
        Err(SymbolicProtocolError::InputTooLarge)
    ));
}

#[test]
fn exact_transport_does_not_round_radical_coefficients_into_js_numbers() {
    let result:Value=serde_json::from_str(&execute_symbolic_json(
        include_bytes!("../fixtures/symbolic_hth_v1.json")
    ).unwrap()).unwrap();
    for amp in result["amplitudes"].as_array().unwrap() {
        for part in ["real","imag"] {
            assert!(amp["numerator"][part]["rational"].is_string());
            assert!(amp["numerator"][part]["radical"].is_string());
        }
        assert!(amp["approximate_real_imag"].as_array().unwrap()
            .iter().all(|x|x.is_number()));
    }
}
