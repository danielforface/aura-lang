//! Issue #28 / Q01-Q04: real Aura CLI boolean-literal SMT regression.
//! Baseline phase: code intentionally unchanged. These regressions must expose RED with Z3.
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

struct Fixture { root: PathBuf, source: PathBuf, report: PathBuf }
impl Fixture {
    fn new(code: &str) -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("aura-bool-verifier-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("main.aura");
        let report = root.join("report.json");
        fs::write(&source, code).unwrap();
        Self { root, source, report }
    }
    fn verify(&self) -> (bool, serde_json::Value, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg("verify").arg(&self.source)
            .arg("--smt-profile").arg("thorough")
            .arg("--report").arg(&self.report)
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .output().expect("run Aura real CLI");
        let report: serde_json::Value = serde_json::from_slice(
            &fs::read(&self.report).unwrap_or_else(|e| panic!("missing report: {e}; stderr: {}", String::from_utf8_lossy(&out.stderr)))
        ).expect("valid JSON");
        (out.status.success(), report, String::from_utf8_lossy(&out.stderr).to_string())
    }
}
impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); } }

fn check(code: &str, expected: &str) {
    let f = Fixture::new(code);
    let (success, json, stderr) = f.verify();
    assert_eq!(json["schema"], "aura.trusted-core.v2", "wrong report schema: {json}");
    #[cfg(feature = "z3")]
    {
        assert_eq!(json["status"], expected, "source: {code}\nstatus: {json}\nstderr: {stderr}");
        assert_eq!(success, expected == "proved", "exit/report disagreement: {json}; stderr: {stderr}");
        assert_eq!(json["ok"], expected == "proved", "unsound success flag: {json}");
        if expected != "proved" {
            assert!(json.get("verify").is_none() || json["verify"].is_null(), "failed proof must not have successful verification evidence");
        }
    }
    #[cfg(not(feature = "z3"))]
    {
        let _ = expected;
        assert!(!success, "no-Z3 CLI must fail: {code}");
        assert_eq!(json["status"], "not_run", "no solver must never claim proof: {json}; {stderr}");
        assert_eq!(json["ok"], false);
        assert!(json.get("verify").is_none() || json["verify"].is_null());
    }
}

#[test]
fn literal_true_is_proved_and_literal_false_must_fail() {
    check("cell main() ->:\n    assert true\n", "proved");
    check("cell main() ->:\n    assert false\n", "failed");
}

#[test]
fn boolean_negation_must_preserve_truth_values() {
    check("cell main() ->:\n    assert !false\n", "proved");
    check("cell main() ->:\n    assert !true\n", "failed");
}

#[test]
fn boolean_and_or_must_not_turn_false_into_a_proof() {
    check("cell main() ->:\n    assert true && !false\n", "proved");
    check("cell main() ->:\n    assert false || true\n", "proved");
    check("cell main() ->:\n    assert true && false\n", "failed");
    check("cell main() ->:\n    assert false || false\n", "failed");
}

#[test]
fn true_branch_is_reachable_and_false_assertion_is_rejected() {
    check("cell main() ->:\n    if true:\n        assert true\n", "proved");
    check("cell main() ->:\n    if true:\n        assert false\n", "failed");
}

#[test]
fn false_branch_is_unreachable_but_else_branch_is_reachable() {
    check("cell main() ->:\n    if false:\n        assert false\n", "proved");
    check("cell main() ->:\n    if false:\n        assert true\n    else:\n        assert false\n", "failed");
}

#[test]
fn negative_conditional_branch_constraints_are_respected() {
    check("cell main() ->:\n    if !false:\n        assert true\n", "proved");
    check("cell main() ->:\n    if !false:\n        assert false\n", "failed");
}

#[test]
fn typed_boolean_strand_initialization_is_decoded() {
    check("cell main() ->:\n    val b: bool = false\n    assert !b\n", "proved");
    check("cell main() ->:\n    val b: bool = true\n    assert !b\n", "failed");
}
