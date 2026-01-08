#![forbid(unsafe_code)]

/// Linear Capability Enforcement for Sockets and Tensors
/// 
/// This module enforces capability-based resource management for hardware-bound
/// resources like Sockets (network) and Tensors (compute). Capabilities represent
/// exclusive access rights to resources and must follow strict ordering rules:
/// 
/// **Socket Lifecycle**: Created → Connected/Listening → Reading/Writing → Closed
/// **Tensor Lifecycle**: Created → Computed → Available for IO → Released
/// 
/// A capability is "consumed" when it transitions to a restricted state.
/// No capability can be used after consumption or in parallel without explicit sharing.

use std::collections::{HashMap, HashSet};

/// Capability kind (what resource is being managed)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CapabilityKind {
    /// Network socket capability (exclusive access to network I/O)
    Socket,
    /// Tensor compute capability (exclusive access to tensor operations)
    Tensor,
    /// Memory region capability (exclusive access to memory region)
    Region,
    /// Concurrent access capability (controlled sharing)
    Concurrent,
}

impl CapabilityKind {
    pub fn display(&self) -> &'static str {
        match self {
            CapabilityKind::Socket => "socket",
            CapabilityKind::Tensor => "tensor",
            CapabilityKind::Region => "region",
            CapabilityKind::Concurrent => "concurrent",
        }
    }
}

/// Capability state machine
/// 
/// Capabilities transition through states as operations are performed.
/// Some transitions are forbidden to prevent resource leaks and misuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CapabilityState {
    /// Freshly allocated, not yet used
    Fresh,
    /// Currently in use (exclusive access active)
    InUse,
    /// Temporarily suspended (borrowed but not moved)
    Suspended,
    /// Permanently consumed (operation completed, capability exhausted)
    Consumed,
    /// Error state (invalid operation detected)
    Error,
}

impl CapabilityState {
    pub fn display(&self) -> &'static str {
        match self {
            CapabilityState::Fresh => "fresh",
            CapabilityState::InUse => "in-use",
            CapabilityState::Suspended => "suspended",
            CapabilityState::Consumed => "consumed",
            CapabilityState::Error => "error",
        }
    }

    /// Check if a capability in this state can be used
    pub fn can_use(&self) -> bool {
        matches!(self, CapabilityState::Fresh | CapabilityState::InUse)
    }

    /// Check if a capability in this state can be shared (borrowed)
    pub fn can_share(&self) -> bool {
        matches!(self, CapabilityState::Fresh | CapabilityState::InUse)
    }
}

/// Capability violation types
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityViolation {
    /// Use after consumption (capability was already consumed)
    UseAfterConsumption { var_name: String, consumed_at: (u32, u32) },
    /// Invalid state transition
    InvalidTransition { var_name: String, from: CapabilityState, to: CapabilityState },
    /// Concurrent use without synchronization
    ConcurrentUseWithoutSync { var_name: String, first_access: (u32, u32), second_access: (u32, u32) },
    /// Resource leak (consumed state not reached at scope end)
    ResourceLeak { var_name: String, current_state: CapabilityState },
    /// Capability shared without proper annotation
    ImproperSharing { var_name: String, shared_at: (u32, u32) },
}

impl CapabilityViolation {
    pub fn message(&self) -> String {
        match self {
            CapabilityViolation::UseAfterConsumption { var_name, consumed_at } => {
                format!(
                    "capability '{}' used after consumption (consumed at {}:{})",
                    var_name, consumed_at.0, consumed_at.1
                )
            }
            CapabilityViolation::InvalidTransition { var_name, from, to } => {
                format!(
                    "invalid state transition for capability '{}': {} → {}",
                    var_name,
                    from.display(),
                    to.display()
                )
            }
            CapabilityViolation::ConcurrentUseWithoutSync { var_name, first_access, second_access } => {
                format!(
                    "capability '{}' used concurrently without synchronization (first at {}:{}, then at {}:{})",
                    var_name, first_access.0, first_access.1, second_access.0, second_access.1
                )
            }
            CapabilityViolation::ResourceLeak { var_name, current_state } => {
                format!(
                    "resource leak: capability '{}' not consumed before scope end (current state: {})",
                    var_name,
                    current_state.display()
                )
            }
            CapabilityViolation::ImproperSharing { var_name, shared_at } => {
                format!(
                    "capability '{}' shared without proper synchronization annotation (shared at {}:{})",
                    var_name, shared_at.0, shared_at.1
                )
            }
        }
    }
}

