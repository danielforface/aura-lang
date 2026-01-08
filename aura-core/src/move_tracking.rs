/// Linear Type Move Tracking for Type-Checker Integration
/// 
/// This module provides the bridge between ownership_enforcement.rs and the 
/// semantic checker (sema.rs). It handles integration of move semantics checks
/// with the existing type checking infrastructure.

use crate::types::Type;
use crate::ownership_enforcement::{OwnershipContext, OwnershipState};
use aura_ast::Span;

/// Linear type classification
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinearTypeKind {
    /// Non-linear copyable type (primitives, immutable aggregates)
    Copyable,
    /// Linear resource type (Tensor, Model, Style, custom resources)
    Linear,
    /// Reference type (borrowed, not linear)
    Reference,
}

/// Determine if a type is linear (must be explicitly consumed).
pub fn classify_type(ty: &Type) -> LinearTypeKind {
    match ty {
        Type::Unknown => LinearTypeKind::Copyable,
        Type::Unit => LinearTypeKind::Copyable,
        Type::Bool => LinearTypeKind::Copyable,
        Type::U32 => LinearTypeKind::Copyable,
        Type::String => LinearTypeKind::Copyable,
        
        // Linear resource types
        Type::Tensor { .. } => LinearTypeKind::Linear,
        Type::Model => LinearTypeKind::Linear,
        Type::Style => LinearTypeKind::Linear,
        
        Type::Named(n) => {
            // Heuristic: types ending in these names are typically linear resources
            match n.as_str() {
                n if n.contains("Tensor") => LinearTypeKind::Linear,
                n if n.contains("Model") => LinearTypeKind::Linear,
                n if n.contains("Socket") => LinearTypeKind::Linear,
                n if n.contains("Region") => LinearTypeKind::Linear,
                n if n.contains("Capability") => LinearTypeKind::Linear,
                // Default to copyable for other named types
                _ => LinearTypeKind::Copyable,
            }
        }
        
        Type::Applied { name, .. } => {
            // Apply same heuristics for generic types
            match name.as_str() {
                n if n.contains("Tensor") => LinearTypeKind::Linear,
                n if n.contains("Vec") => LinearTypeKind::Linear,
                n if n.contains("HashMap") => LinearTypeKind::Linear,
                n if n.contains("Option") => LinearTypeKind::Copyable, // Options are copyable
                n if n.contains("Result") => LinearTypeKind::Copyable, // Results are copyable
                _ => LinearTypeKind::Copyable,
            }
        }
        
        Type::ConstrainedRange { base, .. } => classify_type(base),
    }
}

/// Tracks move operations in expression contexts.
/// 
/// This struct records which values have been moved in an expression sequence
/// and validates that they're not used after being moved.
#[derive(Clone, Debug)]
pub struct MoveTracker {
    /// Values that have been moved in this scope
    moved_values: Vec<String>,
    /// Values that are currently borrowed
    borrowed_values: Vec<String>,
}

impl MoveTracker {
    pub fn new() -> Self {
        MoveTracker {
            moved_values: Vec::new(),
            borrowed_values: Vec::new(),
        }
    }
    
    /// Record a move operation.
    pub fn record_move(&mut self, name: &str) {
        if !self.moved_values.contains(&name.to_string()) {
            self.moved_values.push(name.to_string());
        }
    }
    
    /// Record a borrow operation.
    pub fn record_borrow(&mut self, name: &str) {
        if !self.borrowed_values.contains(&name.to_string()) {
            self.borrowed_values.push(name.to_string());
        }
    }
    
    /// Check if a value has been moved.
    pub fn is_moved(&self, name: &str) -> bool {
        self.moved_values.contains(&name.to_string())
    }
    
    /// Check if a value is borrowed.
    pub fn is_borrowed(&self, name: &str) -> bool {
        self.borrowed_values.contains(&name.to_string())
    }
    
    /// Clear the tracking (for new expression scopes).
    pub fn clear(&mut self) {
        self.moved_values.clear();
        self.borrowed_values.clear();
    }
}

/// Rules for linear type enforcement.
pub struct LinearTypeRules;

