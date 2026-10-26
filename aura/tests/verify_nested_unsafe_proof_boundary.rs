//! Issue #35: nested unsafe source must not erase executable proof obligations.
//! Real CLI runs with Z3 and no-Z3, checking typed status and trust provenance.
use std::{fs,path::PathBuf,process::Command,time::{SystemTime,UNIX_EPOCH}};
struct Fixture {dir:PathBuf,input:PathBuf,report:PathBuf}
impl Fixture {
    fn new(source:&str)->Self {
        let ns=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir=std::env::temp_dir().join(format!("aura-nested-unsafe-{}-{ns}",std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let input=dir.join("main.aura");
        let report=dir.join("result.json");
        fs::write(&input,source).unwrap();
        Self{dir,input,report}
    }
    fn check(&self)->(bool,serde_json::Value,String){
        let run=Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg("verify").arg(&self.input).arg("--report").arg(&self.report)
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .output().expect("execute Aura CLI");
        let bytes=fs::read(&self.report).unwrap_or_else(|e|panic!("missing report: {e}; stderr={}",String::from_utf8_lossy(&run.stderr)));
        let json:serde_json::Value=serde_json::from_slice(&bytes).expect("valid JSON status report");
        (run.status.success(),json,String::from_utf8_lossy(&run.stderr).into_owned())
    }
}
impl Drop for Fixture{fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}}
fn check(source:&str,expected:&str,reason:&str,unsafe_blocks:usize){
    let f=Fixture::new(source);
    let (success,json,stderr)=f.check();
    assert_eq!(json["schema"],"aura.trusted-core.v2","schema: {json}");
    let trusted=json["trusted"]["unsafe_blocks"].as_array().expect("trust manifest array");
    assert_eq!(trusted.len(),unsafe_blocks,"unsafe provenance: {json}");
    for entry in trusted {assert!(entry["offset"].is_number(),"missing unsafe offset: {json}");}
    #[cfg(feature="z3")]
    {
        assert_eq!(json["status"],expected,"source:\n{source}\nreport={json}\nstderr={stderr}");
        assert_eq!(success,expected=="proved","exit/status mismatch: {json}");
        assert_eq!(json["ok"],expected=="proved","false proof success: {json}");
        if expected=="proved" {assert!(json["verify"].is_object(),"missing proof evidence: {json}");}
        else {
            assert_eq!(json["reason_code"],reason,"wrong refusal vs proof failure: {json}");
            assert!(json["verify"].is_null(),"false proof artifact: {json}");
        }
    }
    #[cfg(not(feature="z3"))]
    {
        let _=(expected,reason);
        assert!(!success,"no-Z3 cannot prove: {source}");
        assert_eq!(json["status"],"not_run","status: {json}; stderr:{stderr}");
        assert_eq!(json["reason_code"],"verifier_unavailable");
        assert_eq!(json["ok"],false);
        assert!(json["verify"].is_null(),"no-solver proof evidence: {json}");
    }
}
#[test]
fn reachable_unsafe_false_assertion_must_not_be_reported_proved(){
    check("cell main() ->:\n    unsafe:\n        assert false\n    assert true\n","unsupported","verification_unsupported",1);
}
#[test]
fn harmless_unsafe_block_is_still_not_proved_without_a_model(){
    check("cell main() ->:\n    unsafe:\n        assert true\n    assert true\n","unsupported","verification_unsupported",1);
}
#[test]
fn conditional_unsafe_false_assertion_must_not_disappear(){
    check("cell main() ->:\n    if true:\n        unsafe:\n            assert false\n    assert true\n","unsupported","verification_unsupported",1);
}
#[test]
fn unsafe_explicit_assumption_needs_an_assurance_boundary(){
    check("cell main() ->:\n    unsafe:\n        assume true\n        assert true\n    assert true\n","unsupported","verification_unsupported",1);
}
#[test]
fn ordinary_provable_assertion_control_remains_proved(){
    check("cell main() ->:\n    assert true\n","proved","",0);
}
#[test]
fn ordinary_false_assertion_control_remains_failed(){
    check("cell main() ->:\n    assert false\n","failed","verification_failed",0);
}
