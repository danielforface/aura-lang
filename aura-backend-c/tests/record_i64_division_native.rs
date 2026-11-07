//! Q03-001: typed i64 division through nominal Record construction and
//! projection must agree with the IR Oracle and native UBSan at O0/O2.
//! Source-level signed result inference is NOT qualified by this test.
use aura_backend_c::emit_module;
use aura_ast::Span;
use aura_ir::{
    optimize_module, run_oracle_entry, validate_module, BasicBlock, BinOp,
    BlockId, ExecutionHint, FunctionIR, Inst, InstKind, ModuleIR,
    OracleConfig, OracleValue, Param, RValue, Terminator, Type, ValueId,
};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "aura-q03-i64-div-{}-{nonce}", std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn typed_record_division() -> ModuleIR {
    let span: Span = (0, 0).into();
    let mut m = ModuleIR::new();
    m.record_layouts.insert("SignedQuotient".into(), vec![
        ("result".into(), Type::I64),
        ("numerator".into(), Type::I64),
    ]);
    m.functions.insert("divide_record".into(), FunctionIR {
        name: "divide_record".into(), span,
        params: vec![
            Param { name: "lhs".into(), ty: Type::I64, span, value: ValueId(0) },
            Param { name: "rhs".into(), ty: Type::I64, span, value: ValueId(1) },
        ],
        ret: Type::I64, entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![
                Inst {
                    span, dest: Some(ValueId(2)),
                    kind: InstKind::Binary {
                        op: BinOp::Div, left: ValueId(0), right: ValueId(1),
                    },
                },
                Inst {
                    span, dest: Some(ValueId(3)),
                    kind: InstKind::ConstructRecord {
                        record_name: "SignedQuotient".into(),
                        fields: vec![
                            ("result".into(), ValueId(2)),
                            ("numerator".into(), ValueId(0)),
                        ],
                    },
                },
                Inst {
                    span, dest: Some(ValueId(4)),
                    kind: InstKind::GetField {
                        base: ValueId(3), field_name: "result".into(),
                    },
                },
            ],
            term: Terminator::Return(Some(ValueId(4))),
        }],
    });

    // An Aura Cell must be free to use the helper's spelling without
    // silently hijacking the new generated native runtime function.
    m.functions.insert("aura_i64_div_checked".into(), FunctionIR {
        name: "aura_i64_div_checked".into(), span,
        params: vec![], ret: Type::I64, entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![Inst {
                span, dest: Some(ValueId(0)),
                kind: InstKind::BindStrand {
                    name: "seven".into(), expr: RValue::ConstI64(7),
                },
            }],
            term: Terminator::Return(Some(ValueId(0))),
        }],
    });
    m
}

#[test]
fn typed_i64_record_division_is_ubsan_clean_and_traps_fail_closed() {
    let original = typed_record_division();
    validate_module(&original).expect("original typed Record division IR valid");
    let mut optimized = original.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized typed Record division IR valid");

    let cases: [(i64, i64); 10] = [
        (i64::MIN, 1), (i64::MIN, 2), (i64::MIN, i64::MAX),
        (i64::MAX, -1), (-13, 3), (13, -3), (-13, -3),
        (0, 9), (i64::MAX, 7), (-4_294_967_299, 7),
    ];
    let driver = r#"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
int64_t divide_record(int64_t lhs, int64_t rhs);
int64_t aura_cell_617572615f6936345f6469765f636865636b6564(void);
int main(int argc, char **argv) {
  if (argc > 1) {
    if (argv[1][0] == 'z') { (void)divide_record(7, 0); return 91; }
    (void)divide_record(INT64_MIN, -1);
    return 92;
  }
  if (aura_cell_617572615f6936345f6469765f636865636b6564() != 7) return 93;
  const int64_t cases[][3] = {
    {INT64_MIN, 1, INT64_MIN},
    {INT64_MIN, 2, INT64_MIN / 2},
    {INT64_MIN, INT64_MAX, -1},
    {INT64_MAX, -1, -INT64_MAX},
    {-13, 3, -4},
    {13, -3, -4},
    {-13, -3, 4},
    {0, 9, 0},
    {INT64_MAX, 7, INT64_MAX / 7},
    {-4294967299LL, 7, -613566757LL},
  };
  for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
    if (divide_record(cases[i][0], cases[i][1]) != cases[i][2]) {
      fprintf(stderr, "incorrect Record i64 division case %zu\n", i);
      return 94;
    }
  }
  return 0;
}
"#;

    for (profile, ir) in [("original", &original), ("optimized", &optimized)] {
        for (lhs, rhs) in cases {
            let oracle = run_oracle_entry(
                ir, "divide_record",
                &[OracleValue::I64(lhs), OracleValue::I64(rhs)],
                OracleConfig::default(),
            ).expect("Oracle implements valid signed i64 division");
            assert!(oracle.ok && oracle.stderr.is_empty(),
                "{profile}: unexpected Oracle failure for {lhs}/{rhs}");
            assert_eq!(oracle.return_value, Some(OracleValue::I64(lhs / rhs)),
                "{profile}: Oracle signed division result differs");
        }

        let c = emit_module(ir, None)
            .expect("typed i64 division and nominal Record can emit native C");
        assert!(c.runtime_h.contains("static inline int64_t aura_i64_div_checked("),
            "{profile}: division guard missing");
        assert!(c.module_c.contains("aura_i64_div_checked("),
            "{profile}: native division did not use checked helper");
        assert!(c.module_c.contains(
            "aura_cell_617572615f6936345f6469765f636865636b6564"
        ), "{profile}: user Cell shadows native runtime helper");

        let scratch = Scratch::new();
        fs::write(scratch.0.join("aura_runtime.h"), &c.runtime_h).unwrap();
        fs::write(scratch.0.join("module.c"), &c.module_c).unwrap();
        fs::write(scratch.0.join("driver.c"), driver).unwrap();

        for opt in ["-O0", "-O2"] {
            let exe = scratch.0.join(format!("i64_div_{profile}_{}", &opt[1..]));
            let cc = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror",
                       "-fsanitize=undefined", "-fno-sanitize-recover=all",
                       opt, "-I"])
                .arg(&scratch.0)
                .arg(scratch.0.join("module.c"))
                .arg(scratch.0.join("driver.c"))
                .arg("-o").arg(&exe)
                .output().expect("CI clang is available");
            assert!(cc.status.success(),
                "{profile}/{opt}: native C compilation failed: {}\n{}",
                String::from_utf8_lossy(&cc.stderr), c.module_c);

            let valid = Command::new(&exe).output()
                .expect("native Record i64 division driver runs");
            assert!(valid.status.success(),
                "{profile}/{opt}: incorrect valid native quotient: {}",
                String::from_utf8_lossy(&valid.stderr));

            for trap in ["zero", "overflow"] {
                let rejected = Command::new(&exe).arg(trap).output()
                    .expect("native signed division trap driver starts");
                assert!(!rejected.status.success(),
                    "{profile}/{opt}: dangerous {trap} division succeeded");
                assert!(String::from_utf8_lossy(&rejected.stderr)
                    .contains("Aura i64 division rejected"),
                    "{profile}/{opt}: {trap} failed without checked rejection: {}",
                    String::from_utf8_lossy(&rejected.stderr));
            }
        }
    }
}
