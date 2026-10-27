//! Issue #47 supplementary test-first: Flow-injected calls must not
//! bypass the registered function signature map when the return is discarded.
use aura_core::{lower_program, Checker};

fn parse(source: &str) -> aura_ast::Program {
    aura_parse::parse_source(source).expect("genuinely valid Aura Flow syntax")
}
fn require_rejection(source: &str, callee: &str) {
    let program = parse(source);
    let mut checker = Checker::new();
    let error = checker
        .check_program(&program)
        .expect_err("unknown function in Flow must be rejected by semantic analysis");
    assert!(
        error.message.to_ascii_lowercase().contains("unknown")
            && error.message.contains(callee),
        "expected unknown Flow callee diagnostic, got: {error}"
    );
    let pos = source.find(callee).expect("callee present in Aura fixture");
    assert!(
        error.span.offset() <= pos
            && error.span.offset() + error.span.len() >= pos + callee.len(),
        "Flow callee diagnostic should cover the source symbol: {error}"
    );
    let lowering = lower_program(&program)
        .expect_err("unknown Flow callee must not be allowed to lower to IR");
    assert!(
        lowering.message.to_ascii_lowercase().contains("unknown"),
        "unexpected IR refusal: {lowering}"
    );
}
fn require_known(source: &str) {
    let ast = parse(source);
    let mut checker = Checker::new();
    checker.check_program(&ast).expect("registered injected-argument Flow must type-check");
}

#[test]
fn unregistered_flow_call_on_literal_is_rejected() {
    require_rejection(
        "cell main() ->:\n    1 -> unknown_flow()\n",
        "unknown_flow",
    );
}

#[test]
fn unregistered_flow_call_on_named_local_is_rejected() {
    require_rejection(
        "cell main() ->:\n    val n: u32 = 1\n    n -> missing_flow()\n",
        "missing_flow",
    );
}

#[test]
fn unregistered_flow_call_on_imported_namespace_is_rejected() {
    require_rejection(
        "import aura::io\n\ncell main() ->:\n    \"hello\" -> io::ghost_flow()\n",
        "ghost_flow",
    );
}

#[test]
fn declared_cell_with_injected_flow_argument_remains_legal() {
    require_known(
        "cell sink(n: u32) ->:\n    yield n\n\ncell main() ->:\n    1 -> sink()\n",
    );
}

#[test]
fn registered_builtin_with_injected_flow_argument_remains_legal() {
    require_known(
        "import aura::io\n\ncell main() ->:\n    \"hello\" -> io::println()\n",
    );
}
