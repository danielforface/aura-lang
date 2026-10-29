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


#[test]
fn record_integer_width_overflows_fail_in_checker_before_ir_or_c() {
    // The no-Z3 AVM already rejects these. Checker must reject them too,
    // independent of use, direct initialization, nested defaults or update.
    for (source, label) in [
        ("type Tiny = record { n: u8 = 300 }\ncell main() ->:\n    yield 7\n", "unused u8 default"),
        ("type Tiny = record { n: i8 = 200 }\ncell main() ->:\n    yield 7\n", "unused i8 default"),
        ("type Tiny = record { n: u8 }\ncell main() ->:\n    val t: Tiny = Tiny { n: 256 }\n    yield 7\n", "explicit u8 initializer"),
        ("type Tiny = record { n: i8 }\ncell main() ->:\n    val t: Tiny = Tiny { n: 128 }\n    yield 7\n", "explicit i8 initializer"),
        ("type Tiny = record { n: u8 = 7 }\ncell main() ->:\n    val mut t: Tiny = Tiny {}\n    t.n = 300\n    yield t.n\n", "mutable field update"),
        ("type Tiny = record { n: u8 }\ntype Wrap = record { t: Tiny = Tiny { n: 300 } }\ncell main() ->:\n    yield 7\n", "unused nested default"),
    ] {
        let ast=aura_parse::parse_source(source).expect("numeric source must parse");
        let failure=Checker::new().check_program(&ast)
            .expect_err("out-of-range Record integer must never be accepted");
        assert!(failure.message.contains("out of range"),
            "{label}: expected width diagnostic, got {failure}");
        assert!(failure.span.len()>0,
            "{label}: numeric range error must retain source span");
    }
}

#[test]
fn record_integer_width_boundary_literals_remain_valid_in_checker() {
    for source in [
        "type Tiny = record { n: u8 = 255 }\ncell main() ->:\n    val t: Tiny = Tiny {}\n    yield 7\n",
        "type Tiny = record { n: i8 = 127 }\ncell main() ->:\n    val t: Tiny = Tiny {}\n    yield 7\n",
        "type Tiny = record { n: u8 }\ncell main() ->:\n    val t: Tiny = Tiny { n: 255 }\n    yield 7\n",
        "type Tiny = record { n: u8 = 7 }\ncell main() ->:\n    val mut t: Tiny = Tiny {}\n    t.n = 255\n    yield t.n\n",
    ] {
        let ast=aura_parse::parse_source(source).expect("boundary source parses");
        Checker::new().check_program(&ast)
            .expect("representable Record integer literal must typecheck");
    }
}


#[test]
fn duplicate_nominal_record_type_declarations_are_rejected_even_when_unused() {
    for definitions in [
        "type Point = record { x: u32 = 7 }\ntype Point = record { x: u32 = 13 }\n",
        "type Point = record { x: u32 = 7 }\ntype Point = record { x: bool = true }\n",
    ] {
        let src=format!("{definitions}cell main() ->:\n    yield 7\n");
        let parsed=aura_parse::parse_source(&src)
            .expect("duplicate Record declarations must parse as distinct nodes");
        let error=Checker::new().check_program(&parsed)
            .expect_err("duplicate nominal Record declarations must fail at source registration");
        assert!(error.message.contains("duplicate") && error.message.contains("Point"),
            "expected explicit duplicate Point declaration diagnostic, got {error}");
        assert!(error.span.len()>0, "duplicate Record diagnostic must retain source span");
    }
}

#[test]
fn type_namespace_collisions_are_rejected_at_declaration_even_when_unused() {
    for (label, declarations) in [
        ("record_then_enum", "type Collision = record { x: u32 = 7 }\ntype Collision = enum { First }\n"),
        ("enum_then_record", "type Collision = enum { First }\ntype Collision = record { x: u32 = 7 }\n"),
        ("record_then_alias", "type Collision = record { x: u32 = 7 }\ntype Collision = u32\n"),
        ("alias_then_record", "type Collision = u32\ntype Collision = record { x: u32 = 7 }\n"),
        ("enum_then_alias", "type Collision = enum { First }\ntype Collision = u32\n"),
        ("alias_then_enum", "type Collision = u32\ntype Collision = enum { First }\n"),
        ("enum_then_enum", "type Collision = enum { First }\ntype Collision = enum { Second }\n"),
        ("alias_then_alias", "type Collision = u32\ntype Collision = bool\n"),
    ] {
        let source = format!("{declarations}cell main() ->:\n    yield 7\n");
        let ast = aura_parse::parse_source(&source)
            .unwrap_or_else(|err| panic!("{label}: fixture must parse: {err}"));
        let failure = Checker::new().check_program(&ast)
            .expect_err("type names must be unique across Record, Enum and aliases");
        assert!(failure.message.contains("duplicate") && failure.message.contains("Collision"),
            "{label}: expected explicit duplicate type diagnostic: {failure}");
        assert!(failure.span.len() > 0,
            "{label}: duplicate type diagnostic must point to the second declaration");
    }
    let distinct = "type A = record { x: u32 = 7 }\ntype B = enum { First }\ntype C = u32\ncell main() ->:\n    yield 7\n";
    let ast = aura_parse::parse_source(distinct).expect("distinct types parse");
    Checker::new().check_program(&ast)
        .expect("different type names in one type namespace remain legal");
}

