use aura_core::Checker;

#[test]
fn non_copy_identifier_is_moved_when_passed_to_user_cell() {
    let src = r#"
import aura::tensor

cell consume(x: Tensor) ->:
  # no-op
  val z: u32 = 0

cell main() ->:
  val t: Tensor = tensor::new(1)
  consume(t)
  # After passing to consume(t), `t` is moved.
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
fn tensor_len_borrows_readonly_does_not_move() {
    let src = r#"
import aura::tensor

cell main() ->:
  val t: Tensor = tensor::new(1)
  val a: u32 = tensor::len(t)
  val b: u32 = tensor::len(t)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    Checker::new().check_program(&program).expect("sema");
}

#[test]
fn tensor_set_requires_mutable_binding() {
    let src = r#"
import aura::tensor

cell main() ->:
  val t: Tensor = tensor::new(1)
  tensor::set(t, 0, 1)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    assert!(
        err.message.contains("immutable") && err.message.contains("tensor"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn tensor_set_allows_mutable_binding_and_does_not_move() {
    let src = r#"
import aura::tensor

cell main() ->:
  val mut t: Tensor = tensor::new(1)
  tensor::set(t, 0, 1)
  val n: u32 = tensor::len(t)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    Checker::new().check_program(&program).expect("sema");
}
