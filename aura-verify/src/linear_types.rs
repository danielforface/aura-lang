#![forbid(unsafe_code)]

use std::collections::HashMap;

/// Ownership state of a binding in a function's lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ownership {
    /// Value is owned by this binding (can move or consume)
    Owned,
    /// Value is borrowed immutably (&T) - can read but not modify
    BorrowedImmut,
    /// Value is borrowed mutably (&mut T) - can read and modify
    BorrowedMut,
    /// Value has been moved/consumed (linear type - no longer usable)
    Moved,
    /// Value is in an unknown state (conservative assumption: treat as consumed)
    Unknown,
}

impl Ownership {
    pub fn is_usable(&self) -> bool {
        matches!(self, Ownership::Owned | Ownership::BorrowedImmut | Ownership::BorrowedMut)
    }

    pub fn to_string(&self) -> String {
        match self {
            Ownership::Owned => "owned".to_string(),
            Ownership::BorrowedImmut => "borrowed (immut)".to_string(),
            Ownership::BorrowedMut => "borrowed (mut)".to_string(),
            Ownership::Moved => "moved".to_string(),
            Ownership::Unknown => "unknown".to_string(),
        }
    }
}

/// Represents a binding and its ownership state at a specific program point.
#[derive(Clone, Debug)]
pub struct OwnershipBinding {
    pub name: String,
    pub var_type: String,
    pub ownership: Ownership,
    /// Line number where the binding was created
    pub defined_at_line: u32,
    /// Line number where the binding was moved/consumed (if applicable)
    pub moved_at_line: Option<u32>,
}

impl OwnershipBinding {
    pub fn new(name: String, var_type: String, defined_at_line: u32) -> Self {
        OwnershipBinding {
            name,
            var_type,
            ownership: Ownership::Owned,
            defined_at_line,
            moved_at_line: None,
        }
    }

    /// Check if this binding is still usable at the given line.
    pub fn is_usable_at_line(&self, line: u32) -> bool {
        if let Some(moved_line) = self.moved_at_line {
            line <= moved_line
        } else {
            true
        }
    }
}

/// Linear type enforcement engine for Aura.
/// 
/// Tracks ownership state of bindings throughout function execution
/// and enforces that moved values are not used after moving.
/// 
/// Rules:
/// 1. Each binding starts as `Owned` when created
/// 2. Using a binding preserves its ownership (read)
/// 3. Moving a binding (transferring ownership) transitions it to `Moved`
/// 4. Using a moved binding is a type error (use-after-move)
/// 5. Borrowing a binding creates temporary references without moving
pub struct OwnershipChecker {
    /// Current ownership state of all bindings in scope
    bindings: HashMap<String, OwnershipBinding>,
    /// Error diagnostics
    errors: Vec<OwnershipError>,
    /// Current line number (for tracking move locations)
    current_line: u32,
}

/// Represents a use-after-move or similar ownership violation.
#[derive(Clone, Debug)]
pub struct OwnershipError {
    pub binding_name: String,
    pub error_kind: OwnershipErrorKind,
    pub at_line: u32,
    pub moved_at_line: Option<u32>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnershipErrorKind {
    UseAfterMove,
    DoubleMove,
    UseAfterConsume,
    BorrowAfterMove,
    InvalidBorrow,
}

impl OwnershipChecker {
    /// Create a new ownership checker for a function.
    pub fn new() -> Self {
        OwnershipChecker {
            bindings: HashMap::new(),
            errors: Vec::new(),
            current_line: 0,
        }
    }

    /// Register a new binding (e.g., from a `let` statement).
    pub fn define_binding(&mut self, name: String, var_type: String, line: u32) {
        self.current_line = line;
        self.bindings.insert(name.clone(), OwnershipBinding::new(name, var_type, line));
    }

