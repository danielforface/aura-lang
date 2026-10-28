//! Q03-001: omitted Scalar Record field defaults are real runtime values, not
//! merely a Checker permission. Source -> Checker -> SSA -> Oracle -> native C.
use aura_backend_c::emit_module;
use aura_core::{Checker, lower_program};
use aura_ir::{run_oracle_entry, validate_module, OracleConfig, OracleValue};
use std::{collections::HashMap, fs, path::PathBuf, process::Command,
    time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
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
        "#include <stdint.h>\n",
        "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
        "aura_record_506f696e74 make_point(void);\n",
        "aura_record_506f696e74 override_x(void);\n",
        "uint32_t project_default(void);\n",
        "int main(void){aura_record_506f696e74 a=make_point(),b=override_x();\n",
        "return (a.aura_f_78==7u&&a.aura_f_79==13u",
        "&&b.aura_f_78==19u&&b.aura_f_79==13u",
        "&&project_default()==13u)?0:1;}\n"
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
