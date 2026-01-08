/// Integration Tests for Capability Enforcement
/// 
/// Comprehensive test suite validating all aspects of capability enforcement:
/// - Lifecycle management (Fresh → InUse → Consumed)
/// - Resource leak detection
/// - Concurrent access violations
/// - Type-based capability inference
/// - Diagnostics generation

#[cfg(test)]
mod integration_tests {
    use aura_core::{
        CapabilityKind, CapabilityState, CapabilityViolation, CapabilityContext,
        CapabilityValidator, CapabilityDiagnosticFactory,
        types::Type,
    };

    /// Test 1: Basic socket lifecycle
    #[test]
    fn test_socket_lifecycle_basic() {
        let mut ctx = CapabilityContext::new();
        ctx.set_location(1, 0);
        ctx.define_capability("socket".to_string(), CapabilityKind::Socket).unwrap();
        
        assert_eq!(ctx.get_state("socket"), Some(CapabilityState::Fresh));
        
        ctx.set_location(2, 0);
        ctx.use_capability("socket").unwrap();
        assert_eq!(ctx.get_state("socket"), Some(CapabilityState::InUse));
        
        ctx.set_location(3, 0);
        ctx.consume_capability("socket").unwrap();
        assert_eq!(ctx.get_state("socket"), Some(CapabilityState::Consumed));
    }

    /// Test 2: Socket use after consumption error
    #[test]
    fn test_socket_use_after_consumption() {
        let mut ctx = CapabilityContext::new();
        ctx.set_location(1, 0);
        ctx.define_capability("socket".to_string(), CapabilityKind::Socket).unwrap();
        
        ctx.set_location(2, 0);
        ctx.consume_capability("socket").unwrap();
        
        ctx.set_location(3, 0);
        let result = ctx.use_capability("socket");
        assert!(result.is_err());
        
        if let Err(violation) = result {
            match violation {
                CapabilityViolation::UseAfterConsumption { .. } => {
                    // Expected
                }
                _ => panic!("Wrong violation type"),
            }
        }
    }

    /// Test 3: Tensor capability detection
    #[test]
    fn test_tensor_capability_type() {
        let tensor_type = Type::Tensor {
            elem: Box::new(Type::U32),
            shape: None,
        };
        assert!(CapabilityValidator::is_capability_type(&tensor_type));
        
        let kind = CapabilityValidator::infer_capability_kind(&tensor_type);
        assert_eq!(kind, Some(CapabilityKind::Tensor));
    }

    /// Test 4: Multiple tensor resources
    #[test]
    fn test_multiple_tensors() {
        let mut ctx = CapabilityContext::new();
        
        ctx.set_location(1, 0);
        ctx.define_capability("t1".to_string(), CapabilityKind::Tensor).unwrap();
        ctx.define_capability("t2".to_string(), CapabilityKind::Tensor).unwrap();
        
        ctx.set_location(2, 0);
        ctx.use_capability("t1").unwrap();
        ctx.use_capability("t2").unwrap();
        
        ctx.set_location(3, 0);
        ctx.consume_capability("t1").unwrap();
        ctx.consume_capability("t2").unwrap();
        
        assert_eq!(ctx.get_state("t1"), Some(CapabilityState::Consumed));
        assert_eq!(ctx.get_state("t2"), Some(CapabilityState::Consumed));
    }

    /// Test 5: Resource leak detection at scope exit
    #[test]
    fn test_resource_leak_detection() {
        let mut ctx = CapabilityContext::new();
        
        ctx.set_location(1, 0);
        ctx.define_capability("socket".to_string(), CapabilityKind::Socket).unwrap();
        ctx.use_capability("socket").unwrap();
        
        ctx.enter_scope();
        
        // Don't consume socket
        let result = ctx.exit_scope();
        
        assert!(result.is_err());
        if let Err(violations) = result {
            // Should have resource leak violation
            assert!(!violations.is_empty());
        }
    }

