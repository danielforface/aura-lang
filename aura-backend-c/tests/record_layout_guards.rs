//! AURA-Q03-001 boundary tests: current local scalar Record native subset
//! must not silently reinterpret mismatched, absent, nested or ABI layouts.
use aura_backend_c::emit_module;
use aura_ast::Span;
use aura_ir::{
    validate_module, BasicBlock, BlockId, ExecutionHint, FunctionIR, Inst,
    InstKind, ModuleIR, Param, Terminator, Type, ValueId,
};

fn ir(field_layout: Vec<(String, Type)>, provided: Vec<(String, ValueId)>, ret: Type) -> ModuleIR {
    let span: Span = (0, 0).into();
    let mut m = ModuleIR::new();
    m.record_layouts.insert("Point".into(), field_layout);
    m.functions.insert("get_x".into(), FunctionIR {
        name: "get_x".into(), span,
        params: vec![
            Param { name: "x".into(), ty: Type::U32, span, value: ValueId(0) },
            Param { name: "y".into(), ty: Type::U32, span, value: ValueId(1) },
        ],
        ret, entry: BlockId(0),
        blocks: vec![BasicBlock {
            id: BlockId(0), span, hint: ExecutionHint::Sequential,
            insts: vec![
                Inst {
                    span, dest: Some(ValueId(2)),
                    kind: InstKind::ConstructRecord { record_name: "Point".into(), fields: provided },
                },
                Inst {
                    span, dest: Some(ValueId(3)),
                    kind: InstKind::GetField { base: ValueId(2), field_name: "x".into() },
                },
            ],
            term: Terminator::Return(Some(ValueId(3))),
        }],
    });
    m
}
fn error(m: ModuleIR, term: &str) {
    validate_module(&m).expect("valid lightweight SSA fixture");
    match emit_module(&m, None) {
        Ok(o) => panic!("unsafe Record program claimed successful C emission: {}", o.module_c),
        Err(e) => assert!(e.message.contains("Record") && e.message.contains(term),
            "missing specific Record diagnostic {term:?}, actual: {e}"),
    }
}
#[test]
fn record_field_width_mismatch_cannot_be_implicitly_cast() {
    error(ir(
        vec![("x".into(), Type::U64), ("y".into(), Type::U32)],
        vec![("x".into(), ValueId(0)), ("y".into(), ValueId(1))],
        Type::U32,
    ), "type mismatch");
}
#[test]
fn record_missing_field_cannot_be_zero_filled_by_c() {
    error(ir(
        vec![("x".into(), Type::U32), ("y".into(), Type::U32)],
        vec![("x".into(), ValueId(0))],
        Type::U32,
    ), "field count mismatch");
}
#[test]
fn record_nested_opaque_field_is_not_misrepresented_as_tensor() {
    error(ir(
        vec![("x".into(), Type::Opaque("Nested".into())), ("y".into(), Type::U32)],
        vec![("x".into(), ValueId(0)), ("y".into(), ValueId(1))],
        Type::U32,
    ), "unsupported");
}
#[test]
fn record_function_return_abi_is_not_misrepresented_as_tensor() {
    error(ir(
        vec![("x".into(), Type::U32), ("y".into(), Type::U32)],
        vec![("x".into(), ValueId(0)), ("y".into(), ValueId(1))],
        Type::Record {
            name: "Point".into(),
            fields: vec![("x".into(), Type::U32), ("y".into(), Type::U32)],
        },
    ), "function ABI");
}
#[test]
fn record_opaque_function_parameter_abi_is_explicitly_unqualified() {
    let mut m = ir(
        vec![("x".into(), Type::U32), ("y".into(), Type::U32)],
        vec![("x".into(), ValueId(0)), ("y".into(), ValueId(1))],
        Type::U32,
    );
    m.functions.get_mut("get_x").unwrap().params[0].ty = Type::Opaque("Point".into());
    error(m, "function ABI");
}
