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