/// Single capability binding with metadata
#[derive(Clone, Debug)]
pub struct CapabilityBinding {
    /// Binding name (variable name)
    pub name: String,
    /// Kind of capability
    pub kind: CapabilityKind,
    /// Current state
    pub state: CapabilityState,
    /// Location where defined
    pub defined_at: (u32, u32),
    /// Last state change location
    pub last_change_at: (u32, u32),
    /// State transitions history
    pub history: Vec<(CapabilityState, u32, u32)>,
    /// Whether this capability can be shared
    pub shareable: bool,
    /// Thread(s) accessing this capability
    pub accessing_threads: HashSet<u32>,
}

impl CapabilityBinding {
    pub fn new(name: String, kind: CapabilityKind, line: u32, col: u32) -> Self {
        let binding_name = name.clone();
        CapabilityBinding {
            name: binding_name,
            kind,
            state: CapabilityState::Fresh,
            defined_at: (line, col),
            last_change_at: (line, col),
            history: vec![(CapabilityState::Fresh, line, col)],
            shareable: false,
            accessing_threads: HashSet::new(),
        }
    }

    /// Record a state transition
    pub fn transition(&mut self, new_state: CapabilityState, line: u32, col: u32) -> Result<(), CapabilityViolation> {
        // Validate transition is legal
        match (self.state, new_state) {
            // Fresh → In Use is always valid
            (CapabilityState::Fresh, CapabilityState::InUse) => {}
            // Fresh → Consumed is valid (direct consumption)
            (CapabilityState::Fresh, CapabilityState::Consumed) => {}
            // In Use → Suspended is valid (temporary hold)
            (CapabilityState::InUse, CapabilityState::Suspended) => {}
            // Suspended → In Use is valid (resume)
            (CapabilityState::Suspended, CapabilityState::InUse) => {}
            // In Use → Consumed is valid (finish using)
            (CapabilityState::InUse, CapabilityState::Consumed) => {}
            // Anything → Error is always valid (error reporting)
            (_, CapabilityState::Error) => {}
            // Error → Fresh is valid (recovery)
            (CapabilityState::Error, CapabilityState::Fresh) => {}
            // Consumed → anything is invalid (already consumed)
            (CapabilityState::Consumed, _) if new_state != CapabilityState::Error => {
                return Err(CapabilityViolation::InvalidTransition {
                    var_name: self.name.clone(),
                    from: self.state,
                    to: new_state,
                });
            }
            // Invalid transition
            _ => {
                return Err(CapabilityViolation::InvalidTransition {
                    var_name: self.name.clone(),
                    from: self.state,
                    to: new_state,
                });
            }
        }

        self.state = new_state;
        self.last_change_at = (line, col);
        self.history.push((new_state, line, col));
        Ok(())
    }
}

/// Manages all capabilities in a scope (function)
#[derive(Clone, Debug)]
pub struct CapabilityContext {
    /// Current line and column for error reporting
    current_location: (u32, u32),
    /// All capability bindings in current scope
    bindings: HashMap<String, CapabilityBinding>,
    /// Scope stack for nested blocks
    scope_stack: Vec<HashMap<String, CapabilityBinding>>,
    /// Shared capabilities across threads (require synchronization)
    shared_capabilities: HashSet<String>,
    /// Current thread ID (0 = main)
    current_thread_id: u32,
}

impl CapabilityContext {
    /// Create new context
    pub fn new() -> Self {
        CapabilityContext {
            current_location: (0, 0),
            bindings: HashMap::new(),
            scope_stack: Vec::new(),
            shared_capabilities: HashSet::new(),
            current_thread_id: 0,
        }
    }

    /// Set current location for error reporting
    pub fn set_location(&mut self, line: u32, col: u32) {
        self.current_location = (line, col);
    }

    /// Define a new capability in current scope
    pub fn define_capability(
        &mut self,
        name: String,
        kind: CapabilityKind,
    ) -> Result<(), CapabilityViolation> {
        if self.bindings.contains_key(&name) {
            return Err(CapabilityViolation::ImproperSharing {
                var_name: name.clone(),
                shared_at: self.current_location,
            });
        }
        let binding_name = name.clone();
        self.bindings.insert(
            name,
            CapabilityBinding::new(binding_name, kind, self.current_location.0, self.current_location.1),
        );
        Ok(())
    }

