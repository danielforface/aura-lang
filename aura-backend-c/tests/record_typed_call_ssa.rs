//! EV-196 RED: known user-Cell Call result and argument primitive-category
//! provenance, including Phi merges and terminator consumers.
//! All adversarial mutations start from real source-checked Aura IR.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{optimize_module, run_oracle_entry, validate_module, Inst, InstKind,
    ModuleIR, OracleConfig, OracleValue, Terminator, ValueId};
use std::{fs, process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
cell id_flag(flag: bool) ->:
    yield flag
cell id_number(n: u32) ->:
    yield n
cell choose_calls(flag: bool, a: u32, b: u32) ->:
    val mut selected: u32 = id_number(a)
    if flag:
        selected = id_number(a + 1)
    else:
        selected = id_number(b + 2)
    yield selected
cell branch_on_call(flag: bool, a: u32) ->:
    if id_flag(flag):
        yield id_number(a)
    yield id_number(a + 1)
"#;

fn real_ir() -> ModuleIR {
    let ast=aura_parse::parse_source(SOURCE).expect("EV-196 user-Cell source parses");
    Checker::new().check_program(&ast).expect("EV-196 user-Cell source checks");
    let ir=lower_program(&ast).expect("EV-196 user-Cell source lowers");
    validate_module(&ir).expect("real user-Cell source validates");
    ir
}

fn require_type_refusal(ir: &ModuleIR, context: &str) {
    let err=validate_module(ir)
        .expect_err("type-invalid Call-derived SSA must be refused by the IR validator");
    assert!(err.message.contains("type") || err.message.contains("Call") ||
        err.message.contains("Phi"), "{context}: expected typed Call/SSA error: {err}");
}

#[test]
fn qualified_source_user_calls_are_stable_through_oracle_optimizer_and_native_clang() {
    let baseline=real_ir();
    let mut optimized=baseline.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimizer must preserve source Call SSA");
    let cases:Vec<(&str,Vec<OracleValue>,u32)>=vec![
        ("choose_calls",vec![OracleValue::Bool(true),OracleValue::U32(7),OracleValue::U32(13)],8),
        ("choose_calls",vec![OracleValue::Bool(false),OracleValue::U32(7),OracleValue::U32(13)],15),
        ("branch_on_call",vec![OracleValue::Bool(true),OracleValue::U32(7)],7),
        ("branch_on_call",vec![OracleValue::Bool(false),OracleValue::U32(7)],8),
    ];
    for (name,args,want) in cases {
        for (label,ir) in [("original",&baseline),("optimized",&optimized)] {
            let got=run_oracle_entry(ir,name,&args,OracleConfig::default())
                .expect("real user-Cell source IR Oracle runs");
            assert!(got.ok,"{label} {name}: {got:?}");
            assert_eq!(got.return_value,Some(OracleValue::U32(want)),
                "{label} {name}: {args:?}");
        }
    }
    let emitted=emit_module(&optimized,None).expect("optimized source user-Cell C emission");
    let ns=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root=std::env::temp_dir().join(format!("aura-ev196-{}-{ns}",std::process::id()));
    fs::create_dir_all(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
    let _clean=Cleanup(root.clone());
    fs::write(root.join("aura_runtime.h"),emitted.runtime_h).unwrap();
    fs::write(root.join("module.c"),&emitted.module_c).unwrap();
    fs::write(root.join("driver.c"),concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "uint32_t choose_calls(bool,uint32_t,uint32_t);\n",
        "uint32_t branch_on_call(bool,uint32_t);\n",
        "int main(void){return (choose_calls(true,7,13)==8",
        " && choose_calls(false,7,13)==15",
        " && branch_on_call(true,7)==7",
        " && branch_on_call(false,7)==8)?0:1;}\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe=root.join(format!("ev196_{}", &opt[1..]));
        let built=Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&root).arg(root.join("module.c")).arg(root.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("Clang in Native CI");
        assert!(built.status.success(),"{opt}: {}\n{}",
            String::from_utf8_lossy(&built.stderr),emitted.module_c);
        let ran=Command::new(exe).output().expect("run optimized C driver");
        assert!(ran.status.success(),"{opt}: {}",
            String::from_utf8_lossy(&ran.stderr));
    }
}

#[test]
fn known_u32_call_result_must_not_drive_condbr() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("choose_calls").unwrap();
    let a=f.params[1].value;
    let bb=f.blocks.iter_mut().find(|b| matches!(b.term,Terminator::CondBr{..}))
        .expect("source chooses branch");
    let id=ValueId(0xffff_ff20);
    bb.insts.push(Inst{span:bb.span,dest:Some(id),
        kind:InstKind::Call{callee:"id_number".into(),args:vec![a]}});
    let Terminator::CondBr{cond,..}=&mut bb.term else {unreachable!()};
    *cond=id;
    require_type_refusal(&ir,"numeric user-Cell Call in CondBr");
}

#[test]
fn known_bool_call_result_must_not_be_returned_from_u32_cell() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("choose_calls").unwrap();
    let flag=f.params[0].value;
    let bb=f.blocks.iter_mut().find(|b| matches!(b.term,Terminator::Return(Some(_))))
        .expect("join returns U32");
    let id=ValueId(0xffff_ff21);
    bb.insts.push(Inst{span:bb.span,dest:Some(id),
        kind:InstKind::Call{callee:"id_flag".into(),args:vec![flag]}});
    bb.term=Terminator::Return(Some(id));
    require_type_refusal(&ir,"bool user-Cell Call in U32 Return");
}

#[test]
fn known_bool_call_result_in_u32_phi_must_reject() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("choose_calls").unwrap();
    let flag=f.params[0].value;
    // Mutate a *single* properly dominated Call in a real source join arm.
    // Its branch-local result flows into the existing two-predecessor Phi.
    let calls=f.blocks.iter_mut().flat_map(|b|b.insts.iter_mut())
        .filter(|i|matches!(&i.kind,InstKind::Call{callee,..} if callee=="id_number"))
        .collect::<Vec<_>>();
    assert!(calls.len()>=3,"source has initial and both branch Calls");
    let InstKind::Call{callee,args}=&mut calls[1].kind else {unreachable!()};
    *callee="id_flag".to_string();
    *args=vec![flag];
    require_type_refusal(&ir,"Bool Call entering U32 join Phi");
}

#[test]
fn known_bool_argument_must_not_satisfy_u32_cell_parameter() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("choose_calls").unwrap();
    let flag=f.params[0].value;
    let call=f.blocks.iter_mut().flat_map(|b|b.insts.iter_mut())
        .find(|i|matches!(&i.kind,InstKind::Call{callee,..} if callee=="id_number"))
        .expect("source calls numeric identity");
    let InstKind::Call{args,..}=&mut call.kind else {unreachable!()};
    args[0]=flag;
    require_type_refusal(&ir,"Bool actual to U32 formal");
}

#[test]
fn known_integer_argument_must_not_satisfy_bool_cell_parameter() {
    let mut ir=real_ir();
    let f=ir.functions.get_mut("branch_on_call").unwrap();
    let number=f.params[1].value;
    let call=f.blocks.iter_mut().flat_map(|b|b.insts.iter_mut())
        .find(|i|matches!(&i.kind,InstKind::Call{callee,..} if callee=="id_flag"))
        .expect("source calls Boolean identity");
    let InstKind::Call{args,..}=&mut call.kind else {unreachable!()};
    args[0]=number;
    require_type_refusal(&ir,"U32 actual to Bool formal");
}