#[test]
fn record_integer_range_from_inferred_local_cannot_narrow_out_of_bounds() {
    for (label, src) in [
        ("u8 constructor from local",
         "type Tiny = record { n: u8 }\ncell main() ->:\n    val n = 300\n    val t: Tiny = Tiny { n: n }\n    yield 7\n"),
        ("i8 constructor from local",
         "type Tiny = record { n: i8 }\ncell main() ->:\n    val n = 128\n    val t: Tiny = Tiny { n: n }\n    yield 7\n"),
        ("u8 mutable Record field from local",
         "type Tiny = record { n: u8 = 7 }\ncell main() ->:\n    val n = 300\n    val mut t: Tiny = Tiny {}\n    t.n = n\n    yield t.n\n"),
        ("u8 nested Record field from local",
         "type Tiny = record { n: u8 = 7 }\ntype Outer = record { t: Tiny = Tiny {} }\ncell main() ->:\n    val n = 300\n    val mut o: Outer = Outer {}\n    o.t.n = n\n    yield o.t.n\n"),
    ] {
        let ast = aura_parse::parse_source(src)
            .unwrap_or_else(|e| panic!("{label}: source must parse: {e}"));
        let err = Checker::new().check_program(&ast)
            .expect_err("Record local value must not silently narrow to an unrepresentable field");
        assert!(err.message.contains("out of range"),
            "{label}: expected proven local range diagnostic: {err}");
        assert!(err.span.len() > 0,
            "{label}: diagnostic must retain source span");
    }
}
#[test]
fn unsigned_record_negated_expression_fails_closed_before_native() {
    for (label, src) in [
        ("constructor",
         "type Tiny = record { n: u32 }\ncell main() ->:\n    val t: Tiny = Tiny { n: -1 }\n    yield 7\n"),
        ("field assignment",
         "type Tiny = record { n: u32 = 7 }\ncell main() ->:\n    val mut t: Tiny = Tiny {}\n    t.n = -1\n    yield t.n\n"),
    ] {
        let ast = aura_parse::parse_source(src)
            .unwrap_or_else(|e| panic!("{label}: negative expression must parse: {e}"));
        let err = Checker::new().check_program(&ast)
            .expect_err("unsigned Record must not accept a negative unary expression");
        assert!(err.message.contains("negative") || err.message.contains("unqualified"),
            "{label}: expected negative numeric Record diagnostic: {err}");
        assert!(err.span.len() > 0, "{label}: source span required");
    }
}
#[test]
fn in_range_inferred_local_record_values_remain_valid() {
    for (label, src) in [
        ("u8 max",
         "type Tiny = record { n: u8 }\ncell main() ->:\n    val n = 255\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("i8 max",
         "type Tiny = record { n: i8 }\ncell main() ->:\n    val n = 127\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("mutable u8",
         "type Tiny = record { n: u8 = 7 }\ncell main() ->:\n    val n = 255\n    val mut t: Tiny = Tiny {}\n    t.n = n\n    yield t.n\n"),
    ] {
        let ast = aura_parse::parse_source(src)
            .unwrap_or_else(|e| panic!("{label}: valid boundary source must parse: {e}"));
        Checker::new().check_program(&ast)
            .unwrap_or_else(|e| panic!("{label}: representable local should remain valid: {e}"));
    }
}

