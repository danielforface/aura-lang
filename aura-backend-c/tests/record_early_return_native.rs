//! EV-191 test-first: a source yield in an if arm is a real Return,
//! never an incoming edge to an SSA Phi at the fallthrough join.
use aura_backend_c::emit_module;
use aura_core::{lower_program, Checker};
use aura_ir::{run_oracle_entry, validate_module, BlockId, FunctionIR, InstKind,
    OracleConfig, OracleValue, Terminator};
use std::{collections::{BTreeMap, BTreeSet, VecDeque}, fs, path::PathBuf,
    process::Command, time::{SystemTime, UNIX_EPOCH}};

const SOURCE: &str = r#"
type High = u64[4294967296..4294967316]
cell id_high(v: High) ->:
    yield v
cell early_then(flag: bool, a: u32, b: u32) ->:
    val mut chosen: u32 = a
    if flag:
        chosen = a
        yield chosen
    else:
        chosen = b
    yield chosen
cell early_else(flag: bool, a: u32, b: u32) ->:
    val mut chosen: u32 = a
    if flag:
        chosen = a
    else:
        chosen = b
        yield chosen
    yield chosen
cell early_both(flag: bool, a: u32, b: u32) ->:
    if flag:
        yield a
    else:
        yield b
    yield a
cell early_nested(a: bool, b: bool, x: u32, y: u32) ->:
    val mut chosen: u32 = x
    if a:
        if b:
            yield y
        else:
            chosen = y
    else:
        chosen = x
    yield chosen
cell early_high(flag: bool, a: High, b: High) ->:
    val mut chosen: High = id_high(a)
    if flag:
        yield id_high(b)
    else:
        chosen = id_high(a)
    yield chosen
"#;

// Independent of the validator: verify the precise reachable CFG edge set
// from terminators, not from stale Phi metadata.
fn reachable_predecessors(f: &FunctionIR) -> (BTreeSet<BlockId>, BTreeMap<BlockId, BTreeSet<BlockId>>) {
    let by_id: BTreeMap<_, _> = f.blocks.iter().map(|b| (b.id, b)).collect();
    let mut seen = BTreeSet::from([f.entry]);
    let mut queue = VecDeque::from([f.entry]);
    let mut preds: BTreeMap<BlockId, BTreeSet<BlockId>> = BTreeMap::new();
    while let Some(id) = queue.pop_front() {
        let bb = by_id.get(&id).expect("reachable branch target must exist");
        let edges = match &bb.term {
            Terminator::Return(_) => vec![],
            Terminator::Br(t) => vec![*t],
            Terminator::CondBr { then_bb, else_bb, .. } => vec![*then_bb, *else_bb],
            Terminator::Switch { default_bb, cases, .. } =>
                std::iter::once(*default_bb).chain(cases.iter().map(|(_,b)| *b)).collect(),
        };
        for target in edges {
            preds.entry(target).or_default().insert(id);
            if seen.insert(target) { queue.push_back(target); }
        }
    }
    (seen, preds)
}

