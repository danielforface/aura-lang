//! Q03-001: predecessor-edge Phi values must be read in parallel.
//! Tests a real CFG loop in typed IR, the IR Oracle and linked native C11.
//! This does not claim source-level while syntax or external Win64 Record ABI.
use aura_backend_c::emit_module;
use aura_ir::{
    optimize_module, run_oracle_entry, validate_module, BasicBlock, BinOp, BlockId,
    ExecutionHint, FunctionIR, Inst, InstKind, ModuleIR, OracleConfig, OracleValue,
    Param, RValue, Terminator, Type, ValueId,
};
use aura_ast::span;
use std::{collections::HashMap, fs, path::PathBuf, process::Command,
    time::{SystemTime, UNIX_EPOCH}};

fn v(id: u32) -> ValueId { ValueId(id) }
fn b(id: u32) -> BlockId { BlockId(id) }
fn ins(dest: u32, kind: InstKind) -> Inst {
    Inst { span: span(0, 0), dest: Some(v(dest)), kind }
}
fn block(id: u32, insts: Vec<Inst>, term: Terminator) -> BasicBlock {
    BasicBlock { id: b(id), span: span(0, 0), hint: ExecutionHint::Sequential, insts, term }
}
fn phi(dest: u32, initial: u32, backedge: u32) -> Inst {
    ins(dest, InstKind::Phi { incomings: vec![(b(0), v(initial)), (b(2), v(backedge))] })
}

fn swap_loop(name: &str, nominal: bool, switch_backedge: bool) -> FunctionIR {
    let input = if nominal { Type::Opaque("Pair".into()) } else { Type::U32 };
    FunctionIR {
        name: name.into(),
        span: span(0, 0),
        params: vec![
            Param { name: "left".into(), ty: input.clone(), span: span(0, 0), value: v(0) },
            Param { name: "right".into(), ty: input, span: span(0, 0), value: v(1) },
            Param { name: "steps".into(), ty: Type::U32, span: span(0, 0), value: v(2) },
        ],
        ret: Type::U32,
        entry: b(0),
        blocks: vec![
            // Constants live in the preheader and dominate the loop.
            block(0, vec![
                ins(10, InstKind::BindStrand { name: "zero".into(), expr: RValue::ConstU32(0) }),
                ins(11, InstKind::BindStrand { name: "one".into(), expr: RValue::ConstU32(1) }),
            ], Terminator::Br(b(1))),
            block(1, vec![
                // On each backedge x' = y and y' = x. Sequential
                // assignments turn both into y; simultaneous Phis swap.
                phi(3, 0, 4),
                phi(4, 1, 3),
                phi(5, 2, 6),
                ins(7, InstKind::Binary { op: BinOp::Gt, left: v(5), right: v(10) }),
            ], Terminator::CondBr { cond: v(7), then_bb: b(2), else_bb: b(3) }),
            block(2, vec![
                ins(6, InstKind::Binary { op: BinOp::Sub, left: v(5), right: v(11) }),
            ], if switch_backedge {
                Terminator::Switch { scrut: v(6), cases: vec![(0, b(1))], default_bb: b(1) }
            } else {
                Terminator::Br(b(1))
            }),
            block(3, if nominal {
                vec![ins(8, InstKind::GetField { base: v(3), field_name: "value".into() })]
            } else { vec![] },
            Terminator::Return(Some(if nominal { v(8) } else { v(3) }))),
        ],
    }
}

fn module() -> ModuleIR {
    let mut ir = ModuleIR::new();
    ir.record_layouts.insert("Pair".into(), vec![("value".into(), Type::U32)]);
    ir.functions.insert("swap_scalar_cycles".into(), swap_loop("swap_scalar_cycles", false, false));
    ir.functions.insert("swap_record_cycles".into(), swap_loop("swap_record_cycles", true, true));
    ir
}

fn pair(value: u32) -> OracleValue {
    let mut fields = HashMap::new();
    fields.insert("value".into(), OracleValue::U32(value));
    OracleValue::Record { name: "Pair".into(), fields }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("aura-phi-parallel-{}-{ns}", std::process::id()));
        fs::create_dir_all(&path).expect("create unique native test directory");
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

