//! Aura source FieldAssign compared through the interpreter entry cell.
//! Z3 is explicitly disabled; AVM execution is NOT a formal proof.
#![cfg(not(feature = "z3"))]
use aura_interpret::{Avm, AvmConfig, AvmValue};

const SOURCE: &str = r#"
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

#[test]
fn actual_source_record_field_assignment_matches_native_expected_values_in_avm() {
    for (name, expected) in [("change_x",19),("preserve_y",13)] {
        let mut avm = Avm::new(AvmConfig {
            enable_z3_gate: false,
            ..Default::default()
        });
        let result = avm.exec_entry_cell(SOURCE, name)
            .expect("real parsed field assignment must execute in AVM");
        assert_eq!(result.value, AvmValue::Int(expected),
            "AVM must reflect functional Record local rebind in {name}");
        assert!(!result.verified, "no-Z3 AVM is not proof");
        assert!(result.gate_error.as_deref().unwrap_or("").contains("not run"),
            "unverified execution must disclose absent Z3 proof");
    }
}


const BRANCHED_SOURCE: &str = r#"
type Point = record { x: u32, y: u32 }
cell true_branch() ->:
    val mut p: Point = Point { x: 7, y: 13 }
    if true:
        p.x = 19
    else:
        p.x = 23
    yield p.x
cell false_branch() ->:
    val mut p: Point = Point { x: 7, y: 13 }
    if false:
        p.x = 19
    else:
        p.x = 23
    yield p.x
"#;

#[test]
fn real_source_record_join_via_if_else_has_correct_avm_values_without_proof() {
    for (name, expected) in [("true_branch", 19), ("false_branch", 23)] {
        let mut avm = Avm::new(AvmConfig {
            enable_z3_gate: false,
            ..Default::default()
        });
        let result = avm.exec_entry_cell(BRANCHED_SOURCE, name)
            .expect("AVM should execute source Record Phi-like local rebind");
        assert_eq!(result.value, AvmValue::Int(expected),
            "AVM Record if/else diverged from Clang/IR Oracle expected {name}");
        assert!(!result.verified, "AVM execution cannot claim formal proof");
        assert!(result.gate_error.as_deref().unwrap_or("").contains("not run"),
            "unverified AVM must disclose no proof");
    }
}


const RECORD_FUNCTION_SOURCE: &str = r#"
type Point = record { x: u32, y: u32 }
cell make_point(x: u32, y: u32) ->:
    yield Point { x: x, y: y }
cell identity_point(p: Point) ->:
    yield p
cell project_x(p: Point) ->:
    yield p.x
cell named_projection() ->:
    val p: Point = make_point(y: 13, x: 7)
    yield project_x(p)
cell main() ->:
    val mut p: Point = identity_point(make_point(y: 13, x: 7))
    p.x = 19
    yield project_x(p)
"#;

#[test]
fn real_source_record_cross_cell_named_calls_match_oracle_native_without_z3_proof() {
    // Compare AVM to the IR Oracle for the exact same parsed Aura program,
    // not to a parallel hand-written fixture with similar semantics.
    let ast = aura_parse::parse_source(RECORD_FUNCTION_SOURCE)
        .expect("Record function differential source parses");
    aura_core::Checker::new().check_program(&ast)
        .expect("Record function differential source is semantically valid");
    let ir = aura_core::lower_program(&ast)
        .expect("Record function differential source lowers to IR");
    aura_ir::validate_module(&ir).expect("Record function differential structural SSA");
    for (entry, expected) in [("named_projection", 7), ("main", 19)] {
        let oracle = aura_ir::run_oracle_entry(&ir, entry, &[],
            aura_ir::OracleConfig::default())
            .expect("same-source Record function Oracle");
        assert!(oracle.ok && oracle.stderr.is_empty());
        assert_eq!(oracle.return_value, Some(aura_ir::OracleValue::U32(expected as u32)),
            "same-source Record function IR Oracle observation");
        let mut avm = Avm::new(AvmConfig {
            enable_z3_gate: false,
            ..Default::default()
        });
        let result = avm.exec_entry_cell(RECORD_FUNCTION_SOURCE, entry)
            .expect("AVM must evaluate source user-defined Record cell calls");
        assert_eq!(result.value, AvmValue::Int(expected),
            "AVM Record function call {entry} must match IR Oracle and Clang");
        assert!(!result.verified, "AVM without Z3 must never claim formal proof");
        assert!(result.gate_error.as_deref().unwrap_or("").contains("not run"),
            "AVM Record function execution is not a Z3 proof");
    }
}


#[test]
fn avm_user_cell_rejects_wrong_nominal_record_argument_without_bitcast() {
    let source = r#"
type Point = record { x: u32, y: u32 }
type Twin = record { x: u32, y: u32 }
cell read_x(p: Point) ->:
    yield p.x
cell main() ->:
    yield read_x(Twin { x: 7, y: 13 })
"#;
    let mut avm = Avm::new(AvmConfig {
        enable_z3_gate: false,
        ..Default::default()
    });
    let err = avm.exec_entry_cell(source, "main")
        .expect_err("AVM cannot reinterpret identically shaped Twin as Point");
    assert!(err.to_string().contains("expects nominal Record 'Point'"),
        "wrong Record argument rejection: {err}");
}