    /// Record a read access to a binding (does not consume ownership).
    pub fn read_binding(&mut self, name: &str, line: u32) -> Result<(), OwnershipError> {
        self.current_line = line;

        let binding = self.bindings.get(name).ok_or_else(|| OwnershipError {
            binding_name: name.to_string(),
            error_kind: OwnershipErrorKind::UseAfterMove,
            at_line: line,
            moved_at_line: None,
            message: format!("binding '{}' not found in scope", name),
        })?;

        if !binding.ownership.is_usable() {
            return Err(OwnershipError {
                binding_name: name.to_string(),
                error_kind: OwnershipErrorKind::UseAfterMove,
                at_line: line,
                moved_at_line: binding.moved_at_line,
                message: format!(
                    "cannot use binding '{}': it was moved at line {}",
                    name,
                    binding.moved_at_line.unwrap_or(0)
                ),
            });
        }

        Ok(())
    }

    /// Record a move of a binding (transfers ownership, invalidates further use).
    pub fn move_binding(&mut self, name: &str, line: u32) -> Result<(), OwnershipError> {
        self.current_line = line;

        let binding = self.bindings.get_mut(name).ok_or_else(|| OwnershipError {
            binding_name: name.to_string(),
            error_kind: OwnershipErrorKind::UseAfterMove,
            at_line: line,
            moved_at_line: None,
            message: format!("binding '{}' not found in scope", name),
        })?;

        if binding.moved_at_line.is_some() {
            return Err(OwnershipError {
                binding_name: name.to_string(),
                error_kind: OwnershipErrorKind::DoubleMove,
                at_line: line,
                moved_at_line: binding.moved_at_line,
                message: format!(
                    "binding '{}' was already moved at line {}",
                    name,
                    binding.moved_at_line.unwrap()
                ),
            });
        }

        binding.ownership = Ownership::Moved;
        binding.moved_at_line = Some(line);

        Ok(())
    }

    /// Record a borrow of a binding (creates temporary reference, does not move).
    pub fn borrow_binding(
        &mut self,
        name: &str,
        line: u32,
        mutable: bool,
    ) -> Result<(), OwnershipError> {
        self.current_line = line;

        let binding = self.bindings.get_mut(name).ok_or_else(|| OwnershipError {
            binding_name: name.to_string(),
            error_kind: OwnershipErrorKind::BorrowAfterMove,
            at_line: line,
            moved_at_line: None,
            message: format!("binding '{}' not found in scope", name),
        })?;

        if !binding.ownership.is_usable() {
            return Err(OwnershipError {
                binding_name: name.to_string(),
                error_kind: OwnershipErrorKind::BorrowAfterMove,
                at_line: line,
                moved_at_line: binding.moved_at_line,
                message: format!(
                    "cannot borrow binding '{}': it was moved at line {}",
                    name,
                    binding.moved_at_line.unwrap_or(0)
                ),
            });
        }

        // Update ownership state based on borrow type
        if mutable {
            binding.ownership = Ownership::BorrowedMut;
        } else if binding.ownership != Ownership::BorrowedMut {
            binding.ownership = Ownership::BorrowedImmut;
        }

        Ok(())
    }

    /// Get the ownership state of a binding.
    pub fn binding_ownership(&self, name: &str) -> Option<Ownership> {
        self.bindings.get(name).map(|b| b.ownership)
    }

    /// Record an error without returning early.
    pub fn record_error(&mut self, error: OwnershipError) {
        self.errors.push(error);
    }

    /// Get all recorded errors.
    pub fn errors(&self) -> &[OwnershipError] {
        &self.errors
    }

    /// Get a summary of all bindings and their ownership states.
    pub fn binding_summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Ownership State:".to_string());

        let mut names: Vec<_> = self.bindings.keys().collect();
        names.sort();

        for name in names {
            let binding = &self.bindings[name];
            let status = if let Some(moved_line) = binding.moved_at_line {
                format!("{} (moved at line {})", binding.ownership.to_string(), moved_line)
            } else {
                binding.ownership.to_string()
            };

            lines.push(format!(
                "  {} ({}): {}",
                name, binding.var_type, status
            ));
        }

        lines.join("\n")
    }
}

impl Default for OwnershipChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_define_binding() {
        let mut checker = OwnershipChecker::new();
        checker.define_binding("x".to_string(), "u32".to_string(), 1);