    /// Test 6: Scope exit without leaks
    #[test]
    fn test_scope_exit_clean() {
        let mut ctx = CapabilityContext::new();
        
        ctx.set_location(1, 0);
        ctx.define_capability("socket".to_string(), CapabilityKind::Socket).unwrap();
        
        ctx.enter_scope();
        
        ctx.set_location(2, 0);
        ctx.use_capability("socket").unwrap();
        ctx.consume_capability("socket").unwrap();
        
        let result = ctx.exit_scope();
        assert!(result.is_ok());
    }

    /// Test 7: Region capability tracking
    #[test]
    fn test_region_capability() {
        let mut validator = CapabilityValidator::new(true);
        validator.set_location(1, 0);
        
        let region_type = Type::Named("RegionAlloc".to_string());
        assert!(CapabilityValidator::is_capability_type(&region_type));
        
        validator.register_binding("region".to_string(), &region_type).unwrap();
        assert!(validator.get_state("region").is_some());
    }

    /// Test 8: Concurrent access detection
    #[test]
    fn test_concurrent_access_violation() {
        let mut ctx = CapabilityContext::new();
        
        ctx.set_location(1, 0);
        ctx.define_capability("socket".to_string(), CapabilityKind::Socket).unwrap();
        
        // Thread 0 access
        ctx.set_location(2, 0);
        ctx.use_capability("socket").unwrap();
        
        // Thread 1 access (different thread)
        ctx.current_thread_id = 1;
        ctx.set_location(3, 0);
        ctx.use_capability("socket").unwrap();
        
        let violations = ctx.validate_all();
        assert!(!violations.is_empty());
        
        let has_concurrent_violation = violations.iter().any(|v| {
            matches!(v, CapabilityViolation::ConcurrentUseWithoutSync { .. })
        });
        assert!(has_concurrent_violation);
    }

    /// Test 9: Capability sharing annotation
    #[test]
    fn test_shared_capability() {
        let mut ctx = CapabilityContext::new();
        
        ctx.set_location(1, 0);
        ctx.define_capability("socket".to_string(), CapabilityKind::Socket).unwrap();
        
        ctx.set_location(2, 0);
        ctx.share_capability("socket").unwrap();
        
        // Now should allow concurrent access
        ctx.use_capability("socket").unwrap();
        
        ctx.current_thread_id = 1;
        ctx.set_location(3, 0);
        ctx.use_capability("socket").unwrap();
        
        let violations = ctx.validate_all();
        // Should not have concurrent access violation since it's shared
        assert!(violations.is_empty());
    }

    /// Test 10: Named type classification
    #[test]
    fn test_socket_named_type() {
        let socket = Type::Named("SocketHandle".to_string());
        assert_eq!(
            CapabilityValidator::infer_capability_kind(&socket),
            Some(CapabilityKind::Socket)
        );
    }

    /// Test 11: Non-capability types
    #[test]
    fn test_non_capability_types() {
        assert_eq!(CapabilityValidator::infer_capability_kind(&Type::U32), None);
        assert_eq!(CapabilityValidator::infer_capability_kind(&Type::Bool), None);
        assert_eq!(CapabilityValidator::infer_capability_kind(&Type::String), None);
    }

    /// Test 12: Capability diagnostic generation
    #[test]
    fn test_use_after_consumption_diagnostic() {
        let diag = CapabilityDiagnosticFactory::use_after_consumption(
            "test.aura".to_string(),
            5,
            10,
            "socket",
            CapabilityKind::Socket,
            3,
            5,
        );

        let full_msg = diag.full_message();
        assert!(full_msg.contains("socket"));
        assert!(full_msg.contains("error"));
        assert!(!diag.suggestion.is_none());
    }

    /// Test 13: Resource leak diagnostic
    #[test]
    fn test_resource_leak_diagnostic() {
        let diag = CapabilityDiagnosticFactory::resource_leak(
            "test.aura".to_string(),
            10,
            0,
            "tensor",
            CapabilityKind::Tensor,
            2,
            5,
        );

        let full_msg = diag.full_message();
        assert!(full_msg.contains("resource leak"));
        assert!(full_msg.contains("tensor"));
    }

