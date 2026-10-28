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
    for (entry, expected) in [("named_projection", 7), ("main", 19)] {
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
