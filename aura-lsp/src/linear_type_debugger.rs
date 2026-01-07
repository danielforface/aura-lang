/// Phase 2 Week 3: Pillar 3 (Memory Safety) Type System Integration
///
/// Bridges linear type tracking to the debugger for real-time memory state visualization.
/// 
/// Features:
/// 1. Track ownership state (moved, borrowed, alive)
/// 2. Display linear type invariants in debugger
/// 3. Detect use-after-move at runtime
/// 4. Visualize region lifetimes
/// 5. Verify stdlib (Vec, HashMap, Mutex, etc)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Linear type state for a variable
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwnershipState {
    Alive,        // Not yet moved
    Moved,        // Ownership transferred
    Borrowed,     // Currently borrowed
    EndOfLife,    // Dropped/scoped out
}

/// Ownership tracking for a single variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipTracker {
    pub name: String,
    pub type_name: String,
    pub current_state: OwnershipState,
    pub state_transitions: Vec<StateTransition>,
    pub borrowed_by: Vec<BorrowInfo>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from_state: OwnershipState,
    pub to_state: OwnershipState,
    pub at_line: usize,
    pub at_column: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorrowInfo {
    pub borrowed_at_line: usize,
    pub borrowed_at_column: usize,
    pub borrow_type: String, // "immutable", "mutable"
    pub borrowed_until: BorrowEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BorrowEnd {
    AtLine(usize),
    EndOfScope,
}

/// Region lifetime information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionInfo {
    pub name: String,
    pub created_at_line: usize,
    pub created_at_column: usize,
    pub destroyed_at_line: usize,
    pub destroyed_at_column: usize,
    pub allocated_bytes: usize,
    pub allocations: Vec<AllocationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationInfo {
    pub variable_name: String,
    pub size_bytes: usize,
    pub allocated_at: (usize, usize),
    pub deallocated_at: Option<(usize, usize)>,
}

/// Stdlib verified data structure state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedDataStructureState {
    pub variable_name: String,
    pub type_name: String, // "Vec[T]", "HashMap[K,V]", "Mutex[T]", etc
    pub invariants: Vec<DataStructureInvariant>,
    pub verified: bool,
    pub verification_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataStructureInvariant {
    pub name: String,
    pub description: String,
    pub holds: bool,
    pub details: String,
}

/// Use-after-move error detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseAfterMoveError {
    pub variable_name: String,
    pub moved_at_line: usize,
    pub use_at_line: usize,
    pub use_at_column: usize,
    pub severity: String, // "error", "warning"
}

/// Type system debugger integration
pub struct LinearTypeDebugger {
    ownership_state: HashMap<String, OwnershipTracker>,
    regions: HashMap<String, RegionInfo>,
    data_structures: HashMap<String, VerifiedDataStructureState>,
    errors: Vec<UseAfterMoveError>,
}