    /// Test 14: Concurrent use diagnostic
    #[test]
    fn test_concurrent_use_diagnostic() {
        let diag = CapabilityDiagnosticFactory::concurrent_use_without_sync(
            "test.aura".to_string(),
            8,
            5,
            "socket",
            CapabilityKind::Socket,
            3,
            0,
            7,
            10,
        );

        assert!(!diag.related_locations.is_empty());
        assert!(diag.full_message().contains("concurrent"));
    }

    /// Test 15: State history tracking
    #[test]
    fn test_capability_history() {
        let mut ctx = CapabilityContext::new();
        
        ctx.set_location(1, 0);
        ctx.define_capability("socket".to_string(), CapabilityKind::Socket).unwrap();
        
        ctx.set_location(2, 0);
        ctx.use_capability("socket").unwrap();
        
        ctx.set_location(3, 0);
        ctx.consume_capability("socket").unwrap();
        
        let history = ctx.get_history("socket").unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].0, CapabilityState::Fresh);
        assert_eq!(history[1].0, CapabilityState::InUse);
        assert_eq!(history[2].0, CapabilityState::Consumed);
    }

    /// Test 16: Validator strict mode
    #[test]
    fn test_validator_strict_mode() {
        let validator_strict = CapabilityValidator::new(true);
        let validator_lenient = CapabilityValidator::new(false);
        
        assert_ne!(validator_strict.strict_mode, validator_lenient.strict_mode);
    }

    /// Test 17: Multiple scopes with nesting
    #[test]
    fn test_nested_scopes() {
        let mut ctx = CapabilityContext::new();
        
        ctx.set_location(1, 0);
        ctx.define_capability("outer".to_string(), CapabilityKind::Socket).unwrap();
        
        ctx.enter_scope();
        ctx.set_location(2, 0);
        ctx.define_capability("inner".to_string(), CapabilityKind::Tensor).unwrap();
        
        ctx.set_location(3, 0);
        ctx.consume_capability("inner").unwrap();
        
        let result = ctx.exit_scope();
        assert!(result.is_ok());
    }

    /// Test 18: Validator integration with types
    #[test]
    fn test_validator_register_and_use() {
        let mut validator = CapabilityValidator::new(true);
        validator.set_location(1, 0);
        
        let socket_type = Type::Named("Socket".to_string());
        validator.register_binding("sock".to_string(), &socket_type).unwrap();
        
        validator.set_location(2, 0);
        assert!(validator.use_capability("sock").is_ok());
        
        validator.set_location(3, 0);
        assert!(validator.consume_capability("sock").is_ok());
    }

    /// Test 19: Applied type capability inference
    #[test]
    fn test_applied_type_tensor() {
        let tensor_applied = Type::Applied {
            name: "Tensor".to_string(),
            args: vec![Type::U32],
        };
        
        assert_eq!(
            CapabilityValidator::infer_capability_kind(&tensor_applied),
            Some(CapabilityKind::Tensor)
        );
    }

    /// Test 20: Comprehensive workflow
    #[test]
    fn test_comprehensive_workflow() {
        let mut validator = CapabilityValidator::new(true);
        
        // Create socket
        validator.set_location(1, 0);
        let socket_type = Type::Named("Socket".to_string());
        validator.register_binding("socket".to_string(), &socket_type).unwrap();
        
        // Create tensor
        validator.set_location(2, 0);
        let tensor_type = Type::Tensor {
            elem: Box::new(Type::U32),
            shape: None,
        };
        validator.register_binding("tensor".to_string(), &tensor_type).unwrap();
        
        // Use both
        validator.set_location(3, 0);
        assert!(validator.use_capability("socket").is_ok());
        assert!(validator.use_capability("tensor").is_ok());
        
        // Consume socket
        validator.set_location(4, 0);
        assert!(validator.consume_capability("socket").is_ok());
        
        // Try to use socket again (should fail)
        validator.set_location(5, 0);
        assert!(validator.use_capability("socket").is_err());
        
        // Consume tensor (should work)
        validator.set_location(6, 0);
        assert!(validator.consume_capability("tensor").is_ok());
    }
}