#[test]
fn avm_user_cells_do_not_leak_into_next_source_or_recurse_forever() {
    let mut avm = Avm::new(AvmConfig {
        enable_z3_gate: false,
        ..Default::default()
    });
    let first = avm.exec_entry_cell(RECORD_FUNCTION_SOURCE, "named_projection")
        .expect("first source has registered make_point");
    assert_eq!(first.value, AvmValue::Int(7));
    let no_stale = avm.exec_entry_cell("cell main() ->:\n    yield make_point(7, 13)\n", "main")
        .expect_err("old source's user cells cannot be reachable");
    assert!(no_stale.to_string().contains("unknown call 'make_point'"),
        "AVM must not execute stale source definitions: {no_stale}");
    let rec = avm.exec_entry_cell("cell loop() ->:\n    yield loop()\n", "loop")
        .expect_err("recursive calls must have a finite safety bound");
    assert!(rec.to_string().contains("call depth exceeds"),
        "AVM recursion should fail closed instead of stack overflow: {rec}");
}


const NESTED_RECORD_SOURCE: &str = r#"
type Point = record { x: u32, y: u32 }
type Outer = record { p: Point, tag: u32 }
cell make_outer() ->:
    yield Outer { tag: 99, p: Point { x: 7, y: 13 } }
cell nested_before() ->:
    val o: Outer = make_outer()
    yield o.p.x
cell nested_after() ->:
    val mut o: Outer = make_outer()
    o.p = Point { x: 41, y: 43 }
    yield o.p.x
cell child_phi_true() ->:
    val mut p: Point = make_outer().p
    if true:
        p.x = 19
    yield p.x
cell child_phi_false() ->:
    val mut p: Point = make_outer().p
    if false:
        p.x = 19
    yield p.x
"#;

#[test]
fn real_source_nested_record_avm_matches_same_source_oracle_without_z3() {
    let ast = aura_parse::parse_source(NESTED_RECORD_SOURCE)
        .expect("nested source must parse");
    aura_core::Checker::new().check_program(&ast)
        .expect("nested source must typecheck");
    let ir = aura_core::lower_program(&ast).expect("nested source must lower");
    aura_ir::validate_module(&ir).expect("nested source SSA valid");
    for (entry, expected) in [
        ("nested_before", 7u32),
        ("nested_after", 41u32),
        ("child_phi_true", 19u32),
        ("child_phi_false", 7u32),
    ] {
        let oracle = aura_ir::run_oracle_entry(&ir, entry, &[],
            aura_ir::OracleConfig::default())
            .expect("nested source IR Oracle");
        assert!(oracle.ok && oracle.stderr.is_empty());
        assert_eq!(oracle.return_value, Some(aura_ir::OracleValue::U32(expected)));
        let mut avm = Avm::new(AvmConfig {
            enable_z3_gate: false,
            ..Default::default()
        });
        let result = avm.exec_entry_cell(NESTED_RECORD_SOURCE, entry)
            .expect("AVM must execute nested nominal Record with functional field update");
        assert_eq!(result.value, AvmValue::Int(i64::from(expected)),
            "nested Record Oracle/AVM mismatch in {entry}");
        assert!(!result.verified);
        assert!(result.gate_error.as_deref().unwrap_or("").contains("not run"));
    }
}


const RECORD_DEFAULTS_SOURCE: &str = r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
cell default_x() ->:
    val p: Point = Point {}
    yield p.x
cell default_y() ->:
    val p: Point = Point { x: 19 }
    yield p.y
cell explicit_x() ->:
    val p: Point = Point { x: 19 }
    yield p.x
"#;

#[test]
fn avm_omitted_scalar_record_defaults_match_same_source_ir_oracle() {
    let ast=aura_parse::parse_source(RECORD_DEFAULTS_SOURCE)
        .expect("default Record source must parse");
    aura_core::Checker::new().check_program(&ast)
        .expect("default Record source must typecheck");
    let ir=aura_core::lower_program(&ast).expect("defaults must materialize in IR");
    aura_ir::validate_module(&ir).expect("default SSA valid");
    for (entry, expected) in [
        ("default_x",7u32), ("default_y",13u32), ("explicit_x",19u32)
    ] {
        let oracle=aura_ir::run_oracle_entry(&ir,entry,&[],aura_ir::OracleConfig::default())
            .expect("default Record source Oracle");
        assert!(oracle.ok && oracle.stderr.is_empty());
        assert_eq!(oracle.return_value,Some(aura_ir::OracleValue::U32(expected)),
            "{entry}: IR Oracle default parity");
        let mut vm=Avm::new(AvmConfig {
            enable_z3_gate:false, ..Default::default()
        });
        let result=vm.exec_entry_cell(RECORD_DEFAULTS_SOURCE,entry)
            .expect("AVM must materialize defaults");
        assert_eq!(result.value,AvmValue::Int(i64::from(expected)),
            "{entry}: AVM/Oracle default parity");
        assert!(!result.verified);
        assert!(result.gate_error.as_deref().unwrap_or("").contains("not run"));
    }
}


