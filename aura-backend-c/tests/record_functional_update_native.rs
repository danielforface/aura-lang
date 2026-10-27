//! Q03-001: a Record SetField with an SSA destination is a *functional*
//! update in aura_ir::run_oracle_entry. Test that emitted native C does the
//! same thing at O0/O2 without mutating the input Record SSA value.
//! Direct typed IR test; not a claim about Aura source syntax for SetField.
use aura_ast::Span;
use aura_backend_c::emit_module;
use aura_ir::{
    run_oracle_entry, validate_module, BasicBlock, BlockId, ExecutionHint,
    FunctionIR, Inst, InstKind, ModuleIR, OracleConfig, OracleValue,
    Param, Terminator, Type, ValueId,
};
use std::{fs, path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};

fn function(name: &str, read_updated: bool) -> FunctionIR {
    let span: Span = (0, 0).into();
    FunctionIR {
        name: name.into(), span,
        params: vec![
            Param { name: "old_value".into(), ty: Type::U32, span, value: ValueId(0) },
            Param { name: "replacement".into(), ty: Type::U32, span, value: ValueId(1) },
        ],
        ret: Type::U32, entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![
                Inst { span, dest: Some(ValueId(2)),
                    kind: InstKind::ConstructRecord {
                        record_name: "Point".into(),
                        fields: vec![("x".into(), ValueId(0)), ("y".into(), ValueId(1))],
                    },
                },
                Inst { span, dest: Some(ValueId(3)),
                    kind: InstKind::SetField {
                        base: ValueId(2), field_name: "x".into(), value: ValueId(1),
                    },
                },
                Inst { span, dest: Some(ValueId(4)),
                    kind: InstKind::GetField {
                        base: if read_updated { ValueId(3) } else { ValueId(2) },
                        field_name: "x".into(),
                    },
                },
            ],
            term: Terminator::Return(Some(ValueId(4))),
        }],
    }
}
fn module() -> ModuleIR {
    let mut ir = ModuleIR::new();
    ir.record_layouts.insert("Point".into(), vec![
        ("x".into(), Type::U32), ("y".into(), Type::U32),
    ]);
    ir.functions.insert("read_updated".into(), function("read_updated", true));
    ir.functions.insert("read_original".into(), function("read_original", false));
    ir
}
#[test]
fn functional_record_update_preserves_original_and_compiles_in_clang_o0_o2() {
    let ir = module();
    validate_module(&ir).expect("functional SetField IR must be structurally valid");
    for (function, expected) in [("read_updated", 19u32), ("read_original", 7u32)] {
        let result = run_oracle_entry(&ir, function,
            &[OracleValue::U32(7), OracleValue::U32(19)],
            OracleConfig::default()).expect("IR Oracle Record update");
        assert!(result.ok && result.stderr.is_empty());
        assert_eq!(result.return_value, Some(OracleValue::U32(expected)),
            "IR Oracle functional SSA update contract for {function}");
    }
    let c = emit_module(&ir, None).expect(
        "functional Record SSA SetField must produce typed native C for qualified fields");
    assert!(c.module_c.contains("aura_record_")
        && c.module_c.contains("aura_f_"),
        "Record code cannot be silently erased: {}", c.module_c);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let temp = std::env::temp_dir().join(format!("aura-functional-record-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();
    let runtime_h = temp.join("aura_runtime.h");
    let generated_c = temp.join("record_module.c");
    let driver_c = temp.join("driver.c");
    fs::write(&runtime_h, c.runtime_h).unwrap();
    fs::write(&generated_c, &c.module_c).unwrap();
    fs::write(&driver_c, concat!(
        "#include <stdint.h>\n",
        "uint32_t read_updated(uint32_t, uint32_t);\n",
        "uint32_t read_original(uint32_t, uint32_t);\n",
        "int main(void) {\n",
        "  return read_updated(7u,19u) == 19u && read_original(7u,19u) == 7u ? 0 : 1;\n",
        "}\n",
    )).unwrap();
    for opt in ["-O0", "-O2"] {
        let binary = temp.join(format!("run_{opt}"));
        let compile = Command::new("clang")
            .args(["-std=gnu11", opt, "-Werror", "-I"]).arg(&temp)
            .arg(&generated_c).arg(&driver_c)
            .arg("-o").arg(&binary).output().expect("run Clang");
        assert!(compile.status.success(),
            "{opt} invalid emitted Record update C: {}\n{}",
            String::from_utf8_lossy(&compile.stderr), c.module_c);
        let native = Command::new(&binary).output().expect("run actual native C executable");
        assert!(native.status.success(),
            "{opt} Record update diverged from IR Oracle: status={} stderr={}\n{}",
            native.status, String::from_utf8_lossy(&native.stderr), c.module_c);
    }
    fs::remove_dir_all(temp).unwrap();
}
