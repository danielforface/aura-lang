//! Q03-001 typed-IR F64 comparisons through nominal Records.
//! The C backend already qualified same-type IEEE binary64 Eq/Ne/Lt/Gt/Le/Ge.
//! The Oracle used to reject those six operations, blocking actual differential
//! validation. This suite keeps the source-level Float inference and F32/ABI
//! contracts untouched, and runs the same IR in C at O0/O2 with UBSan.

use aura_backend_c::emit_module;
use aura_ir::{run_oracle_entry, validate_module, OracleConfig, OracleValue, InstKind, Type};
use std::{fs, process::Command};

/// Isolated per-test harness: Cargo also compiles this file as an independent
/// integration test, not just under record_function_abi_native's #[path] mod.
struct Scratch(std::path::PathBuf);
impl Scratch {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after Unix epoch").as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "aura-record_f64_comparisons_native-{}-{nanos}", std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create native test scratch directory");
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

use aura_ast::Span;
use aura_ir::{
    optimize_module, BasicBlock, BinOp, BlockId, ExecutionHint, FunctionIR,
    Inst, ModuleIR, Param, Terminator, ValueId,
};

const OPS: [(&str, BinOp); 6] = [
    ("q03_cmp_eq", BinOp::Eq),
    ("q03_cmp_ne", BinOp::Ne),
    ("q03_cmp_lt", BinOp::Lt),
    ("q03_cmp_gt", BinOp::Gt),
    ("q03_cmp_le", BinOp::Le),
    ("q03_cmp_ge", BinOp::Ge),
];

fn float_compare_record_ir(lhs_type: Type, rhs_type: Type) -> ModuleIR {
    let span: Span = (0, 0).into();
    let mut module = ModuleIR::new();
    module.record_layouts.insert("ComparisonInputs".into(), vec![
        ("left".into(), lhs_type.clone()),
        ("right".into(), rhs_type.clone()),
    ]);
    for (name, op) in OPS {
        module.functions.insert(name.into(), FunctionIR {
            name: name.into(),
            span,
            entry: BlockId(0),
            params: vec![
                Param { name: "left".into(), ty: lhs_type.clone(), span, value: ValueId(0) },
                Param { name: "right".into(), ty: rhs_type.clone(), span, value: ValueId(1) },
            ],
            ret: Type::Bool,
            blocks: vec![BasicBlock {
                id: BlockId(0), span, hint: ExecutionHint::Sequential,
                insts: vec![
                    Inst { span, dest: Some(ValueId(2)), kind: InstKind::ConstructRecord {
                        record_name: "ComparisonInputs".into(),
                        fields: vec![
                            ("left".into(), ValueId(0)), ("right".into(), ValueId(1)),
                        ],
                    }},
                    Inst { span, dest: Some(ValueId(3)), kind: InstKind::GetField {
                        base: ValueId(2), field_name: "left".into(),
                    }},
                    Inst { span, dest: Some(ValueId(4)), kind: InstKind::GetField {
                        base: ValueId(2), field_name: "right".into(),
                    }},
                    Inst { span, dest: Some(ValueId(5)), kind: InstKind::Binary {
                        op, left: ValueId(3), right: ValueId(4),
                    }},
                ],
                term: Terminator::Return(Some(ValueId(5))),
            }],
        });
    }
    module
}

fn expected(a: f64, b: f64) -> [bool; 6] {
    [a == b, a != b, a < b, a > b, a <= b, a >= b]
}

fn comparison_cases() -> Vec<(f64, f64)> {
    let values = [
        0.0, -0.0, 1.0, -1.0, 3.5,
        f64::MIN_POSITIVE, -f64::MIN_POSITIVE,
        f64::from_bits(1), -f64::from_bits(1),
        f64::MAX, f64::INFINITY, f64::NEG_INFINITY,
        f64::from_bits(0x7ff8_0000_0000_0042),
        f64::from_bits(0xfff8_0000_0000_0001),
    ];
    let mut pairs = Vec::new();
    for a in values {
        for b in values {
            pairs.push((a, b));
        }
    }
    // Deterministic finite input range, not a stochastic acceptance claim.
    let mut state: u64 = 0x8e11_2026_0f64_a11c;
    for _ in 0..64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let a = f64::from_bits(state);
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let b = f64::from_bits(state);
        if a.is_finite() && b.is_finite() {
            pairs.push((a, b));
        }
    }
    pairs
}

