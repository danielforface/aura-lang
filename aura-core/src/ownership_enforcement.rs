#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

/// Ownership state of a variable binding in the type system.
/// 
/// Each variable transitions through ownership states as it's used or moved.
/// The state machine prevents use-after-move errors at compile time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnershipState {
    /// Initial state: value is owned and can be moved, borrowed, or used
    Owned,
    
    /// Value has been consumed via move. Subsequent uses are not permitted.
    /// Linear resources (Tensor, Model, Style) cannot be used after Consumed.
    Consumed,
    
    /// Value is borrowed immutably (&T). Can be read but not moved or mutated.
    BorrowedImmut,
    
    /// Value is borrowed mutably (&mut T). Can be read and mutated but not moved.
    BorrowedMut,
    
    /// Value has been returned to caller (transferred ownership).
    Returned,
}

impl OwnershipState {
    /// Check if this state allows the given operation.
    pub fn allows_use(&self) -> bool {
        !matches!(self, OwnershipState::Consumed | OwnershipState::Returned)
    }
    
    pub fn allows_move(&self) -> bool {
        matches!(self, OwnershipState::Owned)
    }
    
    pub fn allows_borrow(&self) -> bool {
        matches!(
            self,
            OwnershipState::Owned | OwnershipState::BorrowedImmut | OwnershipState::BorrowedMut
        )
    }
    
    pub fn allows_mutate(&self) -> bool {
        matches!(
            self,
            OwnershipState::Owned | OwnershipState::BorrowedMut
        )
    }
    
    pub fn display(&self) -> &'static str {
        match self {
            OwnershipState::Owned => "owned",
            OwnershipState::Consumed => "consumed",
            OwnershipState::BorrowedImmut => "borrowed (immut)",
            OwnershipState::BorrowedMut => "borrowed (mut)",
            OwnershipState::Returned => "returned",
        }
    }
}

/// Metadata about a binding including location and type information.
#[derive(Clone, Debug)]
pub struct OwnershipBinding {
    /// Name of the variable
    pub name: String,
    /// Type name (e.g., "Model", "Tensor", "u32")
    pub type_name: String,
    /// Is this a linear type (Tensor, Model, Style)?
    pub is_linear: bool,
    /// Line number where binding was defined
    pub defined_at_line: u32,
    /// Column number where binding was defined
    pub defined_at_col: u32,
    /// Current ownership state
    pub state: OwnershipState,
    /// Line where it was moved/consumed (if applicable)
    pub moved_at_line: Option<u32>,
    /// Column where it was moved/consumed (if applicable)
    pub moved_at_col: Option<u32>,
}

impl OwnershipBinding {
    /// Create a new binding starting in Owned state.
    pub fn new(name: String, type_name: String, is_linear: bool, line: u32, col: u32) -> Self {
        OwnershipBinding {
            name,
            type_name,
            is_linear,
            defined_at_line: line,
            defined_at_col: col,
            state: OwnershipState::Owned,
            moved_at_line: None,
            moved_at_col: None,
        }
    }
    
    /// Check if this binding can be used at the given location.
    pub fn can_use_at(&self, _line: u32) -> bool {
        self.state.allows_use()
    }
    
    /// Mark this binding as moved.
    pub fn mark_moved(&mut self, line: u32, col: u32) {
        self.state = OwnershipState::Consumed;
        self.moved_at_line = Some(line);
        self.moved_at_col = Some(col);
    }
    
    /// Mark this binding as borrowed (immutably).
    pub fn mark_borrowed_immut(&mut self) {
        if self.state == OwnershipState::Owned {
            self.state = OwnershipState::BorrowedImmut;
        }
    }
    
    /// Mark this binding as borrowed (mutably).
    pub fn mark_borrowed_mut(&mut self) {
        if matches!(self.state, OwnershipState::Owned | OwnershipState::BorrowedMut) {
            self.state = OwnershipState::BorrowedMut;
        }
    }
}

