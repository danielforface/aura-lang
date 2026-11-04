//! EV-194 RED: Phi must be a contiguous block-prefix and known-typed
//! terminator operands must match their primitive CFG/return contracts.
//! Positive tests run real Aura source before and after IR optimization.
use aura_core::{lower_program, Checker};
use aura_ir::{optimize_module, run_oracle_entry, validate_module, Inst, InstKind,
    OracleConfig, OracleValue, RValue, Terminator, ValueId};
use std::collections::BTreeMap;

const SOURCE: &str = r#"
cell join(flag: bool, a: u32, b: u32) ->:
    val mut chosen: u32 = a
    if flag:
        chosen = a + 1
    else:
        chosen = b + 2
    yield chosen
cell loop_sum(start: u32, limit: u32) ->:
    val mut cursor: u32 = start
    while cursor < limit:
        cursor = cursor + 1
    yield cursor
cell dense(flag: bool, n: u32, x: u32) ->:
    match n:
        0:
            yield x + 1
        1:
            yield x + 2
        2:
            yield x + 3
        3:
            yield x + 4
        _:
            yield x
    yield x
cell const_join(a: u32, b: u32) ->:
    val mut chosen: u32 = a
    if true:
        chosen = a + 1
    else:
        chosen = b + 1
    yield chosen
"#;

fn real_source() -> aura_ir::ModuleIR {
    let ast = aura_parse::parse_source(SOURCE).expect("real EV-194 source parses");
    Checker::new().check_program(&ast).expect("real EV-194 source checks");
    let ir = lower_program(&ast).expect("real EV-194 source lowers");
    validate_module(&ir).expect("real EV-194 CFG and Phi semantics validate");
    ir
}

fn expect_refusal(ir: &aura_ir::ModuleIR, fragment: &str, label: &str) {
    let err = validate_module(ir).expect_err(label);
    assert!(err.message.to_lowercase().contains(&fragment.to_lowercase()),
        "{label}: expected '{fragment}' in {err}");
}

#[test]
fn source_positive_optimizer_preserves_ssa_and_oracle_outcomes() {
    let original = real_source();
    let cases: BTreeMap<&str,Vec<(Vec<OracleValue>,OracleValue)>> = BTreeMap::from([
        ("join",vec![
            (vec![OracleValue::Bool(true),OracleValue::U32(7),OracleValue::U32(13)],OracleValue::U32(8)),
            (vec![OracleValue::Bool(false),OracleValue::U32(7),OracleValue::U32(13)],OracleValue::U32(15))]),
        ("loop_sum",vec![
            (vec![OracleValue::U32(0),OracleValue::U32(4)],OracleValue::U32(4)),
            (vec![OracleValue::U32(4),OracleValue::U32(4)],OracleValue::U32(4))]),
        ("dense",vec![
            (vec![OracleValue::Bool(true),OracleValue::U32(0),OracleValue::U32(7)],OracleValue::U32(8)),
            (vec![OracleValue::Bool(false),OracleValue::U32(4),OracleValue::U32(7)],OracleValue::U32(7))]),
        ("const_join",vec![
            (vec![OracleValue::U32(7),OracleValue::U32(13)],OracleValue::U32(8))]),
    ]);
    let mut optimized = original.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimizer must preserve reachable SSA and Phi CFG invariants");
    for (name,rows) in cases {
        for (args,expect) in rows {
            for (label,ir) in [("original",&original),("optimized",&optimized)] {
                let got=run_oracle_entry(ir,name,&args,OracleConfig::default())
                    .expect("source Oracle execution");
                assert!(got.ok,"{name} {label}: {got:?}");
                assert_eq!(got.return_value,Some(expect.clone()),"{name} {label}: {args:?}");
            }
        }
    }
}

#[test]
fn malformed_phi_after_non_phi_must_fail_closed() {
    let mut ir=real_source();
    let f=ir.functions.get_mut("join").unwrap();
    let bb=f.blocks.iter_mut().find(|bb|bb.insts.iter().any(|i|matches!(i.kind,InstKind::Phi{..})))
        .expect("source join produces Phi");
    let span=bb.span;
    bb.insts.insert(0,Inst {
        span,dest:Some(ValueId(0xffffffe1)),
        kind:InstKind::BindStrand{name:"$non_phi_before_phi".to_string(),expr:RValue::ConstU32(3)},
    });
    expect_refusal(&ir,"Phi","non-Phi instruction before Phi must be rejected");
}

#[test]
fn malformed_empty_phi_and_missing_destination_must_fail_closed() {
    let good=real_source();
    let mut empty=good.clone();
    let f=empty.functions.get_mut("join").unwrap();
    let entry=f.entry;
    let bb=f.blocks.iter_mut().find(|bb|bb.id==entry).unwrap();
    bb.insts.insert(0,Inst {
        span:bb.span,dest:Some(ValueId(0xffffffe2)),
        kind:InstKind::Phi {incomings:vec![]},
    });
    expect_refusal(&empty,"Phi","empty Phi in entry with no CFG predecessors is forbidden");

    let mut missing=good;
    let f=missing.functions.get_mut("join").unwrap();
    let phi=f.blocks.iter_mut().flat_map(|bb|bb.insts.iter_mut())
        .find(|i|matches!(i.kind,InstKind::Phi{..})).unwrap();
    phi.dest=None;
    expect_refusal(&missing,"Phi","Phi missing SSA destination is forbidden");
}

#[test]
fn known_typed_terminator_contracts_must_fail_closed() {
    let baseline=real_source();
    let mut bad_cond=baseline.clone();
    let f=bad_cond.functions.get_mut("join").unwrap();
    let u32_param=f.params[1].value;
    let entry=f.blocks.iter_mut().find(|bb|matches!(bb.term,Terminator::CondBr{..}))
        .expect("if has a conditional");
    let Terminator::CondBr{cond,..}=&mut entry.term else {unreachable!()};
    *cond=u32_param;
    expect_refusal(&bad_cond,"type","CondBr cannot use a known u32 parameter");

    let mut bad_switch=baseline.clone();
    let f=bad_switch.functions.get_mut("dense").unwrap();
    let bool_param=f.params[0].value;
    let dispatch=f.blocks.iter_mut().find(|bb|matches!(bb.term,Terminator::Switch{..}))
        .expect("dense integer match uses switch");
    let Terminator::Switch{scrut,..}=&mut dispatch.term else {unreachable!()};
    *scrut=bool_param;
    expect_refusal(&bad_switch,"type","Switch cannot use a known bool parameter");

    let mut bad_return=baseline;
    let f=bad_return.functions.get_mut("join").unwrap();
    let bool_param=f.params[0].value;
    let ret=f.blocks.iter_mut().find(|bb|matches!(bb.term,Terminator::Return(Some(_))))
        .expect("source has return");
    ret.term=Terminator::Return(Some(bool_param));
    expect_refusal(&bad_return,"type","U32 return cannot yield known bool parameter");
}
