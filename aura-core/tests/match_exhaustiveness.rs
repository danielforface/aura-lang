use aura_core::Checker;

#[test]
fn match_requires_trailing_wildcard_arm() {
    let src = "val x = 1\nmatch x:\n    1:\n        val a = 0\n";
    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    assert!(
        err.message.contains("non-exhaustive match"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn match_wildcard_must_be_last() {
    let src = "val x = 1\nmatch x:\n    _:\n        val a = 0\n    1:\n        val a = 1\n";
    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    assert!(
        err.message.contains("wildcard") && err.message.contains("last"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn match_rejects_duplicate_literals() {
    let src = "val x = 1\nmatch x:\n    1:\n        val a = 0\n    1:\n        val a = 1\n    _:\n        val a = 2\n";
    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    assert!(
        err.message.contains("duplicate"),
        "unexpected error: {}",
        err.message
    );
}