#[test]
fn inferred_local_integer_bounds_execute_in_oracle_and_strict_native_c_o0_o2() {
    // These are values retained from inferred U32 singleton ranges into
    // narrower by-value Record fields. Both validation and *execution* matter.
    let source = r#"
type Tiny = record { n: u8 }
type Signed = record { n: i8 }
cell make_tiny() ->:
    val n = 255
    yield Tiny { n: n }
cell update_tiny() ->:
    val n = 255
    val mut t: Tiny = Tiny { n: 7 }
    t.n = n
    yield t
cell make_signed() ->:
    val n = 127
    yield Signed { n: n }
"#;
    let ast = aura_parse::parse_source(source).expect("inferred local source parses");
    Checker::new().check_program(&ast).expect("valid inferred local range must check");
    let ir = lower_program(&ast).expect("source lowers into SSA");
    validate_module(&ir).expect("inferred local Record SSA validates");
    for (name, ty, value) in [
        ("make_tiny", "Tiny", 255_u32),
        ("update_tiny", "Tiny", 255_u32),
        ("make_signed", "Signed", 127_u32),
    ] {
        let result=run_oracle_entry(&ir, name, &[], OracleConfig::default())
            .expect("IR oracle executes local-bound Record");
        assert!(result.ok && result.stderr.is_empty(), "{name}: {result:?}");
        assert_eq!(result.return_value, Some(OracleValue::Record {
            name: ty.into(),
            fields: HashMap::from([("n".into(), OracleValue::U32(value))]),
        }), "{name}: Oracle must preserve scalar field value");
    }
    let artifacts=emit_module(&ir,None).expect("qualified inferred locals must emit C");
    let scratch=Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"), artifacts.runtime_h).unwrap();
    fs::write(scratch.0.join("module.c"), &artifacts.module_c).unwrap();
    fs::write(scratch.0.join("driver.c"), concat!(
        "#include <stdint.h>\n",
        "typedef struct { uint8_t aura_f_6e; } aura_record_54696e79;\n",
        "typedef struct { int8_t aura_f_6e; } aura_record_5369676e6564;\n",
        "aura_record_54696e79 make_tiny(void);\n",
        "aura_record_54696e79 update_tiny(void);\n",
        "aura_record_5369676e6564 make_signed(void);\n",
        "int main(void) {\n",
        "  aura_record_54696e79 a=make_tiny(), b=update_tiny();\n",
        "  aura_record_5369676e6564 c=make_signed();\n",
        "  return a.aura_f_6e==255 && b.aura_f_6e==255 && c.aura_f_6e==127 ? 0 : 1;\n",
        "}\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=scratch.0.join(format!("local_width_{}", &opt[1..]));
        let out=Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&scratch.0)
            .arg(scratch.0.join("module.c"))
            .arg(scratch.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("run clang");
        assert!(out.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&out.stderr), artifacts.module_c);
        let run=Command::new(&exe).output().expect("execute native");
        assert!(run.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&run.stderr), artifacts.module_c);
    }
}

#[test]
fn native_record_narrowing_cannot_use_an_out_of_range_ir_constant() {
    // The public Checker would reject this value. The backend is an
    // independent trust boundary and must not accept edited but structurally
    // valid SSA that tries to smuggle a truncating U32 into an u8 Record.
    let source = r#"
type Tiny = record { n: u8 }
cell make_tiny() ->:
    val n = 255
    yield Tiny { n: n }
"#;
    let ast=aura_parse::parse_source(source).expect("positive fixture parses");
    Checker::new().check_program(&ast).expect("baseline typechecks");
    let mut ir=lower_program(&ast).expect("baseline lowers");
    let function=ir.functions.get_mut("make_tiny").expect("source function exists");
    let constant=function.blocks.iter_mut()
        .flat_map(|b| b.insts.iter_mut())
        .find_map(|inst| match &mut inst.kind {
            aura_ir::InstKind::BindStrand {
                expr: aura_ir::RValue::ConstU32(n), ..
            } if *n == 255 => Some(n),
            _ => None,
        }).expect("find U32 constant backing inferred local");
    *constant=300;
    validate_module(&ir).expect("SSA remains structurally valid");
    let err=emit_module(&ir,None).err()
        .expect("C emitter must refuse an unproven narrowing despite SSA validity");
    assert!(err.message.contains("Record") && err.message.contains("no proven in-range constant"),
        "expected exact unsafe-narrowing diagnostic, got {err}");
}