const DRIVER: &str = r#"
#include <stdint.h>
typedef struct { uint32_t aura_f_76616c7565; } aura_record_50616972;
uint32_t swap_scalar_cycles(uint32_t, uint32_t, uint32_t);
uint32_t swap_record_cycles(aura_record_50616972, aura_record_50616972, uint32_t);
int main(void) {
  const uint32_t xs[3] = {7u, 0u, UINT32_MAX};
  const uint32_t ys[3] = {13u, UINT32_MAX, 1u};
  for (unsigned i = 0; i < 3; i++) {
    for (uint32_t n = 0; n <= 8; n++) {
      uint32_t expected = (n & 1u) ? ys[i] : xs[i];
      aura_record_50616972 a = {xs[i]}, b = {ys[i]};
      if (swap_scalar_cycles(xs[i], ys[i], n) != expected) return 1;
      if (swap_record_cycles(a, b, n) != expected) return 2;
    }
  }
  return 0;
}
"#;

#[test]
fn cyclic_loop_carried_phi_is_parallel_for_oracle_and_native_record_c() {
    let original = module();
    let mut optimized = original.clone();
    optimize_module(&mut optimized);
    for (label, ir) in [("original", &original), ("optimized", &optimized)] {
        validate_module(ir).expect("cyclic backedge Phi is valid SSA");
        for (left, right) in [(7u32, 13u32), (0, u32::MAX), (u32::MAX, 1)] {
            for n in 0..=8 {
                let expected = if n % 2 == 0 { left } else { right };
                for (name, args) in [
                    ("swap_scalar_cycles", vec![
                        OracleValue::U32(left), OracleValue::U32(right), OracleValue::U32(n),
                    ]),
                    ("swap_record_cycles", vec![
                        pair(left), pair(right), OracleValue::U32(n),
                    ]),
                ] {
                    let result = run_oracle_entry(ir, name, &args, OracleConfig::default())
                        .unwrap_or_else(|e| panic!("{label}/{name}/{n}: {e}"));
                    assert_eq!(result.return_value, Some(OracleValue::U32(expected)),
                        "{label}/{name}/{n}: Phi inputs must be simultaneous");
                }
            }
        }
        let emitted = emit_module(ir, None).expect("qualified cyclic Record/scalar Phis emit C");
        assert!(emitted.module_c.contains("aura_phi_snapshot_2_1_3"),
            "{label}: backedge Phi source snapshots missing");
        let tmp = Scratch::new();
        fs::write(tmp.0.join("aura_runtime.h"), emitted.runtime_h).unwrap();
        fs::write(tmp.0.join("module.c"), emitted.module_c).unwrap();
        fs::write(tmp.0.join("driver.c"), DRIVER).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = tmp.0.join(format!("phi_{}", &opt[1..]));
            let cc = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Wextra", "-Werror",
                    "-fsanitize=undefined", "-fno-sanitize-recover=all", opt, "-I"])
                .arg(&tmp.0).arg(tmp.0.join("module.c"))
                .arg(tmp.0.join("driver.c")).arg("-o").arg(&exe)
                .output().expect("Clang required by the Q03 Native gate");
            assert!(cc.status.success(), "{label}/{opt} compile: {}\n{}",
                String::from_utf8_lossy(&cc.stderr),
                fs::read_to_string(tmp.0.join("module.c")).unwrap());
            let run = Command::new(&exe).output().expect("execute native Phi swap");
            assert!(run.status.success(), "{label}/{opt} run: {}",
                String::from_utf8_lossy(&run.stderr));
        }
    }
}

#[test]
fn forged_cyclic_phi_input_still_fails_closed() {
    let mut damaged = module();
    let f = damaged.functions.get_mut("swap_scalar_cycles").unwrap();
    let phi = f.blocks[1].insts.get_mut(0).unwrap();
    let InstKind::Phi { incomings } = &mut phi.kind else { panic!("expected Phi") };
    incomings[1].1 = v(9999);
    assert!(validate_module(&damaged).is_err(), "undefined backedge SSA must be rejected");
    assert!(emit_module(&damaged, None).is_err(), "backend must reject invalid Phi inputs");
}
