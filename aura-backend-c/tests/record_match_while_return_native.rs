//! EV-192 test-first: real Aura Match and While early-return CFG.
//! Verify exact reachable predecessor sets, Oracle behavior, and strict Clang.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{run_oracle_entry, validate_module, BlockId, FunctionIR, InstKind,
    OracleConfig, OracleValue, Terminator};
use std::{collections::{BTreeMap, BTreeSet, VecDeque}, fs, path::PathBuf,
    process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
cell match_one(n: u32, a: u32, b: u32) ->:
    val mut chosen: u32 = a
    match n:
        0:
            yield a
        _:
            chosen = b
    yield chosen
cell match_nested(n: u32, flag: bool, a: u32, b: u32) ->:
    val mut chosen: u32 = a
    match n:
        0:
            if flag:
                yield a
            else:
                chosen = b
        _:
            chosen = a
    yield chosen
cell match_all(n: u32, a: u32, b: u32) ->:
    match n:
        0:
            yield a
        _:
            yield b
    yield a
cell while_return(flag: bool, a: u32) ->:
    val mut chosen: u32 = a
    while flag:
        chosen = chosen + 1
        yield chosen
    yield chosen
cell while_nested(start: u32, limit: u32, stop: u32) ->:
    val mut value: u32 = start
    while value < limit:
        if value == stop:
            yield value
        else:
            value = value + 1
    yield value
"#;

fn reachable_predecessors(f: &FunctionIR) -> (BTreeSet<BlockId>, BTreeMap<BlockId, BTreeSet<BlockId>>) {
    let by_id: BTreeMap<_, _> = f.blocks.iter().map(|b| (b.id, b)).collect();
    let mut seen = BTreeSet::from([f.entry]);
    let mut queue = VecDeque::from([f.entry]);
    let mut preds: BTreeMap<BlockId, BTreeSet<BlockId>> = BTreeMap::new();
    while let Some(id) = queue.pop_front() {
        let bb = by_id.get(&id).expect("all reachable branch targets must exist");
        let edges = match &bb.term {
            Terminator::Return(_) => vec![],
            Terminator::Br(t) => vec![*t],
            Terminator::CondBr { then_bb, else_bb, .. } => vec![*then_bb, *else_bb],
            Terminator::Switch { default_bb, cases, .. } =>
                std::iter::once(*default_bb).chain(cases.iter().map(|(_, b)| *b)).collect(),
        };
        for target in edges {
            preds.entry(target).or_default().insert(id);
            if seen.insert(target) { queue.push_back(target); }
        }
    }
    (seen, preds)
}

fn check_exact_phi_cfg(f: &FunctionIR) {
    let (reachable, preds) = reachable_predecessors(f);
    for bb in &f.blocks {
        if !reachable.contains(&bb.id) { continue; }
        for inst in &bb.insts {
            if let InstKind::Phi { incomings } = &inst.kind {
                let found: Vec<_> = incomings.iter().map(|(id, _)| *id).collect();
                let unique: BTreeSet<_> = found.iter().copied().collect();
                assert_eq!(unique.len(), found.len(),
                    "{}: duplicate Phi predecessor in {:?}", f.name, bb.id);
                assert_eq!(unique, preds.get(&bb.id).cloned().unwrap_or_default(),
                    "{}: Phi incoming must equal the true reachable predecessors of {:?}",
                    f.name, bb.id);
            }
        }
    }
    if f.name == "match_all" {
        assert_eq!(reachable.len(), f.blocks.len(),
            "all Match arms return: a fake unreachable join is not permitted");
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("aura-ev192-{}-{now}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

#[test]
fn match_and_while_return_real_source_cfg_oracle_and_clang_o0_o2() {
    let ast = aura_parse::parse_source(SOURCE).expect("Match/While real Aura source parses");
    Checker::new().check_program(&ast).expect("Match/While early yields pass checker");
    let ir = lower_program(&ast).expect("Match/While source lowers");
    validate_module(&ir).expect("all source CFG and Phi incoming edges must be structurally valid");
    for name in ["match_one", "match_nested", "match_all", "while_return", "while_nested"] {
        check_exact_phi_cfg(ir.functions.get(name).expect("source cell exists"));
    }
    let cases: Vec<(&str, Vec<OracleValue>, u32)> = vec![
        ("match_one", vec![OracleValue::U32(0),OracleValue::U32(7),OracleValue::U32(13)],7),
        ("match_one", vec![OracleValue::U32(1),OracleValue::U32(7),OracleValue::U32(13)],13),
        ("match_nested", vec![OracleValue::U32(0),OracleValue::Bool(true),OracleValue::U32(7),OracleValue::U32(13)],7),
        ("match_nested", vec![OracleValue::U32(0),OracleValue::Bool(false),OracleValue::U32(7),OracleValue::U32(13)],13),
        ("match_nested", vec![OracleValue::U32(1),OracleValue::Bool(true),OracleValue::U32(7),OracleValue::U32(13)],7),
        ("match_all", vec![OracleValue::U32(0),OracleValue::U32(7),OracleValue::U32(13)],7),
        ("match_all", vec![OracleValue::U32(1),OracleValue::U32(7),OracleValue::U32(13)],13),
        ("while_return", vec![OracleValue::Bool(false),OracleValue::U32(7)],7),
        ("while_return", vec![OracleValue::Bool(true),OracleValue::U32(7)],8),
        ("while_nested", vec![OracleValue::U32(0),OracleValue::U32(4),OracleValue::U32(2)],2),
        ("while_nested", vec![OracleValue::U32(0),OracleValue::U32(4),OracleValue::U32(5)],4),
        ("while_nested", vec![OracleValue::U32(4),OracleValue::U32(4),OracleValue::U32(2)],4),
    ];
    for (name, args, expected) in cases {
        let actual = run_oracle_entry(&ir, name, &args, OracleConfig::default())
            .expect("IR Oracle supports typed U32 Match and While CFG");
        assert!(actual.ok && actual.stderr.is_empty(), "{name}: {actual:?}");
        assert_eq!(actual.return_value, Some(OracleValue::U32(expected)), "{name}: {args:?}");
    }

    let c = emit_module(&ir, None).expect("source-valid Match/While must emit C");
    let tmp = Scratch::new();
    fs::write(tmp.0.join("aura_runtime.h"), c.runtime_h).unwrap();
    fs::write(tmp.0.join("module.c"), &c.module_c).unwrap();
    fs::write(tmp.0.join("driver.c"), concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "uint32_t match_one(uint32_t,uint32_t,uint32_t);\n",
        "uint32_t match_nested(uint32_t,bool,uint32_t,uint32_t);\n",
        "uint32_t match_all(uint32_t,uint32_t,uint32_t);\n",
        "uint32_t while_return(bool,uint32_t);\n",
        "uint32_t while_nested(uint32_t,uint32_t,uint32_t);\n",
        "int main(void){return (",
        "match_one(0,7,13)==7 && match_one(1,7,13)==13",
        " && match_nested(0,true,7,13)==7 && match_nested(0,false,7,13)==13",
        " && match_nested(1,true,7,13)==7",
        " && match_all(0,7,13)==7 && match_all(1,7,13)==13",
        " && while_return(false,7)==7 && while_return(true,7)==8",
        " && while_nested(0,4,2)==2 && while_nested(0,4,5)==4",
        " && while_nested(4,4,2)==4)?0:1;}\n"
    )).unwrap();
    for opt in ["-O0","-O2"] {
        let exe = tmp.0.join(format!("ev192_{}", &opt[1..]));
        let built = Command::new("clang")
            .args(["-std=gnu11","-Wall","-Werror",opt,"-I"])
            .arg(&tmp.0).arg(tmp.0.join("module.c")).arg(tmp.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("Clang must exist in Native CI");
        assert!(built.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&built.stderr), c.module_c);
        let ran = Command::new(exe).output().expect("execute independent native binary");
        assert!(ran.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&ran.stderr), c.module_c);
    }
}

#[test]
fn while_body_return_must_not_invent_loop_backedge() {
    // Isolate While from the Match RED above to establish an independent
    // Parser/Checker/Lowerer/IR CFG failure rather than a skipped assertion.
    let source = r#"
cell loop_yield(flag: bool, n: u32) ->:
    val mut selected: u32 = n
    while flag:
        selected = selected + 1
        yield selected
    yield selected
"#;
    let ast = aura_parse::parse_source(source).expect("real while-yield source parses");
    Checker::new().check_program(&ast).expect("while-yield source checks");
    let ir = lower_program(&ast).expect("while-yield lowers");
    validate_module(&ir).expect("loop header Phi must not mention a returning body as backedge");
    check_exact_phi_cfg(ir.functions.get("loop_yield").unwrap());
    for (flag, want) in [(false, 7u32), (true, 8u32)] {
        let actual = run_oracle_entry(&ir, "loop_yield",
            &[OracleValue::Bool(flag), OracleValue::U32(7)], OracleConfig::default())
            .expect("Oracle must execute both loop exit paths");
        assert_eq!(actual.return_value, Some(OracleValue::U32(want)));
    }
}

#[test]
fn loop_header_phi_does_not_accept_mixed_u32_and_u64_predecessor_types() {
    // Even though EV-192 can provisionally infer a type from a preheader,
    // final C Phi emission must check every incoming and fail closed.
    let src = r#"
cell wide_loop(wide: u64, start: u32, limit: u32) ->:
    val mut value: u32 = start
    while value < limit:
        value = value + 1
    yield value
"#;
    let ast = aura_parse::parse_source(src).expect("loop source parses");
    Checker::new().check_program(&ast).expect("loop source checks");
    let mut ir = lower_program(&ast).expect("valid source lowers");
    validate_module(&ir).expect("original loop Phi CFG is well formed");
    let f = ir.functions.get_mut("wide_loop").unwrap();
    let wide = f.params[0].value;
    let loop_phi = f.blocks.iter_mut().flat_map(|b| b.insts.iter_mut())
        .find(|i| matches!(i.kind, InstKind::Phi { .. }))
        .expect("loop creates a loop-carried Phi");
    let InstKind::Phi { incomings } = &mut loop_phi.kind else { unreachable!() };
    assert_eq!(incomings.len(), 2);
    incomings[1].1 = wide; // U64 defined at function entry, but wrong U32 loop ABI.
    validate_module(&ir).expect("structural CFG/dominance valid; typed C must reject");
    let err = emit_module(&ir, None).err()
        .expect("provisional loop Phi inference must never hide mixed widths");
    assert!(err.message.contains("Phi") || err.message.contains("SSA"),
        "require explicit typed Phi/SSA refusal: {err}");
}