#[test]
fn typed_numeric_local_cannot_launder_out_of_range_value_into_record() {
    for (label, source) in [
        ("typed u8 literal", "type Tiny = record { n: u8 }\ncell main() ->:\n    val n: u8 = 300\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("typed i8 literal", "type Tiny = record { n: i8 }\ncell main() ->:\n    val n: i8 = 128\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("typed u8 from local", "type Tiny = record { n: u8 }\ncell main() ->:\n    val raw = 300\n    val n: u8 = raw\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("typed i8 from local", "type Tiny = record { n: i8 }\ncell main() ->:\n    val raw = 128\n    val n: i8 = raw\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("typed alias to u8", "type Byte = u8\ntype Tiny = record { n: u8 }\ncell main() ->:\n    val n: Byte = 300\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("typed u8 negative", "type Tiny = record { n: u8 }\ncell main() ->:\n    val n: u8 = -1\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("unused typed u8", "type Tiny = record { n: u8 = 7 }\ncell main() ->:\n    val unused: u8 = 300\n    yield Tiny {}\n"),
        ("mutable typed u8 reassign", "type Tiny = record { n: u8 }\ncell main() ->:\n    val mut n: u8 = 7\n    n = 300\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
    ] {
        let parsed=aura_parse::parse_source(source)
            .unwrap_or_else(|e| panic!("{label}: syntax must parse: {e}"));
        let error=Checker::new().check_program(&parsed)
            .expect_err("typed narrow local must not erase numeric range proof");
        assert!(error.message.contains("out of range") || error.message.contains("negative"),
            "{label}: must diagnose nonrepresentable local value, got {error}");
        assert!(error.span.len()>0, "{label}: retain source location");
    }
}
#[test]
fn typed_local_valid_numeric_limits_remain_checker_legal() {
    for (label, src) in [
        ("u8 exact max", "type Tiny = record { n: u8 }\ncell main() ->:\n    val n: u8 = 255\n    yield Tiny { n: n }\n"),
        ("i8 exact max", "type Signed = record { n: i8 }\ncell main() ->:\n    val n: i8 = 127\n    yield Signed { n: n }\n"),
        ("typed copy", "type Tiny = record { n: u8 }\ncell main() ->:\n    val raw = 255\n    val n: u8 = raw\n    yield Tiny { n: n }\n"),
    ] {
        let parsed=aura_parse::parse_source(src)
            .unwrap_or_else(|e| panic!("{label}: syntax must parse: {e}"));
        Checker::new().check_program(&parsed)
            .unwrap_or_else(|e| panic!("{label}: representable typed local must remain valid: {e}"));
    }
}

#[test]
fn typed_integer_locals_keep_exact_record_values_in_oracle_and_native() {
    let source=r#"
type Tiny = record { n: u8 }
type Signed = record { n: i8 }
cell typed_tiny() ->:
    val raw = 255
    val n: u8 = raw
    yield Tiny { n: n }
cell typed_signed() ->:
    val n: i8 = 127
    yield Signed { n: n }
cell typed_update() ->:
    val mut n: u8 = 7
    n = 255
    val mut t: Tiny = Tiny { n: 7 }
    t.n = n
    yield t
"#;
    let ast=aura_parse::parse_source(source).expect("typed-local valid source parses");
    Checker::new().check_program(&ast).expect("typed-local valid bounds check");
    let ir=lower_program(&ast).expect("typed local source lowers");
    validate_module(&ir).expect("typed local SSA is valid");
    for (cell,name,n) in [
        ("typed_tiny","Tiny",255_u32),
        ("typed_signed","Signed",127_u32),
        ("typed_update","Tiny",255_u32),
    ] {
        let v=run_oracle_entry(&ir,cell,&[],OracleConfig::default())
            .expect("Oracle must execute typed local Record");
        assert!(v.ok && v.stderr.is_empty(), "{cell}: {v:?}");
        assert_eq!(v.return_value,Some(OracleValue::Record{
            name:name.into(),fields:HashMap::from([("n".into(),OracleValue::U32(n))])
        }), "{cell}: exact Record value must survive SSA");
    }
    let artifacts=emit_module(&ir,None).expect("typed local Record native C must emit");
    let tmp=Scratch::new();
    fs::write(tmp.0.join("aura_runtime.h"), artifacts.runtime_h).unwrap();
    fs::write(tmp.0.join("module.c"), &artifacts.module_c).unwrap();
    fs::write(tmp.0.join("driver.c"), concat!(
        "#include <stdint.h>\n",
        "typedef struct { uint8_t aura_f_6e; } aura_record_54696e79;\n",
        "typedef struct { int8_t aura_f_6e; } aura_record_5369676e6564;\n",
        "aura_record_54696e79 typed_tiny(void);\n",
        "aura_record_5369676e6564 typed_signed(void);\n",
        "aura_record_54696e79 typed_update(void);\n",
        "int main(void) {\n",
        "  return (typed_tiny().aura_f_6e == 255 &&",
        " typed_signed().aura_f_6e == 127 &&",
        " typed_update().aura_f_6e == 255) ? 0 : 1;\n",
        "}\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=tmp.0.join(format!("typed_local_{}",&opt[1..]));
        let built=Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&tmp.0).arg(tmp.0.join("module.c")).arg(tmp.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("clang availability");
        assert!(built.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&built.stderr),artifacts.module_c);
        let executed=Command::new(&exe).output().expect("native run");
        assert!(executed.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&executed.stderr),artifacts.module_c);
    }
}

#[test]
fn monomorphic_integer_alias_record_defaults_and_fields_match_oracle_and_native() {
    let source=r#"
type Byte = u8
type Again = Byte
type Small = i8
type Tiny = record { n: Again = 255 }
type Signed = record { n: Small }
cell alias_default() ->:
    val t: Tiny = Tiny {}
    yield t.n
cell alias_explicit() ->:
    val n: Again = 255
    val t: Tiny = Tiny { n: n }
    yield t.n
cell alias_signed() ->:
    val n: Small = 127
    val t: Signed = Signed { n: n }
    yield t.n
"#;
    let parsed=aura_parse::parse_source(source).expect("well formed alias source");
    Checker::new().check_program(&parsed).expect("Core resolves integer aliases");
    let ir=lower_program(&parsed).expect("alias source lowers to qualified IR");
    validate_module(&ir).expect("Record alias SSA must validate");
    for (cell,n) in [("alias_default",255),("alias_explicit",255),("alias_signed",127)] {
        let result=run_oracle_entry(&ir,cell,&[],OracleConfig::default())
            .expect("IR oracle must execute aliased field and default");
        assert!(result.ok && result.stderr.is_empty(), "{cell}: {result:?}");
        assert_eq!(result.return_value,Some(OracleValue::U32(n)),
            "{cell}: alias source should resolve to exact scalar value");
    }
    let artifacts=emit_module(&ir,None).expect("aliased concrete Record fields emit to C");
    let scratch=Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"),artifacts.runtime_h).unwrap();
    fs::write(scratch.0.join("module.c"),&artifacts.module_c).unwrap();
    fs::write(scratch.0.join("driver.c"),concat!(
        "#include <stdint.h>\n",
        "uint8_t alias_default(void);\n",
        "uint8_t alias_explicit(void);\n",
        "int8_t alias_signed(void);\n",
        "int main(void) {\n",
        "  return (alias_default()==255 && alias_explicit()==255",
        " && alias_signed()==127) ? 0 : 1;\n",
        "}\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=scratch.0.join(format!("aliased_record_{}",&opt[1..]));
        let built=Command::new("clang").args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&scratch.0).arg(scratch.0.join("module.c"))
            .arg(scratch.0.join("driver.c")).arg("-o").arg(&exe)
            .output().expect("strict clang available");
        assert!(built.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&built.stderr),artifacts.module_c);
        let run=Command::new(&exe).output().expect("native C executable");
        assert!(run.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&run.stderr),artifacts.module_c);
    }
}

#[test]
fn unusable_monomorphic_type_aliases_fail_at_declaration_even_if_unused() {
    for (label, declarations) in [
        ("self cycle", "type Loop = Loop\n"),
        ("two-alias cycle", "type First = Second\ntype Second = First\n"),
        ("three-alias cycle", "type A = B\ntype B = C\ntype C = A\n"),
        ("missing direct target", "type Phantom = MissingTarget\n"),
        ("missing chained target", "type A = B\ntype B = MissingTarget\n"),
    ] {
        let source = format!("{declarations}cell main() ->:\n    yield 7\n");
        let ast = aura_parse::parse_source(&source)
            .unwrap_or_else(|e| panic!("{label}: source must parse: {e}"));
        let err = Checker::new().check_program(&ast)
            .expect_err("invalid unused monomorphic TypeAlias must fail before execution");
        assert!(err.message.contains("TypeAlias") &&
            (err.message.contains("cyclic") || err.message.contains("unknown")),
            "{label}: expected an explicit cyclic/unknown TypeAlias diagnostic: {err}");
    }
}

#[test]
fn legitimate_monomorphic_type_alias_chains_remain_valid_in_core() {
    let source = "type Byte = u8\ntype AlsoByte = Byte\n\
        type Tiny = record { n: AlsoByte = 255 }\n\
        cell main() ->:\n    val t: Tiny = Tiny {}\n    yield t.n\n";
    let ast = aura_parse::parse_source(source).expect("positive alias parses");
    Checker::new().check_program(&ast).expect("valid primitive alias chain must remain legal");
}

#[test]
fn forward_declared_monomorphic_integer_aliases_keep_exact_record_abi() {
    let source=r#"
type Top = Middle
type Middle = u8
type Tiny = record { n: Top = 255 }
cell forward_default() ->:
    val t: Tiny = Tiny {}
    yield t.n
cell forward_explicit() ->:
    val n: Top = 255
    val t: Tiny = Tiny { n: n }
    yield t.n
"#;
    let ast=aura_parse::parse_source(source).expect("forward source parses");
    Checker::new().check_program(&ast)
        .expect("Checker must resolve forward mono alias to its terminal width");
    let ir=lower_program(&ast).expect("forward mono alias must lower as exact scalar");
    validate_module(&ir).expect("forward alias module SSA must be valid");
    for cell in ["forward_default","forward_explicit"] {
        let result=run_oracle_entry(&ir,cell,&[],OracleConfig::default())
            .expect("forward alias source executes in IR Oracle");
        assert!(result.ok && result.stderr.is_empty(),"{cell}: {result:?}");
        assert_eq!(result.return_value,Some(OracleValue::U32(255)), "{cell}");
    }
    let artifacts=emit_module(&ir,None).expect("forward alias source must emit C");
    let tmp=Scratch::new();
    fs::write(tmp.0.join("aura_runtime.h"),artifacts.runtime_h).unwrap();
    fs::write(tmp.0.join("module.c"),&artifacts.module_c).unwrap();
    fs::write(tmp.0.join("driver.c"),concat!(
        "#include <stdint.h>\n",
        "uint8_t forward_default(void);\n",
        "uint8_t forward_explicit(void);\n",
        "int main(void) { return (forward_default() == 255 &&",
        " forward_explicit() == 255) ? 0 : 1; }\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=tmp.0.join(format!("forward_alias_{}",&opt[1..]));
        let built=Command::new("clang").args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&tmp.0).arg(tmp.0.join("module.c")).arg(tmp.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("strict clang available");
        assert!(built.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&built.stderr),artifacts.module_c);
        let out=Command::new(&exe).output().expect("execute independent linked C");
        assert!(out.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&out.stderr),artifacts.module_c);
    }
}

