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
