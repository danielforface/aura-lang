//! EV-193 test-first: non-Phi SSA use/definition dominance in instructions
//! and terminators. Inputs come from real Aura source, then one IR mutation.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{run_oracle_entry, validate_module, BinOp, FunctionIR, InstKind,
    ModuleIR, OracleConfig, OracleValue, RValue, Terminator, ValueId};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
cell identity(x: u32) ->:
    yield x
cell chain(x: u32) ->:
    val first: u32 = x + 1
    val second: u32 = first + 2
    yield second
cell call_chain(x: u32) ->:
    val first: u32 = x + 1
    yield identity(first)
cell branch_join(flag: bool, x: u32, y: u32) ->:
    val mut selected: u32 = x
    if flag:
        if x == y:
            selected = x + 1
        else:
            selected = x + 2
    else:
        selected = y + 2
    yield selected
cell switch_return(n: u32, x: u32) ->:
    match n:
        0:
            val branch: u32 = x + 1
            yield branch
        1:
            yield x
        2:
            yield x
        3:
            yield x
        _:
            yield x
    yield x
cell count_up(start: u32, limit: u32) ->:
    val mut cursor: u32 = start
    while cursor < limit:
        cursor = cursor + 1
    yield cursor
"#;

fn real_ir() -> ModuleIR {
    let ast = aura_parse::parse_source(SOURCE).expect("EV-193 Aura source parses");
    Checker::new().check_program(&ast).expect("EV-193 Aura source type checks");
    let ir = lower_program(&ast).expect("EV-193 Aura source lowers");
    validate_module(&ir).expect("the real source must be accepted by the validator");
    ir
}

fn must_reject(mutated: &ModuleIR, kind: &str) {
    let err = validate_module(mutated)
        .expect_err("the IR validator must fail closed on malformed non-Phi SSA uses");
    assert!(err.message.contains("SSA") || err.message.contains("domin")
        || err.message.contains("definition"),
        "{kind}: expected SSA dominance/use-before-def diagnostic, got {err}");
}

fn nth_binary(f: &FunctionIR, nth: usize) -> (usize, usize, ValueId) {
    f.blocks.iter().enumerate()
        .flat_map(|(bi, bb)| bb.insts.iter().enumerate()
            .filter(|(_, inst)| matches!(inst.kind, InstKind::Binary { .. }))
            .map(move |(ii, inst)| (bi, ii, inst.dest.expect("Binary defines SSA"))))
        .nth(nth).expect("real source contains requested Binary instruction")
}

#[test]
fn real_source_ir_oracle_and_strict_clang_o0_o2_remain_valid() {
    let ir = real_ir();
    let cases: Vec<(&str, Vec<OracleValue>, u32)> = vec![
        ("chain",vec![OracleValue::U32(7)],10),
        ("call_chain",vec![OracleValue::U32(7)],8),
        ("branch_join",vec![OracleValue::Bool(true),OracleValue::U32(7),OracleValue::U32(7)],8),
        ("branch_join",vec![OracleValue::Bool(true),OracleValue::U32(7),OracleValue::U32(13)],9),
        ("branch_join",vec![OracleValue::Bool(false),OracleValue::U32(7),OracleValue::U32(13)],15),
        ("switch_return",vec![OracleValue::U32(0),OracleValue::U32(7)],8),
        ("switch_return",vec![OracleValue::U32(3),OracleValue::U32(7)],7),
        ("switch_return",vec![OracleValue::U32(9),OracleValue::U32(7)],7),
        ("count_up",vec![OracleValue::U32(0),OracleValue::U32(4)],4),
    ];
    for (name,args,expected) in cases {
        let got = run_oracle_entry(&ir, name, &args, OracleConfig::default())
            .expect("IR Oracle can execute source-qualified control flow");
        assert!(got.ok, "{name}: {got:?}");
        assert_eq!(got.return_value, Some(OracleValue::U32(expected)), "{name}: {args:?}");
    }
    let c = emit_module(&ir, None).expect("real source emits typed C");
    let now=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir=PathBuf::from(std::env::temp_dir())
        .join(format!("aura-ev193-{}-{now}",std::process::id()));
    fs::create_dir_all(&dir).expect("scratch dir");
    struct RemoveOnDrop(PathBuf);
    impl Drop for RemoveOnDrop {
        fn drop(&mut self) { let _=fs::remove_dir_all(&self.0); }
    }
    let _guard=RemoveOnDrop(dir.clone());
    fs::write(dir.join("aura_runtime.h"),c.runtime_h).unwrap();
    fs::write(dir.join("module.c"),&c.module_c).unwrap();
    fs::write(dir.join("driver.c"),concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "uint32_t chain(uint32_t);\nuint32_t call_chain(uint32_t);\n",
        "uint32_t branch_join(bool,uint32_t,uint32_t);\n",
        "uint32_t switch_return(uint32_t,uint32_t);\n",
        "uint32_t count_up(uint32_t,uint32_t);\n",
        "int main(void) {return (chain(7)==10 && call_chain(7)==8",
        " && branch_join(true,7,7)==8 && branch_join(true,7,13)==9",
        " && branch_join(false,7,13)==15 && switch_return(0,7)==8",
        " && switch_return(3,7)==7 && switch_return(9,7)==7",
        " && count_up(0,4)==4)?0:1;}\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=dir.join(format!("ev193_{}", &opt[1..]));
        let built=Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&dir).arg(dir.join("module.c")).arg(dir.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("Native CI has Clang");
        assert!(built.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&built.stderr),c.module_c);
        let run=Command::new(exe).output().expect("execute native driver");
        assert!(run.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&run.stderr),c.module_c);
    }
}