#[test]
fn forward_declared_mono_integer_aliases_cannot_launder_out_of_range_values() {
    let source = "type Top = Middle\ntype Middle = u8\n\
        type Tiny = record { n: Top }\n\
        cell main() ->:\n    val n: Top = 300\n    yield Tiny { n: n }\n";
    let ast=aura_parse::parse_source(source).expect("numeric counterexample parses");
    let err=Checker::new().check_program(&ast)
        .expect_err("forward alias width must not permit 300 in u8");
    assert!(err.message.contains("out of range"),
        "must preserve precise resolved u8 range refusal, got {err}");
}

#[test]
fn refined_monomorphic_alias_declarations_require_representable_base_bounds() {
    for (label, declarations) in [
        ("u8 impossible upper", "type Bad = u8[0..300]\n"),
        ("i8 impossible upper", "type Bad = i8[0..128]\n"),
        ("bool is nonnumeric", "type Bad = bool[0..1]\n"),
        ("float is noninteger", "type Bad = f32[0..1]\n"),
        ("u8 invalid lower", "type Bad = u8[256..300]\n"),
        ("alias widening", "type Small = u8[10..20]\ntype Wide = Small[10..21]\n"),
    ] {
        let src=format!("{declarations}cell main() ->:\n    yield 7\n");
        let parsed=aura_parse::parse_source(&src)
            .unwrap_or_else(|e| panic!("{label}: refined alias source must parse: {e}"));
        let err=Checker::new().check_program(&parsed)
            .expect_err("ill-formed unused refined alias must be rejected at declaration");
        assert!(err.message.contains("range") || err.message.contains("refin"),
            "{label}: expected precise refinement range error: {err}");
    }
}

