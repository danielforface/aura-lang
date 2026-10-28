//! Q03-001 nested nominal Record by-value ABI. Source -> Checker -> SSA
//! -> IR Oracle -> linked independent C translation unit O0/O2.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{run_oracle_entry, validate_module, OracleConfig, OracleValue, Type};
use std::{collections::HashMap, fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
type Point = record { x: u32, y: u32 }
type Outer = record { p: Point, tag: u32 }
cell make_outer(x: u32, y: u32) ->:
    yield Outer { tag: 99, p: Point { y: y, x: x } }
cell nested_x(o: Outer) ->:
    yield o.p.x
cell replace_child(o: Outer, p: Point) ->:
    val mut copy: Outer = o
    copy.p = p
    yield copy
cell nested_roundtrip(x: u32, y: u32) ->:
    val o: Outer = make_outer(x, y)
    yield nested_x(o)
cell child_phi(flag: bool, o: Outer) ->:
    val mut p: Point = o.p
    if flag:
        p.x = 19
    yield p.x
"#;

fn point(x: u32, y: u32) -> OracleValue {
    OracleValue::Record {
        name: "Point".into(),
        fields: HashMap::from([
            ("x".into(), OracleValue::U32(x)),
            ("y".into(), OracleValue::U32(y)),
        ]),
    }
}
fn outer(x: u32, y: u32) -> OracleValue {
    OracleValue::Record {
        name: "Outer".into(),
        fields: HashMap::from([
            ("p".into(), point(x, y)),
            ("tag".into(), OracleValue::U32(99)),
        ]),
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-nested-record-{}-{ns}", std::process::id()));
        fs::create_dir_all(&path).expect("create scratch");
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

#[test]
fn nested_nominal_record_return_argument_projection_and_setfield_cross_native_abi() {
    let ast = aura_parse::parse_source(SOURCE).expect("nested Record source parse");
    Checker::new().check_program(&ast).expect("nested Record source checker");
    let ir = lower_program(&ast).expect("nested Record source lowerer");
    validate_module(&ir).expect("nested Record SSA structural validity");
    for (x, y) in [(7, 13), (19, 23)] {
        let made = run_oracle_entry(&ir, "make_outer",
            &[OracleValue::U32(x), OracleValue::U32(y)], OracleConfig::default())
            .expect("Oracle constructs nested Record");
        assert!(made.ok && made.stderr.is_empty());
        assert_eq!(made.return_value, Some(outer(x, y)));
        let got = run_oracle_entry(&ir, "nested_roundtrip",
            &[OracleValue::U32(x), OracleValue::U32(y)], OracleConfig::default())
            .expect("Oracle projects nested Record");
        assert_eq!(got.return_value, Some(OracleValue::U32(x)));
    }
    let replaced = run_oracle_entry(&ir, "replace_child",
        &[outer(7,13), point(41,43)], OracleConfig::default())
        .expect("Oracle functional nested field update");
    assert_eq!(replaced.return_value, Some(outer(41,43)));
    for (flag, expected) in [(true, 19), (false, 7)] {
        let result = run_oracle_entry(&ir, "child_phi",
            &[OracleValue::Bool(flag), outer(7,13)], OracleConfig::default())
            .expect("Oracle must merge nested Record field projection through Phi");
        assert_eq!(result.return_value, Some(OracleValue::U32(expected)));
    }
    let emitted = emit_module(&ir, None).expect("nested scalar Record C ABI must be emitted");
    let scratch = Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"), emitted.runtime_h).unwrap();
    fs::write(scratch.0.join("module.c"), &emitted.module_c).unwrap();
    fs::write(scratch.0.join("driver.c"), concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
        "typedef struct { aura_record_506f696e74 aura_f_70; uint32_t aura_f_746167; } aura_record_4f75746572;\n",
        "aura_record_4f75746572 make_outer(uint32_t, uint32_t);\n",
        "uint32_t nested_x(aura_record_4f75746572);\n",
        "aura_record_4f75746572 replace_child(aura_record_4f75746572, aura_record_506f696e74);\n",
        "uint32_t nested_roundtrip(uint32_t, uint32_t);\n",
        "uint32_t child_phi(bool, aura_record_4f75746572);\n",
        "int main(void) {\n",
        "  aura_record_4f75746572 a=make_outer(7u,13u);\n",
        "  aura_record_506f696e74 b={41u,43u};\n",
        "  aura_record_4f75746572 c=replace_child(a,b);\n",
        "  return (a.aura_f_70.aura_f_78==7u && a.aura_f_70.aura_f_79==13u",
        " && a.aura_f_746167==99u && nested_x(a)==7u",
        " && c.aura_f_70.aura_f_78==41u && c.aura_f_70.aura_f_79==43u",
        " && c.aura_f_746167==99u && a.aura_f_70.aura_f_78==7u",
        " && nested_roundtrip(19u,23u)==19u",
        " && child_phi(true,a)==19u && child_phi(false,a)==7u)?0:1;\n",
        "}\n"
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let executable=scratch.0.join(format!("nested_{}", &opt[1..]));
        let clang=Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&scratch.0).arg(scratch.0.join("module.c")).arg(scratch.0.join("driver.c"))
            .arg("-o").arg(&executable).output().expect("clang executable");
        assert!(clang.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&clang.stderr), emitted.module_c);
        let result=Command::new(&executable).output().expect("run executable");
        assert!(result.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&result.stderr), emitted.module_c);
    }
}

