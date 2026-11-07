//! Q03-001: real Aura Record source p.x = 19 -> Checker -> SSA SetField
//! -> IR Oracle -> actual Clang C11 native O0/O2 executable.
use aura_backend_c::emit_module;
use aura_core::{Checker, lower_program};
use aura_ir::{run_oracle_entry, validate_module, OracleConfig, OracleValue, InstKind};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};
struct Scratch { path: PathBuf }
impl Scratch {
    fn new() -> Self {
        let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-field-source-{}-{ns}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.path); } }
fn native(source: &str, entry: &str, expected: OracleValue, driver: &str) {
    let ast = aura_parse::parse_source(source).expect("Aura field update must parse");
    Checker::new().check_program(&ast).expect("valid mutable source field assignment");
    let ir = lower_program(&ast).expect("source field assignment must lower to IR");
    validate_module(&ir).expect("structural SSA verification");
    let function = ir.functions.get(entry).expect("source cell");
    assert!(function.blocks.iter().flat_map(|b| b.insts.iter()).any(|i|
        matches!(&i.kind, InstKind::SetField { .. }) && i.dest.is_some()),
        "source p.x = value must generate a fresh SSA SetField");
    let oracle = run_oracle_entry(&ir, entry, &[], OracleConfig::default()).expect("IR Oracle");
    assert!(oracle.ok && oracle.stderr.is_empty());
    assert_eq!(oracle.return_value, Some(expected), "source Oracle field result");
    let artifacts = emit_module(&ir, None).expect("source field assignment C output");
    let tmp = Scratch::new();
    fs::write(tmp.path.join("aura_runtime.h"), artifacts.runtime_h).unwrap();
    let generated = tmp.path.join("module.c");
    fs::write(&generated, &artifacts.module_c).unwrap();
    let test_driver = tmp.path.join("driver.c");
    fs::write(&test_driver, driver).unwrap();
    for opt in ["-O0", "-O2"] {
        let executable = tmp.path.join(format!("record_{}", &opt[1..]));
        let c = Command::new("clang").args(["-std=gnu11","-Werror",opt,"-I"])
            .arg(&tmp.path).arg(&generated).arg(&test_driver).arg("-o")
            .arg(&executable).output().expect("clang");
        assert!(c.status.success(), "Native C {opt}: {}\n{}", String::from_utf8_lossy(&c.stderr),artifacts.module_c);
        let run = Command::new(&executable).output().expect("run executable");
        assert!(run.status.success(), "{opt} binary returned {}: {}\n{}",run.status,String::from_utf8_lossy(&run.stderr),artifacts.module_c);
    }
}
#[test]
fn mutable_source_field_update_is_native_and_other_fields_unchanged() {
    let source = r#"
type Point = record { x: u32, y: u32 }
cell change_x() ->:
    val mut p: Point = Point { y: 13, x: 7 }
    p.x = 19
    yield p.x
cell preserve_y() ->:
    val mut p: Point = Point { y: 13, x: 7 }
    p.x = 19
    yield p.y
"#;
    let driver = "#include <stdint.h>\nuint32_t change_x(void);\nuint32_t preserve_y(void);\nint main(void){return change_x()==19u && preserve_y()==13u?0:1;}\n";
    native(source, "change_x", OracleValue::U32(19), driver);
}
#[test]
fn immutable_source_field_assignment_is_rejected_by_checker() {
    let source = "type Point = record { x: u32, y: u32 }\ncell check() ->:\n    val p: Point = Point { x: 7, y: 13 }\n    p.x = 19\n    yield p.x\n";
    let ast = aura_parse::parse_source(source).expect("assignment must parse");
    let e = Checker::new().check_program(&ast).expect_err("immutable Record binding cannot change");
    assert!(e.message.contains("immutable") && e.message.contains("p"),"specific mutable binding diagnostic {e}");
}
#[test]
fn unknown_source_record_field_assignment_is_rejected_by_checker() {
    let source = "type Point = record { x: u32, y: u32 }\ncell check() ->:\n    val mut p: Point = Point { x: 7, y: 13 }\n    p.ghost = 19\n    yield p.x\n";
    let ast = aura_parse::parse_source(source).expect("assignment must parse");
    let e = Checker::new().check_program(&ast).expect_err("unknown Record field cannot change");
    assert!(e.message.contains("unknown field") && e.message.contains("ghost"),"specific missing field diagnostic {e}");
}
