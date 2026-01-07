use aura_core::Checker;

#[test]
fn enum_ctor_moves_non_copy_identifier_args() {
    let src = r#"
import aura::tensor

type Opt = enum { None, Some(v: Tensor) }

cell main() ->:
  val t: Tensor = tensor::new(1)
  val o: Opt = Opt::Some(t)
  # After constructing Some(t), `t` is moved.
  val n: u32 = tensor::len(t)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    assert!(
        err.message.contains("use after move"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn socket_named_nominal_is_treated_as_linear_and_cannot_be_used_after_move() {
    let src = r#"
type Socket = record { x: u32 }

cell main() ->:
  val s: Socket = Socket { x: 1 }
  val t: Socket = s
  # After binding t = s, `s` is moved.
  val y: u32 = s.x
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    assert!(
        err.message.contains("use after move"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn region_named_nominal_is_treated_as_linear_and_cannot_be_used_after_move() {
    let src = r#"
type Region = record { id: u32 }

cell main() ->:
  val r: Region = Region { id: 0 }
  val q: Region = r
  # After binding q = r, `r` is moved.
  val y: u32 = r.id
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    assert!(
        err.message.contains("use after move"),
        "unexpected error: {}",
        err.message
    );
}