    /// Use a capability (transition to InUse)
    pub fn use_capability(&mut self, name: &str) -> Result<(), CapabilityViolation> {
        let binding = self
            .bindings
            .get_mut(name)
            .ok_or_else(|| CapabilityViolation::UseAfterConsumption {
                var_name: name.to_string(),
                consumed_at: self.current_location,
            })?;

        if !binding.state.can_use() {
            if binding.state == CapabilityState::Consumed {
                return Err(CapabilityViolation::UseAfterConsumption {
                    var_name: name.to_string(),
                    consumed_at: binding.last_change_at,
                });
            }
            return Err(CapabilityViolation::InvalidTransition {
                var_name: name.to_string(),
                from: binding.state,
                to: CapabilityState::InUse,
            });
        }

        binding.accessing_threads.insert(self.current_thread_id);

        if binding.state != CapabilityState::InUse {
            binding.transition(CapabilityState::InUse, self.current_location.0, self.current_location.1)?;
        }

        Ok(())
    }

    /// Consume a capability (transition to Consumed)
    pub fn consume_capability(&mut self, name: &str) -> Result<(), CapabilityViolation> {
        let binding = self
            .bindings
            .get_mut(name)
            .ok_or_else(|| CapabilityViolation::UseAfterConsumption {
                var_name: name.to_string(),
                consumed_at: self.current_location,
            })?;

        // Ensure accessing thread is recorded
        binding.accessing_threads.insert(self.current_thread_id);

        binding.transition(CapabilityState::Consumed, self.current_location.0, self.current_location.1)
    }

    /// Share a capability (mark as shareable, record thread access)
    pub fn share_capability(&mut self, name: &str) -> Result<(), CapabilityViolation> {
        let binding = self
            .bindings
            .get_mut(name)
            .ok_or_else(|| CapabilityViolation::ImproperSharing {
                var_name: name.to_string(),
                shared_at: self.current_location,
            })?;

        if !binding.state.can_share() {
            return Err(CapabilityViolation::ImproperSharing {
                var_name: name.to_string(),
                shared_at: self.current_location,
            });
        }

        binding.shareable = true;
        binding.accessing_threads.insert(self.current_thread_id);
        self.shared_capabilities.insert(name.to_string());

        Ok(())
    }

    /// Check for concurrent access violations
    pub fn check_concurrent_access(&self, name: &str) -> Result<(), CapabilityViolation> {
        if let Some(binding) = self.bindings.get(name) {
            if binding.accessing_threads.len() > 1 && !binding.shareable {
                return Err(CapabilityViolation::ConcurrentUseWithoutSync {
                    var_name: name.to_string(),
                    first_access: binding.defined_at,
                    second_access: binding.last_change_at,
                });
            }
        }
        Ok(())
    }

    /// Enter new scope (e.g., block statement)
    pub fn enter_scope(&mut self) {
        self.scope_stack.push(self.bindings.clone());
    }

    /// Exit scope and check for resource leaks
    pub fn exit_scope(&mut self) -> Result<Vec<CapabilityViolation>, CapabilityViolation> {
        let mut violations = Vec::new();

        // Check all bindings for unconsumed resources
        for (name, binding) in &self.bindings {
            if binding.state != CapabilityState::Consumed {
                violations.push(CapabilityViolation::ResourceLeak {
                    var_name: name.clone(),
                    current_state: binding.state,
                });
            }
        }

        // Restore previous scope
        if let Some(prev_bindings) = self.scope_stack.pop() {
            self.bindings = prev_bindings;
        }

        if violations.is_empty() {
            Ok(violations)
        } else {
            // Return first violation, caller can query for more
            Err(violations.into_iter().next().unwrap())
        }
    }

    /// Get current binding state
    pub fn get_state(&self, name: &str) -> Option<CapabilityState> {
        self.bindings.get(name).map(|b| b.state)
    }

    /// Check if binding exists
    pub fn binding_exists(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// Get binding history
    pub fn get_history(&self, name: &str) -> Option<Vec<(CapabilityState, u32, u32)>> {
        self.bindings.get(name).map(|b| b.history.clone())
    }

    /// Get all violations so far
    pub fn validate_all(&self) -> Vec<CapabilityViolation> {
        let mut violations = Vec::new();

        for (name, binding) in &self.bindings {
            // Check for concurrent access without sync
            if binding.accessing_threads.len() > 1 && !binding.shareable {
                violations.push(CapabilityViolation::ConcurrentUseWithoutSync {
                    var_name: name.clone(),
                    first_access: binding.defined_at,
                    second_access: binding.last_change_at,
                });
            }

            // Note: Resource leak check is deferred to exit_scope
        }

        violations
    }
}

impl Default for CapabilityContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_fresh_to_inuse() {
        let mut binding = CapabilityBinding::new("sock".to_string(), CapabilityKind::Socket, 1, 0);
        assert!(binding.transition(CapabilityState::InUse, 2, 0).is_ok());
        assert_eq!(binding.state, CapabilityState::InUse);
    }

