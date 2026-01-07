use aura_ast::{Stmt, TypeArg};
use aura_parse::{parse_source_with_config, ParseConfig};

#[test]
fn shape_const_exprs_are_gated_by_default() {
    let src = "type T = Tensor<u32, [2 + 3]>\n";
    let cfg = ParseConfig::default();
    let err = parse_source_with_config(src, &cfg).expect_err("expected parse error");
    let msg = err.to_string();
    assert!(
        msg.contains("shape dimensions") && msg.contains("ctfe"),
        "unexpected error message: {msg}"
    );
}

#[test]
fn shape_const_exprs_eval_with_ctfe_feature() {
    let src = "type T = Tensor<u32, [2 + 3*4, (2 + 3) * 4]>\n";
    let mut cfg = ParseConfig::default();
    cfg.features.insert("ctfe".to_string());

    let program = parse_source_with_config(src, &cfg).expect("expected parse success");
    let Stmt::TypeAlias(ta) = &program.stmts[0] else {
        panic!("expected first stmt to be TypeAlias");
    };

    assert_eq!(ta.target.name.node, "Tensor");
    let TypeArg::Shape(dims) = &ta.target.args[1] else {
        panic!("expected second type arg to be Shape");
    };
    assert_eq!(dims.as_slice(), &[14, 20]);
}
