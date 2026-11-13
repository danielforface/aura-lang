//! Q03-001 Native trust boundary: C must never accept mixed numeric,
//! non-Boolean logical or unsupported nominal comparisons merely because
//! the host C language implicitly converts the operands.

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
            "aura-record_comparison_type_guards_native-{}-{nanos}", std::process::id()
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
use aura_core::{lower_program, Checker};

use aura_ir::{
    BasicBlock, BinOp, BlockId, ExecutionHint, FunctionIR, Inst, ModuleIR,
    Param, Terminator, ValueId,
};

#[test]
fn source_record_numeric_comparison_survives_typed_native_guards() {
    // Genuine supported source: nominal Record fields, not an invented IR-only
    // feature. Both optimization profiles must agree with the IR Oracle.
    let src = r#"
type Pair = record { left: u32, right: u32 }
cell equal_pair(x: u32, y: u32) ->:
    val p: Pair = Pair { left: x, right: y }
    yield p.left == p.right
cell lower_pair(x: u32, y: u32) ->:
    val p: Pair = Pair { left: x, right: y }
    yield p.left < p.right
"#;
    let ast = aura_parse::parse_source(src).expect("Record comparison source parses");
    Checker::new().check_program(&ast).expect("Record comparison source checks");
    let original = lower_program(&ast).expect("Record comparison source lowers");
    validate_module(&original).expect("original source IR valid");
    let mut optimized = original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized source IR valid");
    let inputs: [(u32, u32); 8] = [
        (0,0), (0,1), (7,7), (7,3),
        (u32::MAX,u32::MAX), (u32::MAX,0), (0,u32::MAX),
        (2_147_483_648, 2_147_483_647),
    ];
    let driver = concat!(
        "#include <stdint.h>\n#include <stdbool.h>\n",
        "bool equal_pair(uint32_t, uint32_t);\n",
        "bool lower_pair(uint32_t, uint32_t);\n",
        "int main(void) {\n",
        "  uint32_t const x[] = {0u,0u,7u,7u,UINT32_MAX,UINT32_MAX,0u,2147483648u};\n",
        "  uint32_t const y[] = {0u,1u,7u,3u,UINT32_MAX,0u,UINT32_MAX,2147483647u};\n",
        "  for (unsigned i=0;i<8u;++i) {\n",
        "    if(equal_pair(x[i],y[i])!=(x[i]==y[i]))return 11;\n",
        "    if(lower_pair(x[i],y[i])!=(x[i]<y[i]))return 12;\n",
        "  }\n",
        "  return 0;\n}\n"
    );
    for (profile, ir) in [("source", &original), ("optimized", &optimized)] {
        for (x, y) in inputs {
            let args=[OracleValue::U32(x),OracleValue::U32(y)];
            for (fn_name, expected) in [
                ("equal_pair", x == y), ("lower_pair", x < y),
            ] {
                let result=run_oracle_entry(ir,fn_name,&args,OracleConfig::default())
                    .expect("Oracle executes qualified source Record comparison");
                assert!(result.ok && result.stderr.is_empty(), "{profile}/{fn_name}");
                assert_eq!(result.return_value,Some(OracleValue::Bool(expected)),
                    "{profile}/{fn_name}: source Record result changed");
            }
        }
        let c=emit_module(ir,None).expect("qualified source Record comparison emits C");
        let temp=Scratch::new();
        fs::write(temp.0.join("aura_runtime.h"),&c.runtime_h).unwrap();
        fs::write(temp.0.join("module.c"),&c.module_c).unwrap();
        fs::write(temp.0.join("driver.c"),driver).unwrap();
        for opt in ["-O0","-O2"] {
            let exe=temp.0.join(format!("record_compare_{profile}_{}", &opt[1..]));
            let build=Command::new("clang")
                .args(["-std=gnu11","-Wall","-Werror",
                       "-fsanitize=undefined","-fno-sanitize-recover=all",opt,"-I"])
                .arg(&temp.0).arg(temp.0.join("module.c"))
                .arg(temp.0.join("driver.c")).arg("-o").arg(&exe)
                .output().expect("Native CI clang installed");
            assert!(build.status.success(),"{profile}/{opt}: native C: {}\n{}",
                String::from_utf8_lossy(&build.stderr),c.module_c);
            let run=Command::new(&exe).output().expect("linked native Record comparison");
            assert!(run.status.success(),"{profile}/{opt}: invalid numeric comparison: {}",
                String::from_utf8_lossy(&run.stderr));
        }
    }
}

