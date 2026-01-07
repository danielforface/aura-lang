use aura_core::Checker;

/// Test basic ownership state tracking with OwnershipState enum.
/// Tests that non-copy types are tracked and prevented from double-use.

#[test]
fn model_use_after_move_rejected() {
    let src = r#"
import aura::ai
import aura::tensor

cell main() ->:
  val model: Model = ai::load_model("test.onnx")
  val data1: Tensor = tensor::new(10)
  val data2: Tensor = tensor::new(5)
  
  # First inference: model is moved here
  val result1: Tensor = ai::infer(model, data1)
  
  # ERROR: model was consumed by previous ai::infer call
  val result2: Tensor = ai::infer(model, data2)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    
    assert!(
        err.message.contains("used after move"),
        "expected 'used after move' error, got: {}",
        err.message
    );
}

#[test]
fn model_single_use_allowed() {
    let src = r#"
import aura::ai
import aura::tensor

cell main() ->:
  val model: Model = ai::load_model("test.onnx")
  val data: Tensor = tensor::new(10)
  val result: Tensor = ai::infer(model, data)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    // Should compile without error
    Checker::new().check_program(&program).expect("sema should pass");
}

#[test]
fn copy_types_allow_reuse() {
    let src = r#"
cell main() ->:
  val x: u32 = 5
  val y: u32 = x
  val z: u32 = x
  val w: u32 = x + y + z
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    // u32 is Copy type, should allow unrestricted reuse
    Checker::new().check_program(&program).expect("sema should pass");
}

#[test]
fn tensor_multiple_independent_uses() {
    let src = r#"
import aura::tensor

cell main() ->:
  val t1: Tensor = tensor::new(10)
  val t2: Tensor = tensor::new(5)
  
  val len1: u32 = tensor::len(t1)
  val len2: u32 = tensor::len(t2)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    // Different tensors should be independent
    Checker::new().check_program(&program).expect("sema should pass");
}

#[test]
fn tensor_mutable_borrow_allows_reuse() {
    let src = r#"
import aura::tensor

cell main() ->:
  val mut t: Tensor = tensor::new(100)
  # tensor.set is a mutable borrow, not a move
  tensor::set(t, 0, 42)
  tensor::set(t, 1, 43)
  # Can still use t after borrows
  val len: u32 = tensor::len(t)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    // Mutable borrows should allow continued use
    Checker::new().check_program(&program).expect("sema should pass");
}

#[test]
fn tensor_len_read_only_borrow_allows_reuse() {
    let src = r#"
import aura::tensor

cell main() ->:
  val t: Tensor = tensor::new(100)
  # tensor.len is a read-only borrow
  val len1: u32 = tensor::len(t)
  val len2: u32 = tensor::len(t)
  # Can still use t after borrows
  val len3: u32 = tensor::len(t)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    // Read-only borrows should allow continued use
    Checker::new().check_program(&program).expect("sema should pass");
}

#[test]
fn function_parameter_move() {
    let src = r#"
import aura::ai

cell process_model(m: Model) ->:
  val len: u32 = 0

cell main() ->:
  val model: Model = ai::load_model("test.onnx")
  # Passing to function consumes model
  process_model(model)
  
  # ERROR: model was moved into function
  val x: Model = model
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    
    assert!(
        err.message.contains("used after move"),
        "expected 'used after move' error, got: {}",
        err.message
    );
}

#[test]
fn immutable_tensor_move_on_consume() {
    let src = r#"
import aura::tensor

cell consume_tensor(t: Tensor) ->:
  val len: u32 = tensor::len(t)

cell main() ->:
  val t: Tensor = tensor::new(10)
  # Passing tensor to function consumes it
  consume_tensor(t)
  
  # ERROR: tensor was moved into function
  val len: u32 = tensor::len(t)
"#;

    let program = aura_parse::parse_source(src).expect("parse");
    let err = Checker::new().check_program(&program).expect_err("expected sema error");
    
    assert!(
        err.message.contains("used after move"),
        "expected 'used after move' error, got: {}",
        err.message
    );
}
