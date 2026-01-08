/// Capability Validation Integration for Type-Checker
/// 
/// This module provides integration points for the type-checker to validate
/// capabilities during semantic analysis. It wraps CapabilityContext and provides
/// convenient APIs for sema.rs to use capability enforcement.

use crate::capability_enforcement::{CapabilityContext, CapabilityKind, CapabilityState};
use crate::types::Type;

/// Validator for capabilities in type-checking context
pub struct CapabilityValidator {
    /// Context tracking all capabilities
    context: CapabilityContext,
    /// Whether to enforce strict capability checking
    strict_mode: bool,
}

impl CapabilityValidator {
    /// Create new capability validator
    pub fn new(strict_mode: bool) -> Self {
        CapabilityValidator {
            context: CapabilityContext::new(),
            strict_mode,
        }
    }

    /// Set current location for error reporting
    pub fn set_location(&mut self, line: u32, col: u32) {
        self.context.set_location(line, col);
    }

    /// Register a binding that requires capability enforcement
    pub fn register_binding(&mut self, name: String, ty: &Type) -> Result<(), String> {
        let kind = Self::infer_capability_kind(ty);
        
        if let Some(kind) = kind {
            self.context.define_capability(name, kind)
                .map_err(|v| v.message())
        } else {
            // Not a capability type, no enforcement needed
            Ok(())
        }
    }

    /// Check if a type requires capability enforcement
    pub fn is_capability_type(ty: &Type) -> bool {
        Self::infer_capability_kind(ty).is_some()
    }