fn typed_record_comparison(op: BinOp, lhs: &str, rhs: &str) -> ModuleIR {
    let span: aura_ast::Span=(0,0).into();
    let mut ir=ModuleIR::new();
    ir.record_layouts.insert("Mixed".into(), vec![
        ("num".into(),Type::U32),("signed".into(),Type::I64),
        ("flag".into(),Type::Bool),("floating".into(),Type::F64),
    ]);
    let params=[
        ("num",Type::U32),("signed",Type::I64),
        ("flag",Type::Bool),("floating",Type::F64),
    ];
    let fields=["num","signed","flag","floating"];
    let get_field_index = |field: &str| fields.iter()
        .position(|candidate| *candidate == field)
        .expect("fixture only uses known Record fields");
    let insts = vec![
        Inst {
            span, dest:Some(ValueId(4)),
            kind:InstKind::ConstructRecord {
                record_name:"Mixed".into(),
                fields:fields.iter().enumerate()
                    .map(|(i,field)| ((*field).into(), ValueId(i as u32))).collect(),
            },
        },
        Inst {
            span,dest:Some(ValueId(5)),
            kind:InstKind::GetField{base:ValueId(4),field_name:lhs.into()},
        },
        Inst {
            span,dest:Some(ValueId(6)),
            kind:InstKind::GetField{base:ValueId(4),field_name:rhs.into()},
        },
        Inst {
            span,dest:Some(ValueId(7)),
            kind:InstKind::Binary{op,left:ValueId(5),right:ValueId(6)},
        },
    ];
    // The returned type is Bool for the checked comparison/logical operators.
    let _ = get_field_index(lhs);
    let _ = get_field_index(rhs);
    ir.functions.insert("compare_mixed".into(),FunctionIR {
        name:"compare_mixed".into(),span,
        params:params.iter().enumerate().map(|(i,(name,ty))|Param{
            name:(*name).into(),ty:ty.clone(),span,value:ValueId(i as u32)
        }).collect(),
        ret:Type::Bool,entry:BlockId(0),
        blocks:vec![BasicBlock{
            id:BlockId(0),span,hint:ExecutionHint::Sequential,insts,
            term:Terminator::Return(Some(ValueId(7))),
        }],
    });
    ir
}

#[test]
fn typed_record_comparison_rejects_c_implicit_type_coercions() {
    // Each case is a structurally valid nominal Record IR program with a
    // well-typed Record layout; only the binary operation is unqualified.
    // Old C emitter accepted these based on C conversion/truthiness.
    for (op,lhs,rhs) in [
        (BinOp::Eq,"num","signed"),      // u32 == i64 (C accepts)
        (BinOp::Ne,"signed","num"),      // i64 != u32 (C accepts)
        (BinOp::Lt,"num","floating"),   // u32 < f64 (C accepts)
        (BinOp::Ge,"floating","signed"),// f64 >= i64 (C accepts)
        (BinOp::And,"flag","num"),      // bool && u32 (C accepts)
        (BinOp::Or,"num","flag"),       // u32 || bool (C accepts)
        (BinOp::Eq,"num","flag"),       // u32 == bool (C accepts)
        (BinOp::Lt,"flag","flag"),      // bool ordering (C accepts)
    ] {
        let module=typed_record_comparison(op,lhs,rhs);
        validate_module(&module).expect("fixture is structurally valid Record SSA");
        let rejected=emit_module(&module,None).err()
            .expect("unqualified typed IR must never emit successfully in C");
        assert!(rejected.message.contains("unqualified C comparison/logical operands"),
            "{op:?} {lhs}/{rhs}: backend wrong rejection: {rejected}");

        // The independent Oracle refuses unqualified mixed/Boolean-order ops.
        let oracle=run_oracle_entry(
            &module,"compare_mixed",
            &[OracleValue::U32(1),OracleValue::I64(1),
              OracleValue::Bool(true),OracleValue::F64(1.0)],
            OracleConfig::default(),
        );
        assert!(oracle.is_err() || !oracle.as_ref().unwrap().ok,
            "{op:?} {lhs}/{rhs}: Oracle unexpectedly approved the malformed operation");
    }
}