#[test]
fn nested_representable_refined_mono_alias_declarations_remain_legal() {
    let src="type Small = u8[10..20]\ntype Narrow = Small[12..18]\n\
        cell main() ->:\n    yield 7\n";
    let parsed=aura_parse::parse_source(src).expect("legal nested refinement parses");
    Checker::new().check_program(&parsed)
        .expect("representable nested alias refinement must remain valid unused");
}

#[test]
fn refined_alias_integer_local_write_contract_cannot_be_bypassed_in_checker() {
    for (label, body) in [
        ("declaration upper", "    val n: Window = 21\n    yield n\n"),
        ("declaration lower", "    val n: Window = 9\n    yield n\n"),
        ("mutable upper", "    val mut n: Window = 10\n    n = 21\n    yield n\n"),
        ("mutable lower", "    val mut n: Window = 20\n    n = 9\n    yield n\n"),
        ("nested alias upper", "    val n: Narrow = 19\n    yield n\n"),
    ] {
        let src=format!(
            "type Window = u8[10..20]\ntype Narrow = Window[12..18]\ncell main() ->:\n{body}"
        );
        let parsed=aura_parse::parse_source(&src).expect("typed numeric source parses");
        let err=Checker::new().check_program(&parsed)
            .expect_err("refined alias annotated local must reject source proven outside range");
        assert!(err.message.contains("range") || err.message.contains("refin"),
            "{label}: expected explicit range failure, got {err}");
    }
}

