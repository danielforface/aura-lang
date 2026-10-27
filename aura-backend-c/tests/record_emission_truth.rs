//! Issue #51 / AURA-Q03-001: C emission must never silently omit record
//! construction, field reads or updates. This is TEST-FIRST; until typed
//! record layouts exist, explicit backend refusal is the required P0 boundary.
use aura_backend_c::emit_module;
use aura_ast::Span;
use aura_ir::{
    validate_module, BasicBlock, BlockId, ExecutionHint, FunctionIR,
    Inst, InstKind, ModuleIR, Param, Terminator, Type, ValueId,
};

fn module_with_record(read_after_update: bool) -> ModuleIR {
    let span: Span = (0, 0).into();
    let mut module = ModuleIR::new();
    let mut insts = vec![Inst {
        span,
        dest: Some(ValueId(2)),
        kind: InstKind::ConstructRecord {
            record_name: "Point".into(),
            fields: vec![("x".into(), ValueId(0)), ("y".into(), ValueId(1))],
        },
    }];
    if read_after_update {
        insts.push(Inst {
            span,
            dest: None,
            kind: InstKind::SetField {
                base: ValueId(2), field_name: "x".into(), value: ValueId(1),
            },
        });
    }
    insts.push(Inst {
        span,
        dest: Some(ValueId(3)),
        kind: InstKind::GetField { base: ValueId(2), field_name: "x".into() },
    });
    module.functions.insert("read_point_x".into(), FunctionIR {
        name: "read_point_x".into(),
        span,
        params: vec![
            Param { name: "x".into(), ty: Type::U32, span, value: ValueId(0) },
            Param { name: "y".into(), ty: Type::U32, span, value: ValueId(1) },
        ],
        ret: Type::U32,
        entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts, term: Terminator::Return(Some(ValueId(3))),
        }],
    });
    module
}

fn must_explicitly_refuse_unimplemented_record(module: ModuleIR) {
    validate_module(&module).expect("record regression fixture must be valid SSA IR");
    match emit_module(&module, None) {
        Ok(artifact) => panic!(
            "C backend claimed success after silently omitting live Record IR: {}",
            artifact.module_c
        ),
        Err(error) => {
            let msg = error.message.to_ascii_lowercase();
            assert!(
                msg.contains("record") && (
                    msg.contains("unsupported") || msg.contains("unimplemented")
                        || msg.contains("unqualified")
                ),
                "backend must reject Record semantics explicitly, not fail due to invalid IR: {error}"
            );
        }
    }
}

#[test]
fn live_record_construction_and_field_read_cannot_be_silent_noops() {
    must_explicitly_refuse_unimplemented_record(module_with_record(false));
}

#[test]
fn live_record_update_and_field_read_cannot_be_silent_noops() {
    must_explicitly_refuse_unimplemented_record(module_with_record(true));
}

#[test]
fn scalar_only_control_still_emits_c() {
    let span: Span = (0, 0).into();
    let mut module = ModuleIR::new();
    module.functions.insert("scalar_identity".into(), FunctionIR {
        name: "scalar_identity".into(), span,
        params: vec![
            Param { name: "x".into(), ty: Type::U32, span, value: ValueId(0) },
        ],
        ret: Type::U32,
        entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![], term: Terminator::Return(Some(ValueId(0))),
        }],
    });
    validate_module(&module).expect("scalar control IR valid");
    let artifact = emit_module(&module, None).expect("scalar-only C emission must remain available");
    assert!(artifact.module_c.contains("scalar_identity"),
        "valid known scalar function not emitted");
}