#[test]
fn avm_record_boolean_defaults_are_typed_and_scoped_per_source() {
    let true_source=r#"
type Flag = record { enabled: bool = true, count: u32 = 17 }
cell main() ->:
    val f: Flag = Flag {}
    yield f.enabled
"#;
    let false_source=r#"
type Flag = record { enabled: bool = false, count: u32 = 19 }
cell main() ->:
    val f: Flag = Flag {}
    yield f.enabled
"#;
    let mut vm=Avm::new(AvmConfig {
        enable_z3_gate:false,..Default::default()
    });
    for (source, expected) in [(true_source,true),(false_source,false)] {
        let ast=aura_parse::parse_source(source).expect("bool default source parse");
        aura_core::Checker::new().check_program(&ast).expect("bool default source checker");
        let ir=aura_core::lower_program(&ast).expect("bool default lowerer");
        aura_ir::validate_module(&ir).expect("bool default SSA valid");
        let oracle=aura_ir::run_oracle_entry(&ir,"main",&[],aura_ir::OracleConfig::default())
            .expect("bool default IR Oracle");
        assert_eq!(oracle.return_value,Some(aura_ir::OracleValue::Bool(expected)));
        let result=vm.exec_entry_cell(source,"main")
            .expect("bool default AVM execution, no stale source definitions");
        assert_eq!(result.value,AvmValue::Bool(expected));
        assert!(!result.verified);
        assert!(result.gate_error.as_deref().unwrap_or("").contains("not run"));
    }
}


#[test]
fn avm_rejects_nonliteral_omitted_default_without_executing_it() {
    let source=r#"
type Point = record { x: u32 = 1 + 2, y: u32 = 13 }
cell main() ->:
    val p: Point = Point { y: 19 }
    yield p.x
"#;
    let mut vm=Avm::new(AvmConfig {
        enable_z3_gate:false,..Default::default()
    });
    let err=vm.exec_entry_cell(source,"main")
        .expect_err("unqualified default expression must not execute silently");
    assert!(err.to_string().contains("unqualified") &&
            err.to_string().contains("default"),
        "AVM omitted default must fail closed: {err}");
}


const NESTED_NOMINAL_DEFAULT_SOURCE: &str = r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
type Outer = record { p: Point = Point {}, tag: u32 = 99 }
cell default_nested_x() ->:
    val o: Outer = Outer {}
    yield o.p.x
cell override_nested_x() ->:
    val o: Outer = Outer { p: Point { x: 19, y: 23 } }
    yield o.p.x
cell override_outer_tag() ->:
    val o: Outer = Outer { tag: 44 }
    yield o.p.y
"#;

#[test]
fn avm_nested_nominal_record_defaults_match_same_source_ir_oracle() {
    let ast=aura_parse::parse_source(NESTED_NOMINAL_DEFAULT_SOURCE)
        .expect("nested defaults source parses");
    aura_core::Checker::new().check_program(&ast)
        .expect("nested defaults must be qualified by Core");
    let ir=aura_core::lower_program(&ast).expect("nested defaults must lower");
    aura_ir::validate_module(&ir).expect("nested defaults SSA valid");
    for (entry,expected) in [
        ("default_nested_x",7u32),
        ("override_nested_x",19u32),
        ("override_outer_tag",13u32),
    ] {
        let oracle=aura_ir::run_oracle_entry(&ir,entry,&[],aura_ir::OracleConfig::default())
            .expect("nested default IR Oracle");
        assert_eq!(oracle.return_value,Some(aura_ir::OracleValue::U32(expected)));
        let mut vm=Avm::new(AvmConfig {
            enable_z3_gate:false,..Default::default()
        });
        let actual=vm.exec_entry_cell(NESTED_NOMINAL_DEFAULT_SOURCE,entry)
            .expect("AVM must evaluate qualified nested default Record value");
        assert_eq!(actual.value,AvmValue::Int(i64::from(expected)));
        assert!(!actual.verified,"no-Z3 AVM must not claim proof");
        assert!(actual.gate_error.as_deref().unwrap_or("").contains("not run"));
    }
}


#[test]
fn avm_qualified_nested_defaults_fail_closed_for_dynamic_and_recursive_sources() {
    for (source,needle) in [
        (
            "type Point = record { x: u32 = 7 }\ntype Outer = record { p: Point = Point { x: 1 + 2 } }\ncell main() ->:\n    val a: Outer = Outer {}\n    yield a.p.x\n",
            "unqualified",
        ),
        (
            "type Node = record { next: Node = Node {} }\ncell main() ->:\n    val n: Node = Node {}\n    yield 7\n",
            "cyclic",
        ),
    ] {
        let mut vm=Avm::new(AvmConfig {
            enable_z3_gate:false,..Default::default()
        });
        let error=vm.exec_entry_cell(source,"main")
            .expect_err("AVM may not execute arbitrary or cyclic default trees");
        assert!(error.to_string().contains(needle),
            "AVM guard expected {needle:?}; got {error}");
    }
}


const NESTED_ASSIGN_PATH_SOURCE: &str = r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
type Outer = record { p: Point = Point {}, tag: u32 = 99 }
cell true_path() ->:
    val mut o: Outer = Outer {}
    if true:
        o.p.x = 19
    else:
        o.p.x = 23
    yield o.p.x