fn assert_exact_phi_cfg(f: &FunctionIR) {
    let (reachable, preds) = reachable_predecessors(f);
    for bb in &f.blocks {
        if !reachable.contains(&bb.id) { continue; }
        for inst in &bb.insts {
            if let InstKind::Phi { incomings } = &inst.kind {
                let from: Vec<_> = incomings.iter().map(|(bb, _)| *bb).collect();
                let unique: BTreeSet<_> = from.iter().copied().collect();
                let actual = preds.get(&bb.id).cloned().unwrap_or_default();
                assert_eq!(from.len(), unique.len(), "{}: duplicate incoming at {:?}", f.name, bb.id);
                assert_eq!(unique, actual,
                    "{}: Phi incoming blocks are not the *actual* reachable CFG predecessors of {:?}",
                    f.name, bb.id);
            }
        }
    }
    if f.name == "early_both" {
        assert_eq!(reachable.len(), f.blocks.len(),
            "both arms Return: do not manufacture an unreachable join block");
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("aura-ev191-{}-{ns}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

#[test]
fn source_early_returns_have_only_reachable_phi_incomings_and_strict_native_o0_o2() {
    let ast = aura_parse::parse_source(SOURCE).expect("real source with nested branch yields parses");
    Checker::new().check_program(&ast).expect("real source with early yields type checks");
    let ir = lower_program(&ast).expect("real source lowers to IR");
    validate_module(&ir).expect("IR structural validation");
    for name in ["early_then", "early_else", "early_both", "early_nested", "early_high"] {
        assert_exact_phi_cfg(ir.functions.get(name).expect("lowered cell"));
    }

    let u32_cases: &[(&str, Vec<bool>, u32)] = &[
        ("early_then", vec![true], 7),
        ("early_then", vec![false], 13),
        ("early_else", vec![true], 7),
        ("early_else", vec![false], 13),
        ("early_both", vec![true], 7),
        ("early_both", vec![false], 13),
        ("early_nested", vec![true, true], 13),
        ("early_nested", vec![true, false], 13),
        ("early_nested", vec![false, true], 7),
        ("early_nested", vec![false, false], 7),
    ];
    for (name, flags, expected) in u32_cases {
        let mut args: Vec<OracleValue> = flags.iter().map(|x| OracleValue::Bool(*x)).collect();
        args.extend([OracleValue::U32(7), OracleValue::U32(13)]);
        let got = run_oracle_entry(&ir, name, &args, OracleConfig::default())
            .expect("IR oracle must execute each branch");
        assert!(got.ok && got.stderr.is_empty(), "{name}: {got:?}");
        assert_eq!(got.return_value, Some(OracleValue::U32(*expected)), "{name} {flags:?}");
    }
    for (flag, expected) in [(true, 4294967316u64), (false, 4294967296)] {
        let got = run_oracle_entry(&ir, "early_high",
            &[OracleValue::Bool(flag), OracleValue::U64(4294967296),
              OracleValue::U64(4294967316)], OracleConfig::default())
            .expect("high U64 Oracle execution");
        assert!(got.ok, "{got:?}");
        assert_eq!(got.return_value, Some(OracleValue::U64(expected)));
    }
    let c = emit_module(&ir, None).expect("typed C emit");
    let temp = Scratch::new();
    fs::write(temp.0.join("aura_runtime.h"), c.runtime_h).unwrap();
    fs::write(temp.0.join("module.c"), &c.module_c).unwrap();
    fs::write(temp.0.join("driver.c"), concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "uint32_t early_then(bool,uint32_t,uint32_t);\n",
        "uint32_t early_else(bool,uint32_t,uint32_t);\n",
        "uint32_t early_both(bool,uint32_t,uint32_t);\n",
        "uint32_t early_nested(bool,bool,uint32_t,uint32_t);\n",
        "uint64_t early_high(bool,uint64_t,uint64_t);\n",
        "int main(void) {\n",
        "return (early_then(true,7,13)==7 && early_then(false,7,13)==13\n",
        " && early_else(true,7,13)==7 && early_else(false,7,13)==13\n",
        " && early_both(true,7,13)==7 && early_both(false,7,13)==13\n",
        " && early_nested(true,true,7,13)==13 && early_nested(true,false,7,13)==13\n",
        " && early_nested(false,true,7,13)==7 && early_nested(false,false,7,13)==7\n",
        " && early_high(true,4294967296ULL,4294967316ULL)==4294967316ULL\n",
        " && early_high(false,4294967296ULL,4294967316ULL)==4294967296ULL)?0:1;\n}\n"
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let exe = temp.0.join(format!("ev191_{}", &opt[1..]));
        let build = Command::new("clang").args(["-std=gnu11", "-Wall", "-Werror", opt, "-I"])
            .arg(&temp.0).arg(temp.0.join("module.c")).arg(temp.0.join("driver.c"))
            .arg("-o").arg(&exe).output().expect("Clang available");
        assert!(build.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&build.stderr), c.module_c);
        let run = Command::new(exe).output().expect("execute native driver");
        assert!(run.status.success(), "{opt}: {}\n{}",
            String::from_utf8_lossy(&run.stderr), c.module_c);
    }
}
