//! Issue #45 test-first: never fabricate U32 IR for an unknown semantic
//! return type. Exercise genuine Aura source through lexer/parser/checker/lowerer.
use aura_core::{lower_program, Checker};
use aura_ir::Type as IrType;

fn checked_source(source: &str) -> aura_ast::Program {
    let ast = aura_parse::parse_source(source).expect("valid Aura source syntax");
    let mut checker = Checker::new();
    checker.check_program(&ast).expect(
        "the existing semantic checker permits unknown-call return inference; \
         an invalid source must not be misreported as this regression",
    );
    ast
}

fn require_typed_rejection(source: &str) {
    let ast = checked_source(source);
    match lower_program(&ast) {
        Ok(module) => panic!(
            "unresolved semantic result was silently lowered to executable IR: {:?}",
            module.functions
        ),
        Err(e) => {
            let message = e.message.to_ascii_lowercase();
            assert!(
                message.contains("unresolved") && message.contains("return"),
                "rejection must describe unresolved return type, not a parser error: {e}"
            );
        }
    }
}

#[test]
fn unknown_external_call_yield_must_not_fabricate_u32_return() {
    // Issue #47: unregistered free callees are now rejected in sema;
    // the older Issue #45 lowerer case remains for declared-but-unknown returns.
    let source = "cell mystery_bridge() ->:\n    yield unknown_external()\n";
    let ast = aura_parse::parse_source(source).expect("valid Aura source syntax");
    let mut checker = Checker::new();
    let error = checker
        .check_program(&ast)
        .expect_err("unregistered external call must fail during name resolution");
    assert!(
        error.message.to_ascii_lowercase().contains("unknown")
            && error.message.contains("unknown_external"),
        "unexpected semantic error: {error}"
    );
    assert_eq!(
        error.span.offset(),
        source.find("unknown_external()").expect("call in fixture")
    );
    let lowering_error = lower_program(&ast)
        .expect_err("unregistered external call must not produce executable IR");
    assert!(
        lowering_error.message.to_ascii_lowercase().contains("unknown"),
        "unexpected lowering error: {lowering_error}"
    );
}

#[test]
fn unresolved_forward_call_yield_must_not_be_implicitly_u32() {
    require_typed_rejection(
        "cell first() ->:\n    yield later()\n\ncell later() ->:\n    yield 17\n"
    );
}

#[test]
fn genuine_literal_u32_yield_remains_lowerable() {
    let ast = checked_source("cell known() ->:\n    yield 17\n");
    let ir = lower_program(&ast).expect("known U32 source must lower");
    assert_eq!(ir.functions["known"].ret, IrType::U32);
}

#[test]
fn genuine_string_yield_remains_string_typed() {
    let ast = checked_source("cell known() ->:\n    yield \"hello\"\n");
    let ir = lower_program(&ast).expect("known String source must lower");
    assert_eq!(ir.functions["known"].ret, IrType::String);
}

#[test]
fn void_cell_remains_unit_typed() {
    let ast = checked_source("cell known() ->:\n    assert true\n");
    let ir = lower_program(&ast).expect("Unit-returning source must lower");
    assert_eq!(ir.functions["known"].ret, IrType::Unit);
}
