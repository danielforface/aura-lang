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

#[test]
fn raw_z3_tracked_negation_of_ground_truth_is_unsat(){
    use z3::{ast::{Ast,Bool,Int},Config,Context,SatResult,Solver};
    let ctx=Context::new(&Config::new());
    let label=Bool::new_const(&ctx,"negated_goal_0");
    let truth=Int::from_u64(&ctx,1)._eq(&Int::from_u64(&ctx,1));
    let negated_goal=truth.not();
    let solver=Solver::new(&ctx);
    solver.assert(&label.implies(&negated_goal));
    let result=solver.check_assumptions(&[label]);
    eprintln!("RAW Z3 check_assumptions ground equality: truth={truth}; negation={negated_goal}; result={result:?}");
    assert_eq!(result,SatResult::Unsat,"goal guard must be enforced by Z3");
}
#[test]
fn aura_fast_ground_integer_reflexivity_must_prove(){
    let src="cell main() ->:\n    assert 1 == 1\n";
    let parsed=aura_parse::parse_source(src).expect("parse");
    let mut prover=Z3Prover::new();let mut nexus=NexusContext::default();
    verify_program_z3_profile(&parsed,&mut prover,&(),&mut nexus,SmtProfile::Fast)
        .expect("Fast must prove plain ground reflexivity too");
}

#[test]
fn raw_z3_replicates_aura_solver_parameters_on_empty_state(){
    use z3::{ast::{Ast,Bool,Int},Config,Context,Params,SatResult,Solver};
    let ctx=Context::new(&Config::new());
    let mut params=Params::new(&ctx);
    params.set_u32("timeout",2_000);
    params.set_u32("smt.random_seed",0);
    params.set_u32("sat.random_seed",0);
    params.set_bool("unsat_core",true);
    let solver=Solver::new(&ctx);
    solver.set_params(&params);
    let label=Bool::new_const(&ctx,"negated_goal_0");
    let eq=Int::from_u64(&ctx,1)._eq(&Int::from_u64(&ctx,1));
    let negative=eq.not();
    solver.assert(&label.implies(&negative));
    let assumptions=[label];
    eprintln!("REPLICATED raw params, guarded goal: {}",negative);
    let result=solver.check_assumptions(&assumptions);
    eprintln!("REPLICATED raw params, result: {:?}", result);
    assert_eq!(result,SatResult::Unsat,"same params and proof protocol must be UNSAT");
}
#[test]
fn prover_context_raw_negation_is_unsat(){
    use z3::{ast::{Ast,Bool,Int},SatResult,Solver};
    let prover=Z3Prover::new();
    let ctx=prover.ctx_static();
    let solver=Solver::new(ctx);
    let label=Bool::new_const(ctx,"negated_goal_0");
    let eq=Int::from_u64(ctx,1)._eq(&Int::from_u64(ctx,1));
    solver.assert(&label.implies(&eq.not()));
    let sat=solver.check_assumptions(&[label]);
    eprintln!("LEAKED prover context raw check: {:?}",sat);
    assert_eq!(sat,SatResult::Unsat);
}

#[test]
fn aura_thorough_false_ground_equality_must_not_prove() {
    verify_source("cell main() ->:\n    assert 1 == 2\n")
        .expect_err("false ground equality must never be proved");
}
#[test]
fn aura_fast_false_ground_equality_must_not_prove() {
    let src="cell main() ->:\n    assert 1 == 2\n";
    let parsed=aura_parse::parse_source(src).expect("parse");
    let mut prover=Z3Prover::new();let mut nexus=NexusContext::default();
    verify_program_z3_profile(&parsed,&mut prover,&(),&mut nexus,SmtProfile::Fast)
        .expect_err("false ground equality must be refuted in Fast as well");
}