cell false_path() ->:
    val mut o: Outer = Outer {}
    if false:
        o.p.x = 19
    else:
        o.p.x = 23
    yield o.p.x
cell sibling() ->:
    val mut o: Outer = Outer {}
    o.p.x = 41
    yield o.p.y
cell copy_before() ->:
    val mut o: Outer = Outer {}
    val old: Outer = o
    o.p.x = 41
    yield old.p.x
"#;

#[test]
fn avm_nested_direct_assignment_matches_functional_ir_oracle() {
    let ast=aura_parse::parse_source(NESTED_ASSIGN_PATH_SOURCE)
        .expect("nested lvalue path source parse");
    aura_core::Checker::new().check_program(&ast)
        .expect("nested source Checker");
    let ir=aura_core::lower_program(&ast).expect("nested source SSA");
    aura_ir::validate_module(&ir).expect("SSA valid");
    for (entry,value) in [
        ("true_path",19u32),("false_path",23u32),
        ("sibling",13u32),("copy_before",7u32),
    ] {
        let oracle=aura_ir::run_oracle_entry(&ir,entry,&[],
            aura_ir::OracleConfig::default()).expect("nested path Oracle");
        assert_eq!(oracle.return_value,Some(aura_ir::OracleValue::U32(value)));
        let mut vm=Avm::new(AvmConfig {enable_z3_gate:false,..Default::default()});
        let actual=vm.exec_entry_cell(NESTED_ASSIGN_PATH_SOURCE,entry)
            .expect("AVM nested path lvalue");
        assert_eq!(actual.value,AvmValue::Int(i64::from(value)));
        assert!(!actual.verified);
        assert!(actual.gate_error.as_deref().unwrap_or("").contains("not run"));
    }
}


#[test]
fn avm_nested_record_lvalue_rejects_wrong_scalar_and_wrong_nominal_without_z3() {
    let wrong_scalar=r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
type Outer = record { p: Point = Point {}, tag: u32 = 99 }
cell main() ->:
    val mut o: Outer = Outer {}
    o.p.x = true
    yield o.p.x
"#;
    let wrong_nominal=r#"
type Point = record { x: u32 = 7, y: u32 = 13 }
type Twin = record { x: u32 = 7, y: u32 = 13 }
type Outer = record { p: Point = Point {}, tag: u32 = 99 }
cell main() ->:
    val mut o: Outer = Outer {}
    o.p = Twin {}
    yield o.p.x
"#;
    for (src,diagnostic) in [
        (wrong_scalar,"type mismatch"),
        (wrong_nominal,"nominal"),
    ] {
        let mut vm=Avm::new(AvmConfig {enable_z3_gate:false,..Default::default()});
        let e=vm.exec_entry_cell(src,"main")
            .expect_err("raw no-Z3 AVM must fail closed on ill-typed Record path assignment");
        assert!(e.to_string().contains(diagnostic),
            "expected {diagnostic:?}, got {e}");
    }
}


#[test]
fn avm_direct_record_constructor_rejects_missing_unknown_duplicate_and_wrong_field_types() {
    let prelude="type Point = record { x: u32, y: u32 }\n";
    for (body,needle) in [
        ("Point { x: 7 }","missing required"),
        ("Point { x: 7, y: 13, z: 19 }","unknown field"),
        ("Point { x: 7, x: 19, y: 13 }","duplicate"),
        ("Point { x: true, y: 13 }","type mismatch"),
        ("Bogus { x: 7 }","unknown Record"),
    ] {
        let source=format!(
            "{prelude}cell main() ->:\n    val p = {body}\n    yield p\n"
        );
        let mut vm=Avm::new(AvmConfig {
            enable_z3_gate:false,..Default::default()
        });
        let err=vm.exec_entry_cell(&source,"main")
            .expect_err("no-Z3 AVM must refuse malformed Record construction");
        assert!(err.to_string().contains(needle),
            "malformed constructor {body}: expected {needle:?}, got {err}");
    }
}

#[test]
fn avm_nested_record_constructor_refuses_structural_nominal_bitcasts_and_raw_overflow() {
    for (source,needle) in [
        (
            concat!(
                "type Point = record { x: u32, y: u32 }\n",
                "type Twin = record { x: u32, y: u32 }\n",
                "type Outer = record { p: Point, tag: u32 }\n",
                "cell main() ->:\n",
                "    val o = Outer { p: Twin { x: 7, y: 13 }, tag: 99 }\n",
                "    yield o.p.x\n"
            ),
            "type mismatch",
        ),
        (
            concat!(
                "type Tiny = record { n: u8 }\n",
                "cell main() ->:\n",
                "    val t = Tiny { n: 300 }\n",
                "    yield t.n\n"
            ),
            "type mismatch",
        ),
    ] {
        let mut vm=Avm::new(AvmConfig {
            enable_z3_gate:false,..Default::default()
        });
        let err=vm.exec_entry_cell(source,"main")
            .expect_err("raw AVM Record construction must validate nominal child and integer range");
        assert!(err.to_string().contains(needle),
            "expected {needle:?} from Record constructor: {err}");
    }
}