impl LinearTypeDebugger {
    pub fn new() -> Self {
        LinearTypeDebugger {
            ownership_state: HashMap::new(),
            regions: HashMap::new(),
            data_structures: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Track ownership transition
    pub fn record_ownership_transition(
        &mut self,
        var_name: String,
        from: OwnershipState,
        to: OwnershipState,
        line: usize,
        column: usize,
        reason: String,
    ) {
        let tracker = self
            .ownership_state
            .entry(var_name.clone())
            .or_insert(OwnershipTracker {
                name: var_name,
                type_name: "unknown".to_string(),
                current_state: from,
                state_transitions: Vec::new(),
                borrowed_by: Vec::new(),
                region: None,
            });

        tracker.state_transitions.push(StateTransition {
            from_state: from,
            to_state: to,
            at_line: line,
            at_column: column,
            reason,
        });

        tracker.current_state = to;
    }

    /// Record a borrow
    pub fn record_borrow(
        &mut self,
        var_name: &str,
        borrow_type: String,
        at_line: usize,
        at_column: usize,
        until: BorrowEnd,
    ) {
        if let Some(tracker) = self.ownership_state.get_mut(var_name) {
            tracker.borrowed_by.push(BorrowInfo {
                borrowed_at_line: at_line,
                borrowed_at_column: at_column,
                borrow_type,
                borrowed_until: until,
            });
            tracker.current_state = OwnershipState::Borrowed;
        }
    }

    /// Create region
    pub fn create_region(
        &mut self,
        name: String,
        at_line: usize,
        at_column: usize,
    ) {
        self.regions.insert(
            name.clone(),
            RegionInfo {
                name,
                created_at_line: at_line,
                created_at_column: at_column,
                destroyed_at_line: 0,
                destroyed_at_column: 0,
                allocated_bytes: 0,
                allocations: Vec::new(),
            },
        );
    }

    /// Allocate in region
    pub fn allocate_in_region(
        &mut self,
        region_name: &str,
        var_name: String,
        size_bytes: usize,
        at_line: usize,
        at_column: usize,
    ) {
        if let Some(region) = self.regions.get_mut(region_name) {
            region.allocated_bytes += size_bytes;
            region.allocations.push(AllocationInfo {
                variable_name: var_name,
                size_bytes,
                allocated_at: (at_line, at_column),
                deallocated_at: None,
            });
        }
    }

    /// Deallocate in region
    pub fn deallocate_in_region(
        &mut self,
        region_name: &str,
        var_name: &str,
        at_line: usize,
        at_column: usize,
    ) {
        if let Some(region) = self.regions.get_mut(region_name) {
            for alloc in &mut region.allocations {
                if alloc.variable_name == var_name && alloc.deallocated_at.is_none() {
                    alloc.deallocated_at = Some((at_line, at_column));
                    region.allocated_bytes = region.allocated_bytes.saturating_sub(alloc.size_bytes);
                }
            }
        }
    }

    /// Track verified data structure state
    pub fn track_data_structure(
        &mut self,
        var_name: String,
        type_name: String,
        invariants: Vec<DataStructureInvariant>,
    ) {
        let verified = invariants.iter().all(|inv| inv.holds);

        self.data_structures.insert(
            var_name.clone(),
            VerifiedDataStructureState {
                variable_name: var_name,
                type_name,
                invariants,
                verified,
                verification_error: None,
            },
        );
    }

    /// Detect use-after-move
    pub fn check_use_after_move(
        &mut self,
        var_name: &str,
        use_line: usize,
        use_column: usize,
    ) -> bool {
        if let Some(tracker) = self.ownership_state.get(var_name) {
            if tracker.current_state == OwnershipState::Moved {
                // Find the line where it was moved
                let moved_at = tracker
                    .state_transitions
                    .iter()
                    .find(|t| t.to_state == OwnershipState::Moved)
                    .map(|t| t.at_line)
                    .unwrap_or(0);

                self.errors.push(UseAfterMoveError {
                    variable_name: var_name.to_string(),
                    moved_at_line: moved_at,
                    use_at_line: use_line,
                    use_at_column: use_column,
                    severity: "error".to_string(),
                });

                return true; // Error detected
            }
        }

        false
    }

    /// Get ownership state for debugger display
    pub fn get_ownership_display(&self, var_name: &str) -> Option<OwnershipDisplay> {
        self.ownership_state.get(var_name).map(|tracker| {
            let state_str = match tracker.current_state {
                OwnershipState::Alive => "🟢 Alive".to_string(),
                OwnershipState::Moved => "❌ Moved".to_string(),
                OwnershipState::Borrowed => "📋 Borrowed".to_string(),
                OwnershipState::EndOfLife => "⚪ End of Life".to_string(),
            };

            OwnershipDisplay {
                variable_name: tracker.name.clone(),
                type_name: tracker.type_name.clone(),
                state: state_str,
                transitions: tracker.state_transitions.clone(),
                borrows: tracker.borrowed_by.clone(),
            }
        })
    }

    /// Get region memory usage for debugger display
    pub fn get_region_memory_display(&self, region_name: &str) -> Option<RegionMemoryDisplay> {
        self.regions.get(region_name).map(|region| {
            let allocations_summary = region
                .allocations
                .iter()
                .map(|a| format!("{}: {}B", a.variable_name, a.size_bytes))
                .collect();

            RegionMemoryDisplay {
                region_name: region.name.clone(),
                total_allocated: region.allocated_bytes,
                allocation_count: region.allocations.len(),
                allocations: allocations_summary,
            }
        })
    }

    /// Get all use-after-move errors
    pub fn get_errors(&self) -> Vec<UseAfterMoveError> {
        self.errors.clone()
    }
}

// ========== DEBUGGER DISPLAY MODELS ==========

#[derive(Debug, Clone, Serialize)]
pub struct OwnershipDisplay {
    pub variable_name: String,
    pub type_name: String,
    pub state: String,
    pub transitions: Vec<StateTransition>,
    pub borrows: Vec<BorrowInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegionMemoryDisplay {
    pub region_name: String,
    pub total_allocated: usize,
    pub allocation_count: usize,
    pub allocations: Vec<String>,
}

// ========== TESTS ==========

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ownership_transition_tracking() {
        let mut debugger = LinearTypeDebugger::new();

        debugger.record_ownership_transition(
            "x".to_string(),
            OwnershipState::Alive,
            OwnershipState::Moved,
            10,
            5,
            "passed to function".to_string(),
        );

        let state = debugger.get_ownership_display("x");
        assert!(state.is_some());
        assert_eq!(state.unwrap().state, "❌ Moved");
    }

    #[test]
    fn test_use_after_move_detection() {
        let mut debugger = LinearTypeDebugger::new();

        // Record move
        debugger.record_ownership_transition(
            "x".to_string(),
            OwnershipState::Alive,
            OwnershipState::Moved,
            10,
            5,
            "moved".to_string(),
        );

        // Try to use after move
        let is_error = debugger.check_use_after_move("x", 15, 3);
        assert!(is_error);
        assert_eq!(debugger.get_errors().len(), 1);
    }

    #[test]
    fn test_borrow_tracking() {
        let mut debugger = LinearTypeDebugger::new();

        debugger.record_ownership_transition(
            "x".to_string(),
            OwnershipState::Alive,
            OwnershipState::Alive,
            5,
            0,
            "declaration".to_string(),
        );

        debugger.record_borrow(
            "x",
            "immutable".to_string(),
            10,
            5,
            BorrowEnd::AtLine(15),
        );

        let display = debugger.get_ownership_display("x");
        assert!(display.is_some());
        assert_eq!(display.unwrap().borrows.len(), 1);
    }

    #[test]
    fn test_region_allocation() {
        let mut debugger = LinearTypeDebugger::new();

        debugger.create_region("r1".to_string(), 5, 0);
        debugger.allocate_in_region("r1", "v1".to_string(), 100, 6, 0);
        debugger.allocate_in_region("r1", "v2".to_string(), 200, 7, 0);

        let display = debugger.get_region_memory_display("r1");
        assert!(display.is_some());
        assert_eq!(display.unwrap().total_allocated, 300);
    }

    #[test]
    fn test_data_structure_verification() {
        let mut debugger = LinearTypeDebugger::new();

        let invariants = vec![
            DataStructureInvariant {
                name: "len <= capacity".to_string(),
                description: "Vector length never exceeds capacity".to_string(),
                holds: true,
                details: "10 <= 20".to_string(),
            },
            DataStructureInvariant {
                name: "no double free".to_string(),
                description: "Pointer only freed once".to_string(),
                holds: true,
                details: "freed at line 45 only".to_string(),
            },
        ];

        debugger.track_data_structure(
            "vec".to_string(),
            "Vec[i32]".to_string(),
            invariants,
        );

        let display = debugger.ownership_state.get("vec");
        assert!(display.is_none()); // Data structures stored separately

        let ds = debugger.data_structures.get("vec");
        assert!(ds.is_some());
        assert!(ds.unwrap().verified);
    }
}
