//! Issue #53 / AURA-Q03-002: live Enum construction, discriminant reads,
//! and payload projections must never silently disappear in the C emitter.
//! Test FIRST. Typed tagged-union native emission remains a separate goal.
use aura_backend_c::emit_module;
use aura_ast::Span;
use aura_ir::{
    validate_module, BasicBlock, BlockId, ExecutionHint, FunctionIR, Inst,
    InstKind, ModuleIR, Param, Terminator, Type, ValueId,
};

fn live_enum(construct_and_project_payload: bool) -> ModuleIR {
    let span: Span = (0, 0).into();
    let mut m = ModuleIR::new();
    let mut instructions = vec![Inst {
        span,
        dest: Some(ValueId(1)),
        kind: InstKind::ConstructEnumVariant {
            enum_name: "OptionU32".into(),
            variant_name: "Some".into(),
            tag: 1,
            args: vec![ValueId(0)],
        },
    }];
    instructions.push(if construct_and_project_payload {
        Inst {
            span, dest: Some(ValueId(2)),
            kind: InstKind::GetEnumPayload {
                base: ValueId(1), variant_name: "Some".into(), payload_index: 0,
            },
        }
    } else {
        Inst {
            span, dest: Some(ValueId(2)),
            kind: InstKind::GetEnumTag { base: ValueId(1) },
        }
    });
    m.functions.insert("enum_result".into(), FunctionIR {
        name: "enum_result".into(),
        span,
        params: vec![Param {
            name: "input".into(), ty: Type::U32, span, value: ValueId(0),
        }],
        ret: Type::U32,
        entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: instructions, term: Terminator::Return(Some(ValueId(2))),
        }],
    });
    m
}

fn require_enum_refusal(module: ModuleIR) {
    validate_module(&module).expect("test fixture must be SSA-valid before C emission");
    match emit_module(&module, None) {
        Ok(output) => panic!(
            "C backend silently accepted unimplemented live Enum IR, emitted C:\n{}",
            output.module_c
        ),
        Err(err) => {
            let msg = err.message.to_ascii_lowercase();
            assert!(
                msg.contains("enum") && (msg.contains("unqualified") || msg.contains("unsupported")
                    || msg.contains("unimplemented")),
                "C backend must report unqualified Enum specifically, not a spurious IR failure: {err}",
            );
        }
    }
}

#[test]
fn live_enum_tag_projection_is_not_silent_c_emission() {
    require_enum_refusal(live_enum(false));
}

#[test]
fn live_enum_payload_projection_is_not_silent_c_emission() {
    require_enum_refusal(live_enum(true));
}

#[test]
fn scalar_function_remains_emittable() {
    let span: Span = (0, 0).into();
    let mut m = ModuleIR::new();
    m.functions.insert("scalar_copy".into(), FunctionIR {
        name: "scalar_copy".into(), span,
        params: vec![Param { name: "x".into(), ty: Type::U32, span, value: ValueId(0) }],
        ret: Type::U32, entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![], term: Terminator::Return(Some(ValueId(0))),
        }],
    });
    validate_module(&m).expect("valid control fixture");
    let emitted = emit_module(&m, None).expect("known scalar C emission must still work");
    assert!(emitted.module_c.contains("scalar_copy"));
}