impl LinearTypeRules {
    /// Rule 1: Each linear value can only be used once (moved).
    /// 
    /// After a linear value is moved into a function call or assignment,
    /// it cannot be used again in the same function.
    pub fn check_no_use_after_move(
        ownership: &OwnershipContext,
        name: &str,
        line: u32,
    ) -> Result<(), String> {
        if ownership.binding_exists(name) {
            if let Some(bindings) = ownership.current_bindings() {
                if let Some(binding) = bindings.get(name) {
                    if !binding.can_use_at(line) {
                        return Err(format!(
                            "cannot use binding '{}' after it was moved at line {}:{}",
                            name,
                            binding.moved_at_line.unwrap_or(0),
                            binding.moved_at_col.unwrap_or(0)
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Rule 2: Linear values must be consumed before function ends.
    /// 
    /// If a function accepts ownership of a linear value, it must either:
    /// - Move it to another function (pass it along)
    /// - Return it to the caller
    /// - Explicitly consume it
    pub fn check_linear_consumed(ownership: &OwnershipContext) -> Result<(), Vec<String>> {
        let unconsumed = ownership.check_linear_resources_consumed();
        if !unconsumed.is_empty() {
            let errors: Vec<String> = unconsumed
                .iter()
                .map(|v| v.message.clone())
                .collect();
            return Err(errors);
        }
        Ok(())
    }
    
    /// Rule 3: Cannot move a borrowed value.
    /// 
    /// If a value has been borrowed (via &T or &mut T), it cannot be moved
    /// until all borrows go out of scope.
    pub fn check_no_move_while_borrowed(
        ownership: &OwnershipContext,
        name: &str,
    ) -> Result<(), String> {
        if let Some(bindings) = ownership.current_bindings() {
            if let Some(binding) = bindings.get(name) {
                if matches!(binding.state, OwnershipState::BorrowedImmut | OwnershipState::BorrowedMut) {
                    return Err(format!(
                        "cannot move binding '{}' while it's borrowed",
                        name
                    ));
                }
            }
        }
        Ok(())
    }
    
    /// Rule 4: Cannot use a moved value.
    /// 
    /// Once a linear value is moved, the original binding becomes invalid.
    pub fn check_no_use_after_move_simple(
        is_moved: bool,
        name: &str,
    ) -> Result<(), String> {
        if is_moved {
            Err(format!("cannot use binding '{}' after it was moved", name))
        } else {
            Ok(())
        }
    }
    
    /// Rule 5: Borrowing must respect exclusivity.
    /// 
    /// At most one mutable borrow of a value can exist at a time.
    /// Multiple immutable borrows are allowed simultaneously.
    pub fn check_borrow_exclusivity(
        ownership: &OwnershipContext,
        name: &str,
        is_mutable: bool,
    ) -> Result<(), String> {
        if let Some(bindings) = ownership.current_bindings() {
            if let Some(binding) = bindings.get(name) {
                if is_mutable && binding.state == OwnershipState::BorrowedImmut {
                    return Err(format!(
                        "cannot take mutable borrow of '{}': already borrowed immutably",
                        name
                    ));
                }
                if is_mutable && binding.state == OwnershipState::BorrowedMut {
                    return Err(format!(
                        "cannot take mutable borrow of '{}': already borrowed mutably",
                        name
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Diagnostic information for linear type violations.
#[derive(Clone, Debug)]
pub struct LinearTypeViolationDiagnostic {
    pub message: String,
    pub span: Span,
    pub move_site: Option<(u32, u32)>, // (line, col)
    pub suggestion: Option<String>,
}

impl LinearTypeViolationDiagnostic {
    pub fn new(message: String, span: Span) -> Self {
        LinearTypeViolationDiagnostic {
            message,
            span,
            move_site: None,
            suggestion: None,
        }
    }
    
    pub fn with_move_site(mut self, line: u32, col: u32) -> Self {
        self.move_site = Some((line, col));
        self
    }
    
    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_copyable_types() {
        assert_eq!(classify_type(&Type::U32), LinearTypeKind::Copyable);
        assert_eq!(classify_type(&Type::Bool), LinearTypeKind::Copyable);
        assert_eq!(classify_type(&Type::String), LinearTypeKind::Copyable);
        assert_eq!(classify_type(&Type::Unit), LinearTypeKind::Copyable);
    }

    #[test]
    fn test_classify_linear_types() {
        let tensor_type = Type::Tensor {
            elem: Box::new(Type::U32),
            shape: None,
        };
        assert_eq!(classify_type(&tensor_type), LinearTypeKind::Linear);
        
        assert_eq!(classify_type(&Type::Model), LinearTypeKind::Linear);
        assert_eq!(classify_type(&Type::Style), LinearTypeKind::Linear);
    }

    #[test]
    fn test_classify_named_linear_types() {
        let tensor_named = Type::Named("TensorBuffer".to_string());
        assert_eq!(classify_type(&tensor_named), LinearTypeKind::Linear);
        
        let model_named = Type::Named("ModelHandle".to_string());
        assert_eq!(classify_type(&model_named), LinearTypeKind::Linear);
    }

    #[test]
    fn test_classify_generic_linear_types() {
        let vec_type = Type::Applied {
            name: "Vec".to_string(),
            args: vec![Type::U32],
        };
        assert_eq!(classify_type(&vec_type), LinearTypeKind::Linear);
        
        let option_type = Type::Applied {
            name: "Option".to_string(),
            args: vec![Type::U32],
        };
        assert_eq!(classify_type(&option_type), LinearTypeKind::Copyable);
    }

    #[test]
    fn test_move_tracker() {
        let mut tracker = MoveTracker::new();
        assert!(!tracker.is_moved("x"));
        
        tracker.record_move("x");
        assert!(tracker.is_moved("x"));
    }

    #[test]
    fn test_borrow_tracker() {
        let mut tracker = MoveTracker::new();
        assert!(!tracker.is_borrowed("x"));
        
        tracker.record_borrow("x");
        assert!(tracker.is_borrowed("x"));
    }

    #[test]
    fn test_linear_type_rules_use_after_move() {
        let result = LinearTypeRules::check_no_use_after_move_simple(true, "x");
        assert!(result.is_err());
        
        let result = LinearTypeRules::check_no_use_after_move_simple(false, "x");
        assert!(result.is_ok());
    }
}