/// Represents an ownership violation error.
#[derive(Clone, Debug)]
pub struct OwnershipViolation {
    /// The binding that was violated
    pub binding_name: String,
    /// Type of violation
    pub error_kind: ViolationKind,
    /// Location of the violation
    pub at_line: u32,
    pub at_col: u32,
    /// Where the binding was moved (if applicable)
    pub moved_at_line: Option<u32>,
    pub moved_at_col: Option<u32>,
    /// Detailed error message
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViolationKind {
    /// Use-after-move: tried to use a value after it was consumed
    UseAfterMove,
    /// Double-move: tried to move the same value twice
    DoubleMove,
    /// Borrow-after-move: tried to borrow a value after it was moved
    BorrowAfterMove,
    /// Move-after-borrow: tried to move a value that's currently borrowed
    MoveAfterBorrow,
    /// Use-not-moved: linear resource wasn't consumed before function end
    UseNotMoved,
    /// Invalid operation for state: e.g., mutate borrowed-immut value
    InvalidOperation,
}

/// The ownership enforcement engine for a function scope.
/// 
/// Tracks ownership state of all bindings in scope and detects violations.
/// Each scope level can have its own bindings (for nested blocks).
#[derive(Clone, Debug)]
pub struct OwnershipContext {
    /// Stack of binding scopes (one per block level)
    scopes: Vec<HashMap<String, OwnershipBinding>>,
    /// Stack of borrow scopes (track what's currently borrowed)
    borrow_scopes: Vec<HashSet<String>>,
    /// Accumulated errors/violations
    violations: Vec<OwnershipViolation>,
    /// Current line number (for diagnostics)
    current_line: u32,
    current_col: u32,
}

impl OwnershipContext {
    /// Create a new ownership context for a function.
    pub fn new() -> Self {
        OwnershipContext {
            scopes: vec![HashMap::new()],
            borrow_scopes: vec![HashSet::new()],
            violations: Vec::new(),
            current_line: 0,
            current_col: 0,
        }
    }
    
    /// Enter a new scope (e.g., entering a block).
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.borrow_scopes.push(HashSet::new());
    }
    
