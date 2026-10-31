//! Q02-002 / Issue #39: real C compiler and executable native scalar fidelity.
//! Test-first baseline. These tests construct well-typed, SSA-valid IR through
//! the PUBLIC C backend, compile its emitted C with clang, and execute a driver.
use aura_backend_c::emit_module;
use aura_ast::Span;
use aura_ir::{
    BasicBlock, BinOp, BlockId, ExecutionHint, FunctionIR, Inst, InstKind,
    ModuleIR, Param, Terminator, Type, UnaryOp, ValueId,
};
use std::{fs, path::PathBuf, process::Command, sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};

enum Arithmetic { Binary(BinOp), Unary(UnaryOp) }

// Parallel Cargo tests can sample the same clock nanosecond in one process.
// A per-process sequence prevents two RAII Scratch instances deleting each other's artifacts.
static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

struct Scratch { root: PathBuf }
impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let seq = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("aura-q02-native-{}-{nonce}-{seq}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}

fn native_scalar_fixture(ty: Type, arithmetic: Arithmetic, driver: &str, expected_decl: &str) {
    let span: Span = (0, 0).into();
    let unary = matches!(arithmetic, Arithmetic::Unary(_));
    let param = |name: &str, value: u32| Param {
        name: name.to_string(), ty: ty.clone(), span, value: ValueId(value),
    };
    let mut module = ModuleIR::new();
    let inst = match arithmetic {
        Arithmetic::Binary(op) => Inst {
            span, dest: Some(ValueId(2)),
            kind: InstKind::Binary { op, left: ValueId(0), right: ValueId(1) },
        },
        Arithmetic::Unary(op) => Inst {
            span, dest: Some(ValueId(2)),
            kind: InstKind::Unary { op, operand: ValueId(0) },
        },
    };
    module.functions.insert("scalar_op".to_string(), FunctionIR {
        name: "scalar_op".into(),
        span,
        params: if unary { vec![param("x", 0)] } else { vec![param("x", 0), param("y", 1)] },
        ret: ty,
        entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![inst],
            term: Terminator::Return(Some(ValueId(2))),
        }],
    });
    let artifacts = emit_module(&module, None).expect("IR validation and real C emission");
    let temp = Scratch::new();
    let header = temp.root.join("aura_runtime.h");
    let source = temp.root.join("module.c");
    let harness = temp.root.join("driver.c");
    let binary = temp.root.join("scalar-native-test");
    fs::write(&header, artifacts.runtime_h).expect("runtime header");
    fs::write(&source, &artifacts.module_c).expect("generated C module");
    fs::write(&harness, driver).expect("native C driver");

    // First compile and execute the emitted artifact, so a RED result can be
    // distinguished from Rust string-only expectations and SSA validation.
    let cc = Command::new("clang")
        .args(["-std=gnu11", "-O0", "-I"]).arg(&temp.root)
        .arg(&source).arg(&harness).arg("-o").arg(&binary)
        .output().expect("clang must be installed by CI workflow");
    assert!(cc.status.success(),
        "native compilation failed: {}\nsource={}\ndriver={}",
        String::from_utf8_lossy(&cc.stderr), artifacts.module_c, driver);
    let run = Command::new(&binary).output().expect("run compiled native artifact");
    assert!(run.status.success(),
        "incorrect native numerical result: status={:?} stderr={}\ngenerated C={}",
        run.status, String::from_utf8_lossy(&run.stderr), artifacts.module_c);
    assert!(artifacts.module_c.contains(expected_decl),
        "wrong arithmetic result type: expected {expected_decl:?}; generated C={}",
        artifacts.module_c);
}

#[test]
fn signed_i64_add_above_32_bit_boundary_remains_signed_64() {
    native_scalar_fixture(Type::I64, Arithmetic::Binary(BinOp::Add),
        "#include <stdint.h>\nint64_t scalar_op(int64_t x, int64_t y);\nint main(void) { return scalar_op(-4294967300LL, -7LL) == -4294967307LL ? 0 : 1; }\n",
        "int64_t v2 =");
}

#[test]
fn unsigned_u64_add_above_32_bit_boundary_keeps_high_bits() {
    native_scalar_fixture(Type::U64, Arithmetic::Binary(BinOp::Add),
        "#include <stdint.h>\nuint64_t scalar_op(uint64_t x, uint64_t y);\nint main(void) { return scalar_op(4294967296ULL, 11ULL) == 4294967307ULL ? 0 : 1; }\n",
        "uint64_t v2 =");
}

#[test]
fn f64_arithmetic_keeps_fractional_precision() {
    native_scalar_fixture(Type::F64, Arithmetic::Binary(BinOp::Add),
        "#include <stdint.h>\ndouble scalar_op(double x, double y);\nint main(void) { return scalar_op(1.5, 2.25) == 3.75 ? 0 : 1; }\n",
        "double v2 =");
}

#[test]
fn signed_i64_unary_neg_preserves_full_width() {
    native_scalar_fixture(Type::I64, Arithmetic::Unary(UnaryOp::Neg),
        "#include <stdint.h>\nint64_t scalar_op(int64_t x);\nint main(void) { return scalar_op(-4294967300LL) == 4294967300LL ? 0 : 1; }\n",
        "int64_t v2 =");
}

#[test]
fn existing_u32_addition_remains_correct_control() {
    native_scalar_fixture(Type::U32, Arithmetic::Binary(BinOp::Add),
        "#include <stdint.h>\nuint32_t scalar_op(uint32_t x, uint32_t y);\nint main(void) { return scalar_op(9u, 5u) == 14u ? 0 : 1; }\n",
        "uint32_t v2 =");
}


#[test]
fn unqualified_wide_division_in_main_must_fail_closed_not_drop_backend_error() {
    let span: Span = (0, 0).into();
    let mut module = ModuleIR::new();
    module.functions.insert("main".to_string(), FunctionIR {
        name: "main".into(),
        span,
        params: vec![
            Param { name: "x".into(), ty: Type::U64, span, value: ValueId(0) },
            Param { name: "y".into(), ty: Type::U64, span, value: ValueId(1) },
        ],
        ret: Type::U64,
        entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![Inst {
                span, dest: Some(ValueId(2)),
                kind: InstKind::Binary {
                    op: BinOp::Div, left: ValueId(0), right: ValueId(1),
                },
            }],
            term: Terminator::Return(Some(ValueId(2))),
        }],
    });
    match emit_module(&module, None) {
        Ok(_) => panic!("unqualified U64 division in main must NOT produce a successful C artifact"),
        Err(e) => assert!(
            e.message.contains("unqualified C numeric division"),
            "incorrect backend rejection kind: {e}"
        ),
    }
}
