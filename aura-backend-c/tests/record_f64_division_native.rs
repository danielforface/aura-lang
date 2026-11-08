//! Typed IR -> nominal Record -> F64 division -> IR Oracle and Native C.
//! This intentionally does not widen Aura source Float arithmetic, signed
//! source inference, native F32, extern C ABIs or any shared AST/IR schema.

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
            "aura-record_f64_division_native-{}-{nanos}", std::process::id()
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

fn make_division_record() -> ModuleIR {
    let span: Span = (0,0).into();
    let mut module = ModuleIR::new();
    module.record_layouts.insert("Fraction".into(), vec![
        ("numerator".into(), Type::F64), ("quotient".into(), Type::F64),
    ]);
    module.functions.insert("divide_record_f64".into(), FunctionIR {
        name: "divide_record_f64".into(), span, entry: BlockId(0),
        params: vec![
            Param {name:"lhs".into(),ty:Type::F64,span,value:ValueId(0)},
            Param {name:"rhs".into(),ty:Type::F64,span,value:ValueId(1)},
        ],
        ret:Type::F64,
        blocks:vec![BasicBlock {
            id:BlockId(0),span,hint:ExecutionHint::Sequential,
            insts:vec![
                Inst {span,dest:Some(ValueId(2)),
                    kind:InstKind::Binary {
                        op:BinOp::Div,left:ValueId(0),right:ValueId(1),
                    }},
                Inst {span,dest:Some(ValueId(3)),
                    kind:InstKind::ConstructRecord {
                        record_name:"Fraction".into(),
                        fields:vec![
                            ("numerator".into(),ValueId(0)),
                            ("quotient".into(),ValueId(2)),
                        ],
                    }},
                Inst {span,dest:Some(ValueId(4)),
                    kind:InstKind::GetField {
                        base:ValueId(3),field_name:"quotient".into(),
                    }},
            ],
            term:Terminator::Return(Some(ValueId(4))),
        }],
    });
    module
}

fn bit_cases() -> Vec<(f64,f64)> {
    let mut cases=vec![
        (1.0,2.0),(-13.0,2.0),(13.0,-3.0),(-13.0,-3.0),
        (f64::MAX,2.0),(f64::MAX,0.5),
        (f64::MIN_POSITIVE,2.0),(f64::from_bits(1),2.0),
        (f64::from_bits(1),f64::MIN_POSITIVE),
        (1.0,0.0),(-1.0,0.0),(1.0,-0.0),(-1.0,-0.0),
        (0.0,0.0),(-0.0,0.0),(-0.0,-0.0),(-0.0,3.0),
        (0.0,-3.0),(f64::INFINITY,-2.0),(f64::NEG_INFINITY,3.0),
        (f64::INFINITY,f64::INFINITY),
        (1.0,f64::INFINITY),(1.0,f64::NEG_INFINITY),
        (f64::NAN,3.0),(3.0,f64::NAN),
        (0.1,3.0),(1.0,3.0),(-1.0,3.0),
        (f64::from_bits(0x8000_0000_0000_0001),2.0),
        (f64::from_bits(0x7fefffffffffffee),f64::MAX),
    ];
    // Deterministic finite-finite stress vectors, including the extreme
    // exponent range. No random API/service or unrecorded seed is involved.
    let mut state=0xd0e5_1ee7_9fab_1023u64;
    for _ in 0..256 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let left = f64::from_bits(state);
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let right = f64::from_bits(state);
        if left.is_finite() && right.is_finite() {
            cases.push((left,right));
        }
    }
    cases
}

