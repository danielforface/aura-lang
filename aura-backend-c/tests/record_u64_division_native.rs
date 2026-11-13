//! Q03-001: true uint64_t division through typed IR and nominal Records.
//! Previously the C backend rejected valid U64/U64 Div despite supporting
//! U64 params, Record fields and other U64 arithmetic. Test the entire width,
//! particularly divisors with only high bits set. No source-level U64 operator
//! inference, OracleValue::U64, or cross-agent AST/IR schema is implied.
use aura_ast::Span;
use aura_backend_c::emit_module;
use aura_ir::{
    optimize_module, validate_module, BasicBlock, BinOp, BlockId,
    ExecutionHint, FunctionIR, Inst, InstKind, ModuleIR, Param,
    Terminator, Type, ValueId,
};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "aura-q03-u64-record-div-{}-{nonce}", std::process::id()
        ));
        fs::create_dir_all(&path).expect("create Native U64 scratch dir");
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn typed_record_u64_division() -> ModuleIR {
    let span: Span = (0, 0).into();
    let mut module = ModuleIR::new();
    module.record_layouts.insert("UnsignedQuotient".into(), vec![
        ("numerator".into(), Type::U64),
        ("quotient".into(), Type::U64),
    ]);
    module.functions.insert("divide_record_u64".into(), FunctionIR {
        name: "divide_record_u64".into(), span,
        params: vec![
            Param { name: "left".into(), ty: Type::U64, span, value: ValueId(0) },
            Param { name: "right".into(), ty: Type::U64, span, value: ValueId(1) },
        ],
        ret: Type::U64, entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![
                Inst { span, dest: Some(ValueId(2)), kind: InstKind::Binary {
                    op: BinOp::Div, left: ValueId(0), right: ValueId(1),
                }},
                Inst { span, dest: Some(ValueId(3)),
                    kind: InstKind::ConstructRecord {
                        record_name: "UnsignedQuotient".into(),
                        fields: vec![
                            ("quotient".into(), ValueId(2)),
                            ("numerator".into(), ValueId(0)),
                        ],
                    },
                },
                Inst { span, dest: Some(ValueId(4)),
                    kind: InstKind::GetField {
                        base: ValueId(3), field_name: "quotient".into(),
                    },
                },
            ],
            term: Terminator::Return(Some(ValueId(4))),
        }],
    });

    // Runtime helpers occupy C's ordinary-identifier namespace. A user
    // Cell with this spelling must remain addressable with a unique export.
    module.functions.insert("aura_u64_div_checked".into(), FunctionIR {
        name: "aura_u64_div_checked".into(), span,
        params: vec![
            Param { name: "value".into(), ty: Type::U64, span, value: ValueId(0) },
        ],
        ret: Type::U64, entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![],
            term: Terminator::Return(Some(ValueId(0))),
        }],
    });
    module
}

fn known_pairs() -> Vec<(u64, u64)> {
    let mut pairs = vec![
        (0, 1), (1, 1), (1, u64::MAX),
        (u64::MAX, 1), (u64::MAX, u64::MAX),
        (u64::MAX, u64::MAX - 1),
        (u64::MAX, 2), (u64::MAX, 3),
        (u64::MAX, 4_294_967_296),
        (u64::MAX, 4_294_967_297),
        (u64::MAX, 1u64 << 63),
        (1u64 << 63, 1), (1u64 << 63, 2),
        (1u64 << 63, 1u64 << 63),
        (1u64 << 63, (1u64 << 63) + 1),
        (4_294_967_296, 4_294_967_296),
        (4_294_967_297, 4_294_967_296),
        (4_294_967_296, 4_294_967_297),
        (4_294_967_295, 4_294_967_296),
        (4_294_967_295, 4_294_967_295),
        (9_223_372_036_854_775_807, 4_294_967_296),
        (u64::MAX - 1, u64::MAX),
    ];
    let mut state = 0xa110_ce64_cafe_5eed_u64;
    for _ in 0..256 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let left = state;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let right = if state == 0 { 1 } else { state };
        pairs.push((left, right));
    }
    pairs
}

