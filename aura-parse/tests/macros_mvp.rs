use aura_parse::{parse_source_with_config, ParseConfig};

#[test]
fn macros_are_gated_by_default() {
    let src = "macro twice(x):\n    val y = x + x\n\ncell main() ->:\n    twice!(41)\n";
    let cfg = ParseConfig::default();
    let err = parse_source_with_config(src, &cfg).expect_err("expected parse error");
    let msg = err.to_string();
    assert!(
        msg.contains("unstable feature") && msg.contains("macros"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn macro_call_expands_into_statements() {
    let src = "macro twice(x):\n    val y = x + x\n\ncell main() ->:\n    twice!(41)\n";
    let mut cfg = ParseConfig::default();
    cfg.features.insert("macros".to_string());

    let program = parse_source_with_config(src, &cfg).expect("parse ok");

    // Macro def + call should be removed after expansion.
    let text = format!("{program:?}");
    assert!(!text.contains("MacroDef"), "macro def should be removed");
    assert!(!text.contains("MacroCall"), "macro call should be removed");

    // And the expanded 'val y = 41 + 41' should exist.
    assert!(text.contains("StrandDef"), "expected a val binding in expanded program");
    assert!(text.contains("IntLit(41"), "expected macro arg to be substituted");
}