#[test]
fn cyclic_by_value_record_layout_fails_closed() {
    let ast = aura_parse::parse_source(SOURCE).unwrap();
    let mut ir = lower_program(&ast).unwrap();
    ir.record_layouts.insert("Point".into(),
        vec![("parent".into(), Type::Opaque("Outer".into()))]);
    validate_module(&ir).expect("structural SSA validation not a schema cycle check");
    let err=emit_module(&ir, None).err().expect("cyclic-by-value Record graph must be rejected");
    assert!(err.message.contains("Record") && err.message.contains("cycle"),
        "expected specific cycle error, got: {err}");
}


#[test]
fn structurally_identical_twin_cannot_be_embedded_in_point_typed_nested_record() {
    let ast = aura_parse::parse_source(SOURCE).expect("source parse");
    let mut ir = lower_program(&ast).expect("typed source lowers");
    ir.record_layouts.insert("Twin".into(), vec![
        ("x".into(), Type::U32), ("y".into(), Type::U32),
    ]);
    let maker = ir.functions.get_mut("make_outer").expect("maker");
    let child = maker.blocks.iter_mut().flat_map(|b| b.insts.iter_mut())
        .find(|inst| matches!(&inst.kind, aura_ir::InstKind::ConstructRecord { record_name, .. }
            if record_name == "Point"))
        .expect("inner Point constructor");
    let aura_ir::InstKind::ConstructRecord { record_name, .. } = &mut child.kind else {
        unreachable!("selected constructor")
    };
    *record_name = "Twin".into();
    validate_module(&ir).expect("structural IR valid; ABI nominal guard must refuse");
    let err = emit_module(&ir, None).err()
        .expect("embedding nominal Twin as Point must not be emitted");
    assert!(err.message.contains("Record") && err.message.contains("nominal mismatch")
        && err.message.contains("Twin") && err.message.contains("Point"),
        "specific nested nominal diagnostic required: {err}");
}


const NESTED_DEFAULT_SOURCE: &str = r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
type Outer = record { p: Point = Point {}, tag: u32 = 99 }
cell make_default() ->:
    yield Outer {}
cell override_child() ->:
    yield Outer { p: Point { x: 19, y: 23 } }
cell override_tag() ->:
    yield Outer { tag: 44 }
cell project_default() ->:
    val o: Outer = Outer {}
    yield o.p.x
"#;