#[test]
fn avm_rejects_malformed_unused_record_schema_before_executing_entry_cell() {
    for (declarations,expected) in [
        ("type Bad = record { x: u32 = true }\n","default"),
        ("type Bad = record { x: u32 = 1 + 2 }\n","unqualified"),
        ("type Bad = record { x: u32 = 7, x: u32 = 13 }\n","duplicate"),
        ("type Node = record { next: Node = Node {} }\n","cyclic"),
        (
            concat!(
                "type Point = record { x: u32 = 7 }\n",
                "type Twin = record { x: u32 = 13 }\n",
                "type Outer = record { p: Point = Twin {} }\n"
            ),
            "nominal",
        ),
    ] {
        let source=format!("{declarations}cell main() ->:\n    yield 7\n");
        let mut vm=Avm::new(AvmConfig {
            enable_z3_gate:false,..Default::default()
        });
        let err=vm.exec_entry_cell(&source,"main")
            .expect_err("no-Z3 AVM cannot execute a source with malformed unused Record declaration");
        assert!(err.to_string().contains(expected),
            "expected {expected:?} for unused schema: {err}");
    }
}


#[test]
fn avm_rejects_out_of_range_unused_scalar_record_default_even_without_use() {
    for (declarations, expected) in [
        ("type Tiny = record { n: u8 = 300 }\n", "range"),
        ("type Tiny = record { n: i8 = 200 }\n", "range"),
    ] {
        let source=format!("{declarations}cell main() ->:\n    yield 7\n");
        let mut vm=Avm::new(AvmConfig {
            enable_z3_gate:false,..Default::default()
        });
        let err=vm.exec_entry_cell(&source,"main")
            .expect_err("AVM must validate width of unused Record default before entry execution");
        assert!(err.to_string().contains(expected),
            "expected {expected:?} width diagnostic, got {err}");
    }
}


#[test]
fn avm_rejects_duplicate_nominal_record_declarations_before_any_execution() {
    for defs in [
        "type Point = record { x: u32 = 7 }\ntype Point = record { x: u32 = 13 }\n",
        "type Point = record { x: u32 = 7 }\ntype Point = record { x: bool = true }\n",
    ] {
        let src=format!("{defs}cell main() ->:\n    yield 7\n");
        let mut vm=Avm::new(AvmConfig {
            enable_z3_gate:false,..Default::default()
        });
        let error=vm.exec_entry_cell(&src,"main")
            .expect_err("raw AVM must not silently overwrite a nominal Record schema");
        let msg=error.to_string();
        assert!(msg.contains("duplicate") && msg.contains("Point"),
            "expected duplicate nominal Record diagnostic, got {msg}");
    }
}

#[test]
fn raw_avm_rejects_cross_kind_type_namespace_collisions_before_entry() {
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
        let mut vm = Avm::new(AvmConfig { enable_z3_gate: false, ..Default::default() });
        let failure = vm.exec_entry_cell(&source, "main")
            .expect_err("raw no-Z3 AVM must not execute source with duplicate type names");
        let message = failure.to_string();
        assert!(message.contains("duplicate") && message.contains("Collision"),
            "{label}: expected exact type collision diagnosis, got {message}");
    }
}

#[test]
fn raw_avm_refuses_unrepresentable_local_and_negative_unsigned_record_values() {
    for (label, src) in [
        ("u8 local overflow",
         "type Tiny = record { n: u8 }\ncell main() ->:\n    val n = 300\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("i8 local overflow",
         "type Tiny = record { n: i8 }\ncell main() ->:\n    val n = 128\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n"),
        ("u8 local mutation",
         "type Tiny = record { n: u8 = 7 }\ncell main() ->:\n    val n = 300\n    val mut t: Tiny = Tiny {}\n    t.n = n\n    yield t.n\n"),
        ("u32 negative constructor",
         "type Tiny = record { n: u32 }\ncell main() ->:\n    val t: Tiny = Tiny { n: -1 }\n    yield t.n\n"),
        ("u32 negative mutation",
         "type Tiny = record { n: u32 = 7 }\ncell main() ->:\n    val mut t: Tiny = Tiny {}\n    t.n = -1\n    yield t.n\n"),
    ] {
        let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let err=vm.exec_entry_cell(src, "main")
            .expect_err("unrepresentable Record value must not execute in no-Z3 AVM");
        assert!(err.to_string().contains("Record"),
            "{label}: expected explicit Record runtime typing refusal: {err}");
    }
}

#[test]
fn raw_avm_must_validate_typed_integer_locals_even_if_never_used() {
    for (label, source) in [
        ("unused u8", "type Tiny = record { n: u8 = 7 }\ncell main() ->:\n    val n: u8 = 300\n    yield 7\n"),
        ("typed u8 copy", "type Tiny = record { n: u8 }\ncell main() ->:\n    val raw = 300\n    val n: u8 = raw\n    yield 7\n"),
        ("unused i8", "type Tiny = record { n: i8 = 7 }\ncell main() ->:\n    val n: i8 = 128\n    yield 7\n"),
        ("unsigned negative", "type Tiny = record { n: u8 = 7 }\ncell main() ->:\n    val n: u8 = -1\n    yield 7\n"),
    ] {
        let mut avm=Avm::new(AvmConfig { enable_z3_gate:false,..Default::default() });
        let err=avm.exec_entry_cell(source,"main")
            .expect_err("raw AVM must reject invalid typed locals before success");
        let message=err.to_string();
        assert!(message.contains("typed") || message.contains("range"),
            "{label}: expected narrow-local range diagnostic: {message}");
    }
}

