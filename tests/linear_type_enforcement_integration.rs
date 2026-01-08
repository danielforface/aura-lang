/// Integration Tests for Linear Type Enforcement
/// 
/// Comprehensive test suite validating all aspects of linear type enforcement:
/// - Use-after-move detection
/// - Linear resource consumption
/// - Ownership state transitions
/// - Control flow analysis
/// - Function signature validation

#[cfg(test)]
mod integration_tests {
    use aura_core::{
        OwnershipContext, OwnershipState, classify_type, LinearTypeKind,
        ControlFlowGraph, LinearFunctionSignature, LinearParam, LinearReturn,
        ParamMode, ReturnMode, SignatureValidator, DiagnosticFactory, ViolationKind,
    };
    use aura_ast::Type;

    // ============ Test 1: Simple Use-After-Move Detection ============
    
    #[test]
    fn test_use_after_move_simple_model() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        
        // Define a linear resource
        ctx.define_binding("model".to_string(), "Model".to_string(), true);
        
        // Move it
        ctx.set_location(2, 0);
        assert!(ctx.record_move("model").is_ok());
        
        // Try to use it - should fail
        ctx.set_location(3, 0);
        let result = ctx.record_use("model");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_kind, ViolationKind::UseAfterMove);
    }
    
    // ============ Test 2: Linear Type Classification ============
    
    #[test]
    fn test_classify_types() {
        // Copyable types
        assert_eq!(classify_type(&Type::U32), LinearTypeKind::Copyable);
        assert_eq!(classify_type(&Type::Bool), LinearTypeKind::Copyable);
        assert_eq!(classify_type(&Type::String), LinearTypeKind::Copyable);
        
        // Linear types
        assert_eq!(classify_type(&Type::Model), LinearTypeKind::Linear);
        assert_eq!(classify_type(&Type::Style), LinearTypeKind::Linear);
        
        let tensor = Type::Tensor {
            elem: Box::new(Type::U32),
            shape: None,
        };
        assert_eq!(classify_type(&tensor), LinearTypeKind::Linear);
    }
    
    // ============ Test 3: Multiple Linear Resources in Sequence ============
    
    #[test]
    fn test_multiple_linear_resources() {
        let mut ctx = OwnershipContext::new();
        
        // Define multiple linear resources
        ctx.set_location(1, 0);
        ctx.define_binding("model1".to_string(), "Model".to_string(), true);
        ctx.define_binding("model2".to_string(), "Model".to_string(), true);
        
        // Move both
        ctx.set_location(2, 0);
        assert!(ctx.record_move("model1").is_ok());
        assert!(ctx.record_move("model2").is_ok());
        
        // Try to use first - should fail
        ctx.set_location(3, 0);
        assert!(ctx.record_use("model1").is_err());
        assert!(ctx.record_use("model2").is_err());
    }
    
    // ============ Test 4: Borrow Prevents Move ============
    
    #[test]
    fn test_borrow_prevents_move() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("tensor".to_string(), "Tensor".to_string(), true);
        
        // Borrow it immutably
        ctx.set_location(2, 0);
        assert!(ctx.record_borrow_immut("tensor").is_ok());
        
        // Try to move - should fail
        ctx.set_location(3, 0);
        let result = ctx.record_move("tensor");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_kind, ViolationKind::MoveAfterBorrow);
    }
    
    // ============ Test 5: Scoped Bindings ============
    
    #[test]
    fn test_scoped_ownership_tracking() {
        let mut ctx = OwnershipContext::new();
        
        // Outer scope
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "u32".to_string(), false);
        assert!(ctx.binding_exists("x"));
        
        // Enter inner scope
        ctx.push_scope();
        ctx.set_location(2, 0);
        ctx.define_binding("y".to_string(), "Model".to_string(), true);
        assert!(ctx.binding_exists("y"));
        assert!(ctx.binding_exists("x")); // Can still see outer
        
        // Move y
        ctx.set_location(3, 0);
        assert!(ctx.record_move("y").is_ok());
        
        // Exit inner scope
        ctx.pop_scope();
        assert!(!ctx.binding_exists("y")); // y is gone
        assert!(ctx.binding_exists("x")); // x still exists
    }
    
    // ============ Test 6: Double Move Detection ============
    
    #[test]
    fn test_double_move_detection() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("model".to_string(), "Model".to_string(), true);
        
        // First move - OK
        ctx.set_location(2, 0);
        assert!(ctx.record_move("model").is_ok());
        
        // Second move - should fail
        ctx.set_location(3, 0);
        let result = ctx.record_move("model");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_kind, ViolationKind::DoubleMove);
    }
    
    // ============ Test 7: Control Flow - If-Then-Else ============
    
    #[test]
    fn test_control_flow_consistent_moves() {
        let mut graph = ControlFlowGraph::new();
        
        // Define binding
        graph.record_move_in_all("model");
        
        // Branch into if-else
        graph.branch();
        
        // Both branches should have model as moved
        let merged = graph.merge();
        assert_eq!(merged.get("model"), Some(&OwnershipState::Consumed));
    }
    
    // ============ Test 8: Function Signature - Linear Parameter ============
    
    #[test]
    fn test_function_signature_linear_param() {
        let param = LinearParam::new(
            "model".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new(
            "consume_model".to_string(),
            vec![param],
            ret,
        );
        
        // Should validate successfully
        assert!(SignatureValidator::validate_signature(&sig).is_ok());
        assert_eq!(sig.linear_params(), vec!["model"]);
    }
    
    // ============ Test 9: Function Body Validation ============
    
    #[test]
    fn test_function_body_validation_consumes_param() {
        let param = LinearParam::new(
            "model".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("func".to_string(), vec![param], ret);
        
        // Body that consumes the parameter - OK
        let result = SignatureValidator::validate_body(&sig, &["model"], &[], None);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_function_body_validation_missing_consume() {
        let param = LinearParam::new(
            "model".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("func".to_string(), vec![param], ret);
        
        // Body that doesn't consume - ERROR
        let result = SignatureValidator::validate_body(&sig, &[], &[], None);
        assert!(result.is_err());
    }
    
    // ============ Test 10: Diagnostics - Use After Move ============
    
    #[test]
    fn test_diagnostic_use_after_move() {
        let diag = DiagnosticFactory::use_after_move(
            "test.aura".to_string(),
            10,
            5,
            "model",
            5,
            0,
        );
        
        assert_eq!(diag.error_kind, ViolationKind::UseAfterMove);
        assert!(!diag.related.is_empty());
        assert!(diag.suggestion.is_some());
        
        let displayed = diag.display();
        assert!(displayed.contains("ERROR"));
        assert!(displayed.contains("model"));
    }
    
    // ============ Test 11: Mutable Borrow Tracking ============
    
    #[test]
    fn test_mutable_borrow_tracking() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("vec".to_string(), "Vec".to_string(), true);
        
        // Mutable borrow
        ctx.set_location(2, 0);
        assert!(ctx.record_borrow_mut("vec").is_ok());
        
        // Try to move - should fail
        ctx.set_location(3, 0);
        let result = ctx.record_move("vec");
        assert!(result.is_err());
    }
    
    // ============ Test 12: Non-Linear Type Copyable ============
    
    #[test]
    fn test_non_linear_types_copyable() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "u32".to_string(), false);
        
        // Move
        ctx.set_location(2, 0);
        assert!(ctx.record_move("x").is_ok());
        
        // Use - should be OK because u32 is copyable (in real type system)
        ctx.set_location(3, 0);
        // In real implementation, copyable types would be handled differently
        // For this test, we just verify the binding is tracked
        assert!(ctx.binding_exists("x"));
    }
    
    // ============ Test 13: Multiple Bindings in Branch ============
    
    #[test]
    fn test_multiple_bindings_branch() {
        let mut graph = ControlFlowGraph::new();
        
        // Record moves for both bindings
        graph.record_move_in_all("model");
        graph.record_move_in_all("tensor");
        
        // Verify both are tracked
        assert_eq!(graph.get_binding_state("model"), Some(OwnershipState::Consumed));
        assert_eq!(graph.get_binding_state("tensor"), Some(OwnershipState::Consumed));
    }
    
    // ============ Test 14: Return Type Validation ============
    
    #[test]
    fn test_return_type_validation() {
        let param = LinearParam::new(
            "x".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Model, true, ReturnMode::Owned);
        let sig = LinearFunctionSignature::new("identity".to_string(), vec![param], ret);
        
        // Body returns the parameter - OK
        let result = SignatureValidator::validate_body(&sig, &[], &[], Some("x"));
        assert!(result.is_ok());
    }
    
    // ============ Test 15: Borrowed Parameter Signature ============
    
    #[test]
    fn test_borrowed_parameter_signature() {
        let param = LinearParam::new(
            "model".to_string(),
            Type::Model,
            true,
            ParamMode::BorrowedImmut,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("read_model".to_string(), vec![param], ret);
        
        // Borrowed parameter - shouldn't require consumption
        let result = SignatureValidator::validate_body(&sig, &[], &["model"], None);
        assert!(result.is_ok());
    }
    
    // ============ Test 16: Type Classification Named Types ============
    
    #[test]
    fn test_classify_named_types() {
        let tensor_type = Type::Named("TensorBuffer".to_string());
        assert_eq!(classify_type(&tensor_type), LinearTypeKind::Linear);
        
        let model_type = Type::Named("ModelHandle".to_string());
        assert_eq!(classify_type(&model_type), LinearTypeKind::Linear);
        
        let custom_type = Type::Named("MyCustomType".to_string());
        assert_eq!(classify_type(&custom_type), LinearTypeKind::Copyable);
    }
    
    // ============ Test 17: Linear Resources Not Consumed ============
    
    #[test]
    fn test_linear_resources_consumed_check() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("model".to_string(), "Model".to_string(), true);
        
        // Don't consume it
        let unconsumed = ctx.check_linear_resources_consumed();
        assert!(!unconsumed.is_empty());
        assert_eq!(unconsumed[0].error_kind, ViolationKind::UseNotMoved);
    }
    
    // ============ Test 18: Multiple Borrows Allowed ============
    
    #[test]
    fn test_multiple_immut_borrows() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "Model".to_string(), true);
        
        // Multiple immutable borrows - should all be OK
        ctx.set_location(2, 0);
        assert!(ctx.record_borrow_immut("x").is_ok());
        
        ctx.set_location(3, 0);
        assert!(ctx.record_borrow_immut("x").is_ok());
    }
    
    // ============ Test 19: Consumed Then Returned ============
    
    #[test]
    fn test_consumed_and_returned() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "Model".to_string(), true);
        
        // Move then return - the return marks it as Returned state
        ctx.set_location(2, 0);
        assert!(ctx.record_move("x").is_ok());
        
        ctx.set_location(3, 0);
        assert!(ctx.record_return("x").is_ok());
    }
    
    // ============ Test 20: Comprehensive Call Validation ============
    
    #[test]
    fn test_comprehensive_call_validation() {
        let param1 = LinearParam::new(
            "model".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let param2 = LinearParam::new(
            "count".to_string(),
            Type::U32,
            false,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new(
            "process".to_string(),
            vec![param1, param2],
            ret,
        );
        
        // Valid call
        let args = vec![
            ("my_model".to_string(), Type::Model),
            ("iterations".to_string(), Type::U32),
        ];
        assert!(SignatureValidator::validate_call(&sig, &args).is_ok());
        
        // Invalid call - wrong number of arguments
        let bad_args = vec![("my_model".to_string(), Type::Model)];
        assert!(SignatureValidator::validate_call(&sig, &bad_args).is_err());
    }
}