#[test]
fn typed_record_bool_logic_and_numeric_order_stay_accepted() {
    // Guard changes must not outlaw qualified Bool and exact numeric
    // comparisons. Execute all eight cases through an independent native
    // driver, in separate O0/O2 executables, not just text-only emission.
    let cases = [
        (BinOp::Eq,"flag","flag",true),
        (BinOp::Ne,"flag","flag",false),
        (BinOp::And,"flag","flag",true),
        (BinOp::Or,"flag","flag",true),
        (BinOp::Eq,"num","num",true),
        (BinOp::Lt,"num","num",false),
        (BinOp::Ge,"floating","floating",true),
        (BinOp::Eq,"signed","signed",true),
    ];
    let mut original = ModuleIR::new();
    let mut driver = String::from("#include <stdbool.h>\n#include <stdint.h>\n");
    for (index, (op, left, right, expected)) in cases.iter().enumerate() {
        let one = typed_record_comparison(*op, left, right);
        validate_module(&one).expect("valid typed Record comparison SSA");
        if *left != "floating" {
            // Oracle models U32, I64 and Bool. Typed F64 ordering is only
            // claimed at the native C boundary, not as Oracle coverage.
            let oracle = run_oracle_entry(
                &one, "compare_mixed",
                &[OracleValue::U32(1), OracleValue::I64(1),
                  OracleValue::Bool(true), OracleValue::F64(1.0)],
                OracleConfig::default(),
            ).expect("Oracle models qualified primitive comparison");
            assert!(oracle.ok && oracle.stderr.is_empty());
            assert_eq!(oracle.return_value, Some(OracleValue::Bool(*expected)),
                "{op:?} {left}/{right}: Oracle result mismatch");
        }
        if original.record_layouts.is_empty() {
            original.record_layouts = one.record_layouts.clone();
        }
        let mut func = one.functions.get("compare_mixed").unwrap().clone();
        let name = format!("qualified_comparison_{index}");
        func.name = name.clone();
        original.functions.insert(name.clone(), func);
        driver.push_str(&format!(
            "bool {name}(uint32_t, int64_t, bool, double);\n"
        ));
    }
    driver.push_str("int main(void) {\n");
    for (index, (_, _, _, expected)) in cases.iter().enumerate() {
        let value = if *expected { "true" } else { "false" };
        driver.push_str(&format!(
            "  if (qualified_comparison_{index}(1u, 1LL, true, 1.0) != {value}) return {};\n",
            index + 1
        ));
    }
    driver.push_str("  return 0;\n}\n");

    validate_module(&original).expect("combined native comparison IR valid");
    let mut optimized = original.clone();
    aura_ir::optimize_module(&mut optimized);
    validate_module(&optimized).expect("optimized native comparison IR valid");
    for (profile, ir) in [("original", &original), ("optimized", &optimized)] {
        let c = emit_module(ir, None).expect("qualified typed Record comparisons emit C");
        let temp = Scratch::new();
        fs::write(temp.0.join("aura_runtime.h"), &c.runtime_h).unwrap();
        fs::write(temp.0.join("module.c"), &c.module_c).unwrap();
        fs::write(temp.0.join("driver.c"), &driver).unwrap();
        for opt in ["-O0", "-O2"] {
            let exe = temp.0.join(format!("valid_record_compare_{profile}_{}", &opt[1..]));
            let build = Command::new("clang")
                .args(["-std=gnu11", "-Wall", "-Werror",
                       "-fsanitize=undefined", "-fno-sanitize-recover=all",
                       opt, "-I"])
                .arg(&temp.0).arg(temp.0.join("module.c"))
                .arg(temp.0.join("driver.c")).arg("-o").arg(&exe)
                .output().expect("Native CI clang installed");
            assert!(build.status.success(), "{profile}/{opt}: C compile: {}\n{}",
                String::from_utf8_lossy(&build.stderr), c.module_c);
            let run = Command::new(&exe).output()
                .expect("compiled native Record comparison driver");
            assert!(run.status.success(),
                "{profile}/{opt}: Bool/numeric operation mismatched: {}",
                String::from_utf8_lossy(&run.stderr));
        }
    }
}