#[test]
fn nested_nominal_record_default_and_overrides_are_real_c_abi_values() {
    let ast=aura_parse::parse_source(NESTED_DEFAULT_SOURCE)
        .expect("nested source default must parse");
    Checker::new().check_program(&ast)
        .expect("nested nominal literal default must be qualified");
    let ir=lower_program(&ast).expect("nested defaults lower to full SSA");
    validate_module(&ir).expect("nested default SSA valid");
    for (entry,expected) in [
        ("make_default",outer(7,13)),
        ("override_child",outer(19,23)),
        ("override_tag",OracleValue::Record {
            name:"Outer".into(),
            fields:HashMap::from([
                ("p".into(),point(7,13)),
                ("tag".into(),OracleValue::U32(44)),
            ]),
        }),
        ("project_default",OracleValue::U32(7)),
    ] {
        let result=run_oracle_entry(&ir,entry,&[],OracleConfig::default())
            .expect("Oracle nested source defaults");
        assert!(result.ok && result.stderr.is_empty(),"{entry}");
        assert_eq!(result.return_value,Some(expected),"{entry}");
    }
    let c=emit_module(&ir,None).expect("nested nominal default must emit C");
    let scratch=Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"),c.runtime_h).unwrap();
    fs::write(scratch.0.join("module.c"),&c.module_c).unwrap();
    fs::write(scratch.0.join("driver.c"),concat!(
        "#include <stdint.h>\n",
        "typedef struct { uint32_t aura_f_78; uint32_t aura_f_79; } aura_record_506f696e74;\n",
        "typedef struct { aura_record_506f696e74 aura_f_70; uint32_t aura_f_746167; } aura_record_4f75746572;\n",
        "aura_record_4f75746572 make_default(void);\n",
        "aura_record_4f75746572 override_child(void);\n",
        "aura_record_4f75746572 override_tag(void);\n",
        "uint32_t project_default(void);\n",
        "int main(void) {\n",
        "  aura_record_4f75746572 a=make_default(),b=override_child(),c=override_tag();\n",
        "  return (a.aura_f_70.aura_f_78==7u&&a.aura_f_70.aura_f_79==13u",
        "&&a.aura_f_746167==99u",
        "&&b.aura_f_70.aura_f_78==19u&&b.aura_f_70.aura_f_79==23u",
        "&&b.aura_f_746167==99u",
        "&&c.aura_f_70.aura_f_78==7u&&c.aura_f_70.aura_f_79==13u",
        "&&c.aura_f_746167==44u&&project_default()==7u)?0:1;\n",
        "}\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=scratch.0.join(format!("nested_defaults_{}",&opt[1..]));
        let compiled=Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&scratch.0).arg(scratch.0.join("module.c"))
            .arg(scratch.0.join("driver.c")).arg("-o").arg(&exe)
            .output().expect("clang available");
        assert!(compiled.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&compiled.stderr),c.module_c);
        let ran=Command::new(&exe).output().unwrap();
        assert!(ran.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&ran.stderr),c.module_c);
    }
}


#[test]
fn nominal_nested_default_rejects_mismatch_dynamic_cycle_and_missing_required_child() {
    let inputs=[
        (
            "type Point = record { x: u32 = 7 }\ntype Twin = record { x: u32 = 13 }\ntype Outer = record { p: Point = Twin {} }\ncell main() ->:\n    yield 7\n",
            "type mismatch",
        ),
        (
            "type Point = record { x: u32 = 7 }\ntype Outer = record { p: Point = Point { x: 1 + 2 } }\ncell main() ->:\n    yield 7\n",
            "unqualified",
        ),
        (
            "type Node = record { next: Node = Node {} }\ncell main() ->:\n    yield 7\n",
            "cyclic",
        ),
        (
            "type Point = record { x: u32, y: u32 = 13 }\ntype Outer = record { p: Point = Point {} }\ncell main() ->:\n    yield 7\n",
            "missing required field",
        ),
    ];
    for (source,needle) in inputs {
        let ast=aura_parse::parse_source(source).expect("negative source syntax parses");
        let error=Checker::new().check_program(&ast)
            .expect_err("invalid nested default must be rejected even when unused");
        assert!(error.message.contains(needle),
            "expected {needle:?}, got {error}");
        assert!(error.span.len()>0,"negative diagnostic needs source span");
    }
}
