//! EV-194 RED: Phi must be a contiguous block-prefix and known-typed
//! terminator operands must match their primitive CFG/return contracts.
//! Positive tests run real Aura source before and after IR optimization.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{optimize_module, run_oracle_entry, validate_module, Inst, InstKind,
    OracleConfig, OracleValue, RValue, Terminator, ValueId};
use std::{collections::BTreeMap, fs, process::Command, time::{SystemTime, UNIX_EPOCH}};

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

# Q03-005 regression: the entry block remains reachable after constant
# branch folding, but its old fallthrough edge into the Phi join disappears.
cell const_fallthrough(a: u32) ->:
    val mut chosen: u32 = a
    if true:
        chosen = a + 3
    yield chosen

type Counter = record { count: u32, tag: u32 }
cell const_record_fallthrough(a: u32) ->:
    val mut counter: Counter = Counter { count: a, tag: 7 }
    if true:
        counter.count = 19
    yield counter.count
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
        ("const_fallthrough",vec![
            (vec![OracleValue::U32(7)],OracleValue::U32(10)),
            (vec![OracleValue::U32(u32::MAX as u64)],OracleValue::U32(2))]),
        ("const_record_fallthrough",vec![
            (vec![OracleValue::U32(7)],OracleValue::U32(19)),
            (vec![OracleValue::U32(u32::MAX as u64)],OracleValue::U32(19))]),
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
fn constant_branch_folding_reconciles_edges_even_when_all_blocks_remain_reachable() {
    let mut module = real_source();
    for name in ["const_fallthrough", "const_record_fallthrough"] {
        let function = module.functions.get(name).expect("source cell exists");
        let phi_count = function.blocks.iter().flat_map(|b| &b.insts)
            .filter(|i| matches!(&i.kind, InstKind::Phi { incomings } if incomings.len() >= 2))
            .count();
        assert!(phi_count > 0, "{name}: real source must lower a fallthrough Phi");
    }
    optimize_module(&mut module);
    validate_module(&module)
        .expect("optimized source must not retain Phi incoming for a folded-away CFG edge");
    for name in ["const_fallthrough", "const_record_fallthrough"] {
        let optimized = module.functions.get(name).unwrap();
        assert!(optimized.blocks.iter().flat_map(|bb| &bb.insts)
            .all(|inst| !matches!(&inst.kind, InstKind::Phi { incomings }
                if incomings.len() == 1)),
            "{name}: single predecessor Phi must be reduced to its live value");
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

#[test]
fn optimized_real_source_compiles_and_runs_strict_native_clang_o0_o2() {
    let mut optimized=real_source();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("post-optimization IR remains structurally valid");
    let c=emit_module(&optimized,None).expect("optimized source should emit C");
    let ns=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root=std::env::temp_dir().join(format!("aura-ev194-{}-{ns}",std::process::id()));
    fs::create_dir_all(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) { let _=fs::remove_dir_all(&self.0); }
    }
    let _cleanup=Cleanup(root.clone());
    fs::write(root.join("aura_runtime.h"),c.runtime_h).unwrap();
    fs::write(root.join("module.c"),&c.module_c).unwrap();
    fs::write(root.join("driver.c"),concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "uint32_t join(bool,uint32_t,uint32_t);\n",
        "uint32_t loop_sum(uint32_t,uint32_t);\n",
        "uint32_t dense(bool,uint32_t,uint32_t);\n",
        "uint32_t const_join(uint32_t,uint32_t);\n",
        "uint32_t const_fallthrough(uint32_t);\n",
        "uint32_t const_record_fallthrough(uint32_t);\n",
        "int main(void){return (",
        "join(true,7,13)==8 && join(false,7,13)==15",
        " && loop_sum(0,4)==4 && loop_sum(4,4)==4",
        " && dense(true,0,7)==8 && dense(false,4,7)==7",
        " && const_join(7,13)==8",
        " && const_fallthrough(7)==10",
        " && const_fallthrough(4294967295u)==2",
        " && const_record_fallthrough(7)==19",
        " && const_record_fallthrough(4294967295u)==19)?0:1;}\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=root.join(format!("ev194_{}", &opt[1..]));
        let built=Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&root).arg(root.join("module.c")).arg(root.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("Native CI provides Clang");
        assert!(built.status.success(),"{opt} optimized C compilation: {}\n{}",
            String::from_utf8_lossy(&built.stderr),c.module_c);
        let ran=Command::new(exe).output().expect("execute optimized source native C");
        assert!(ran.status.success(),"{opt} optimized C execution: {}",
            String::from_utf8_lossy(&ran.stderr));
    }
}
