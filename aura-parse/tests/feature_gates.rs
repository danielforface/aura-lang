use aura_parse::{parse_source_with_config, ParseConfig};

#[test]
fn callsite_generics_are_gated_by_default() {
    let src = "cell main() ->:\n    val y = foo<Int>(1)\n";
    let cfg = ParseConfig::default();
    let err = parse_source_with_config(src, &cfg).expect_err("expected parse error");
    let msg = err.to_string();
    assert!(
        msg.contains("call-site generic arguments") && msg.contains("callsite-generics"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn callsite_generics_parse_when_feature_enabled() {
    let src = "cell main() ->:\n    val y = foo<Int>(1)\n";
    let mut cfg = ParseConfig::default();
    cfg.features.insert("callsite-generics".to_string());

    parse_source_with_config(src, &cfg).expect("expected parse success");
}
