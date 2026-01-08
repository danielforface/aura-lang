/// Control Flow Ownership Analysis
/// 
/// Tracks ownership states through branching and merging control flow paths.
/// Ensures that ownership constraints are satisfied regardless of which path
/// execution takes.

use std::collections::HashMap;
use crate::ownership_enforcement::OwnershipState;

/// Represents a control flow path through a function.
/// 
/// Each path tracks what bindings are consumed/borrowed at that point.
#[derive(Clone, Debug)]
pub struct ControlFlowPath {
    /// Path identifier (for debugging)
    pub path_id: usize,
    /// Bindings and their states along this path
    pub bindings: HashMap<String, OwnershipState>,
    /// Whether this path is reachable
    pub reachable: bool,
}

/// Control flow graph for ownership analysis.
/// 
/// Tracks ownership states across branches (if/else, match, loops) to ensure
/// that all code paths satisfy linear type constraints.
#[derive(Clone, Debug)]
pub struct ControlFlowGraph {
    /// All paths through the current scope
    paths: Vec<ControlFlowPath>,
    /// Next path ID
    next_path_id: usize,
    /// Convergence points (where paths merge)
    convergence_points: Vec<usize>,
}

impl ControlFlowGraph {
    /// Create a new control flow graph.
    pub fn new() -> Self {
        let mut graph = ControlFlowGraph {
            paths: Vec::new(),
            next_path_id: 0,
            convergence_points: Vec::new(),
        };
        // Start with one initial path
        graph.paths.push(ControlFlowPath {
            path_id: graph.next_path_id,
            bindings: HashMap::new(),
            reachable: true,
        });
        graph.next_path_id = 1;
        graph
    }
    
    /// Create a branch point (e.g., entering an if/else).
    /// 
    /// Splits current paths into multiple branches.
    pub fn branch(&mut self) {
        // Each existing reachable path becomes two paths (true and false branches)
        let current_paths = self.paths.clone();
        self.paths.clear();
        
        for path in current_paths {
            if path.reachable {
                // True branch
                let mut true_branch = ControlFlowPath {
                    path_id: self.next_path_id,
                    bindings: path.bindings.clone(),
                    reachable: true,
                };
                self.paths.push(true_branch);
                self.next_path_id += 1;
                
                // False branch
                let false_branch = ControlFlowPath {
                    path_id: self.next_path_id,
                    bindings: path.bindings.clone(),
                    reachable: true,
                };
                self.paths.push(false_branch);
                self.next_path_id += 1;
            } else {
                // Dead code path
                self.paths.push(path);
            }
        }
    }
    
    /// Merge paths at a convergence point (e.g., after if/else).
    /// 
    /// For each binding:
    /// - If all paths agree on state, use that state
    /// - If paths disagree, the binding is in an uncertain state
    /// - If a binding is moved in some paths but not others, it's an error
    pub fn merge(&mut self) -> HashMap<String, OwnershipState> {
        if self.paths.is_empty() {
            return HashMap::new();
        }
        
        // Collect all bindings across all paths
        let mut all_bindings: HashMap<String, Vec<OwnershipState>> = HashMap::new();
        for path in &self.paths {
            if path.reachable {
                for (name, state) in &path.bindings {
                    all_bindings
                        .entry(name.clone())
                        .or_insert_with(Vec::new)
                        .push(*state);
                }
            }
        }
        
        // Determine merged state for each binding
        let mut merged = HashMap::new();
        for (name, states) in all_bindings {
            if states.is_empty() {
                continue;
            }
            
            // All states must be identical for merge to succeed
            let first = states[0];
            if states.iter().all(|&s| s == first) {
                merged.insert(name, first);
            } else {
                // Paths disagree - binding is in uncertain state
                // This is a potential error that needs reporting
                // For now, use conservative approach: assume Consumed if any path has it
                if states.iter().any(|&s| s == OwnershipState::Consumed) {
                    merged.insert(name, OwnershipState::Consumed);
                } else {
                    merged.insert(name, OwnershipState::Owned);
                }
            }
        }
        
        // Merge all paths into one
        self.paths = vec![ControlFlowPath {
            path_id: self.next_path_id,
            bindings: merged.clone(),
            reachable: self.paths.iter().any(|p| p.reachable),
        }];
        self.next_path_id += 1;
        self.convergence_points.push(self.next_path_id - 1);
        
        merged
    }
    
    /// Record a move in the current path(s).
    pub fn record_move_in_all(&mut self, binding: &str) {
        for path in &mut self.paths {
            if path.reachable {
                path.bindings.insert(binding.to_string(), OwnershipState::Consumed);
            }
        }
    }
    
