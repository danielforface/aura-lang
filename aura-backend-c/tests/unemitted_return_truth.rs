//! Q03-007 / Issue #55: the C emitter must not silently fabricate a
//! return value for a live, structurally valid SSA producer it did not emit.
//! The oracle validator is structural, not a full typing proof.
use aura_backend_c::emit_module;
use aura_ast::Span;
use aura_ir::{
    validate_module, BasicBlock, BlockId, ExecutionHint, FunctionIR,
    Inst, InstKind, ModuleIR, Param, RValue, Terminator, Type, ValueId,
};

fn emitted_return_test(insts: Vec<Inst>, ret: Type, rv: ValueId) -> ModuleIR {
    let span: Span = (0, 0).into();
    let mut m = ModuleIR::new();
    m.functions.insert("return_value".into(), FunctionIR {
        name: "return_value".into(), span,
        params: vec![],
        ret, entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts, term: Terminator::Return(Some(rv)),
        }],
    });
    m
}

#[test]
fn live_capability_producer_cannot_be_silently_replaced_by_zero_tensor() {
    let span: Span = (0, 0).into();
    let module = emitted_return_test(
        vec![Inst {
            span,
            dest: Some(ValueId(0)),
            kind: InstKind::AllocCapability { name: "owned_capability".into() },
        }],
        Type::Opaque("Capability".into()),
        ValueId(0),
    );
    validate_module(&module).expect("SSA structure valid; typing is not proved by this oracle");
    match emit_module(&module, None) {
        Ok(artifact) => panic!(
            "C emitter silently returned fabricated value for live capability SSA: {}",
            artifact.module_c
        ),
        Err(err) => {
            let m = err.message.to_ascii_lowercase();
            assert!(m.contains("return") && (m.contains("unemitted") || m.contains("unavailable"))
                && m.contains("v0"),
                "failure must specifically identify unemitted return SSA, not unrelated IR issue: {err}");
        }
    }
}

#[test]
fn defined_local_scalar_value_still_returns_normally() {
    let span: Span = (0, 0).into();
    let module = emitted_return_test(
        vec![Inst {
            span, dest: Some(ValueId(0)),
            kind: InstKind::BindStrand { name: "value".into(), expr: RValue::ConstU32(17) },
        }],
        Type::U32,
        ValueId(0),
    );
    validate_module(&module).expect("scalar fixture validates");
    let out = emit_module(&module, None).expect("defined scalar may emit C");
    assert!(out.module_c.contains("return v0;"),
        "defined scalar result must be returned, not replaced by a constant Tensor: {}", out.module_c);
}

#[test]
fn direct_typed_parameter_return_still_emits_c() {
    let span: Span = (0, 0).into();
    let mut module = emitted_return_test(vec![], Type::U64, ValueId(3));
    let f = module.functions.get_mut("return_value").unwrap();
    f.params.push(Param {
        name: "input".into(), ty: Type::U64, span, value: ValueId(3),
    });
    validate_module(&module).expect("valid parameter SSA");
    let emitted = emit_module(&module, None).expect("typed input returns should work");
    assert!(emitted.module_c.contains("return input;"));
}