    /// Infer the capability kind from a type
    pub fn infer_capability_kind(ty: &Type) -> Option<CapabilityKind> {
        match ty {
            Type::Named(name) => {
                match name.as_str() {
                    n if n.contains("Socket") => Some(CapabilityKind::Socket),
                    n if n.contains("Tensor") => Some(CapabilityKind::Tensor),
                    n if n.contains("Region") => Some(CapabilityKind::Region),
                    n if n.contains("Concurrent") => Some(CapabilityKind::Concurrent),
                    _ => None,
                }
            }
            Type::Tensor { .. } => Some(CapabilityKind::Tensor),
            Type::Applied { name, .. } => {
                match name.as_str() {
                    n if n.contains("Socket") => Some(CapabilityKind::Socket),
                    n if n.contains("Tensor") => Some(CapabilityKind::Tensor),
                    n if n.contains("Region") => Some(CapabilityKind::Region),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Record use of a capability (reading/accessing it)
    pub fn use_capability(&mut self, name: &str) -> Result<(), String> {
        if !self.context.binding_exists(name) {
            // Not tracked, skip validation
            return Ok(());
        }

        self.context.use_capability(name)
            .map_err(|v| v.message())
    }

    /// Record consumption of a capability (closing/moving it)
    pub fn consume_capability(&mut self, name: &str) -> Result<(), String> {
        if !self.context.binding_exists(name) {
            // Not tracked, skip validation
            return Ok(());
        }

        self.context.consume_capability(name)
            .map_err(|v| v.message())
    }

    /// Record sharing of a capability (making it available for concurrent access)
    pub fn share_capability(&mut self, name: &str) -> Result<(), String> {
        if !self.context.binding_exists(name) {
            // Not tracked, skip validation
            return Ok(());
        }

        self.context.share_capability(name)
            .map_err(|v| v.message())
    }

    /// Enter a new scope (block statement)
    pub fn enter_scope(&mut self) {
        self.context.enter_scope();
    }

    /// Exit a scope and check for resource leaks
    pub fn exit_scope(&mut self) -> Result<(), Vec<String>> {
        match self.context.exit_scope() {
            Ok(violations) => {
                if violations.is_empty() {
                    Ok(())
                } else {
                    Err(violations.iter().map(|v| v.message()).collect())
                }
            }
            Err(v) => Err(vec![v.message()])
        }
    }

    /// Validate all current capabilities (check for violations)
    pub fn validate_all(&self) -> Vec<String> {
        if self.strict_mode {
            self.context.validate_all()
                .iter()
                .map(|v| v.message())
                .collect()
        } else {
            Vec::new() // In non-strict mode, only report at scope end
        }
    }

    /// Get current state of a capability
    pub fn get_state(&self, name: &str) -> Option<CapabilityState> {
        self.context.get_state(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_socket_binding() {
        let mut validator = CapabilityValidator::new(true);
        validator.set_location(1, 0);
        
        let socket_type = Type::Named("Socket".to_string());
        assert!(validator.register_binding("sock".to_string(), &socket_type).is_ok());
    }

    #[test]
    fn test_register_tensor_binding() {
        let mut validator = CapabilityValidator::new(true);
        validator.set_location(1, 0);
        
        let tensor_type = Type::Tensor {
            elem: Box::new(Type::U32),
            shape: None,
        };
        assert!(validator.register_binding("t".to_string(), &tensor_type).is_ok());
    }

    #[test]
    fn test_non_capability_type() {
        assert!(!CapabilityValidator::is_capability_type(&Type::U32));
        assert!(!CapabilityValidator::is_capability_type(&Type::Bool));
        assert!(!CapabilityValidator::is_capability_type(&Type::String));
    }

    #[test]
    fn test_socket_capability_type() {
        let socket = Type::Named("Socket".to_string());
        assert!(CapabilityValidator::is_capability_type(&socket));
    }

    #[test]
    fn test_use_then_consume() {
        let mut validator = CapabilityValidator::new(true);
        validator.set_location(1, 0);
        
        let socket_type = Type::Named("Socket".to_string());
        validator.register_binding("sock".to_string(), &socket_type).unwrap();
        
        validator.set_location(2, 0);
        assert!(validator.use_capability("sock").is_ok());
        
        validator.set_location(3, 0);
        assert!(validator.consume_capability("sock").is_ok());
    }

    #[test]
    fn test_use_after_consume_error() {
        let mut validator = CapabilityValidator::new(true);
        validator.set_location(1, 0);
        
        let socket_type = Type::Named("Socket".to_string());
        validator.register_binding("sock".to_string(), &socket_type).unwrap();
        
        validator.set_location(2, 0);
        validator.consume_capability("sock").unwrap();
        
        validator.set_location(3, 0);
        assert!(validator.use_capability("sock").is_err());
    }

    #[test]
    fn test_infer_capability_kind() {
        let socket = Type::Named("SocketHandle".to_string());
        assert_eq!(CapabilityValidator::infer_capability_kind(&socket), Some(CapabilityKind::Socket));
        
        let tensor = Type::Named("TensorBuffer".to_string());
        assert_eq!(CapabilityValidator::infer_capability_kind(&tensor), Some(CapabilityKind::Tensor));
        
        let region = Type::Named("RegionAlloc".to_string());
        assert_eq!(CapabilityValidator::infer_capability_kind(&region), Some(CapabilityKind::Region));
        
        let u32_type = Type::U32;
        assert_eq!(CapabilityValidator::infer_capability_kind(&u32_type), None);
    }

    #[test]
    fn test_scope_management() {
        let mut validator = CapabilityValidator::new(true);
        validator.set_location(1, 0);
        
        let socket_type = Type::Named("Socket".to_string());
        validator.register_binding("sock".to_string(), &socket_type).unwrap();
        
        validator.enter_scope();
        validator.set_location(2, 0);
        validator.consume_capability("sock").unwrap();
        
        // Should not error because we consume before exiting scope
        assert!(validator.exit_scope().is_ok());
    }

    #[test]
    fn test_scope_leak_detection() {
        let mut validator = CapabilityValidator::new(true);
        validator.set_location(1, 0);
        
        let socket_type = Type::Named("Socket".to_string());
        validator.register_binding("sock".to_string(), &socket_type).unwrap();
        
        validator.enter_scope();
        // Don't consume the capability
        
        // Should error on scope exit due to resource leak
        assert!(validator.exit_scope().is_err());
    }

    #[test]
    fn test_share_capability() {
        let mut validator = CapabilityValidator::new(true);
        validator.set_location(1, 0);
        
        let socket_type = Type::Named("Socket".to_string());
        validator.register_binding("sock".to_string(), &socket_type).unwrap();
        
        validator.set_location(2, 0);
        assert!(validator.share_capability("sock").is_ok());
        
        // After sharing, should be usable
        assert!(validator.use_capability("sock").is_ok());
    }
}