    #[test]
    fn test_capability_inuse_to_consumed() {
        let mut binding = CapabilityBinding::new("sock".to_string(), CapabilityKind::Socket, 1, 0);
        binding.transition(CapabilityState::InUse, 2, 0).unwrap();
        assert!(binding.transition(CapabilityState::Consumed, 3, 0).is_ok());
        assert_eq!(binding.state, CapabilityState::Consumed);
    }

    #[test]
    fn test_capability_consumed_invalid_transition() {
        let mut binding = CapabilityBinding::new("sock".to_string(), CapabilityKind::Socket, 1, 0);
        binding.transition(CapabilityState::Consumed, 2, 0).unwrap();
        // Should not be able to transition out of Consumed
        assert!(binding.transition(CapabilityState::InUse, 3, 0).is_err());
    }

    #[test]
    fn test_context_define_and_use() {
        let mut ctx = CapabilityContext::new();
        ctx.set_location(1, 0);
        assert!(ctx.define_capability("sock".to_string(), CapabilityKind::Socket).is_ok());
        assert!(ctx.use_capability("sock").is_ok());
        assert_eq!(ctx.get_state("sock"), Some(CapabilityState::InUse));
    }

    #[test]
    fn test_context_use_after_consume() {
        let mut ctx = CapabilityContext::new();
        ctx.set_location(1, 0);
        ctx.define_capability("sock".to_string(), CapabilityKind::Socket).unwrap();
        ctx.set_location(2, 0);
        ctx.consume_capability("sock").unwrap();
        ctx.set_location(3, 0);
        assert!(ctx.use_capability("sock").is_err());
    }

    #[test]
    fn test_context_concurrent_access_without_sync() {
        let mut ctx = CapabilityContext::new();
        ctx.set_location(1, 0);
        ctx.define_capability("sock".to_string(), CapabilityKind::Socket).unwrap();
        ctx.use_capability("sock").unwrap();

        // Simulate access from different thread
        ctx.current_thread_id = 1;
        ctx.use_capability("sock").unwrap();

        // Should detect concurrent access
        let violations = ctx.validate_all();
        assert!(violations.iter().any(|v| matches!(v, CapabilityViolation::ConcurrentUseWithoutSync { .. })));
    }

    #[test]
    fn test_context_shared_capability() {
        let mut ctx = CapabilityContext::new();
        ctx.set_location(1, 0);
        ctx.define_capability("sock".to_string(), CapabilityKind::Socket).unwrap();
        ctx.share_capability("sock").unwrap();

        // Now should allow concurrent access
        ctx.use_capability("sock").unwrap();
        ctx.current_thread_id = 1;
        ctx.use_capability("sock").unwrap();

        // Should not report concurrent access violation
        let violations = ctx.validate_all();
        assert!(!violations.iter().any(|v| matches!(v, CapabilityViolation::ConcurrentUseWithoutSync { .. })));
    }

    #[test]
    fn test_context_define_duplicate() {
        let mut ctx = CapabilityContext::new();
        ctx.set_location(1, 0);
        ctx.define_capability("sock".to_string(), CapabilityKind::Socket).unwrap();
        ctx.set_location(2, 0);
        assert!(ctx.define_capability("sock".to_string(), CapabilityKind::Socket).is_err());
    }

    #[test]
    fn test_binding_history() {
        let mut binding = CapabilityBinding::new("sock".to_string(), CapabilityKind::Socket, 1, 0);
        binding.transition(CapabilityState::InUse, 2, 0).unwrap();
        binding.transition(CapabilityState::Consumed, 3, 0).unwrap();

        let history = binding.history;
        assert_eq!(history.len(), 3); // Fresh + InUse + Consumed
        assert_eq!(history[0].0, CapabilityState::Fresh);
        assert_eq!(history[1].0, CapabilityState::InUse);
        assert_eq!(history[2].0, CapabilityState::Consumed);
    }

    #[test]
    fn test_capability_state_can_use() {
        assert!(CapabilityState::Fresh.can_use());
        assert!(CapabilityState::InUse.can_use());
        assert!(!CapabilityState::Consumed.can_use());
        assert!(!CapabilityState::Suspended.can_use());
    }

    #[test]
    fn test_capability_state_can_share() {
        assert!(CapabilityState::Fresh.can_share());
        assert!(CapabilityState::InUse.can_share());
        assert!(!CapabilityState::Consumed.can_share());
        assert!(!CapabilityState::Suspended.can_share());
    }
}
