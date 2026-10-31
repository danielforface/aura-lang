//! AURA-Q03-001 / Issue #51: IR SetField semantics must not diverge from
//! the published IR Oracle. Until a consistent functional update or mutable
//! write contract is implemented, SetField should explicitly fail closed.
use aura_backend_c::emit_module;
use aura_ast::Span;
use aura_ir::{
    run_oracle_entry, validate_module, BasicBlock, BlockId, ExecutionHint,
    FunctionIR, Inst, InstKind, ModuleIR, OracleConfig, OracleValue,
    Param, Terminator, Type, ValueId,
};

#[test]
fn record_setfield_without_result_must_not_mutate_oracle_original_silently() {
    let span: Span = (0, 0).into();
    let mut m = ModuleIR::new();
    m.record_layouts.insert("Point".into(), vec![
        ("x".into(), Type::U32), ("y".into(), Type::U32),
    ]);
    m.functions.insert("read_original".into(), FunctionIR {
        name: "read_original".into(), span,
        params: vec![
            Param { name: "old_x".into(), ty: Type::U32, span, value: ValueId(0) },
            Param { name: "new_x".into(), ty: Type::U32, span, value: ValueId(1) },
        ],
        ret: Type::U32, entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![
                Inst {
                    span, dest: Some(ValueId(2)),
                    kind: InstKind::ConstructRecord {
                        record_name: "Point".into(),
                        fields: vec![("x".into(), ValueId(0)), ("y".into(), ValueId(1))],
                    },
                },
                Inst {
                    span, dest: None,
                    kind: InstKind::SetField {
                        base: ValueId(2), field_name: "x".into(), value: ValueId(1),
                    },
                },
                Inst {
                    span, dest: Some(ValueId(3)),
                    kind: InstKind::GetField { base: ValueId(2), field_name: "x".into() },
                },
            ],
            term: Terminator::Return(Some(ValueId(3))),
        }],
    });
    validate_module(&m).expect("SSA fixture passes lightweight structural validation");
    let oracle = run_oracle_entry(&m, "read_original",
        &[OracleValue::U32(7), OracleValue::U32(19)], OracleConfig::default())
        .expect("IR oracle should interpret Record SetField");
    assert!(oracle.ok);
    assert_eq!(oracle.return_value, Some(OracleValue::U32(7)),
        "Oracle dest=None SetField does not replace original record value");
    // The previous C emitter incorrectly wrote to v2.x and would return 19
    // while the oracle says 7. Explicit refusal is safer than divergence.
    match emit_module(&m, None) {
        Ok(out) => panic!(
            "unqualified C SetField changed Record semantics but claimed successful emission: {}",
            out.module_c
        ),
        Err(err) => {
            let msg = err.message.to_ascii_lowercase();
            assert!(msg.contains("record") && msg.contains("setfield") && msg.contains("unqualified"),
                "must fail for semantic SetField uncertainty, not unrelated issue: {err}");
        }
    }
}
