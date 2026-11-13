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
fn unqualified_mixed_width_division_in_main_must_fail_closed_not_drop_backend_error() {
    // U64/U64 division is now a qualified full-width native operation.
    // Preserve this regression's original *main-error-propagation* purpose
    // with U64/I64, which is still unqualified and must not be suppressed by
    // a special native C main wrapper.
    let span: Span = (0, 0).into();
    let mut module = ModuleIR::new();
    module.functions.insert("main".to_string(), FunctionIR {
        name: "main".into(),
        span,
        params: vec![
            Param { name: "x".into(), ty: Type::U64, span, value: ValueId(0) },
            Param { name: "y".into(), ty: Type::I64, span, value: ValueId(1) },
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
        Ok(_) => panic!("unqualified U64/I64 division in main must NOT produce a successful C artifact"),
        Err(e) => assert!(
            e.message.contains("unqualified C numeric division"),
            "incorrect backend rejection kind: {e}"
        ),
    }
}


#[test]
fn typed_i32_modular_arithmetic_is_ubsan_clean_with_signed_extremes() {
    // AURA-Q03 native arithmetic accepts precisely typed IR and must avoid
    // UB for I32 Add/Sub/Mul/Neg even when signed bounds are exceeded.
    // The SOURCE-level I32 Return ABI is a separate qualified boundary;
    // exercise the supported C backend with explicit IR type metadata,
    // not an invented signed source Return contract.
    let span: Span = (0,0).into();
    let mut source = ModuleIR::new();
    for (cell, arithmetic) in [
        ("add32", Arithmetic::Binary(BinOp::Add)),
        ("sub32", Arithmetic::Binary(BinOp::Sub)),
        ("mul32", Arithmetic::Binary(BinOp::Mul)),
        ("neg32", Arithmetic::Unary(UnaryOp::Neg)),
    ] {
        let unary = matches!(arithmetic, Arithmetic::Unary(_));
        let params = if unary {
            vec![Param { name:"x".into(), ty:Type::I32,span,value:ValueId(0) }]
        } else {
            vec![
                Param { name:"x".into(), ty:Type::I32,span,value:ValueId(0) },
                Param { name:"y".into(), ty:Type::I32,span,value:ValueId(1) }
            ]
        };
        let kind = match arithmetic {
            Arithmetic::Binary(op) => InstKind::Binary {
                op, left:ValueId(0), right:ValueId(1),
            },
            Arithmetic::Unary(op) => InstKind::Unary {
                op, operand:ValueId(0),
            },
        };
        source.functions.insert(cell.to_string(), FunctionIR {
            name:cell.into(), span, params, ret:Type::I32,entry:BlockId(0),
            blocks:vec![BasicBlock {
                id:BlockId(0),span,hint:ExecutionHint::Sequential,
                insts:vec![Inst { span, dest:Some(ValueId(2)),kind }],
                term:Terminator::Return(Some(ValueId(2))),
            }],
        });
    }
    let mut optimized=source.clone();
    aura_ir::optimize_module(&mut optimized);
    let driver = r#"
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
int32_t add32(int32_t,int32_t);
int32_t sub32(int32_t,int32_t);
int32_t mul32(int32_t,int32_t);
int32_t neg32(int32_t);
int main(int argc,char** argv) {
  if (argc!=4) return 2;
  const char* mode=argv[1];
  const int32_t x=(int32_t)strtoll(argv[2],NULL,10);
  const int32_t y=(int32_t)strtoll(argv[3],NULL,10);
  int32_t out;
  if (!strcmp(mode,"add32")) out=add32(x,y);
  else if (!strcmp(mode,"sub32")) out=sub32(x,y);
  else if (!strcmp(mode,"mul32")) out=mul32(x,y);
  else if (!strcmp(mode,"neg32")) out=neg32(x);
  else return 3;
  printf("%lld\n",(long long)out);
  return 0;
}
"#;
    let samples: &[(i32,i32)]=&[
        (0,0), (1,-1), (-1,-1),
        (i32::MAX,1), (i32::MIN,-1),
        (i32::MIN,1), (i32::MAX,i32::MAX),
        (i32::MIN,i32::MIN), (i32::MIN,0),
        (i32::MAX,-1), (-2_000_000_000,2_000_000_000)
    ];
    for (label,ir) in [("source_ir",&source),("optimized_ir",&optimized)] {
        let artifacts=emit_module(ir,None).expect("typed i32 IR emits C without signed UB");
        assert!(artifacts.module_c.contains("aura_i32_from_wrapped_bits"),
            "{label}: I32 must use modular native C conversion");
        let root=Scratch::new();
        fs::write(root.root.join("aura_runtime.h"),&artifacts.runtime_h).unwrap();
        fs::write(root.root.join("module.c"),&artifacts.module_c).unwrap();
        fs::write(root.root.join("driver.c"),driver).unwrap();
        for opt in ["-O0","-O2"] {
            let exe=root.root.join(format!("signed_i32_{label}_{}",&opt[1..]));
            let compile=Command::new("clang")
                .args(["-std=gnu11","-Wall","-Werror",
                       "-fsanitize=undefined","-fno-sanitize-recover=all",
                       opt,"-I"])
                .arg(&root.root).arg(root.root.join("module.c"))
                .arg(root.root.join("driver.c")).arg("-o").arg(&exe)
                .output().expect("Clang UBSan must exist on Native runner");
            assert!(compile.status.success(),
                "{label}/{opt} typed IR C compilation failed: {}\n{}",
                String::from_utf8_lossy(&compile.stderr),artifacts.module_c);
            for &(x,y) in samples {
                for (op,want) in [
                    ("add32",x.wrapping_add(y)),
                    ("sub32",x.wrapping_sub(y)),
                    ("mul32",x.wrapping_mul(y)),
                    ("neg32",x.wrapping_neg()),
                ] {
                    let output=Command::new(&exe).arg(op)
                        .arg(x.to_string()).arg(y.to_string())
                        .output().expect("i32 native executable runs");
                    assert!(output.status.success(),
                        "{label}/{opt} {op}({x},{y}): undefined behavior:\n{}",
                        String::from_utf8_lossy(&output.stderr));
                    assert_eq!(String::from_utf8_lossy(&output.stdout),
                        format!("{want}\n"),
                        "{label}/{opt} {op}({x},{y}) is not modular i32");
                }
            }
        }
    }
}