#[test]
fn refined_alias_local_boundary_values_remain_source_legal() {
    for (label,body) in [
        ("lower", "    val n: Window = 10\n    yield n\n"),
        ("upper", "    val n: Window = 20\n    yield n\n"),
        ("mutable in range", "    val mut n: Window = 10\n    n = 20\n    yield n\n"),
        ("nested lower", "    val n: Narrow = 12\n    yield n\n"),
        ("nested upper", "    val n: Narrow = 18\n    yield n\n"),
    ] {
        let src=format!(
            "type Window = u8[10..20]\ntype Narrow = Window[12..18]\ncell main() ->:\n{body}"
        );
        let parsed=aura_parse::parse_source(&src).expect("valid boundary parses");
        Checker::new().check_program(&parsed)
            .unwrap_or_else(|err|panic!("{label}: valid source range boundary: {err}"));
    }
}

#[test]
fn refined_alias_record_default_and_updates_match_oracle_and_linked_native() {
    let src=r#"
type Window = u8[10..20]
type Tiny = record { n: Window = 20 }
cell default_value() ->:
    val t: Tiny = Tiny {}
    yield t.n
cell explicit_value() ->:
    val t: Tiny = Tiny { n: 10 }
    yield t.n
cell updated_value() ->:
    val mut t: Tiny = Tiny { n: 10 }
    t.n = 20
    yield t.n
"#;
    let parsed=aura_parse::parse_source(src).expect("valid refined Record parses");
    Checker::new().check_program(&parsed).expect("valid refined Record boundaries check");
    let ir=lower_program(&parsed).expect("refined scalar Record lowers to concrete ABI");
    validate_module(&ir).expect("refined Record IR valid");
    for (name,value) in [("default_value",20),("explicit_value",10),("updated_value",20)] {
        let result=run_oracle_entry(&ir,name,&[],OracleConfig::default())
            .expect("Oracle executes refined Record");
        assert!(result.ok && result.stderr.is_empty(),"{name}: {result:?}");
        assert_eq!(result.return_value,Some(OracleValue::U32(value)),"{name}");
    }
    let artifacts=emit_module(&ir,None).expect("C emitter accepts resolved refined u8 field");
    let scratch=Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"),artifacts.runtime_h).unwrap();
    fs::write(scratch.0.join("module.c"),&artifacts.module_c).unwrap();
    fs::write(scratch.0.join("driver.c"),concat!(
        "#include <stdint.h>\n",
        "uint8_t default_value(void);\n",
        "uint8_t explicit_value(void);\n",
        "uint8_t updated_value(void);\n",
        "int main(void) { return (default_value()==20 && explicit_value()==10",
        " && updated_value()==20) ? 0 : 1; }\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=scratch.0.join(format!("refined_record_{}",&opt[1..]));
        let build=Command::new("clang").args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&scratch.0).arg(scratch.0.join("module.c")).arg(scratch.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("clang installed");
        assert!(build.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&build.stderr),artifacts.module_c);
        let run=Command::new(&exe).output().expect("native executable");
        assert!(run.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&run.stderr),artifacts.module_c);
    }
}

#[test]
fn refined_alias_record_field_and_unused_default_outside_range_fail_in_core() {
    for (label,source) in [
        ("unused default", "type Window = u8[10..20]\ntype Tiny = record { n: Window = 21 }\ncell main() ->:\n    yield 7\n"),
        ("explicit field", "type Window = u8[10..20]\ntype Tiny = record { n: Window }\ncell main() ->:\n    val t: Tiny = Tiny { n: 21 }\n    yield t.n\n"),
        ("field mutation", "type Window = u8[10..20]\ntype Tiny = record { n: Window }\ncell main() ->:\n    val mut t: Tiny = Tiny { n: 10 }\n    t.n = 21\n    yield t.n\n"),
    ] {
        let ast=aura_parse::parse_source(source).expect("invalid numeric source parses");
        let err=Checker::new().check_program(&ast)
            .expect_err("refined Record source must refuse out-of-interval field");
        assert!(err.message.contains("range") || err.message.contains("refin"),
            "{label}: require refined range error, got {err}");
    }
}