#[test]
fn raw_avm_preserves_valid_typed_local_bounds_without_proof() {
    for (label,src,expected) in [
        ("typed u8","type Tiny = record { n: u8 }\ncell main() ->:\n    val n: u8 = 255\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n",255),
        ("typed i8","type Signed = record { n: i8 }\ncell main() ->:\n    val n: i8 = 127\n    val t: Signed = Signed { n: n }\n    yield t.n\n",127),
    ] {
        let mut avm=Avm::new(AvmConfig { enable_z3_gate:false, ..Default::default() });
        let result=avm.exec_entry_cell(src,"main")
            .unwrap_or_else(|e| panic!("{label}: typed boundary must execute: {e}"));
        assert_eq!(result.value,AvmValue::Int(expected), "{label}");
        assert!(!result.verified, "raw AVM cannot claim proof");
    }
}

#[test]
fn raw_avm_rejects_typed_integer_local_reassignment_even_if_unused() {
    for (label,src) in [
        ("u8 direct overflow",
         "type Tiny = record { n: u8 }\ncell main() ->:\n    val mut n: u8 = 7\n    n = 300\n    yield 7\n"),
        ("i8 direct overflow",
         "type Signed = record { n: i8 }\ncell main() ->:\n    val mut n: i8 = 7\n    n = 128\n    yield 7\n"),
        ("u8 indirect overflow",
         "type Tiny = record { n: u8 }\ncell main() ->:\n    val mut n: u8 = 7\n    val raw = 300\n    n = raw\n    yield 7\n"),
        ("u8 negative",
         "type Tiny = record { n: u8 }\ncell main() ->:\n    val mut n: u8 = 7\n    n = -1\n    yield 7\n"),
    ] {
        let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let failure=vm.exec_entry_cell(src,"main")
            .expect_err("typed numeric local must not accept a violating reassignment");
        let msg=failure.to_string();
        assert!(msg.contains("typed") && msg.contains("range"),
            "{label}: expected source-local width diagnosis, got {msg}");
    }
}
#[test]
fn raw_avm_typed_integer_guards_are_call_frame_isolated() {
    let source=r#"
type Tiny = record { n: u8 }
cell inner() ->:
    val mut n: u8 = 7
    n = 255
    yield n
cell main() ->:
    val mut n: i8 = 7
    val seen = inner()
    n = 127
    yield n
"#;
    let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
    let result=vm.exec_entry_cell(source,"main")
        .expect("valid typed local assignments across call frames must succeed");
    assert_eq!(result.value,AvmValue::Int(127));
    assert!(!result.verified, "raw AVM is not proof");
}

#[test]
fn raw_avm_numeric_parameters_and_nested_call_frames_obey_declared_widths() {
    for (label,src) in [
        ("u8 parameter overflow", "type Tiny = record { n: u8 }\ncell identity(n: u8) ->:\n    yield n\ncell main() ->:\n    yield identity(300)\n"),
        ("i8 parameter overflow", "type Signed = record { n: i8 }\ncell identity(n: i8) ->:\n    yield n\ncell main() ->:\n    yield identity(128)\n"),
        ("nested bad typed write", "type Tiny = record { n: u8 }\ncell bad() ->:\n    val mut n: u8 = 7\n    n = 300\n    yield n\ncell main() ->:\n    yield bad()\n"),
    ] {
        let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let err=vm.exec_entry_cell(src,"main")
            .expect_err("numeric contracts must be enforced across user call frames");
        let msg=err.to_string();
        assert!(msg.contains("typed") && msg.contains("range"),
            "{label}: expected typed integer width diagnostic, got {msg}");
    }
    for (label,src,expected) in [
        ("u8 boundary parameter", "type Tiny = record { n: u8 }\ncell identity(n: u8) ->:\n    yield n\ncell main() ->:\n    yield identity(255)\n",255),
        ("i8 boundary parameter", "type Signed = record { n: i8 }\ncell identity(n: i8) ->:\n    yield n\ncell main() ->:\n    yield identity(127)\n",127),
    ] {
        let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let result=vm.exec_entry_cell(src,"main")
            .unwrap_or_else(|err| panic!("{label}: valid parameter must execute: {err}"));
        assert_eq!(result.value,AvmValue::Int(expected), "{label}");
        assert!(!result.verified);
    }
}
#[test]
fn raw_avm_integer_contract_registry_clears_between_source_runs() {
    let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
    for (src, expected) in [
        ("cell main() ->:\n    val mut n: u8 = 7\n    n = 255\n    yield n\n",255),
        ("cell main() ->:\n    val mut n: i8 = 7\n    n = 127\n    yield n\n",127),
    ] {
        let result=vm.exec_entry_cell(src,"main")
            .expect("each source must have a distinct typed-local registry");
        assert_eq!(result.value,AvmValue::Int(expected));
        assert!(!result.verified);
    }
}