    /// Exit the current scope, cleaning up bindings.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
            self.borrow_scopes.pop();
        }
    }
    
    /// Update current source location for diagnostic purposes.
    pub fn set_location(&mut self, line: u32, col: u32) {
        self.current_line = line;
        self.current_col = col;
    }
    
    /// Register a new binding in the current scope.
    pub fn define_binding(&mut self, name: String, type_name: String, is_linear: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(
                name.clone(),
                OwnershipBinding::new(name, type_name, is_linear, self.current_line, self.current_col),
            );
        }
    }
    
    /// Get mutable reference to a binding in any scope (searches from innermost).
    fn find_binding_mut(&mut self, name: &str) -> Option<&mut OwnershipBinding> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.get_mut(name) {
                return Some(binding);
            }
        }
        None
    }
    
    /// Get reference to a binding in any scope (searches from innermost).
    fn find_binding(&self, name: &str) -> Option<&OwnershipBinding> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.get(name) {
                return Some(binding);
            }
        }
        None
    }
    
    /// Record a use (read) of a binding - does not consume ownership.
    pub fn record_use(&mut self, name: &str) -> Result<(), OwnershipViolation> {
        if let Some(binding) = self.find_binding(name) {
            if !binding.can_use_at(self.current_line) {
                return Err(OwnershipViolation {
                    binding_name: name.to_string(),
                    error_kind: ViolationKind::UseAfterMove,
                    at_line: self.current_line,
                    at_col: self.current_col,
                    moved_at_line: binding.moved_at_line,
                    moved_at_col: binding.moved_at_col,
                    message: format!(
                        "cannot use binding '{}' after it was moved at line {}",
                        name,
                        binding.moved_at_line.unwrap_or(self.current_line)
                    ),
                });
            }
            Ok(())
        } else {
            Err(OwnershipViolation {
                binding_name: name.to_string(),
                error_kind: ViolationKind::UseAfterMove,
                at_line: self.current_line,
                at_col: self.current_col,
                moved_at_line: None,
                moved_at_col: None,
                message: format!("binding '{}' not found in scope", name),
            })
        }
    }
    
    /// Record a move (transfer of ownership) of a binding.
    pub fn record_move(&mut self, name: &str) -> Result<(), OwnershipViolation> {
        let current_line = self.current_line;
        let current_col = self.current_col;
        
        // Check if currently borrowed
        let is_borrowed = self.borrow_scopes.last().map(|b| b.contains(name)).unwrap_or(false);
        if is_borrowed {
            return Err(OwnershipViolation {
                binding_name: name.to_string(),
                error_kind: ViolationKind::MoveAfterBorrow,
                at_line: current_line,
                at_col: current_col,
                moved_at_line: None,
                moved_at_col: None,
                message: format!(
                    "cannot move binding '{}' while it's borrowed",
                    name
                ),
            });
        }
        
        if let Some(binding) = self.find_binding_mut(name) {
            // Check if already consumed
            if binding.state == OwnershipState::Consumed {
                return Err(OwnershipViolation {
                    binding_name: name.to_string(),
                    error_kind: ViolationKind::DoubleMove,
                    at_line: current_line,
                    at_col: current_col,
                    moved_at_line: binding.moved_at_line,
                    moved_at_col: binding.moved_at_col,
                    message: format!(
                        "cannot move binding '{}' again: already consumed",
                        name
                    ),
                });
            }
            
            binding.mark_moved(current_line, current_col);
            Ok(())
        } else {
            Err(OwnershipViolation {
                binding_name: name.to_string(),
                error_kind: ViolationKind::UseAfterMove,
                at_line: self.current_line,
                at_col: self.current_col,
                moved_at_line: None,
                moved_at_col: None,
                message: format!("binding '{}' not found in scope", name),
            })
        }
    }
    
    /// Record an immutable borrow of a binding.
    pub fn record_borrow_immut(&mut self, name: &str) -> Result<(), OwnershipViolation> {
        if let Some(binding) = self.find_binding_mut(name) {
            if !binding.state.allows_borrow() {
                return Err(OwnershipViolation {
                    binding_name: name.to_string(),
                    error_kind: ViolationKind::BorrowAfterMove,
                    at_line: self.current_line,
                    at_col: self.current_col,
                    moved_at_line: binding.moved_at_line,
                    moved_at_col: binding.moved_at_col,
                    message: format!(
                        "cannot borrow '{}': it was moved at line {}",
                        name,
                        binding.moved_at_line.unwrap_or(self.current_line)
                    ),
                });
            }
            binding.mark_borrowed_immut();
            if let Some(borrows) = self.borrow_scopes.last_mut() {
                borrows.insert(name.to_string());
            }
            Ok(())
        } else {
            Err(OwnershipViolation {
                binding_name: name.to_string(),
                error_kind: ViolationKind::UseAfterMove,
                at_line: self.current_line,
                at_col: self.current_col,
                moved_at_line: None,
                moved_at_col: None,
                message: format!("binding '{}' not found in scope", name),
            })
        }
    }
    
    /// Record a mutable borrow of a binding.
    pub fn record_borrow_mut(&mut self, name: &str) -> Result<(), OwnershipViolation> {
        if let Some(binding) = self.find_binding_mut(name) {
            if !binding.state.allows_borrow() {
                return Err(OwnershipViolation {
                    binding_name: name.to_string(),
                    error_kind: ViolationKind::BorrowAfterMove,
                    at_line: self.current_line,
                    at_col: self.current_col,
                    moved_at_line: binding.moved_at_line,
                    moved_at_col: binding.moved_at_col,
                    message: format!(
                        "cannot mutably borrow '{}': it was moved at line {}",
                        name,
                        binding.moved_at_line.unwrap_or(self.current_line)
                    ),
                });
            }
            binding.mark_borrowed_mut();
            if let Some(borrows) = self.borrow_scopes.last_mut() {
                borrows.insert(name.to_string());
            }
            Ok(())
        } else {
            Err(OwnershipViolation {
                binding_name: name.to_string(),
                error_kind: ViolationKind::UseAfterMove,
                at_line: self.current_line,
                at_col: self.current_col,
                moved_at_line: None,
                moved_at_col: None,
                message: format!("binding '{}' not found in scope", name),
            })
        }
    }
    
    /// Record a return of a binding (moves ownership to caller).
    pub fn record_return(&mut self, name: &str) -> Result<(), OwnershipViolation> {
        if let Some(binding) = self.find_binding_mut(name) {
            binding.state = OwnershipState::Returned;
            Ok(())
        } else {
            Err(OwnershipViolation {
                binding_name: name.to_string(),
                error_kind: ViolationKind::UseAfterMove,
                at_line: self.current_line,
                at_col: self.current_col,
                moved_at_line: None,
                moved_at_col: None,
                message: format!("binding '{}' not found in scope", name),
            })
        }
    }
    
    /// Check that all linear resources in scope have been consumed.
    pub fn check_linear_resources_consumed(&mut self) -> Vec<OwnershipViolation> {
        let mut unconsumed = Vec::new();
        if let Some(scope) = self.scopes.last() {
            for binding in scope.values() {
                if binding.is_linear && binding.state != OwnershipState::Returned {
                    if !matches!(binding.state, OwnershipState::Consumed | OwnershipState::Returned) {
                        unconsumed.push(OwnershipViolation {
                            binding_name: binding.name.clone(),
                            error_kind: ViolationKind::UseNotMoved,
                            at_line: self.current_line,
                            at_col: self.current_col,
                            moved_at_line: None,
                            moved_at_col: None,
                            message: format!(
                                "linear resource '{}' of type '{}' must be consumed before function ends",
                                binding.name, binding.type_name
                            ),
                        });
                    }
                }
            }
        }
        unconsumed
    }
    
    /// Record a violation directly.
    pub fn record_violation(&mut self, violation: OwnershipViolation) {
        self.violations.push(violation);
    }
    
    /// Get all recorded violations.
    pub fn violations(&self) -> &[OwnershipViolation] {
        &self.violations
    }
    
    /// Get bindings in current scope.
    pub fn current_bindings(&self) -> Option<&HashMap<String, OwnershipBinding>> {
        self.scopes.last()
    }
    
    /// Check if a binding exists in any scope.
    pub fn binding_exists(&self, name: &str) -> bool {
        self.find_binding(name).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_use() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "Model".to_string(), true);
        
        ctx.set_location(2, 0);
        assert!(ctx.record_use("x").is_ok());
    }

    #[test]
    fn test_use_after_move() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "Model".to_string(), true);
        
        ctx.set_location(2, 0);
        assert!(ctx.record_move("x").is_ok());
        
        ctx.set_location(3, 0);
        let result = ctx.record_use("x");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_kind, ViolationKind::UseAfterMove);
    }

    #[test]
    fn test_double_move() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "Model".to_string(), true);
        
        ctx.set_location(2, 0);
        assert!(ctx.record_move("x").is_ok());
        
        ctx.set_location(3, 0);
        let result = ctx.record_move("x");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_kind, ViolationKind::DoubleMove);
    }

    #[test]
    fn test_borrow_then_move() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "Model".to_string(), true);
        
        ctx.set_location(2, 0);
        assert!(ctx.record_borrow_immut("x").is_ok());
        
        ctx.set_location(3, 0);
        let result = ctx.record_move("x");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_kind, ViolationKind::MoveAfterBorrow);
    }

    #[test]
    fn test_linear_resource_not_consumed() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "Model".to_string(), true);
        
        let unconsumed = ctx.check_linear_resources_consumed();
        assert!(unconsumed.len() > 0);
        assert_eq!(unconsumed[0].error_kind, ViolationKind::UseNotMoved);
    }

    #[test]
    fn test_scoped_bindings() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "u32".to_string(), false);
        
        ctx.push_scope();
        ctx.set_location(2, 0);
        ctx.define_binding("y".to_string(), "Model".to_string(), true);
        assert!(ctx.record_use("x").is_ok()); // Can use outer binding
        assert!(ctx.record_use("y").is_ok());
        
        ctx.pop_scope();
        // y is no longer in scope
        assert!(!ctx.binding_exists("y"));
        assert!(ctx.binding_exists("x")); // x still exists
    }

    #[test]
    fn test_mutable_borrow() {
        let mut ctx = OwnershipContext::new();
        ctx.set_location(1, 0);
        ctx.define_binding("x".to_string(), "Vector".to_string(), true);
        
        ctx.set_location(2, 0);
        assert!(ctx.record_borrow_mut("x").is_ok());
        
        ctx.set_location(3, 0);
        let result = ctx.record_move("x");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_kind, ViolationKind::MoveAfterBorrow);
    }
}
