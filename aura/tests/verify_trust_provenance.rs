//! AURA-Q01-007: trust assumptions and preconditions must be observable
//! independently from proof success and cannot vanish inside cell bodies.
use std::{fs, path::PathBuf, process::{Command,Output}, time::{SystemTime,UNIX_EPOCH}};
struct Fixture {dir:PathBuf,source:PathBuf,report:PathBuf}
impl Fixture {
    fn new(code:&str)->Self{
        let nanos=SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        let dir=std::env::temp_dir().join(format!("aura-trust-origin-{}-{nanos}",std::process::id()));
        fs::create_dir_all(&dir).expect("mkdir");
        let source=dir.join("main.aura");
        fs::write(&source,code).expect("write source");
        let report=dir.join("audit.json");
        Self{dir,source,report}
    }
    fn run(&self)->(Output,serde_json::Value){
        let out=Command::new(env!("CARGO_BIN_EXE_aura"))
            .arg("verify").arg(&self.source).arg("--report").arg(&self.report)
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace"))
            .output().expect("run real Aura CLI");
        let bytes=fs::read(&self.report).unwrap_or_else(|e| panic!("missing report: {e}; stderr={}",String::from_utf8_lossy(&out.stderr)));
        let json:serde_json::Value=serde_json::from_slice(&bytes).expect("valid report JSON");
        (out,json)
    }
}
impl Drop for Fixture{fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}}
fn assert_spans(report:&serde_json::Value,key:&str,count:usize){
    let spans=report["trusted"][key].as_array().unwrap_or_else(||panic!("missing trusted.{key}: {report}"));
    assert_eq!(spans.len(),count,"wrong trusted.{key}: {report}");
    for s in spans {
        assert!(s.get("offset").is_some(),"trust entry missing source offset: {s}");
    }
}
#[test]
fn explicit_assume_is_separate_from_requires_even_without_a_solver(){
    let f=Fixture::new("cell main() ->:\n    assume 1 == 1\n    assert 1 == 1\n");
    let (out,json)=f.run();
    #[cfg(not(feature = "z3"))]
    assert_ne!(json["status"],"proved","no-solver report must never claim proof");
    assert_spans(&json,"explicit_assumes",1);
    assert_spans(&json,"preconditions",0);
    let origins=json["trusted"]["assumption_origins"].as_array().expect("typed origin entries");
    assert_eq!(origins.len(),1);
    assert_eq!(origins[0]["kind"],"assume");
    assert_eq!(origins[0]["cell"],"main");
    assert!(origins[0]["span"]["offset"].is_number());
    assert!(out.status.success() == (json["status"]=="proved"),"CLI and report truthfulness must agree");
}
#[test]
fn contract_requires_is_not_mislabeled_user_assume(){
    let f=Fixture::new("cell main() ->:\n    requires 1 == 1\n    assert 1 == 1\n");
    let (_,json)=f.run();
    assert_spans(&json,"preconditions",1);
    assert_spans(&json,"explicit_assumes",0);
    let origins=json["trusted"]["assumption_origins"].as_array().expect("typed origin entries");
    assert_eq!(origins.len(),1);
    assert_eq!(origins[0]["kind"],"requires");
    assert_eq!(origins[0]["cell"],"main");
}
#[test]
fn nested_unsafe_block_inside_cell_is_never_omitted(){
    let f=Fixture::new("cell main() ->:\n    unsafe:\n        assert 1 == 1\n");
    let (_,json)=f.run();
    assert_spans(&json,"unsafe_blocks",1);
}
#[test]
fn nested_assume_inside_conditional_is_counted(){
    let f=Fixture::new("cell main() ->:\n    if 1 == 1:\n        assume 1 == 1\n    assert 1 == 1\n");
    let (_,json)=f.run();
    assert_spans(&json,"explicit_assumes",1);
}

#[cfg(feature = "z3")]
#[test]
fn known_bug_boolean_literal_if_true_should_verify(){
    let f=Fixture::new("cell main() ->:\n    if true:\n        assume 1 == 1\n    assert 1 == 1\n");
    let (out,json)=f.run();
    assert!(out.status.success(),"Issue #28 must permit boolean literals in verifier if");
    assert_eq!(json["status"],"proved");
    assert_spans(&json,"explicit_assumes",1);
}