#[test]
fn typed_f64_record_six_ieee_comparisons_match_oracle_and_compiled_c() {
    let original = float_compare_record_ir(Type::F64, Type::F64);
    validate_module(&original).expect("original nominal F64 comparison IR valid");
    let mut optimized = original.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized nominal F64 comparison IR valid");
    let cases = comparison_cases();
    assert!(cases.len() >= 196, "all 14x14 IEEE boundary pairs present");

    let mut driver = String::from(
        "#include <stdbool.h>\n#include <stdint.h>\n#include <string.h>\n"
    );
    for (name, _) in OPS {
        driver.push_str(&format!("bool {name}(double, double);\n"));
    }
    driver.push_str("int main(void) {\n");
    for (index, (a, b)) in cases.iter().enumerate() {
        let flags = expected(*a, *b);
        let expected_literals = flags.iter()
            .map(|flag| if *flag { "1" } else { "0" })
            .collect::<Vec<_>>().join(", ");
        driver.push_str(&format!(
            "  {{ uint64_t la = UINT64_C(0x{:016x}), lb = UINT64_C(0x{:016x});\n",
            a.to_bits(), b.to_bits()
        ));
        driver.push_str("    double lhs, rhs;\n");
        driver.push_str("    memcpy(&lhs, &la, sizeof(lhs));\n");
        driver.push_str("    memcpy(&rhs, &lb, sizeof(rhs));\n");
        driver.push_str(&format!(
            "    const unsigned char want[6] = {{{expected_literals}}};\n"
        ));
        for (op_index, (name, _)) in OPS.iter().enumerate() {
            driver.push_str(&format!(
                "    if ((unsigned char){name}(lhs, rhs) != want[{op_index}]) return {};\n",
                1 + (index % 200) * 6 + op_index
            ));
        }
        driver.push_str("  }\n");
    }
    driver.push_str("  return 0;\n}\n");

    for (profile, ir) in [("original", &original), ("optimized", &optimized)] {
        for (a, b) in &cases {
            let expected_flags = expected(*a, *b);
            for (op_index, (name, _)) in OPS.iter().enumerate() {
                let result = run_oracle_entry(
                    ir, name,
                    &[OracleValue::F64(*a), OracleValue::F64(*b)],
                    OracleConfig::default(),
                ).expect("Oracle evaluates qualified typed F64 comparison");
                assert!(result.ok && result.stderr.is_empty(),
                    "{profile}/{name}: Oracle failed for {:016x}/{:016x}",
                    a.to_bits(), b.to_bits());
                assert_eq!(result.return_value,
                    Some(OracleValue::Bool(expected_flags[op_index])),
                    "{profile}/{name}: IEEE-754 comparison mismatch at {:016x}/{:016x}",
                    a.to_bits(), b.to_bits());
            }
        }
        let emitted = emit_module(ir, None)
            .expect("fully typed nominal F64 equality/order emits native C");
        let tmp = Scratch::new();
        fs::write(tmp.0.join("aura_runtime.h"), &emitted.runtime_h).unwrap();
        fs::write(tmp.0.join("module.c"), &emitted.module_c).unwrap();
        fs::write(tmp.0.join("driver.c"), &driver).unwrap();

        for opt in ["-O0", "-O2"] {
            let exe = tmp.0.join(format!("f64_cmp_{profile}_{}", &opt[1..]));
            let cc = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror",
                       "-fsanitize=undefined", "-fno-sanitize-recover=all",
                       opt, "-I"])
                .arg(&tmp.0)
                .arg(tmp.0.join("module.c"))
                .arg(tmp.0.join("driver.c"))
                .arg("-o").arg(&exe)
                .output().expect("Clang available in required Native CI");
            assert!(cc.status.success(),
                "{profile}/{opt}: native IEEE comparison compile: {}\n{}",
                String::from_utf8_lossy(&cc.stderr), emitted.module_c);
            let actual = Command::new(&exe).output()
                .expect("linked F64 comparison executable");
            assert!(actual.status.success(),
                "{profile}/{opt}: IEEE comparison diverged: status={:?}, stderr={}",
                actual.status, String::from_utf8_lossy(&actual.stderr));
        }
    }
}

#[test]
fn typed_f64_record_comparisons_reject_mixed_operands() {
    for (left, right, args) in [
        (Type::F64, Type::I64,
            [OracleValue::F64(1.0), OracleValue::I64(1)]),
        (Type::I64, Type::F64,
            [OracleValue::I64(1), OracleValue::F64(1.0)]),
        (Type::F64, Type::Bool,
            [OracleValue::F64(1.0), OracleValue::Bool(true)]),
        (Type::U32, Type::F64,
            [OracleValue::U32(1), OracleValue::F64(1.0)]),
    ] {
        let ir = float_compare_record_ir(left.clone(), right.clone());
        validate_module(&ir).expect("negative Record comparison structurally valid");
        for (name, _) in OPS {
            let err = emit_module(&ir, None).err()
                .expect("mixed F64 comparison must be rejected by Native C");
            assert!(err.message.contains("unqualified C comparison/logical operands"),
                "{left:?}/{right:?}: unexpected native error: {err}");
            let result = run_oracle_entry(
                &ir, name, &args, OracleConfig::default(),
            );
            assert!(result.is_err(),
                "{left:?}/{right:?} {name}: Oracle accepted mixed typed values");
        }
    }
}
