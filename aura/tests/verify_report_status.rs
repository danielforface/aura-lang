//! AURA-Q01-002/006: real CLI report status/legacy-ok consistency.
use std::{path::{Path,PathBuf},process::{Command,Output},time::{SystemTime,UNIX_EPOCH}};

struct Fixture { dir: PathBuf, source: PathBuf }
impl Fixture {
    fn new(code: &str) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
        let dir = std::env::temp_dir().join(format!("aura-report-v2-{}-{nanos}",std::process::id()));
        std::fs::create_dir_all(&dir).expect("make fixture");
        let source = dir.join("main.aura");
        std::fs::write(&source, code).expect("write source");
        Self {dir,source}
    }
    fn verify_report(&self) -> (Output,serde_json::Value) {
        let report = self.dir.join("report.json");
        let output = Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg("verify").arg(&self.source).arg("--report").arg(&report)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace"))
            .output().expect("run real CLI");
        let json: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&report).expect("verification command should write JSON report")
        ).expect("valid JSON");
        (output,json)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); }
}
fn assert_failed(output:&Output,report:&serde_json::Value,status:&str,reason:&str) {
    assert!(!output.status.success(),"failure must exit nonzero; stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr));
    assert_eq!(report["schema"],"aura.trusted-core.v2");
    assert_eq!(report["status"],status);
    assert_eq!(report["reason_code"],reason);
    assert_eq!(report["ok"],false);
    assert!(report["verify"].is_null(),"non-proved report must not contain proof evidence");
    assert!(report["error"].as_str().is_some_and(|s| !s.is_empty()),"failure error must be present");
}
#[cfg(not(feature="z3"))]
#[test]
fn no_verifier_is_not_run_in_report() {
    let f=Fixture::new("cell main() ->:\n    assert false\n");
    let (out,json)=f.verify_report();
    assert_failed(&out,&json,"not_run","verifier_unavailable");
}
#[cfg(not(feature="z3"))]
#[test]
fn malformed_source_is_invalid_input_even_without_solver() {
    let f=Fixture::new("cell main( ->:\n    assert false\n");
    let (out,json)=f.verify_report();
    assert_failed(&out,&json,"invalid_input","parse_error");
}
#[cfg(feature="z3")]
#[test]
fn false_assertion_is_failed_not_automatically_counterexample() {
    let f=Fixture::new("cell main() ->:\n    val x: u32 = 1\n    assert x == 2\n");
    let (out,json)=f.verify_report();
    assert_failed(&out,&json,"failed","verification_failed");
}
#[cfg(feature="z3")]
#[test]
fn successful_proof_has_explicit_status_and_evidence() {
    let f=Fixture::new("cell main() ->:\n    val x: u32 = 1\n    assert x == 1\n");
    let (out,json)=f.verify_report();
    assert!(out.status.success(),"true assertion must verify; stderr={}",
        String::from_utf8_lossy(&out.stderr));
    assert_eq!(json["schema"],"aura.trusted-core.v2");
    assert_eq!(json["status"],"proved");
    assert_eq!(json["ok"],true);
    assert!(json["error"].is_null());
    assert!(json["reason_code"].is_null());
    assert!(json["verify"].is_object(),"success requires actual verification evidence");
}
