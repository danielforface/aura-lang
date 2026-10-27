//! Issue #47, TEST-FIRST: a discarded call to an unregistered function must
//! fail at the semantic boundary, not become an arbitrary Aura IR Call.
//! This suite intentionally precedes any production source change.
use aura_core::{lower_program, Checker};
use aura_ir::InstKind;

fn parsed(source: &str) -> aura_ast::Program {
    aura_parse::parse_source(source).expect("fixture must be valid Aura syntax")
}

fn reject_unknown_callee(source: &str, spelling: &str, name_fragment: &str) {
    let program = parsed(source);
    let mut checker = Checker::new();
    let error = checker
        .check_program(&program)
        .expect_err("an undeclared discarded call must fail semantic checking");
    let detail = error.message.to_ascii_lowercase();
    assert!(
        detail.contains("unknown") && detail.contains(&name_fragment.to_ascii_lowercase()),
        "unexpected semantic error (must identify unknown callee): {error}"
    );
    let expected = source.find(spelling).expect("callee spelling in fixture");
    assert_eq!(
        error.span.offset(),
        expected,
        "semantic diagnostic must anchor the source call, not a parent statement"
    );
    let lower_error = lower_program(&program)
        .expect_err("lower_program must also fail closed on the unknown call");
    assert!(
        lower_error.message.to_ascii_lowercase().contains("unknown"),
        "lowering rejected for an unrelated reason: {lower_error}"
    );
}

fn accept_declared_call(source: &str, ir_callee: &str) {
    let program = parsed(source);
    let mut checker = Checker::new();
    checker.check_program(&program).expect("declared call must type check");
    let module = lower_program(&program).expect("declared call must lower");
    let main = module.functions.get("main").expect("main must exist");
    assert!(
        main.blocks.iter().flat_map(|b| b.insts.iter()).any(|inst|
            matches!(&inst.kind, InstKind::Call { callee, .. } if callee == ir_callee)),
        "declared call must survive in IR with its real symbol: {ir_callee}"
    );
}

#[test]
fn discarded_call_to_undeclared_function_is_not_legal() {
    reject_unknown_callee(
        "cell main() ->:\n    never_declared()\n",
        "never_declared()",
        "never_declared",
    );
}

#[test]
fn discarded_undeclared_call_inside_reachable_if_is_not_legal() {
    reject_unknown_callee(
        "cell main() ->:\n    if true:\n        missing_nested()\n",
        "missing_nested()",
        "missing_nested",
    );
}

#[test]
fn unknown_name_in_imported_builtin_namespace_is_not_an_extension_hook() {
    reject_unknown_callee(
        "import aura::io\n\ncell main() ->:\n    io::ghost_unregistered()\n",
        "io::ghost_unregistered()",
        "ghost_unregistered",
    );
}

#[test]
fn locally_declared_cell_with_discarded_return_stays_valid() {
    accept_declared_call(
        "cell helper() ->:\n    yield 42\n\ncell main() ->:\n    helper()\n",
        "helper",
    );
}

#[test]
fn registered_io_builtin_stays_valid() {
    accept_declared_call(
        "import aura::io\n\ncell main() ->:\n    io::println(\"hello\")\n",
        "io.println",
    );
}

#[test]
fn explicit_trusted_extern_signature_stays_valid() {
    accept_declared_call(
        "trusted extern cell native_ping(x: u32) -> u32\n\ncell main() ->:\n    native_ping(1)\n",
        "native_ping",
    );
}
