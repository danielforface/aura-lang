//! Issue #41 test-first: source-valid SSA Phi joins must produce typed C storage,
//! compile with Clang, and preserve both branches of 64-bit native values.
use aura_backend_c::emit_module;
use aura_ast::Span;
use aura_ir::{
    BasicBlock, BlockId, ExecutionHint, FunctionIR, Inst, InstKind,
    ModuleIR, Param, Terminator, Type, ValueId,
};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

struct Scratch { root: PathBuf }
impl Scratch {
    fn new() -> Self {
        let tick = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("aura-phi-native-{}-{tick}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}

fn compile_and_execute(module: &ModuleIR, driver: &str, expected_type: Option<&str>) {
    // IR validation and optimization happen inside the public C emitter.
    let emitted = emit_module(module, None).expect("well-formed typed SSA IR should emit");
    let temp = Scratch::new();
    let h = temp.root.join("aura_runtime.h");
    let c = temp.root.join("module.c");
    let d = temp.root.join("driver.c");
    fs::write(&h, emitted.runtime_h).unwrap();
    fs::write(&c, &emitted.module_c).unwrap();
    fs::write(&d, driver).unwrap();
    // Exercise the SAME emitted C under both unoptimized and optimized
    // Clang pipelines. Optimizations must not alter either SSA join path.
    for optimization in ["-O0", "-O2"] {
        let exe = temp.root.join(format!("phi-case-{}", &optimization[1..]));
        let compile = Command::new("clang")
            .args(["-std=gnu11", optimization, "-I"]).arg(&temp.root)
            .arg(&c).arg(&d).arg("-o").arg(&exe)
            .output().expect("clang installed in CI");
        assert!(compile.status.success(),
            "{optimization} generated C must compile: stderr={}\nC={}\ndriver={}",
            String::from_utf8_lossy(&compile.stderr), emitted.module_c, driver);
        let out = Command::new(&exe).output().expect("run native output");
        assert!(out.status.success(),
            "{optimization} wrong Phi branch value: status={:?}; stderr={}\nC={}",
            out.status, String::from_utf8_lossy(&out.stderr), emitted.module_c);
    }
    if let Some(ct) = expected_type {
        assert!(emitted.module_c.contains(&format!("{ct} v3;")),
            "Phi destination should be explicitly declared with correct width {ct}: {}",
            emitted.module_c);
    }
}

fn diamond(ty: Type) -> ModuleIR {
    let span: Span = (0, 0).into();
    let empty = || Vec::<Inst>::new();
    let mk = |id, insts, term| BasicBlock {
        id: BlockId(id), span, hint: ExecutionHint::Sequential,
        insts, term,
    };
    let phi = Inst {
        span, dest: Some(ValueId(3)),
        kind: InstKind::Phi {
            incomings: vec![(BlockId(1), ValueId(1)), (BlockId(2), ValueId(2))],
        },
    };
    let mut module = ModuleIR::new();
    module.functions.insert("choose_wide".into(), FunctionIR {
        name: "choose_wide".into(), span,
        params: vec![
            Param { name: "cond".into(), ty: Type::Bool, span, value: ValueId(0) },
            Param { name: "x".into(), ty: ty.clone(), span, value: ValueId(1) },
            Param { name: "y".into(), ty: ty.clone(), span, value: ValueId(2) },
        ],
        ret: ty,
        entry: BlockId(0),
        blocks: vec![
            mk(0, empty(), Terminator::CondBr {
                cond: ValueId(0), then_bb: BlockId(1), else_bb: BlockId(2),
            }),
            mk(1, empty(), Terminator::Br(BlockId(3))),
            mk(2, empty(), Terminator::Br(BlockId(3))),
            mk(3, vec![phi], Terminator::Return(Some(ValueId(3)))),
        ],
    });
    module
}

#[test]
fn phi_i64_joins_both_branches_without_truncating() {
    compile_and_execute(
        &diamond(Type::I64),
        "#include <stdint.h>\n#include <stdbool.h>\nint64_t choose_wide(bool cond,int64_t x,int64_t y);\nint main(void){ return choose_wide(true,-4294967307LL,4294967319LL)==-4294967307LL && choose_wide(false,-4294967307LL,4294967319LL)==4294967319LL ? 0 : 1; }\n",
        Some("int64_t"),
    );
}
#[test]
fn phi_u64_joins_both_branches_and_keeps_high_bits() {
    compile_and_execute(
        &diamond(Type::U64),
        "#include <stdint.h>\n#include <stdbool.h>\nuint64_t choose_wide(bool cond,uint64_t x,uint64_t y);\nint main(void){ return choose_wide(true,4294967307ULL,4294967319ULL)==4294967307ULL && choose_wide(false,4294967307ULL,4294967319ULL)==4294967319ULL ? 0 : 1; }\n",
        Some("uint64_t"),
    );
}
#[test]
fn phi_u32_control_still_creates_real_storage() {
    compile_and_execute(
        &diamond(Type::U32),
        "#include <stdint.h>\n#include <stdbool.h>\nuint32_t choose_wide(bool cond,uint32_t x,uint32_t y);\nint main(void){ return choose_wide(true,17u,29u)==17u && choose_wide(false,17u,29u)==29u ? 0 : 1; }\n",
        Some("uint32_t"),
    );
}
#[test]
fn direct_parameter_return_without_phi_compiles_and_runs() {
    let span: Span = (0,0).into();
    let mut module=ModuleIR::new();
    module.functions.insert("choose_wide".into(), FunctionIR {
        name: "choose_wide".into(),span,
        params: vec![Param{name:"x".into(),ty:Type::U32,span,value:ValueId(0)}],
        ret:Type::U32,entry:BlockId(0),
        blocks: vec![BasicBlock{
            id:BlockId(0),span,hint:ExecutionHint::Sequential,
            insts:vec![],term:Terminator::Return(Some(ValueId(0))),
        }],
    });
    compile_and_execute(
        &module,
        "#include <stdint.h>\nuint32_t choose_wide(uint32_t x);\nint main(void){return choose_wide(23u)==23u?0:1;}\n",
        None,
    );
}


// Issue #43: the previous Phi prepass only knew function-parameter types.
// These incoming SSA values are computed separately inside predecessor blocks.
fn diamond_with_computed_incomings(ty: Type) -> ModuleIR {
    use aura_ir::BinOp;
    let span: Span = (0,0).into();
    let mk = |id: u32, insts: Vec<Inst>, term: Terminator| BasicBlock {
        id: BlockId(id), span, hint: ExecutionHint::Sequential, insts, term
    };
    let compute = |id: u32, op: BinOp| Inst {
        span, dest: Some(ValueId(id)),
        kind: InstKind::Binary { op, left: ValueId(1), right: ValueId(2) }
    };
    let phi = Inst {
        span, dest: Some(ValueId(3)), kind: InstKind::Phi {
            incomings: vec![(BlockId(1), ValueId(4)), (BlockId(2), ValueId(5))]
        }
    };
    let mut module = ModuleIR::new();
    module.functions.insert("choose_wide".into(), FunctionIR {
        name: "choose_wide".into(), span,
        params: vec![
            Param { name: "cond".into(), ty: Type::Bool, span, value: ValueId(0) },
            Param { name: "x".into(), ty: ty.clone(), span, value: ValueId(1) },
            Param { name: "y".into(), ty: ty.clone(), span, value: ValueId(2) },
        ],
        ret: ty, entry: BlockId(0),
        blocks: vec![
            mk(0, vec![], Terminator::CondBr {
                cond: ValueId(0), then_bb: BlockId(1), else_bb: BlockId(2)
            }),
            mk(1, vec![compute(4, BinOp::Add)], Terminator::Br(BlockId(3))),
            mk(2, vec![compute(5, BinOp::Sub)], Terminator::Br(BlockId(3))),
            mk(3, vec![phi], Terminator::Return(Some(ValueId(3))))
        ]
    });
    module
}

#[test]
fn computed_predecessor_i64_phi_preserves_negative_high_bits() {
    compile_and_execute(&diamond_with_computed_incomings(Type::I64),
        "#include <stdint.h>\n#include <stdbool.h>\nint64_t choose_wide(bool cond,int64_t x,int64_t y);\nint main(void){ return choose_wide(true,-4294967300LL,7LL)==-4294967293LL && choose_wide(false,-4294967300LL,7LL)==-4294967307LL ? 0 : 1; }\n",
        Some("int64_t"));
}
#[test]
fn computed_predecessor_u64_phi_preserves_positive_high_bits() {
    compile_and_execute(&diamond_with_computed_incomings(Type::U64),
        "#include <stdint.h>\n#include <stdbool.h>\nuint64_t choose_wide(bool cond,uint64_t x,uint64_t y);\nint main(void){ return choose_wide(true,4294967300ULL,7ULL)==4294967307ULL && choose_wide(false,4294967300ULL,7ULL)==4294967293ULL ? 0 : 1; }\n",
        Some("uint64_t"));
}
#[test]
fn computed_predecessor_u32_phi_keeps_control_flow_results() {
    compile_and_execute(&diamond_with_computed_incomings(Type::U32),
        "#include <stdint.h>\n#include <stdbool.h>\nuint32_t choose_wide(bool cond,uint32_t x,uint32_t y);\nint main(void){ return choose_wide(true,100u,7u)==107u && choose_wide(false,100u,7u)==93u ? 0 : 1; }\n",
        Some("uint32_t"));
}


#[test]
fn computed_predecessor_f64_phi_keeps_fractional_values() {
    compile_and_execute(&diamond_with_computed_incomings(Type::F64),
        "#include <stdint.h>\n#include <stdbool.h>\ndouble choose_wide(bool cond,double x,double y);\nint main(void){ return choose_wide(true,1.5,2.25)==3.75 && choose_wide(false,1.5,2.25)==-0.75 ? 0 : 1; }\n",
        Some("double"));
}

#[test]
fn mixed_signed_unsigned_phi_inputs_do_not_silently_coerce_or_default_u32() {
    // This deliberately exercises the C backend boundary with an invalid
    // mixed-width signedness Phi. Either IR validation or the C emitter must
    // explicitly reject it, not return an executable artifact or cast to U32.
    let mut module = diamond(Type::U64);
    let f = module.functions.get_mut("choose_wide").expect("test function");
    f.params[2].ty = Type::I64;
    match emit_module(&module, None) {
        Ok(_) => panic!("mixed U64/I64 Phi is unqualified, must fail closed"),
        Err(e) => {
            let msg = e.message.to_ascii_lowercase();
            assert!(
                msg.contains("incompatible") || msg.contains("unqualified")
                    || msg.contains("validation") || msg.contains("type"),
                "failure must explain a typing/validation boundary: {e}"
            );
        }
    }
}
