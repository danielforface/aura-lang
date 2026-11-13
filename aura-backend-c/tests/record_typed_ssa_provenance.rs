//! EV-195 test-first: known SSA primitive type provenance through
//! Phi, Binary and BindStrand::Local. Build valid Aura IR then mutate
//! only the relevant SSA instruction/Terminator operand.
use aura_core::{lower_program, Checker};
use aura_ir::{optimize_module, run_oracle_entry, validate_module, BinOp, Inst,
    InstKind, ModuleIR, OracleConfig, OracleValue, RValue, Terminator, ValueId};

const SOURCE: &str = r#"
cell join(flag: bool, a: u32, b: u32) ->:
    val mut chosen: u32 = a
    if flag:
        chosen = a + 1
    else:
        chosen = b + 2
    yield chosen
cell switch_case(flag: bool, n: u32, x: u32) ->:
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
cell loop_sum(start: u32, limit: u32) ->:
    val mut cursor: u32 = start
    while cursor < limit:
        cursor = cursor + 1
    yield cursor
"#;

fn real_ir() -> ModuleIR {
    let ast=aura_parse::parse_source(SOURCE).expect("real Aura source parses");
    Checker::new().check_program(&ast).expect("real Aura source checks");
    let ir=lower_program(&ast).expect("real Aura lowers");
    validate_module(&ir).expect("source-valid SSA must validate");
    ir
}

fn expect_known_type_rejection(ir: &ModuleIR, label: &str) {
    let err=validate_module(ir).expect_err("known incompatible SSA categories must reject");
    assert!(err.message.contains("type") || err.message.contains("Phi"),
        "{label}: expected typed SSA diagnostic: {err}");
}

#[test]
fn source_and_optimizer_remain_qualified_with_loops_joins_switches() {
    let src=real_ir();
    let mut optimized=src.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimizer preserves valid SSA invariants");
    let cases: Vec<(&str,Vec<OracleValue>,u32)> = vec![
        ("join",vec![OracleValue::Bool(true),OracleValue::U32(7),OracleValue::U32(13)],8),
        ("join",vec![OracleValue::Bool(false),OracleValue::U32(7),OracleValue::U32(13)],15),
        ("switch_case",vec![OracleValue::Bool(true),OracleValue::U32(0),OracleValue::U32(7)],8),
        ("switch_case",vec![OracleValue::Bool(false),OracleValue::U32(3),OracleValue::U32(7)],11),
        ("switch_case",vec![OracleValue::Bool(true),OracleValue::U32(8),OracleValue::U32(7)],7),
        ("loop_sum",vec![OracleValue::U32(0),OracleValue::U32(4)],4),
        ("loop_sum",vec![OracleValue::U32(4),OracleValue::U32(4)],4),
    ];
    for (name,args,want) in cases {
        for (label,ir) in [("original",&src),("optimized",&optimized)] {
            let got=run_oracle_entry(ir,name,&args,OracleConfig::default())
                .expect("real source Oracle");
            assert!(got.ok,"{label} {name}: {got:?}");
            assert_eq!(got.return_value,Some(OracleValue::U32(want)),
                "{label} {name}: {args:?}");
        }
    }
}

#[test]
fn phi_mixed_known_boolean_and_integer_incomings_must_reject() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("join").unwrap();
    let bool_param=f.params[0].value;
    let phi=f.blocks.iter_mut().flat_map(|b|b.insts.iter_mut())
        .find(|i|matches!(i.kind,InstKind::Phi { .. })).expect("join has Phi");
    let InstKind::Phi{incomings}=&mut phi.kind else {unreachable!()};
    assert_eq!(incomings.len(),2,"must exercise genuine two-way join");
    incomings[0].1=bool_param; // dominates both edges; only type is wrong
    expect_known_type_rejection(&ir,"mixed Phi bool / u32");
}

#[test]
fn bool_phi_returned_by_u32_cell_must_reject() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("join").unwrap();
    let bool_param=f.params[0].value;
    let phi=f.blocks.iter_mut().flat_map(|b|b.insts.iter_mut())
        .find(|i|matches!(i.kind,InstKind::Phi { .. })).expect("join Phi");
    let InstKind::Phi{incomings}=&mut phi.kind else {unreachable!()};
    for (_,v) in incomings.iter_mut() {*v=bool_param;}
    // Valid predecessor provenance and dominating SSA, but derived Phi Bool
    // cannot satisfy the U32 cell's Return ABI.
    expect_known_type_rejection(&ir,"typed Phi Bool used as U32 Return");
}

#[test]
fn integer_binary_used_as_condbr_condition_must_reject() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("join").unwrap();
    let a=f.params[1].value;
    let b=f.params[2].value;
    let bb=f.blocks.iter_mut().find(|b|matches!(b.term,Terminator::CondBr{..}))
        .expect("source if has CondBr");
    let synthetic=ValueId(0xffff_ff10);
    bb.insts.push(Inst {span:bb.span,dest:Some(synthetic),
        kind:InstKind::Binary{op:BinOp::Add,left:a,right:b}});
    let Terminator::CondBr{cond,..}=&mut bb.term else {unreachable!()};
    *cond=synthetic;
    expect_known_type_rejection(&ir,"U32 Binary Add as CondBr condition");
}

#[test]
fn boolean_binary_used_as_integer_return_must_reject() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("join").unwrap();
    let a=f.params[1].value;
    let b=f.params[2].value;
    let bb=f.blocks.iter_mut().find(|b|matches!(b.term,Terminator::Return(Some(_))))
        .expect("source join returns U32");
    let synthetic=ValueId(0xffff_ff11);
    bb.insts.push(Inst {span:bb.span,dest:Some(synthetic),
        kind:InstKind::Binary{op:BinOp::Eq,left:a,right:b}});
    bb.term=Terminator::Return(Some(synthetic));
    expect_known_type_rejection(&ir,"Bool Binary Eq as U32 Return");
}

#[test]
fn boolean_local_alias_used_as_integer_switch_must_reject() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("switch_case").unwrap();
    let flag=f.params[0].value;
    let bb=f.blocks.iter_mut().find(|b|matches!(b.term,Terminator::Switch{..}))
        .expect("dense integer cases dispatch via Switch");
    let alias=ValueId(0xffff_ff12);
    bb.insts.push(Inst {span:bb.span,dest:Some(alias),
        kind:InstKind::BindStrand{name:"$bool_local_alias".to_string(),expr:RValue::Local(flag)}});
    let Terminator::Switch{scrut,..}=&mut bb.term else {unreachable!()};
    *scrut=alias;
    expect_known_type_rejection(&ir,"Bool BindStrand::Local as integer Switch");
}

#[test]
fn integer_local_alias_used_as_boolean_condbr_must_reject() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("join").unwrap();
    let a=f.params[1].value;
    let bb=f.blocks.iter_mut().find(|b|matches!(b.term,Terminator::CondBr{..}))
        .expect("source if has CondBr");
    let alias=ValueId(0xffff_ff13);
    bb.insts.push(Inst {span:bb.span,dest:Some(alias),
        kind:InstKind::BindStrand{name:"$integer_local_alias".to_string(),expr:RValue::Local(a)}});
    let Terminator::CondBr{cond,..}=&mut bb.term else {unreachable!()};
    *cond=alias;
    expect_known_type_rejection(&ir,"U32 BindStrand::Local as Bool CondBr");
}
