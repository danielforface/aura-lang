//! Issue #20: report destinations must never preserve false-success results.
//! Real aura verify --report process tests, no Z3 feature required.
use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, time::{SystemTime, UNIX_EPOCH}};
struct Fixture { dir: PathBuf, src: PathBuf, report: PathBuf }
impl Fixture {
    fn new(source: &str) -> Self {
        let nanos=SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let dir=std::env::temp_dir().join(format!("aura-report-durability-{}-{nanos}",std::process::id()));
        fs::create_dir_all(&dir).expect("mkdir");
        let src=dir.join("main.aura");
        fs::write(&src,source).expect("write Aura source");
        let report=dir.join("verification.json");
        Self{dir,src,report}
    }
    fn verify(&self,source:&Path,out:&Path)->Output{
        Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg("verify").arg(source).arg("--report").arg(out)
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace"))
            .output().expect("run real Aura CLI")
    }
}
impl Drop for Fixture {
    fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}
}
const OLD_PROOF: &str = r#"{"schema":"aura.trusted-core.v2","status":"proved","ok":true,"verify":{"proofs":[]}}"#;
#[test]
fn missing_source_must_replace_stale_proved_report() {
    let f=Fixture::new("cell main() ->:\n    assert true\n");
    fs::write(&f.report,OLD_PROOF).expect("prior report");
    let missing=f.dir.join("deleted.aura");
    let out=f.verify(&missing,&f.report);
    assert!(!out.status.success(),"input missing must fail");
    let json:serde_json::Value=serde_json::from_slice(&fs::read(&f.report).expect("failure report exists")).expect("valid failure JSON");
    assert_eq!(json["ok"],false,"failed read cannot leave prior ok:true result");
    assert_eq!(json["status"],"invalid_input");
    assert_eq!(json["reason_code"],"io_error");
}
#[test]
fn invalid_source_replaces_previous_success() {
    let f=Fixture::new("cell main( ->:\n    assert true\n");
    fs::write(&f.report,OLD_PROOF).expect("prior report");
    let out=f.verify(&f.src,&f.report);
    assert!(!out.status.success(),"bad syntax must fail");
    let json:serde_json::Value=serde_json::from_slice(&fs::read(&f.report).expect("failure report")).expect("JSON");
    assert_eq!(json["ok"],false);
    assert_eq!(json["status"],"invalid_input");
}
#[test]
fn directory_as_report_destination_must_not_hide_write_error() {
    let f=Fixture::new("cell main( ->:\n");
    let report_dir=f.dir.join("destination-is-directory");
    fs::create_dir_all(&report_dir).expect("mkdir");
    let out=f.verify(&f.src,&report_dir);
    assert!(!out.status.success());
    let diagnostic=String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        diagnostic.contains("report") && (diagnostic.contains("destination")||diagnostic.contains("directory")||diagnostic.contains("write")),
        "failure must expose report output issue, not silently swallow it: {diagnostic}"
    );
}
#[test]
fn report_destination_must_not_destroy_its_input_source() {
    let code="cell main() ->:\n    assert true\n";
    let f=Fixture::new(code);
    let out=f.verify(&f.src,&f.src);
    assert!(!out.status.success(),"same source/report destination must be rejected");
    assert_eq!(fs::read_to_string(&f.src).expect("source should remain"),code);
    let diagnostic=String::from_utf8_lossy(&out.stderr).to_ascii_lowercase();
    assert!(diagnostic.contains("report") && diagnostic.contains("source"),
        "rejection must explain invalid report destination: {diagnostic}");
}

#[cfg(unix)]
#[test]
fn symlink_alias_to_source_must_not_destroy_input() {
    let code = "cell main() ->:\n    assert true\n";
    let f = Fixture::new(code);
    let alias=f.dir.join("source-alias.aura");
    std::os::unix::fs::symlink(&f.src,&alias).expect("make alias");
    let out=f.verify(&f.src,&alias);
    assert!(!out.status.success(),"alias report target must be rejected");
    assert_eq!(fs::read_to_string(&f.src).expect("source still exists"),code);
    let diagnostic=String::from_utf8_lossy(&out.stderr).to_ascii_lowercase();
    assert!(diagnostic.contains("report") && diagnostic.contains("source"),
        "explain unsafe alias: {diagnostic}");
}
