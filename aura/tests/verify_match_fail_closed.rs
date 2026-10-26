//! Issue #31 / AURA-Q01-003/005/008: match-arm proof obligations must not vanish.
//! Real CLI regression. The verifier currently does not model match branches.
//! The safe interim result is typed UNSUPPORTED; false proofs are unacceptable.
use std::{fs,path::PathBuf,process::Command,time::{SystemTime,UNIX_EPOCH}};

struct Fixture { dir:PathBuf,source:PathBuf,report:PathBuf }
impl Fixture {
    fn new(code:&str)->Self {
        let nanos=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!("aura-match-proof-{}-{nanos}",std::process::id()));
        fs::create_dir_all(&dir).expect("fixture dir");
        let source=dir.join("main.aura");
        fs::write(&source,code).expect("source");
        let report=dir.join("result.json");
        Self{dir,source,report}
    }
    fn verify(&self)->(bool,serde_json::Value,String){
        let run=Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg("verify").arg(&self.source).arg("--report").arg(&self.report)
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace"))
            .output().expect("real Aura CLI");
        let report:serde_json::Value=serde_json::from_slice(
            &fs::read(&self.report).unwrap_or_else(|e|panic!("missing report: {e}; stderr={}",String::from_utf8_lossy(&run.stderr)))
        ).expect("JSON");
        (run.status.success(),report,String::from_utf8_lossy(&run.stderr).into_owned())
    }
}
impl Drop for Fixture {fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}}

fn assert_cli(code:&str,with_solver_status:&str,reason:&str) {
    let f=Fixture::new(code);
    let (success,json,stderr)=f.verify();
    assert_eq!(json["schema"],"aura.trusted-core.v2","incorrect schema: {json}");
    #[cfg(feature="z3")]
    {
        assert_eq!(json["status"],with_solver_status,"source:\n{code}\nreport={json}\nstderr={stderr}");
        assert_eq!(success,with_solver_status=="proved","exit/report disagreement: {json}");
        assert_eq!(json["ok"],with_solver_status=="proved","unsafe ok state: {json}");
        if with_solver_status!="proved" {
            assert_eq!(json["reason_code"],reason,"wrong failure classification: {json}");
            assert!(json["verify"].is_null(),"unproven match has proof evidence: {json}");
        } else {
            assert!(json["verify"].is_object(),"successful proof requires evidence: {json}");
        }
    }
    #[cfg(not(feature="z3"))]
    {
        let _=(with_solver_status,reason);
        assert!(!success,"no solver must never prove: {code}");
        assert_eq!(json["status"],"not_run","no-Z3 report: {json}; stderr={stderr}");
        assert_eq!(json["reason_code"],"verifier_unavailable");
        assert_eq!(json["ok"],false);
        assert!(json["verify"].is_null(),"cannot have proof without Z3");
    }
}
const REACHABLE_FALSE:&str="cell main() ->:\n    val x: u32 = 1\n    match x:\n        1:\n            assert false\n        _:\n            assert true\n    assert true\n";
const POSITIVE_MATCH:&str="cell main() ->:\n    val x: u32 = 1\n    match x:\n        1:\n            assert true\n        _:\n            assert true\n    assert true\n";
const NESTED_MATCH:&str="cell main() ->:\n    if true:\n        match 1:\n            1:\n                assert false\n            _:\n                assert true\n    assert true\n";

#[test]
fn reachable_false_assertion_in_match_must_not_disappear(){
    assert_cli(REACHABLE_FALSE,"unsupported","verification_unsupported");
}
#[test]
fn harmless_match_must_not_claim_a_proof_without_match_modeling(){
    assert_cli(POSITIVE_MATCH,"unsupported","verification_unsupported");
}
#[test]
fn nested_match_must_not_be_skipped_under_if_path(){
    assert_cli(NESTED_MATCH,"unsupported","verification_unsupported");
}
#[test]
fn normal_boolean_proofs_remain_supported(){
    assert_cli("cell main() ->:\n    assert true\n    assert 1 == 1\n","proved","");
}
#[test]
fn normal_false_assertion_remains_failed_not_unsupported(){
    assert_cli("cell main() ->:\n    assert false\n","failed","verification_failed");
}
