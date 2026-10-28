//! Q03-001: omitted Scalar Record field defaults are real runtime values, not
//! merely a Checker permission. Source -> Checker -> SSA -> Oracle -> native C.
use aura_backend_c::emit_module;
use aura_core::{Checker, lower_program};
use aura_ir::{run_oracle_entry, validate_module, OracleConfig, OracleValue};
use std::{collections::HashMap, fs, path::PathBuf, process::Command,
    time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
type Flag = record { enabled: bool = true, count: u32 = 17 }
cell flag_enabled() ->:
    val f: Flag = Flag { count: 19 }
    yield f.enabled
cell make_point() ->:
    yield Point {}
cell override_x() ->:
    yield Point { x: 19 }
cell project_default() ->:
    val p: Point = Point { x: 23 }
    yield p.y
"#;
fn point(x: u32, y: u32) -> OracleValue {
    OracleValue::Record { name: "Point".into(),
        fields: HashMap::from([("x".into(), OracleValue::U32(x)),
                               ("y".into(), OracleValue::U32(y))]) }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let n=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path=std::env::temp_dir().join(format!("aura-record-defaults-{}-{n}",std::process::id()));
        fs::create_dir_all(&path).unwrap();Self(path)
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _=fs::remove_dir_all(&self.0); } }
#[test]
fn omitted_scalar_defaults_are_qualified_in_oracle_and_native_c_abi() {
    let ast=aura_parse::parse_source(SOURCE).expect("default source parses");
    Checker::new().check_program(&ast).expect("source defaults are legal Aura");
    let ir=lower_program(&ast).expect("default source lowers");
    validate_module(&ir).expect("default SSA validated");
    for (name, expected) in [
        ("make_point",point(7,13)),
        ("override_x",point(19,13)),
        ("project_default",OracleValue::U32(13)),
        ("flag_enabled",OracleValue::Bool(true)),
    ] {
        let obs=run_oracle_entry(&ir,name,&[],OracleConfig::default())
            .expect("Oracle executes omitted defaults");
        assert!(obs.ok && obs.stderr.is_empty());
        assert_eq!(obs.return_value,Some(expected), "{name}: defaults must be materialized");
    }
    let artifacts=emit_module(&ir,None).expect("native omitted defaults must emit");
    let tmp=Scratch::new();
    fs::write(tmp.0.join("aura_runtime.h"),artifacts.runtime_h).unwrap();
    fs::write(tmp.0.join("module.c"),&artifacts.module_c).unwrap();
    fs::write(tmp.0.join("driver.c"),concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
        "aura_record_506f696e74 make_point(void);\n",
        "aura_record_506f696e74 override_x(void);\n",
        "uint32_t project_default(void);\n",
        "bool flag_enabled(void);\n",
        "int main(void){aura_record_506f696e74 a=make_point(),b=override_x();\n",
        "return (a.aura_f_78==7u&&a.aura_f_79==13u",
        "&&b.aura_f_78==19u&&b.aura_f_79==13u",
        "&&project_default()==13u&&flag_enabled())?0:1;}\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=tmp.0.join(format!("record_default_{}",&opt[1..]));
        let compile=Command::new("clang").args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&tmp.0).arg(tmp.0.join("module.c")).arg(tmp.0.join("driver.c"))
            .arg("-o").arg(&exe).output().unwrap();
        assert!(compile.status.success(),"{opt}: {}\n{}",String::from_utf8_lossy(&compile.stderr),artifacts.module_c);
        let run=Command::new(&exe).output().unwrap();
        assert!(run.status.success(),"{opt} failed: {}\n{}",String::from_utf8_lossy(&run.stderr),artifacts.module_c);
    }
}


#[test]
fn wrong_typed_omitted_default_is_rejected_by_checker_before_codegen() {
    let source=r#"
type Point = record { x: u32 = true, y: u32 = 13 }
cell bad() ->:
    yield Point { y: 9 }
"#;
    let ast=aura_parse::parse_source(source).expect("well formed default syntax");
    let e=Checker::new().check_program(&ast)
        .expect_err("default bool must not flow into u32 field");
    assert!(e.message.contains("type") || e.message.contains("assign"),
        "expected typed omitted-default diagnostic, got {e}");
}

#[test]
fn omitted_nonliteral_default_is_refused_until_qualified() {
    let source=r#"
type Point = record { x: u32 = 1 + 2, y: u32 = 13 }
cell bad() ->:
    yield Point { y: 9 }
"#;
    let ast=aura_parse::parse_source(source).expect("well formed default expression");
    let e=Checker::new().check_program(&ast)
        .expect_err("unsupported nonliteral default cannot be lowered by a partial ABI");
    assert!(e.message.contains("default") && e.message.contains("unqualified"),
        "expected explicit fail-closed default diagnostic, got {e}");
}


#[test]
fn malformed_record_defaults_are_rejected_at_declaration_even_when_unused_or_overridden() {
    let samples = [
        (
            "type Unused = record { x: u32 = true, y: u32 }\ncell main() ->:\n    yield 7\n",
            "type",
        ),
        (
            "type Point = record { x: u32 = 1 + 2, y: u32 }\ncell main() ->:\n    val p: Point = Point { x: 7, y: 13 }\n    yield p.x\n",
            "unqualified",
        ),
        (
            "type Point = record { x: u32 = 7, x: u32 = 13 }\ncell main() ->:\n    yield 7\n",
            "duplicate",
        ),
    ];
    for (source, fragment) in samples {
        let parsed = aura_parse::parse_source(source)
            .expect("malformed declaration's source syntax must still parse");
        let error = Checker::new().check_program(&parsed)
            .expect_err("invalid Record declaration must not depend on use sites");
        assert!(error.message.contains(fragment),
            "expected {fragment:?} declaration diagnostic, got {error}");
        assert!(error.span.len() > 0, "declaration errors must retain a source span");
    }
}
