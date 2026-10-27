//! Q03-001: Record Phi must be nominally typed; no structural coincidence,
 //! scalar fallback or silent Tensor conversion across CFG joins.
use aura_ast::Span;
use aura_backend_c::emit_module;
use aura_ir::{
    validate_module, BasicBlock, BlockId, ExecutionHint, FunctionIR, Inst,
    InstKind, ModuleIR, Param, RValue, Terminator, Type, ValueId,
};
fn module(mixed_scalar: bool) -> ModuleIR {
    let span: Span = (0, 0).into();
    let mut m = ModuleIR::new();
    // Identical *shapes* deliberately must NOT make distinct nominal types
    // interchangeable at a Phi join.
    m.record_layouts.insert("Point".into(), vec![("x".into(), Type::U32)]);
    m.record_layouts.insert("Other".into(), vec![("x".into(), Type::U32)]);
    let mk = |id: u32, kind: InstKind| Inst { span, dest: Some(ValueId(id)), kind };
    m.functions.insert("choice".into(), FunctionIR {
        name: "choice".into(), span,
        params: vec![
            Param { name: "flag".into(), ty: Type::Bool, span, value: ValueId(0) },
            Param { name: "x".into(), ty: Type::U32, span, value: ValueId(1) },
        ],
        ret: Type::U32, entry: BlockId(0),
        blocks: vec![
            BasicBlock {
                id: BlockId(0), span, hint: ExecutionHint::Sequential,
                insts: vec![], term: Terminator::CondBr {
                    cond: ValueId(0), then_bb: BlockId(1), else_bb: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1), span, hint: ExecutionHint::Sequential,
                insts: vec![mk(2, InstKind::ConstructRecord {
                    record_name: "Point".into(),
                    fields: vec![("x".into(), ValueId(1))],
                })],
                term: Terminator::Br(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2), span, hint: ExecutionHint::Sequential,
                insts: vec![
                    if mixed_scalar {
                        mk(3, InstKind::BindStrand {
                            name: "not_a_record".into(),
                            expr: RValue::Local(ValueId(1)),
                        })
                    } else {
                        mk(3, InstKind::ConstructRecord {
                            record_name: "Other".into(),
                            fields: vec![("x".into(), ValueId(1))],
                        })
                    },
                ], term: Terminator::Br(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3), span, hint: ExecutionHint::Sequential,
                insts: vec![
                    mk(4, InstKind::Phi {
                        incomings: vec![
                            (BlockId(1), ValueId(2)), (BlockId(2), ValueId(3))
                        ],
                    }),
                    mk(5, InstKind::GetField { base: ValueId(4), field_name: "x".into() }),
                ],
                term: Terminator::Return(Some(ValueId(5))),
            },
        ],
    });
    m
}
fn must_refuse(mixed_scalar: bool, reason: &str) {
    let ir = module(mixed_scalar);
    validate_module(&ir).expect("lightweight valid SSA CFG with both incoming defs");
    match emit_module(&ir, None) {
        Ok(c) => panic!("unqualified Record Phi wrongly emitted C artifact: {}", c.module_c),
        Err(e) => assert!(
            e.message.contains("Record Phi") && e.message.contains(reason),
            "expected Record Phi {reason} diagnostic; got {e}"
        ),
    }
}
#[test]
fn record_phi_rejects_distinct_nominal_types_even_with_identical_fields() {
    must_refuse(false, "mixes nominal");
}
#[test]
fn record_phi_rejects_mixed_record_and_scalar_without_tensor_fallback() {
    must_refuse(true, "missing nominal type");
}