        assert!(checker.bindings.contains_key("x"));
        assert_eq!(checker.binding_ownership("x"), Some(Ownership::Owned));
    }

    #[test]
    fn test_read_binding_owned() {
        let mut checker = OwnershipChecker::new();
        checker.define_binding("x".to_string(), "u32".to_string(), 1);

        assert!(checker.read_binding("x", 2).is_ok());
        assert_eq!(checker.binding_ownership("x"), Some(Ownership::Owned));
    }

    #[test]
    fn test_move_binding() {
        let mut checker = OwnershipChecker::new();
        checker.define_binding("x".to_string(), "u32".to_string(), 1);

        assert!(checker.move_binding("x", 2).is_ok());
        assert_eq!(checker.binding_ownership("x"), Some(Ownership::Moved));
    }

    #[test]
    fn test_use_after_move() {
        let mut checker = OwnershipChecker::new();
        checker.define_binding("x".to_string(), "u32".to_string(), 1);

        let _ = checker.move_binding("x", 2);
        let result = checker.read_binding("x", 3);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_kind, OwnershipErrorKind::UseAfterMove);
    }

    #[test]
    fn test_double_move() {
        let mut checker = OwnershipChecker::new();
        checker.define_binding("x".to_string(), "u32".to_string(), 1);

        let _ = checker.move_binding("x", 2);
        let result = checker.move_binding("x", 3);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_kind, OwnershipErrorKind::DoubleMove);
    }

    #[test]
    fn test_borrow_after_move() {
        let mut checker = OwnershipChecker::new();
        checker.define_binding("x".to_string(), "u32".to_string(), 1);

        let _ = checker.move_binding("x", 2);
        let result = checker.borrow_binding("x", 3, false);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_kind, OwnershipErrorKind::BorrowAfterMove);
    }

    #[test]
    fn test_borrow_immut() {
        let mut checker = OwnershipChecker::new();
        checker.define_binding("x".to_string(), "u32".to_string(), 1);

        assert!(checker.borrow_binding("x", 2, false).is_ok());
        assert_eq!(checker.binding_ownership("x"), Some(Ownership::BorrowedImmut));
    }

    #[test]
    fn test_borrow_mut() {
        let mut checker = OwnershipChecker::new();
        checker.define_binding("x".to_string(), "u32".to_string(), 1);

        assert!(checker.borrow_binding("x", 2, true).is_ok());
        assert_eq!(checker.binding_ownership("x"), Some(Ownership::BorrowedMut));
    }

    #[test]
    fn test_binding_not_in_scope() {
        let mut checker = OwnershipChecker::new();

        let result = checker.read_binding("undefined", 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_binding_summary() {
        let mut checker = OwnershipChecker::new();
        checker.define_binding("x".to_string(), "u32".to_string(), 1);
        checker.define_binding("y".to_string(), "bool".to_string(), 2);

        let summary = checker.binding_summary();
        assert!(summary.contains("x (u32)"));
        assert!(summary.contains("y (bool)"));
        assert!(summary.contains("owned"));
    }

    #[test]
    fn test_is_usable_at_line() {
        let binding = OwnershipBinding::new("x".to_string(), "u32".to_string(), 10);
        assert!(binding.is_usable_at_line(15));
        
        let mut binding = OwnershipBinding::new("x".to_string(), "u32".to_string(), 10);
        binding.moved_at_line = Some(20);
        
        assert!(binding.is_usable_at_line(19));
        assert!(binding.is_usable_at_line(20));
        assert!(!binding.is_usable_at_line(21));  // After move, no longer usable
    }

    #[test]
    fn test_ownership_to_string() {
        assert_eq!(Ownership::Owned.to_string(), "owned");
        assert_eq!(Ownership::BorrowedImmut.to_string(), "borrowed (immut)");
        assert_eq!(Ownership::BorrowedMut.to_string(), "borrowed (mut)");
        assert_eq!(Ownership::Moved.to_string(), "moved");
    }
}