#[test]
fn lowered_native_must_not_defer_refined_user_call_argument_proofs() {
    for (label, source) in [
        ("direct refined argument overflow", r#"
type Window = u8[10..20]
cell accept(n: Window) ->:
    yield n
cell main() ->:
    yield accept(21)
"#),
        ("unconstrained dynamic integer argument", r#"
type Window = u8[10..20]
cell accept(n: Window) ->:
    yield n
cell proxy(n: u32) ->:
    yield accept(n)
"#),
        ("wrong nested refined range", r#"
type Window = u8[10..20]
type Narrow = Window[12..18]
cell accept(n: Narrow) ->:
    yield n
cell main() ->:
    val n: Window = 20
    yield accept(n)
"#),
    ] {
        let ast=aura_parse::parse_source(source)
            .unwrap_or_else(|e| panic!("{label}: qualified source must parse: {e}"));
        // This is specifically a compilation safety gate. The lowerer
        // must not silently trust its defer_range_proofs checker mode.
        let err=lower_program(&ast)
            .expect_err("unproved refined user-cell call argument must not lower to native IR");
        assert!(err.message.contains("range") || err.message.contains("refin") ||
                err.message.contains("prove"),
            "{label}: require a refinement-specific diagnostic, got {err}");
    }
}

#[test]
fn lowered_refined_user_call_with_proven_value_remains_legal() {
    let src=r#"
type Window = u8[10..20]
cell accept(n: Window) ->:
    yield n
cell entry_value() ->:
    val n: Window = 20
    yield accept(n)
"#;
    let ast=aura_parse::parse_source(src).expect("valid argument source parses");
    let ir=lower_program(&ast).expect("proven refined argument must lower");
    validate_module(&ir).expect("refined call IR validates");
    let result=run_oracle_entry(&ir,"entry_value",&[],OracleConfig::default())
        .expect("oracle executes valid refined user cell call");
    assert!(result.ok && result.stderr.is_empty(), "{result:?}");
    assert_eq!(result.return_value,Some(OracleValue::U32(20)));
    let artifacts=emit_module(&ir,None).unwrap_or_else(|e|
        panic!("a refined parameter has qualified u8 native ABI: {e}; IR = {ir:#?}"));
    let scratch=Scratch::new();
    fs::write(scratch.0.join("aura_runtime.h"),artifacts.runtime_h).unwrap();
    fs::write(scratch.0.join("module.c"),&artifacts.module_c).unwrap();
    fs::write(scratch.0.join("driver.c"),concat!(
        "#include <stdint.h>\n",
        "uint8_t accept(uint8_t);\n",
        "uint8_t entry_value(void);\n",
        "int main(void) { return (accept(10)==10 && accept(20)==20",
        " && entry_value()==20) ? 0 : 1; }\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=scratch.0.join(format!("refined_param_{}",&opt[1..]));
        let build=Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&scratch.0).arg(scratch.0.join("module.c"))
            .arg(scratch.0.join("driver.c")).arg("-o").arg(&exe)
            .output().expect("strict clang available");
        assert!(build.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&build.stderr),artifacts.module_c);
        let run=Command::new(&exe).output().expect("linked refined user-call executable");
        assert!(run.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&run.stderr),artifacts.module_c);
    }
}

#[test]
fn native_refined_user_call_rejects_lost_range_proof_after_ir_optimization() {
    // Native output is a separate trust boundary. Changing already-optimized
    // source-proven IR must not launder a now-unproved call into the U8 ABI.
    // In the old emitter, constant provenance alone allowed ANY 0..255 value.
    let source=r#"
type Window = u8[10..20]
cell accept(n: Window) ->:
    yield n
cell entry_value() ->:
    yield accept(20)
"#;
    let ast=aura_parse::parse_source(source).expect("legal source parses");
    let original=lower_program(&ast).expect("legal refined call lowers");
    validate_module(&original).expect("baseline IR validates");
    for scenario in ["elided guard with tampered constant", "elided guard with original constant"] {
        let mut ir=original.clone();
        let entry=ir.functions.get_mut("entry_value").expect("lowered user call");
        // Simulate a forged post-optimization IR bundle with a stripped
        // certified range check, regardless of whether the optimizer keeps
        // guards in the new production architecture. This is an adversarial
        // boundary test, not an assertion about optimizer implementation.
        aura_ir::optimize_function(entry);
        for block in &mut entry.blocks {
            block.insts.retain(|inst| !matches!(&inst.kind,
                aura_ir::InstKind::RangeCheckU32{..} |
                aura_ir::InstKind::RangeCheck{..}));
        }
        let mut changed=false;
        for inst in entry.blocks.iter_mut().flat_map(|b| b.insts.iter_mut()) {
            if let aura_ir::InstKind::BindStrand{
                expr:aura_ir::RValue::ConstU32(n),..
            } = &mut inst.kind {
                if *n==20 && scenario=="elided guard with tampered constant" {
                    *n=21; changed=true;
                }
            }
        }
        if scenario=="elided guard with tampered constant" {
            assert!(changed,"must actually edit an IR scalar source value");
        }
        validate_module(&ir).expect("forged IR remains structurally valid");
        let err=emit_module(&ir,None).err()
            .expect("C emitter must refuse U32 -> refined U8 without retained range proof");
        assert!(err.message.contains("ABI") || err.message.contains("range") ||
                err.message.contains("narrow") || err.message.contains("type mismatch"),
            "{scenario}: expected an exact boundary error, got {err}");
    }
}
