//! Q03-001 Record function ABI: source-level Record return, by-value parameter,
 //! cross-cell call, and native strict C11 O0/O2 executable cross-check.
 //! This test was introduced while the C backend explicitly rejected Record ABI.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{run_oracle_entry, validate_module, InstKind, OracleConfig, OracleValue, Type};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
type Point = record { x: u32, y: u32 }
cell make_point(x: u32, y: u32) ->:
    yield Point { y: y, x: x }
cell project_x(p: Point) ->:
    yield p.x
cell forward_point(p: Point) ->:
    yield project_x(p)
cell identity_point(p: Point) ->:
    yield p
cell branch_from_call(flag: bool) ->:
    val mut p: Point = make_point(7, 13)
    if flag:
        p.x = 19
    else:
        p.x = 23
    yield p.x
cell maybe_update_param(flag: bool, p: Point) ->:
    if flag:
        p.x = 19
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
        let mut fields = std::collections::HashMap::new();
        fields.insert("x".into(), OracleValue::U32(x));
        fields.insert("y".into(), OracleValue::U32(y));
        let point = OracleValue::Record { name: "Point".into(), fields };
        let forward = run_oracle_entry(&ir, "forward_point", &[point.clone()], OracleConfig::default())
            .expect("Oracle forwards nominal Record parameter by value");
        assert_eq!(forward.return_value, Some(OracleValue::U32(x)));
        let identity = run_oracle_entry(&ir, "identity_point", &[point.clone()], OracleConfig::default())
            .expect("Oracle returns nominal Record parameter by value");
        assert_eq!(identity.return_value, Some(point));
    }

    for (flag, expected) in [(true, 19), (false, 23)] {
        let value = run_oracle_entry(&ir, "branch_from_call",
            &[OracleValue::Bool(flag)], OracleConfig::default())
            .expect("Oracle must join Record returned from a function call");
        assert_eq!(value.return_value, Some(OracleValue::U32(expected)));
    }
    for (flag, expected) in [(true, 19), (false, 7)] {
        let mut fields = std::collections::HashMap::new();
        fields.insert("x".into(), OracleValue::U32(7));
        fields.insert("y".into(), OracleValue::U32(13));
        let point = OracleValue::Record { name: "Point".into(), fields };
        let value = run_oracle_entry(&ir, "maybe_update_param",
            &[OracleValue::Bool(flag), point], OracleConfig::default())
            .expect("Oracle must join a named Record parameter from the unchanged edge");
        assert_eq!(value.return_value, Some(OracleValue::U32(expected)));
    }
    let output = emit_module(&ir, None).expect("nominal Record function ABI should emit native C");
    assert!(output.module_c.contains("aura_record_506f696e74"),
        "qualified nominal Point type required, not Tensor fallback");
    let tmp = Scratch::new();
    fs::write(tmp.0.join("aura_runtime.h"), output.runtime_h).unwrap();
    fs::write(tmp.0.join("module.c"), &output.module_c).unwrap();
    // Independent C translation unit: exercise the real platform linker ABI
    // for by-value struct parameters and returns, not merely one-TU inlining.
    fs::write(tmp.0.join("driver.c"), concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
        "aura_record_506f696e74 make_point(uint32_t, uint32_t);\n",
        "uint32_t project_x(aura_record_506f696e74);\n",
        "uint32_t forward_point(aura_record_506f696e74);\n",
        "aura_record_506f696e74 identity_point(aura_record_506f696e74);\n",
        "uint32_t branch_from_call(bool);\n",
        "uint32_t maybe_update_param(bool, aura_record_506f696e74);\n",
        "uint32_t round_trip(uint32_t, uint32_t);\n",
        "int main(void) {\n",
        "  aura_record_506f696e74 p = make_point(31u, 37u);\n",
        "  return (p.aura_f_78 == 31u && p.aura_f_79 == 37u",
        " && project_x(p) == 31u",
        " && forward_point(p) == 31u",
        " && identity_point(p).aura_f_78 == 31u",
        " && branch_from_call(true) == 19u",
        " && branch_from_call(false) == 23u",
        " && maybe_update_param(true,p) == 19u",
        " && maybe_update_param(false,p) == 31u",
        " && round_trip(7u, 13u) == 7u",
        " && round_trip(19u, 23u) == 19u) ? 0 : 1;\n",
        "}\n"
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let exe = tmp.0.join(format!("record_abi_{}", &opt[1..]));
        let build = Command::new("clang")
            .args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
            .arg(&tmp.0).arg(tmp.0.join("module.c")).arg(tmp.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("clang");
        assert!(build.status.success(), "{opt}: {}\n{}", String::from_utf8_lossy(&build.stderr), output.module_c);
        let run = Command::new(exe).output().expect("run native Record ABI");
        assert!(run.status.success(), "{opt}: {}\n{}", String::from_utf8_lossy(&run.stderr), output.module_c);
    }
}


fn twin_layout(ir: &mut aura_ir::ModuleIR) {
    ir.record_layouts.insert("Twin".into(), vec![
        ("x".into(), Type::U32), ("y".into(), Type::U32),
    ]);
}

#[test]
fn record_abi_refuses_nominally_distinct_but_structurally_identical_argument() {
    let ast = aura_parse::parse_source(SOURCE).unwrap();
    let mut ir = lower_program(&ast).unwrap();
    twin_layout(&mut ir);
    ir.functions.get_mut("project_x").unwrap().params[0].ty = Type::Opaque("Twin".into());
    validate_module(&ir).expect("structural IR valid; typing is C ABI responsibility");
    let err = emit_module(&ir, None).err().expect("nominal mismatch must reject C emission");
    assert!(err.message.contains("Record ABI call 'project_x'") &&
        err.message.contains("expects nominal 'Twin'"), "{err}");
}

#[test]
fn record_abi_refuses_scalar_argument_for_record_parameter() {
    let ast = aura_parse::parse_source(SOURCE).unwrap();
    let mut ir = lower_program(&ast).unwrap();
    let round = ir.functions.get_mut("round_trip").unwrap();
    let x_param = round.params[0].value;
    let call = round.blocks.iter_mut().flat_map(|b| b.insts.iter_mut())
        .find(|i| matches!(&i.kind, InstKind::Call { callee, .. } if callee == "project_x"))
        .expect("source cell contains call");
    let InstKind::Call { args, .. } = &mut call.kind else { unreachable!() };
    *args = vec![x_param];
    validate_module(&ir).expect("structural SSA valid before type rejection");
    let err = emit_module(&ir, None).err().expect("passing scalar as nominal Record must reject");
    assert!(err.message.contains("Record ABI call 'project_x'") &&
        err.message.contains("expects nominal 'Point'"), "{err}");
}

#[test]
fn record_abi_refuses_returning_the_wrong_nominal_record() {
    let ast = aura_parse::parse_source(SOURCE).unwrap();
    let mut ir = lower_program(&ast).unwrap();
    twin_layout(&mut ir);
    ir.functions.get_mut("make_point").unwrap().ret = Type::Opaque("Twin".into());
    validate_module(&ir).expect("structural SSA valid before type rejection");
    let err = emit_module(&ir, None).err().expect("returning Point as Twin must reject");
    assert!(err.message.contains("Record ABI return") &&
        err.message.contains("expects nominal 'Twin'"), "{err}");
}
