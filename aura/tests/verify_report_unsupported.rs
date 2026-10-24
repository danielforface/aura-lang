//! AURA-Q01-005: a refused SMT feature is neither a proved nor a refuted property.
#![cfg(feature = "z3")]
use std::{fs,path::PathBuf,process::{Command,Output},time::{SystemTime,UNIX_EPOCH}};
struct Fixture {dir:PathBuf,source:PathBuf}
impl Fixture{
  fn new(code:&str)->Self{
    let nanos=SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    let dir=std::env::temp_dir().join(format!("aura-unsupported-{}-{nanos}",std::process::id()));
    fs::create_dir_all(&dir).expect("fixture dir");
    let source=dir.join("main.aura");fs::write(&source,code).expect("source");
    Self{dir,source}
  }
  fn verify(&self,profile:&str)->(Output,serde_json::Value){
    let out=self.dir.join(format!("{profile}.json"));
    let run=Command::new(env!("CARGO_BIN_EXE_aura"))
        .arg("verify").arg(&self.source).args(["--smt-profile",profile])
        .arg("--report").arg(&out)
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace"))
        .output().expect("execute Aura");
    let json=serde_json::from_slice(&fs::read(&out).expect("status report")).expect("json");
    (run,json)
  }
}
impl Drop for Fixture {fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}}
const FORALL:&str="cell main() ->:\n    assert forall(x: u32): x == x\n";
#[test]
fn quantifier_refused_by_fast_profile_is_explicit_unsupported(){
  let f=Fixture::new(FORALL);
  let (run,json)=f.verify("fast");
  assert!(!run.status.success(),"restricted quantifier must not be accepted");
  assert_eq!(json["schema"],"aura.trusted-core.v2");
  assert_eq!(json["status"],"unsupported","must not report failed proof or counterexample; stderr={}",String::from_utf8_lossy(&run.stderr));
  assert_eq!(json["reason_code"],"verification_unsupported");
  assert_eq!(json["ok"],false);
  assert!(json["verify"].is_null(),"unproven formula cannot have proof evidence");
}
#[test]
fn same_quantifier_can_be_proved_under_thorough_profile(){
  let f=Fixture::new(FORALL);
  let (run,json)=f.verify("thorough");
  assert!(run.status.success(),"tautology should verify in Thorough; stdout={} stderr={}",
    String::from_utf8_lossy(&run.stdout),String::from_utf8_lossy(&run.stderr));
  assert_eq!(json["status"],"proved");
  assert_eq!(json["ok"],true);
  assert!(json["verify"].is_object());
}