#[test]
fn raw_avm_type_alias_integer_width_is_not_erased_for_local_and_calls() {
    for (label,source) in [
        ("unused aliased u8", "type Byte = u8\ncell main() ->:\n    val n: Byte = 300\n    yield 7\n"),
        ("unused chained i8", "type Small = i8\ntype Another = Small\ncell main() ->:\n    val n: Another = 128\n    yield 7\n"),
        ("aliased local mutable overflow", "type Byte = u8\ncell main() ->:\n    val mut n: Byte = 7\n    n = 300\n    yield 7\n"),
        ("aliased u8 user parameter", "type Byte = u8\ncell identity(n: Byte) ->:\n    yield n\ncell main() ->:\n    yield identity(300)\n"),
        ("aliased i8 user parameter", "type Small = i8\ncell identity(n: Small) ->:\n    yield n\ncell main() ->:\n    yield identity(128)\n"),
        ("aliased Record field overflow", "type Byte = u8\ntype Tiny = record { n: Byte }\ncell main() ->:\n    val t: Tiny = Tiny { n: 300 }\n    yield t.n\n"),
        ("unused aliased Record default", "type Byte = u8\ntype Tiny = record { n: Byte = 300 }\ncell main() ->:\n    yield 7\n"),
    ] {
        let mut avm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let error=avm.exec_entry_cell(source,"main")
            .expect_err("raw no-Z3 AVM must not erase type alias numeric width");
        let message=error.to_string();
        assert!(message.contains("range") || message.contains("mismatch"),
            "{label}: expected alias-resolved numeric range diagnostic, got {message}");
    }
}

#[test]
fn raw_avm_type_alias_integer_boundaries_and_defaults_execute_without_proof() {
    for (label, source, expected) in [
        ("u8 local and Record",
         "type Byte = u8\ntype Tiny = record { n: Byte }\ncell main() ->:\n    val n: Byte = 255\n    val t: Tiny = Tiny { n: n }\n    yield t.n\n", 255),
        ("chained i8 and Record",
         "type Small = i8\ntype Number = Small\ntype Signed = record { n: Number }\ncell main() ->:\n    val n: Number = 127\n    val s: Signed = Signed { n: n }\n    yield s.n\n",127),
        ("u8 aliased Record default",
         "type Byte = u8\ntype Tiny = record { n: Byte = 255 }\ncell main() ->:\n    val t: Tiny = Tiny {}\n    yield t.n\n",255),
        ("u8 aliased parameter",
         "type Byte = u8\ncell identity(n: Byte) ->:\n    yield n\ncell main() ->:\n    yield identity(255)\n",255),
        ("u8 aliased mutable",
         "type Byte = u8\ncell main() ->:\n    val mut n: Byte = 7\n    n = 255\n    yield n\n",255),
    ] {
        let mut avm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let result=avm.exec_entry_cell(source,"main")
            .unwrap_or_else(|e|panic!("{label}: legal alias-bound integer should execute: {e}"));
        assert_eq!(result.value, AvmValue::Int(expected),"{label}");
        assert!(!result.verified,"raw AVM cannot attest Z3 proof");
    }
}

#[test]
fn raw_avm_type_alias_resolution_does_not_leak_between_source_loads() {
    let mut avm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
    let first="type Width = u8\ncell main() ->:\n    val n: Width = 200\n    yield n\n";
    assert_eq!(avm.exec_entry_cell(first,"main").unwrap().value,AvmValue::Int(200));
    let second="type Width = i8\ncell main() ->:\n    val n: Width = 200\n    yield n\n";
    let error=avm.exec_entry_cell(second,"main")
        .expect_err("new source must resolve Width as i8, not stale u8");
    assert!(error.to_string().contains("range") || error.to_string().contains("mismatch"),
        "source-specific alias state required: {error}");
}

#[test]
fn raw_avm_rejects_unused_cyclic_or_unknown_monomorphic_aliases() {
    for (label,declarations) in [
        ("direct cycle", "type Loop = Loop\n"),
        ("two aliases", "type First = Second\ntype Second = First\n"),
        ("transitive cycle", "type First = Second\ntype Second = Third\ntype Third = First\n"),
        ("unknown target", "type Phantom = MissingTarget\n"),
        ("unknown after chain", "type First = Second\ntype Second = MissingTarget\n"),
    ] {
        let source=format!("{declarations}cell main() ->:\n    yield 7\n");
        let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let err=vm.exec_entry_cell(&source,"main")
            .expect_err("invalid unused alias must fail during source registration");
        let diagnostic=err.to_string();
        assert!(diagnostic.contains("TypeAlias") &&
            (diagnostic.contains("cyclic") || diagnostic.contains("unknown")),
            "{label}: expected alias cycle/unknown diagnostic: {diagnostic}");
    }
}

#[test]
fn raw_avm_legal_forward_monomorphic_integer_alias_record_executes_unverified() {
    let source=r#"
type Top = Middle
type Middle = u8
type Tiny = record { n: Top = 255 }
cell main() ->:
    val n: Top = 255
    val t: Tiny = Tiny { n: n }
    yield t.n
"#;
    let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
    let out=vm.exec_entry_cell(source,"main").expect("forward alias must resolve in raw AVM");
    assert_eq!(out.value,AvmValue::Int(255));
    assert!(!out.verified,"raw AVM is not formal proof");
}

