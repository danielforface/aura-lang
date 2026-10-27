//! Issue #37: source-valid nested property statements cannot be silently
//! discarded from verifier traversal. Test FIRST against the unpatched dispatcher.
use std::{fs,path::PathBuf,process::Command,time::{SystemTime,UNIX_EPOCH}};
struct Fixture{root:PathBuf,source:PathBuf,report:PathBuf}
impl Fixture{
 fn new(text:&str)->Self{
  let tick=SystemTime::now().duration_since(UNIX_EPOCH).expect("time").as_nanos();
  let root=std::env::temp_dir().join(format!("aura-stmt-exhaustive-{}-{tick}",std::process::id()));
  fs::create_dir_all(&root).expect("fixture dir");
  let source=root.join("main.aura");let report=root.join("result.json");
  fs::write(&source,text).expect("fixture source");
  Self{root,source,report}
 }
 fn run(&self)->(bool,serde_json::Value,String){
  let out=Command::new(env!("CARGO_BIN_EXE_aura")).arg("verify").arg(&self.source)
   .arg("--report").arg(&self.report)
   .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace"))
   .output().expect("execute actual Aura CLI");
  let bytes=fs::read(&self.report).unwrap_or_else(|e|panic!("report missing: {e}; stderr {}",String::from_utf8_lossy(&out.stderr)));
  let json:serde_json::Value=serde_json::from_slice(&bytes).expect("JSON report");
  (out.status.success(),json,String::from_utf8_lossy(&out.stderr).into_owned())
 }
}
impl Drop for Fixture{fn drop(&mut self){let _=fs::remove_dir_all(&self.root);}}
fn check(source:&str,expected:&str,reason:&str){
 let f=Fixture::new(source);let (success,j,stderr)=f.run();
 assert_eq!(j["schema"],"aura.trusted-core.v2","schema: {j}");
 #[cfg(feature="z3")]{
  assert_eq!(j["status"],expected,"source:\n{source}\nreport={j}\nstderr={stderr}");
  assert_eq!(success,expected=="proved","exit/status discrepancy: {j}");
  assert_eq!(j["ok"],expected=="proved","false verified outcome: {j}");
  if expected=="proved"{assert!(j["verify"].is_object(),"positive proof missing: {j}");}
  else {assert_eq!(j["reason_code"],reason,"wrong failure boundary: {j}");assert!(j["verify"].is_null(),"false proof artifact: {j}");}
 }
 #[cfg(not(feature="z3"))]{
  let _=(expected,reason);
  assert!(!success,"missing solver cannot claim proof: {source}");
  assert_eq!(j["status"],"not_run","unexpected status: {j}; stderr:{stderr}");
  assert_eq!(j["reason_code"],"verifier_unavailable");
  assert_eq!(j["ok"],false);
  assert!(j["verify"].is_null(),"no-Z3 proof artifact: {j}");
 }
}
#[test]
fn nested_property_literal_is_not_an_implicit_inert_declaration(){
 check("cell main() ->:\n    audit: true\n    assert true\n","unsupported","verification_unsupported");
}
#[test]
fn nested_property_under_reachable_if_must_not_vanish(){
 check("cell main() ->:\n    if true:\n        audit: false\n    assert true\n","unsupported","verification_unsupported");
}
#[test]
fn nested_property_with_dynamic_expression_needs_explicit_model(){
 check("cell main() ->:\n    val b: bool = true\n    audit: b\n    assert true\n","unsupported","verification_unsupported");
}
#[test]
fn property_followed_by_unprovable_assertion_must_not_prove(){
 check("cell main() ->:\n    audit: true\n    assert false\n","unsupported","verification_unsupported");
}
#[test]
fn ordinary_provable_control_still_proves(){
 check("cell main() ->:\n    assert true\n","proved","");
}
#[test]
fn ordinary_false_assertion_control_still_fails(){
 check("cell main() ->:\n    assert false\n","failed","verification_failed");
}
#[test]
fn already_qualified_nested_unsafe_remains_explicitly_unsupported(){
 check("cell main() ->:\n    unsafe:\n        assert false\n","unsupported","verification_unsupported");
}
#[test]
fn already_qualified_nested_match_remains_explicitly_unsupported(){
 check("cell main() ->:\n    match 1:\n        1:\n            assert false\n        _:\n            assert true\n","unsupported","verification_unsupported");
}