#[test]
fn typed_u64_division_preserves_high_bits_in_record_native_o0_o2() {
    let original = typed_record_u64_division();
    validate_module(&original).expect("original nominal U64 Record IR valid");
    let mut optimized = original.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized nominal U64 Record IR valid");
    let pairs = known_pairs();

    let helper_cell_hex = "617572615f7536345f6469765f636865636b6564";
    let mut driver = String::from(
        "#include <stdint.h>\n#include <inttypes.h>\n"
    );
    driver.push_str("uint64_t divide_record_u64(uint64_t, uint64_t);\n");
    driver.push_str(&format!(
        "uint64_t aura_cell_{helper_cell_hex}(uint64_t);\n"
    ));
    driver.push_str("int main(int argc, char** argv) {\n  (void)argv;\n");
    driver.push_str("  if (argc > 1) {\n");
    driver.push_str("    (void)divide_record_u64(UINT64_MAX, UINT64_C(0));\n");
    driver.push_str("    return 97;\n  }\n");
    driver.push_str(&format!(
        "  if (aura_cell_{helper_cell_hex}(UINT64_MAX) != UINT64_MAX) return 98;\n"
    ));
    driver.push_str("  static const uint64_t inputs[][3] = {\n");
    for &(left, right) in &pairs {
        assert_ne!(right, 0, "all positive test divisors are nonzero");
        driver.push_str(&format!(
            "    {{ UINT64_C(0x{left:016x}), UINT64_C(0x{right:016x}), UINT64_C(0x{:016x}) }},\n",
            left / right
        ));
    }
    driver.push_str("  };\n");
    driver.push_str("  for (unsigned i = 0; i < sizeof(inputs) / sizeof(inputs[0]); ++i) {\n");
    driver.push_str("    uint64_t got = divide_record_u64(inputs[i][0], inputs[i][1]);\n");
    driver.push_str("    if (got != inputs[i][2]) return 1;\n");
    driver.push_str("  }\n  return 0;\n}\n");

    for (profile, ir) in [("original", &original), ("optimized", &optimized)] {
        let generated = emit_module(ir, None)
            .expect("same-type U64 Record division should produce native C");
        assert!(generated.runtime_h.contains(
            "static inline uint64_t aura_u64_div_checked("
        ), "{profile}: full-width runtime guard missing");
        assert!(generated.module_c.contains("aura_u64_div_checked("),
            "{profile}: IR division failed to use the full-width checked helper");
        assert!(generated.module_c.contains(&format!("aura_cell_{helper_cell_hex}")),
            "{profile}: user Cell collides with native helper symbol");

        let scratch = Scratch::new();
        fs::write(scratch.0.join("aura_runtime.h"), &generated.runtime_h).unwrap();
        fs::write(scratch.0.join("module.c"), &generated.module_c).unwrap();
        fs::write(scratch.0.join("driver.c"), &driver).unwrap();
        for opt in ["-O0", "-O2"] {
            let binary = scratch.0.join(format!("u64_div_{profile}_{}", &opt[1..]));
            let cc = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror", "-fsanitize=undefined",
                       "-fno-sanitize-recover=all", opt, "-I"])
                .arg(&scratch.0)
                .arg(scratch.0.join("module.c"))
                .arg(scratch.0.join("driver.c"))
                .arg("-o").arg(&binary).output().expect("native CI has Clang");
            assert!(cc.status.success(),
                "{profile}/{opt}: u64 compilation: {}\n{}",
                String::from_utf8_lossy(&cc.stderr), generated.module_c);

            let success = Command::new(&binary).output().expect("run u64 division binary");
            assert!(success.status.success(),
                "{profile}/{opt}: wrong high-bit U64 division: {}",
                String::from_utf8_lossy(&success.stderr));

            let rejected = Command::new(&binary).arg("zero").output()
                .expect("run u64 zero divisor rejection");
            assert!(!rejected.status.success(),
                "{profile}/{opt}: zero divisor was incorrectly accepted");
            assert!(String::from_utf8_lossy(&rejected.stderr)
                .contains("Aura u64 division rejected: zero divisor"),
                "{profile}/{opt}: zero fault missing expected guard: {}",
                String::from_utf8_lossy(&rejected.stderr));
        }
    }
}

#[test]
fn typed_u64_division_rejects_mixed_widths_and_noninteger_operands() {
    let span: Span = (0, 0).into();
    for (lhs, rhs) in [
        (Type::U64, Type::U32),
        (Type::U32, Type::U64),
        (Type::U64, Type::I64),
        (Type::I64, Type::U64),
        (Type::U64, Type::F64),
        (Type::F64, Type::U64),
        (Type::U64, Type::Bool),
    ] {
        let mut module = ModuleIR::new();
        module.functions.insert("wrong_division".into(), FunctionIR {
            name: "wrong_division".into(), span,
            params: vec![
                Param { name: "lhs".into(), ty: lhs.clone(), span, value: ValueId(0) },
                Param { name: "rhs".into(), ty: rhs.clone(), span, value: ValueId(1) },
            ],
            ret: Type::U64, entry: BlockId(0),
            blocks: vec![BasicBlock {
                id: BlockId(0), span, hint: ExecutionHint::Sequential,
                insts: vec![Inst { span, dest: Some(ValueId(2)),
                    kind: InstKind::Binary {
                        op: BinOp::Div, left: ValueId(0), right: ValueId(1)
                    },
                }],
                term: Terminator::Return(Some(ValueId(2))),
            }],
        });
        validate_module(&module).expect("mixed-type division fixture is valid structural IR");
        let rejected = emit_module(&module, None)
            .err().expect("mixed type U64 Div must never emit native C");
        assert!(rejected.message.contains("unqualified C numeric division"),
            "{lhs:?}/{rhs:?}: wrong native negative result: {rejected}");
    }
}
