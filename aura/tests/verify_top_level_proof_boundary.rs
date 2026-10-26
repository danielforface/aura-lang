//! Issue #33: top-level executable statements must not disappear from proof obligations.
//! Real Aura CLI tests, both real Z3 and no-Z3 configurations.
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};
struct Fixture { root: PathBuf, input: PathBuf, report: PathBuf }
impl Fixture {
    fn new(source: &str) -> Self {
        let id=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("aura-top-level-proof-{}-{id}",std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let input=root.join("main.aura");
        let report=root.join("proof.json");
        fs::write(&input,source).unwrap();
        Self {root,input,report}
    }
    fn verify(&self)->(bool,serde_json::Value,String){
        let out=Command::new(env!("CARGO_BIN_EXE_aura"))
          .arg("verify").arg(&self.input)
          .arg("--smt-profile").arg("thorough")
          .arg("--report").arg(&self.report)
          .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
          .output().expect("run Aura CLI");
        let content=fs::read(&self.report)
            .unwrap_or_else(|e|panic!("missing report: {e}; stderr={}",String::from_utf8_lossy(&out.stderr)));
        let json:serde_json::Value=serde_json::from_slice(&content).expect("typed JSON report");
        (out.status.success(),json,String::from_utf8_lossy(&out.stderr).into_owned())
    }
}
impl Drop for Fixture { fn drop(&mut self) {let _=fs::remove_dir_all(&self.root);} }
fn check(source:&str,expected_status:&str,reason:&str){
    let f=Fixture::new(source);
    let (success,j,stderr)=f.verify();
    assert_eq!(j["schema"],"aura.trusted-core.v2","schema: {j}");
    #[cfg(feature="z3")]
    {
        assert_eq!(j["status"],expected_status,"source:\n{source}\nreport:{j}\nstderr:{stderr}");
        assert_eq!(success,expected_status=="proved","exit/report mismatch: {j}");
        assert_eq!(j["ok"],expected_status=="proved","false success: {j}");
        if expected_status=="proved" { assert!(j["verify"].is_object(),"missing proof evidence: {j}"); }
        else {
            assert_eq!(j["reason_code"],reason,"incorrect failure kind: {j}");
            assert!(j["verify"].is_null(),"false proof artifact: {j}");
        }
    }
    #[cfg(not(feature="z3"))] {
        let _=(expected_status,reason);
        assert!(!success,"no solver available: {source}");
        assert_eq!(j["status"],"not_run","report: {j}, stderr:{stderr}");
        assert_eq!(j["reason_code"],"verifier_unavailable");
        assert_eq!(j["ok"],false);
        assert!(j["verify"].is_null(),"fabricated proof: {j}");
    }
}
#[test]
fn top_level_false_assertion_must_not_be_reported_proved(){
    check("assert false\ncell main() ->:\n    assert true\n","failed","verification_failed");
}
#[test]
fn top_level_true_assertion_should_be_checked_not_silent(){
    check("assert true\ncell main() ->:\n    assert true\n","proved","");
}
#[test]
fn top_level_if_reachable_false_assertion_must_not_be_discarded(){
    check("if true:\n    assert false\ncell main() ->:\n    assert true\n","failed","verification_failed");
}
#[test]
fn top_level_match_is_unsupported_until_modeled(){
    check("match 1:\n    1:\n        assert false\n    _:\n        assert true\ncell main() ->:\n    assert true\n","unsupported","verification_unsupported");
}
#[test]
fn top_level_assumption_cannot_silently_drop_trust_source(){
    check("assume true\ncell main() ->:\n    assert true\n","unsupported","verification_unsupported");
}
#[test]
fn top_level_global_initialization_must_not_be_ignored(){
    check("val x: u32 = 1\ncell main() ->:\n    assert true\n","unsupported","verification_unsupported");
}
#[test]
fn ordinary_cell_false_control_still_fails(){
    check("cell main() ->:\n    assert false\n","failed","verification_failed");
}
#[test]
fn ordinary_cell_true_control_still_proves(){
    check("cell main() ->:\n    assert true\n","proved","");
}
