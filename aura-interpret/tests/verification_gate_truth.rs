//! Issue #48: no solver gate must never manufacture proof success.
//! Test FIRST against original AVM behavior; real public API + CLI JSON.
use aura_interpret::{Avm, AvmConfig, AvmValue};
use std::{io::Write, process::{Command, Stdio}};

const SOURCE: &str = "cell main() ->:\n    yield 17\n";

#[test]
fn exec_source_with_disabled_gate_reports_unverified() {
    let mut vm = Avm::new(AvmConfig {
        enable_z3_gate: false,
        ..Default::default()
    });
    let result = vm.exec_source(SOURCE).expect("unverified source may execute");
    assert!(!result.verified, "Z3 disabled: AVM must not advertise a proof");
    assert_eq!(result.gate_span, None, "no fabricated counterexample span");
}

#[test]
fn exec_entry_cell_without_solver_executes_but_cannot_claim_verification() {
    let mut vm = Avm::new(AvmConfig {
        enable_z3_gate: false,
        ..Default::default()
    });
    let result = vm.exec_entry_cell(SOURCE, "main").expect("unverified entry should execute");
    assert!(matches!(result.value, AvmValue::Int(17)),
        "disabling proof gate must not silently block explicitly requested unverified execution");
    assert!(!result.verified,
        "executed without a solver does not mean verified");
}

#[test]
fn cli_no_z3_json_must_not_report_verified_true() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aura-interpret"))
        .args(["run", "--no-z3", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().expect("start public Aura AVM CLI");
    child.stdin.take().expect("stdin").write_all(SOURCE.as_bytes()).expect("input");
    let out = child.wait_with_output().expect("complete Aura CLI");
    assert!(out.status.success(), "CLI error: {}", String::from_utf8_lossy(&out.stderr));
    let response: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("real AVM --json invalid: {e}; stdout={}",String::from_utf8_lossy(&out.stdout)));
    assert_eq!(response["verified"], false,
        "without Z3 CLI must not claim verified: {response}");
}

#[cfg(not(feature = "z3"))]
#[test]
fn solver_free_build_requested_verification_fails_closed() {
    let mut vm = Avm::new(AvmConfig {
        enable_z3_gate: true,
        ..Default::default()
    });
    let result = vm.exec_entry_cell(SOURCE, "main")
        .expect("unavailable solver returns a truthful nonverified outcome");
    assert!(!result.verified);
    assert!(matches!(result.value, AvmValue::Unit),
        "requested proof with no compiled solver must not execute");
    assert!(result.gate_error.as_deref().unwrap_or("").contains("unavailable"));
}
