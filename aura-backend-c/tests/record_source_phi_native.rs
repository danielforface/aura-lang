//! Q03-001: real Aura mutable local Record across IF/ELSE joins must
//! preserve its nominal type through a typed SSA Phi, not fake a Tensor.
//! Test first: no C implementation of Record Phi is qualified at baseline.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{run_oracle_entry, validate_module, InstKind, OracleConfig, OracleValue};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-record-phi-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

#[test]
fn actual_source_record_field_updates_join_through_nominal_phi_native_o0_o2() {
    let source = r#"
type Point = record { x: u32, y: u32 }
cell choose_update(flag: bool) ->:
    val mut p: Point = Point { y: 13, x: 7 }
    if flag:
        p.x = 19
    else:
        p.x = 23
    yield p.x
"#;
    let ast = aura_parse::parse_source(source).expect("Aura source with if-else record assignment parses");
    Checker::new().check_program(&ast).expect("both Record field writes valid in Checker");
    let ir = lower_program(&ast).expect("source Record field assignments lower");
    validate_module(&ir).expect("real source IR passes structural validator");
    let f = ir.functions.get("choose_update").expect("cell lowered");
    let phi_count = f.blocks.iter().flat_map(|b| b.insts.iter())
        .filter(|i| matches!(&i.kind, InstKind::Phi { .. })).count();
    assert!(phi_count >= 1, "Record value must be merged across source if/else using SSA Phi");
    for (flag, expected) in [(true, 19u32), (false, 23u32)] {
        let oracle = run_oracle_entry(&ir, "choose_update", &[OracleValue::Bool(flag)],
            OracleConfig::default()).expect("IR Oracle runs both Record branches");
        assert!(oracle.ok && oracle.stderr.is_empty());
        assert_eq!(oracle.return_value, Some(OracleValue::U32(expected)),
            "IR Oracle must preserve branch Record field");
    }
    let c = emit_module(&ir, None)
        .expect("real Record Phi must emit typed nominal C without Tensor fallback");
    assert!(c.module_c.contains("aura_record_"),
        "nominal Record C layout must be explicit: {}", c.module_c);
    let scratch = Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"), c.runtime_h).unwrap();
    let module_c = scratch.0.join("module.c");
    let driver_c = scratch.0.join("driver.c");
    fs::write(&module_c, &c.module_c).unwrap();
    fs::write(&driver_c, concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "uint32_t choose_update(bool flag);\n",
        "int main(void) {\n",
        "  return choose_update(true)==19u && choose_update(false)==23u ? 0 : 1;\n",
        "}\n",
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let binary = scratch.0.join(format!("record_phi_{}", &opt[1..]));
        let built = Command::new("clang").args(["-std=gnu11", "-Werror", opt, "-I"])
            .arg(&scratch.0).arg(&module_c).arg(&driver_c)
            .arg("-o").arg(&binary).output().expect("run real Clang");
        assert!(built.status.success(), "{opt}: Record Phi C failed: {}\n{}",
            String::from_utf8_lossy(&built.stderr), c.module_c);
        let run = Command::new(binary).output().expect("execute Record Phi native binary");
        assert!(run.status.success(), "{opt}: Record Phi branch values incorrect: {}\n{}",
            String::from_utf8_lossy(&run.stderr), c.module_c);
    }
}
