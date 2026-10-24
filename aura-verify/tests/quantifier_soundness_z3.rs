//! Issue #24: distinguish correct raw Z3 from Aura's generated proof model.
//! These tests are intentionally expected to expose current Thorough false negatives.
#![cfg(feature = "z3")]
use aura_nexus::NexusContext;
use aura_verify::{verify_program_z3_profile, SmtProfile, Z3Prover};

fn verify_source(src: &str) -> Result<(), aura_verify::VerifyError> {
    let parsed = aura_parse::parse_source(src).expect("Aura source must parse");
    eprintln!("PARSED SOURCE AST: {:#?}", parsed.stmts);
    let mut prover = Z3Prover::new();
    let mut nexus = NexusContext::default();
    verify_program_z3_profile(&parsed, &mut prover, &(), &mut nexus, SmtProfile::Thorough)
}
#[test]
fn raw_z3_negated_universal_reflexivity_is_unsat() {
    use z3::{ast::{Ast,Int},Config,Context,SatResult,Solver};
    let ctx=Context::new(&Config::new());
    let x=Int::new_const(&ctx, "quant_x");
    let q=z3::ast::forall_const(&ctx, &[&x], &[], &x._eq(&x));
    let solver=Solver::new(&ctx);
    solver.assert(&q.not());
    let result=solver.check();
    eprintln!("RAW Z3 universal: {q}; result for negation: {result:?}");
    assert_eq!(result,SatResult::Unsat,"raw Z3 should prove reflexivity");
}
#[test]
fn aura_thorough_ground_integer_reflexivity_must_prove() {
    let src="cell main() ->:\n    assert 1 == 1\n";
    verify_source(src).expect("Issue #24: ground reflexivity must prove under Thorough");
}
#[test]
fn aura_thorough_universal_reflexivity_must_prove() {
    let src="cell main() ->:\n    assert forall(x: u32): x == x\n";
    verify_source(src).expect("Issue #24: universal reflexivity must prove under Thorough");
}
#[test]
fn aura_thorough_symbolic_assignment_equality_must_prove() {
    let src="cell main() ->:\n    val x: u32 = 1\n    assert x == 1\n";
    verify_source(src).expect("control: assignment equality should prove");
}
#[test]
fn aura_thorough_false_universal_property_must_fail() {
    let src="cell main() ->:\n    assert forall(x: u32): x == 1\n";
    assert!(verify_source(src).is_err(),"invalid universally quantified formula cannot prove");
}