#[test]
fn typed_f64_division_matches_oracle_and_native_ieee754_o0_o2() {
    let original=make_division_record();
    validate_module(&original).expect("typed nominal F64 Record IR structurally valid");
    let mut optimized=original.clone();
    optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized typed F64 Record IR valid");
    let pairs=bit_cases();
    assert!(pairs.len() >= 260, "expected broad deterministic F64 stress coverage");

    // A linked C executable independently checks every expected return
    // bit pattern. NaN payloads are not prescribed by IEEE-754 across
    // compilers, so NaN is checked by classification; all other outputs
    // must match exactly, including +/-0 and subnormal rounding.
    let mut driver=String::from(concat!(
        "#include <stdint.h>\n#include <stdio.h>\n",
        "#include <string.h>\n#include <stddef.h>\n",
        "double divide_record_f64(double, double);\n",
        "static double read_bits(uint64_t b) {\n",
        "  double d; memcpy(&d,&b,sizeof(d)); return d;\n}\n",
        "static uint64_t write_bits(double d) {\n",
        "  uint64_t b; memcpy(&b,&d,sizeof(b)); return b;\n}\n",
        "static const uint64_t cases[][4] = {\n",
    ));
    for &(lhs,rhs) in &pairs {
        let want=lhs/rhs;
        driver.push_str(&format!(
            "  {{UINT64_C(0x{:016x}), UINT64_C(0x{:016x}), UINT64_C(0x{:016x}), {}u}},\n",
            lhs.to_bits(),rhs.to_bits(),want.to_bits(),
            if want.is_nan() {1} else {0}
        ));
    }
    driver.push_str(concat!(
        "};\nint main(void) {\n",
        "  for (size_t i=0;i<sizeof(cases)/sizeof(cases[0]);++i) {\n",
        "    double result=divide_record_f64(read_bits(cases[i][0]),read_bits(cases[i][1]));\n",
        "    uint64_t bits=write_bits(result);\n",
        "    if (cases[i][3]) {\n",
        "      if ((bits & UINT64_C(0x7ff0000000000000)) != UINT64_C(0x7ff0000000000000)\n",
        "          || !(bits & UINT64_C(0x000fffffffffffff))) {\n",
        "        fprintf(stderr,\"non-NaN F64 result at %zu\\n\",i); return 10;\n",
        "      }\n",
        "    } else if (bits != cases[i][2]) {\n",
        "      fprintf(stderr,\"F64 bit mismatch at %zu: %016llx != %016llx\\n\", i,\n",
        "        (unsigned long long)bits,(unsigned long long)cases[i][2]); return 11;\n",
        "    }\n",
        "  }\n",
        "  return 0;\n}\n",
    ));

    for (profile,ir) in [("original",&original),("optimized",&optimized)] {
        for &(lhs,rhs) in &pairs {
            let result=run_oracle_entry(
                ir,"divide_record_f64",
                &[OracleValue::F64(lhs),OracleValue::F64(rhs)],
                OracleConfig::default(),
            ).expect("typed F64 division IR Oracle execution");
            assert!(result.ok && result.stderr.is_empty(),
                "{profile}: Oracle rejected F64({lhs:?}) / F64({rhs:?})");
            match result.return_value {
                Some(OracleValue::F64(value)) if (lhs/rhs).is_nan() => {
                    assert!(value.is_nan(),"{profile}: Oracle NaN classification");
                }
                Some(OracleValue::F64(value)) => {
                    assert_eq!(value.to_bits(),(lhs/rhs).to_bits(),
                        "{profile}: Oracle F64 bit pattern mismatch");
                }
                other => panic!("{profile}: wrong Oracle F64 return: {other:?}"),
            }
        }
        let c=emit_module(ir,None).expect("qualified typed F64 division emits native C");
        assert!(c.module_c.contains(" / "),
            "{profile}: native F64 quotient was not emitted");
        let scratch=Scratch::new();
        fs::write(scratch.0.join("aura_runtime.h"),&c.runtime_h).unwrap();
        fs::write(scratch.0.join("module.c"),&c.module_c).unwrap();
        fs::write(scratch.0.join("driver.c"),&driver).unwrap();
        for opt in ["-O0","-O2"] {
            let exe=scratch.0.join(format!("record_f64_{profile}_{}",&opt[1..]));
            let cc=Command::new("clang")
                .args(["-std=gnu11","-Wall","-Werror",
                    "-fsanitize=undefined","-fno-sanitize-recover=all",
                    opt,"-I"]).arg(&scratch.0)
                .arg(scratch.0.join("module.c"))
                .arg(scratch.0.join("driver.c"))
                .arg("-o").arg(&exe).output().expect("Native CI clang");
            assert!(cc.status.success(),"{profile}/{opt}: F64 native C compile: {}\n{}",
                String::from_utf8_lossy(&cc.stderr),c.module_c);
            let run=Command::new(&exe).output().expect("linked F64 Record division");
            assert!(run.status.success(),"{profile}/{opt}: F64 native result: {}",
                String::from_utf8_lossy(&run.stderr));
        }
    }
}

#[test]
fn typed_f64_division_rejects_mixed_non_ieee_abi_operands() {
    for (lhs,right) in [(Type::F64,Type::I64),(Type::I64,Type::F64),
                        (Type::U32,Type::F64),(Type::F64,Type::Bool)] {
        let span:Span=(0,0).into();
        let mut module=ModuleIR::new();
        module.functions.insert("unqualified".into(),FunctionIR{
            name:"unqualified".into(),span,entry:BlockId(0),ret:Type::F64,
            params:vec![
                Param{name:"lhs".into(),ty:lhs.clone(),span,value:ValueId(0)},
                Param{name:"rhs".into(),ty:right.clone(),span,value:ValueId(1)},
            ],
            blocks:vec![BasicBlock {
                id:BlockId(0),span,hint:ExecutionHint::Sequential,
                insts:vec![Inst{
                    span,dest:Some(ValueId(2)),
                    kind:InstKind::Binary {
                        op:BinOp::Div,left:ValueId(0),right:ValueId(1)
                    },
                }],
                term:Terminator::Return(Some(ValueId(2))),
            }],
        });
        validate_module(&module).expect("negative fixture is structurally valid IR");
        let err=emit_module(&module,None).err()
            .expect("mixed or non-F64 division must be rejected");
        assert!(err.message.contains("unqualified C numeric division"),
            "{lhs:?}/{right:?}: wrong native typed rejection: {err}");
    }
}