    /// Record a borrow in the current path(s).
    pub fn record_borrow_in_all(&mut self, binding: &str, is_mutable: bool) {
        let state = if is_mutable {
            OwnershipState::BorrowedMut
        } else {
            OwnershipState::BorrowedImmut
        };
        for path in &mut self.paths {
            if path.reachable {
                path.bindings.insert(binding.to_string(), state);
            }
        }
    }
    
    /// Get the current merged state of a binding.
    pub fn get_binding_state(&self, name: &str) -> Option<OwnershipState> {
        if self.paths.len() == 1 {
            return self.paths[0].bindings.get(name).copied();
        }
        
        // Multiple paths - find consensus state
        let states: Vec<OwnershipState> = self.paths
            .iter()
            .filter(|p| p.reachable)
            .filter_map(|p| p.bindings.get(name).copied())
            .collect();
        
        if states.is_empty() {
            return None;
        }
        
        let first = states[0];
        if states.iter().all(|&s| s == first) {
            Some(first)
        } else {
            // Paths disagree - conservative: assume Consumed
            Some(OwnershipState::Consumed)
        }
    }
    
    /// Check that all paths moved the same bindings.
    /// 
    /// This is used to validate that linear types are consumed consistently
    /// across all branches.
    pub fn check_consistent_moves(&self, linear_bindings: &[&str]) -> Result<(), Vec<String>> {
        if self.paths.is_empty() || self.paths.iter().all(|p| !p.reachable) {
            return Ok(());
        }
        
        let mut errors = Vec::new();
        
        for binding in linear_bindings {
            let moved_count = self.paths
                .iter()
                .filter(|p| p.reachable)
                .filter(|p| p.bindings.get(*binding) == Some(&OwnershipState::Consumed))
                .count();
            
            let reachable_count = self.paths.iter().filter(|p| p.reachable).count();
            
            // Not all paths moved this binding
            if moved_count > 0 && moved_count < reachable_count {
                errors.push(format!(
                    "binding '{}' moved in {} but not {} reachable paths",
                    binding, moved_count, reachable_count - moved_count
                ));
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Get all reachable paths.
    pub fn reachable_paths(&self) -> Vec<&ControlFlowPath> {
        self.paths.iter().filter(|p| p.reachable).collect()
    }
    
    /// Mark a path as unreachable (dead code).
    pub fn mark_unreachable(&mut self, path_id: usize) {
        if let Some(path) = self.paths.iter_mut().find(|p| p.path_id == path_id) {
            path.reachable = false;
        }
    }
}

/// Ownership analysis for specific control flow structures.
pub struct OwnershipFlowAnalyzer;

impl OwnershipFlowAnalyzer {
    /// Analyze an if-then-else for ownership consistency.
    /// 
    /// Returns the merged ownership state after the if-else block.
    pub fn analyze_if_else(
        condition: &str,
        then_bindings: HashMap<String, OwnershipState>,
        else_bindings: HashMap<String, OwnershipState>,
    ) -> HashMap<String, OwnershipState> {
        let mut merged = HashMap::new();
        
        // Collect all bindings from both branches
        let mut all_names = then_bindings.keys().chain(else_bindings.keys());
        let unique_names: std::collections::HashSet<_> = all_names.collect();
        
        for name in unique_names {
            let then_state = then_bindings.get(name as &str);
            let else_state = else_bindings.get(name as &str);
            
            match (then_state, else_state) {
                (Some(&s1), Some(&s2)) if s1 == s2 => {
                    merged.insert(name.to_string(), s1);
                }
                (Some(&OwnershipState::Consumed), Some(&OwnershipState::Consumed)) => {
                    merged.insert(name.to_string(), OwnershipState::Consumed);
                }
                (Some(&OwnershipState::Consumed), Some(_)) | (Some(_), Some(&OwnershipState::Consumed)) => {
                    // One branch moved it, other didn't - ERROR
                    // Should be reported as inconsistent move
                    merged.insert(name.to_string(), OwnershipState::Consumed);
                }
                (Some(&s), None) => {
                    // Only then branch has it - could be error if else doesn't
                    merged.insert(name.to_string(), s);
                }
                (None, Some(&s)) => {
                    // Only else branch has it
                    merged.insert(name.to_string(), s);
                }
                (None, None) => {}
            }
        }
        
        merged
    }
    
    /// Analyze a match expression for ownership consistency.
    pub fn analyze_match(
        scrutinee: &str,
        branch_bindings: Vec<HashMap<String, OwnershipState>>,
    ) -> Result<HashMap<String, OwnershipState>, Vec<String>> {
        if branch_bindings.is_empty() {
            return Ok(HashMap::new());
        }
        
        let mut merged = HashMap::new();
        let mut all_names = std::collections::HashSet::new();
        
        // Collect all binding names across all branches
        for branch in &branch_bindings {
            for name in branch.keys() {
                all_names.insert(name.clone());
            }
        }
        
        let mut errors = Vec::new();
        
        for name in all_names {
            let states: Vec<Option<OwnershipState>> = branch_bindings
                .iter()
                .map(|b| b.get(&name).copied())
                .collect();
            
            // Check if all branches that have this binding agree on state
            let first_state = states.iter().find(|s| s.is_some()).copied();
            
            if let Some(Some(first)) = first_state {
                if states
                    .iter()
                    .filter(|s| s.is_some())
                    .all(|s| s == &Some(first))
                {
                    merged.insert(name, first);
                } else {
                    errors.push(format!(
                        "binding '{}' has different ownership states in different match branches",
                        name
                    ));
                }
            }
        }
        
        if errors.is_empty() {
            Ok(merged)
        } else {
            Err(errors)
        }
    }
    
    /// Analyze a loop for ownership constraints.
    /// 
    /// In a loop, variables must either:
    /// 1. Not be linear (copyable)
    /// 2. Be moved every iteration and re-created
    /// 3. Be borrowed but not moved
    pub fn analyze_loop(
        loop_bindings: HashMap<String, OwnershipState>,
        linear_bindings: Vec<&str>,
    ) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        for binding in linear_bindings {
            match loop_bindings.get(binding) {
                Some(OwnershipState::Consumed) => {
                    // Good: linear binding was consumed in loop
                    // It must be re-created each iteration
                }
                Some(OwnershipState::Owned) | None => {
                    // WARNING: linear binding not moved in loop
                    // This might indicate a logic error
                    errors.push(format!(
                        "linear binding '{}' not consumed in loop body",
                        binding
                    ));
                }
                Some(_) => {
                    // Borrowed states are fine for loops
                }
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_flow_graph_new() {
        let graph = ControlFlowGraph::new();
        assert_eq!(graph.paths.len(), 1);
        assert!(graph.paths[0].reachable);
    }

    #[test]
    fn test_control_flow_branching() {
        let mut graph = ControlFlowGraph::new();
        assert_eq!(graph.paths.len(), 1);
        
        graph.branch();
        assert_eq!(graph.paths.len(), 2);
        assert!(graph.paths.iter().all(|p| p.reachable));
    }

    #[test]
    fn test_control_flow_merge() {
        let mut graph = ControlFlowGraph::new();
        graph.record_move_in_all("x");
        graph.branch();
        
        // After merge, x should be Consumed
        let merged = graph.merge();
        assert_eq!(merged.get("x"), Some(&OwnershipState::Consumed));
    }

    #[test]
    fn test_if_else_consistent_moves() {
        let mut then_bindings = HashMap::new();
        then_bindings.insert("x".to_string(), OwnershipState::Consumed);
        
        let mut else_bindings = HashMap::new();
        else_bindings.insert("x".to_string(), OwnershipState::Consumed);
        
        let merged = OwnershipFlowAnalyzer::analyze_if_else("cond", then_bindings, else_bindings);
        assert_eq!(merged.get("x"), Some(&OwnershipState::Consumed));
    }

    #[test]
    fn test_match_consistent_moves() {
        let mut branch1 = HashMap::new();
        branch1.insert("x".to_string(), OwnershipState::Consumed);
        
        let mut branch2 = HashMap::new();
        branch2.insert("x".to_string(), OwnershipState::Consumed);
        
        let result = OwnershipFlowAnalyzer::analyze_match("val", vec![branch1, branch2]);
        assert!(result.is_ok());
        let merged = result.unwrap();
        assert_eq!(merged.get("x"), Some(&OwnershipState::Consumed));
    }

    #[test]
    fn test_consistent_moves_check() {
        let mut path1 = ControlFlowPath {
            path_id: 0,
            bindings: HashMap::new(),
            reachable: true,
        };
        path1.bindings.insert("model".to_string(), OwnershipState::Consumed);
        
        let mut path2 = ControlFlowPath {
            path_id: 1,
            bindings: HashMap::new(),
            reachable: true,
        };
        path2.bindings.insert("model".to_string(), OwnershipState::Consumed);
        
        let mut graph = ControlFlowGraph {
            paths: vec![path1, path2],
            next_path_id: 2,
            convergence_points: Vec::new(),
        };
        
        let result = graph.check_consistent_moves(&["model"]);
        assert!(result.is_ok());
    }
}