#[test]
fn raw_avm_rejects_invalid_unused_refined_alias_declarations() {
    for (label,decl) in [
        ("u8 too wide", "type Bad = u8[0..300]\n"),
        ("i8 too wide", "type Bad = i8[0..128]\n"),
        ("bool range", "type Bad = bool[0..1]\n"),
        ("float range", "type Bad = f32[0..1]\n"),
        ("u8 impossible lower", "type Bad = u8[256..300]\n"),
        ("nested widening", "type Small = u8[10..20]\ntype Wide = Small[10..21]\n"),
    ] {
        let src=format!("{decl}cell main() ->:\n    yield 7\n");
        let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let err=vm.exec_entry_cell(&src,"main")
            .expect_err("raw noZ3 AVM cannot execute source with invalid unused refined alias");
        assert!(err.to_string().contains("range") || err.to_string().contains("refin"),
            "{label}: expected refinement range diagnostic: {err}");
    }
}

#[test]
fn raw_avm_accepts_valid_unused_nested_refined_alias_declarations_without_proof() {
    let src="type Small = u8[10..20]\ntype Narrow = Small[12..18]\n\
        cell main() ->:\n    yield 7\n";
    let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
    let out=vm.exec_entry_cell(src,"main")
        .expect("valid unused source range declaration should not prevent execution");
    assert_eq!(out.value,AvmValue::Int(7));
    assert!(!out.verified,"raw AVM is not SMT proof");
}

#[test]
fn raw_avm_refined_alias_values_must_obey_source_interval_in_locals_and_calls() {
    for (label, source) in [
        ("local high", "type Window = u8[10..20]\ncell main() ->:\n    val n: Window = 21\n    yield 7\n"),
        ("local low", "type Window = u8[10..20]\ncell main() ->:\n    val n: Window = 9\n    yield 7\n"),
        ("mutation", "type Window = u8[10..20]\ncell main() ->:\n    val mut n: Window = 10\n    n = 21\n    yield n\n"),
        ("nested", "type Window = u8[10..20]\ntype Narrow = Window[12..18]\ncell main() ->:\n    val n: Narrow = 19\n    yield n\n"),
        ("parameter", "type Window = u8[10..20]\ncell identity(n: Window) ->:\n    yield n\ncell main() ->:\n    yield identity(21)\n"),
    ] {
        let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let err=vm.exec_entry_cell(source,"main")
            .expect_err("raw AVM must reject refined alias out-of-interval value");
        let txt=err.to_string();
        assert!(txt.contains("range") || txt.contains("refin"),
            "{label}: expected explicit source interval refusal: {txt}");
    }
}

#[test]
fn raw_avm_refined_alias_local_mutation_and_user_call_valid_boundaries() {
    for (label,source,value) in [
        ("local lower", "type Window = u8[10..20]\ncell main() ->:\n    val n: Window = 10\n    yield n\n",10),
        ("local upper", "type Window = u8[10..20]\ncell main() ->:\n    val n: Window = 20\n    yield n\n",20),
        ("mutation", "type Window = u8[10..20]\ncell main() ->:\n    val mut n: Window = 10\n    n = 20\n    yield n\n",20),
        ("nested", "type Window = u8[10..20]\ntype Narrow = Window[12..18]\ncell main() ->:\n    val n: Narrow = 18\n    yield n\n",18),
        ("parameter", "type Window = u8[10..20]\ncell identity(n: Window) ->:\n    yield n\ncell main() ->:\n    yield identity(20)\n",20),
    ] {
        let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let out=vm.exec_entry_cell(source,"main")
            .unwrap_or_else(|e|panic!("{label}: valid refined alias boundary: {e}"));
        assert_eq!(out.value, AvmValue::Int(value),"{label}");
        assert!(!out.verified,"AVM execution does not certify Z3 proof");
    }
}

#[test]
fn raw_avm_refined_alias_record_defaults_and_field_writes_enforce_interval() {
    for (label,source) in [
        ("unused default", "type Window = u8[10..20]\ntype Tiny = record { n: Window = 21 }\ncell main() ->:\n    yield 7\n"),
        ("explicit field", "type Window = u8[10..20]\ntype Tiny = record { n: Window }\ncell main() ->:\n    val t: Tiny = Tiny { n: 21 }\n    yield t.n\n"),
        ("mutation", "type Window = u8[10..20]\ntype Tiny = record { n: Window }\ncell main() ->:\n    val mut t: Tiny = Tiny { n: 10 }\n    t.n = 21\n    yield t.n\n"),
    ] {
        let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
        let err=vm.exec_entry_cell(source,"main")
            .expect_err("raw AVM must enforce refined Record source interval");
        let message=err.to_string();
        assert!(message.contains("range") || message.contains("refin"),
            "{label}: expected range failure: {message}");
    }
    let valid="type Window = u8[10..20]\ntype Tiny = record { n: Window = 20 }\ncell main() ->:\n    val mut t: Tiny = Tiny {}\n    t.n = 10\n    yield t.n\n";
    let mut vm=Avm::new(AvmConfig{enable_z3_gate:false,..Default::default()});
    let out=vm.exec_entry_cell(valid,"main")
        .expect("valid refined field default and mutation");
    assert_eq!(out.value,AvmValue::Int(10));
    assert!(!out.verified);
}
