//! Q03-001 Record function ABI: source-level Record return, by-value parameter,
 //! cross-cell call, and native strict C11 O0/O2 executable cross-check.
 //! This test was introduced while the C backend explicitly rejected Record ABI.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{run_oracle_entry, validate_module, OracleConfig, OracleValue};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
type Point = record { x: u32, y: u32 }
cell make_point(x: u32, y: u32) ->:
    yield Point { y: y, x: x }
cell project_x(p: Point) ->:
    yield p.x
cell round_trip(x: u32, y: u32) ->:
    val p: Point = make_point(x, y)
    yield project_x(p)
"#;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("aura-record-abi-{}-{ns}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

#[test]
fn source_record_return_and_parameter_are_qualified_by_ir_oracle_and_native() {
    let ast = aura_parse::parse_source(SOURCE).expect("source Record functions parse");
    Checker::new().check_program(&ast).expect("source nominal Record call typechecks");
    let ir = lower_program(&ast).expect("source Record call lowers");
    validate_module(&ir).expect("Record function SSA structurally valid");
    for (x, y) in [(7,13), (19,23)] {
        let result = run_oracle_entry(&ir, "round_trip",
            &[OracleValue::U32(x), OracleValue::U32(y)],
            OracleConfig::default()).expect("Oracle executes Record by-value call");
        assert!(result.ok && result.stderr.is_empty());
        assert_eq!(result.return_value, Some(OracleValue::U32(x)));
    }

    let output = emit_module(&ir, None).expect("nominal Record function ABI should emit native C");
    assert!(output.module_c.contains("aura_record_506f696e74"),
        "qualified nominal Point type required, not Tensor fallback");
    let tmp = Scratch::new();
    fs::write(tmp.0.join("aura_runtime.h"), output.runtime_h).unwrap();
    fs::write(tmp.0.join("module.c"), &output.module_c).unwrap();
    // Source all generated definitions in one translation unit to test both
    // the struct-return ABI and the parameter ABI without redefining a type.
    fs::write(tmp.0.join("driver.c"), concat!(
        "#include <stdint.h>\n#include \"module.c\"\n",
        "int main(void) {\n",
        "  aura_record_506f696e74 p = make_point(31u, 37u);\n",
        "  return (p.aura_f_78 == 31u && p.aura_f_79 == 37u",
        " && project_x(p) == 31u",
        " && round_trip(7u, 13u) == 7u",
        " && round_trip(19u, 23u) == 19u) ? 0 : 1;\n",
        "}\n"
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let exe = tmp.0.join(format!("record_abi_{}", &opt[1..]));
        let build = Command::new("clang")
            .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
            .arg(&tmp.0).arg(tmp.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("clang");
        assert!(build.status.success(), "{opt}: {}\n{}", String::from_utf8_lossy(&build.stderr), output.module_c);
        let run = Command::new(exe).output().expect("run native Record ABI");
        assert!(run.status.success(), "{opt}: {}\n{}", String::from_utf8_lossy(&run.stderr), output.module_c);
    }
}