#[test]
fn adversarial_instruction_uses_before_definition_and_undefined_fail_closed() {
    let baseline=real_ir();
    let f=baseline.functions.get("chain").unwrap();
    let (first_bi,first_ii,_)=nth_binary(f,0);
    let (_,_,second_value)=nth_binary(f,1);
    for kind in ["forward_reference","undefined_value","duplicate_definition"] {
        let mut forged=baseline.clone();
        let f=forged.functions.get_mut("chain").unwrap();
        match kind {
            "forward_reference" => {
                let InstKind::Binary {left,..} =
                    &mut f.blocks[first_bi].insts[first_ii].kind else {unreachable!()};
                *left=second_value;
            },
            "undefined_value" => {
                let InstKind::Binary {left,..} =
                    &mut f.blocks[first_bi].insts[first_ii].kind else {unreachable!()};
                *left=ValueId(0xfffffff0);
            },
            "duplicate_definition" => {
                let (bi,ii,_) = nth_binary(f,1);
                f.blocks[bi].insts[ii].dest=Some(f.blocks[first_bi].insts[first_ii]
                    .dest.expect("first producer"));
            },
            _=>unreachable!(),
        }
        must_reject(&forged,kind);
    }
}

#[test]
fn adversarial_return_and_condbr_from_non_dominating_sibling_fail_closed() {
    let baseline=real_ir();
    let f=baseline.functions.get("branch_join").unwrap();
    let branch_result=f.blocks.iter().flat_map(|bb| bb.insts.iter())
        .find_map(|inst| if let InstKind::Phi { incomings }=&inst.kind {
            (incomings.len()==2).then_some(incomings[0].1)
        } else { None }).expect("outer join has Phi with two incoming branches");
    let entry_cond=f.blocks.iter().enumerate().find_map(|(index,bb)|
        matches!(bb.term,Terminator::CondBr{..}).then_some(index))
        .expect("entry has conditional");
    let branch_only_bool=f.blocks.iter().flat_map(|bb| bb.insts.iter())
        .find_map(|inst| match &inst.kind {
            InstKind::Binary {op:BinOp::Eq,..}=>inst.dest,
            _=>None,
        }).expect("then arm computes a Boolean for nested if");
    for kind in ["return_sibling","condbr_branch_local","return_undefined"] {
        let mut forged=baseline.clone();
        let f=forged.functions.get_mut("branch_join").unwrap();
        match kind {
            "return_sibling"|"return_undefined" => {
                let return_value=if kind=="return_sibling" {
                    branch_result
                }else{ValueId(0xfffffff1)};
                let last=f.blocks.iter_mut().find(|bb|matches!(bb.term,Terminator::Return(Some(_))))
                    .expect("source has Return");
                last.term=Terminator::Return(Some(return_value));
            },
            "condbr_branch_local" => {
                let Terminator::CondBr {cond,..}=&mut f.blocks[entry_cond].term else {unreachable!()};
                *cond=branch_only_bool;
            },
            _=>unreachable!(),
        }
        must_reject(&forged,kind);
    }
}

#[test]
fn adversarial_switch_scrutinee_and_call_arguments_fail_closed() {
    let baseline=real_ir();
    let f=baseline.functions.get("switch_return").unwrap();
    let branch_only_u32=f.blocks.iter().flat_map(|bb|bb.insts.iter())
        .find_map(|inst|match &inst.kind {
            InstKind::Binary {op:BinOp::Add,..}=>inst.dest,
            _=>None,
        }).expect("case zero computes a branch-local U32");
    let mut switch_forged=baseline.clone();
    let s=switch_forged.functions.get_mut("switch_return").unwrap();
    let dispatch=s.blocks.iter_mut().find(|bb|matches!(bb.term,Terminator::Switch{..}))
        .expect("dense literal patterns use Switch");
    let Terminator::Switch {scrut,..}=&mut dispatch.term else {unreachable!()};
    *scrut=branch_only_u32;
    must_reject(&switch_forged,"switch_branch_local");

    let mut call_forged=baseline;
    let f=call_forged.functions.get_mut("call_chain").unwrap();
    let instruction=f.blocks.iter_mut().flat_map(|bb|bb.insts.iter_mut())
        .find(|inst|matches!(&inst.kind,InstKind::Call {callee,..} if callee=="identity"))
        .expect("real source contains identity call");
    let InstKind::Call{args,..}=&mut instruction.kind else {unreachable!()};
    args[0]=ValueId(0xfffffff2);
    must_reject(&call_forged,"call_undefined_argument");
}
