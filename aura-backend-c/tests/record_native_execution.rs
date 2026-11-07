//! AURA-Q03-001: real source -> typed Record IR -> emitted C -> Clang
//! compilation and actual native execution at both O0 and O2.
//! TEST FIRST. These fixtures are intentionally unsupported at baseline.
//! Do not change assertions into error expectations to make this green.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::validate_module;
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

struct Scratch { path: PathBuf }
impl Scratch {
    fn new() -> Self {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-record-native-{}-{time}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.path); }
}
fn check_native(source: &str, driver: &str) {
    let ast = aura_parse::parse_source(source).expect("Aura Record source must parse");
    Checker::new().check_program(&ast).expect("Aura source must be semantically valid");
    let ir = lower_program(&ast).expect("Aura source must lower to actual IR");
    validate_module(&ir).expect("source-generated IR must structurally validate");
    let artifacts = emit_module(&ir, None).expect("typed Record source must emit valid C");
    assert!(artifacts.module_c.contains("aura_record_") && artifacts.module_c.contains("aura_f_"),
        "real typed Record field access and named struct must be emitted: {}", artifacts.module_c);
    let temp = Scratch::new();
    fs::write(temp.path.join("aura_runtime.h"), artifacts.runtime_h).unwrap();
    let generated_source = temp.path.join("module.c");
    let harness = temp.path.join("driver.c");
    fs::write(&generated_source, &artifacts.module_c).unwrap();
    fs::write(&harness, driver).unwrap();
    for opt in ["-O0", "-O2"] {
        let binary = temp.path.join(format!("record_{}", &opt[1..]));
        let compilation = Command::new("clang")
            .args(["-std=gnu11", opt, "-Werror", "-I"]).arg(&temp.path)
            .arg(&generated_source).arg(&harness).arg("-o").arg(&binary)
            .output().expect("clang present in CI");
        assert!(compilation.status.success(),
            "{opt} actual emitted C must compile, stderr={}\nC={}",
            String::from_utf8_lossy(&compilation.stderr), artifacts.module_c);
        let output = Command::new(&binary).output().expect("execute compiled Record");
        assert!(output.status.success(),
            "{opt} actual Record values wrong, status={:?}, stderr={}\nC={}",
            output.status, String::from_utf8_lossy(&output.stderr), artifacts.module_c);
    }
}

#[test]
fn source_record_reordered_initializers_preserve_named_field_values_in_native_c() {
    let source = r#"
type Point = record {
    x: u32,
    y: u32,
}
cell point_x() ->:
    val p: Point = Point { y: 13, x: 7 }
    yield p.x
cell point_y() ->:
    val p: Point = Point { x: 7, y: 13 }
    yield p.y
"#;
    let driver = "#include <stdint.h>\nuint32_t point_x(void);\nuint32_t point_y(void);\nint main(void) { return point_x() == 7u && point_y() == 13u ? 0 : 1; }\n";
    check_native(source, driver);
    // Same parsed program must report the same scalar values in the IR
    // oracle as the -O0/-O2 native executables above.
    let ast = aura_parse::parse_source(source).expect("Record source parses twice");
    let ir = lower_program(&ast).expect("Record source lowers for Oracle");
    for (function, expected) in [("point_x", 7u32), ("point_y", 13u32)] {
        let observation = aura_ir::run_oracle_entry(
            &ir, function, &[], aura_ir::OracleConfig::default()
        ).expect("IR Oracle executes source Record field");
        assert!(observation.ok && observation.stderr.is_empty());
        assert_eq!(observation.return_value, Some(aura_ir::OracleValue::U32(expected)),
            "real native result and IR Oracle field semantics differ for {function}");
    }
}

#[test]
fn source_record_mixed_u64_and_bool_keeps_full_native_scalar_width() {
    let source = r#"
type Wide = record {
    x: u64,
    active: bool,
}
cell wide_x(x: u64) ->:
    val p: Wide = Wide { active: true, x: x }
    yield p.x
"#;
    let driver = "#include <stdint.h>\nuint64_t wide_x(uint64_t x);\nint main(void) { return wide_x(4294967329ULL) == 4294967329ULL ? 0 : 1; }\n";
    check_native(source, driver);
}
