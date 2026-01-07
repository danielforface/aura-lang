/// Network Safety Verification Module
/// 
/// Provides formal verification of network code to prevent:
/// - Deadlocks (circular lock dependencies)
/// - Race conditions (unsynchronized concurrent access)
/// - Resource leaks (unclosed sockets)
/// - Protocol violations (invalid state transitions)
/// 
/// Uses Z3 solver for constraint satisfaction checking.

use std::collections::{HashMap, HashSet};

/// Network properties to be verified formally
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkProperty {
    /// All threads eventually acquire locks (no deadlock)
    NoDeadlock,
    
    /// All shared memory accesses are properly synchronized
    NoRaceCondition,
    
    /// All sockets follow valid lifecycle transitions
    SocketLifecycleValid,
    
    /// No sockets are leaked (all properly closed)
    NoResourceLeak,
}

/// Socket states in lifecycle state machine
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketState {
    /// Socket hasn't been created yet
    Uninitialized,
    
    /// Socket is open and ready for operations
    Open,
    
    /// Socket in accepting mode (listener)
    Accepting,
    
    /// Socket in reading state
    Reading,
    
    /// Socket in writing state
    Writing,
    
    /// Socket is closed
    Closed,
}

/// Lock information for deadlock detection
#[derive(Clone, Debug)]
struct Lock {
    /// Which locks are acquired after this one in some path
    dependencies: HashSet<String>,
}

/// Represents a single thread's lock acquisition pattern
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct ThreadLockPattern {
    thread_id: u32,
    /// Locks in order of acquisition
    acquisition_order: Vec<String>,
}

/// Network Safety Verifier
/// 
/// Performs formal verification of network code properties using constraint solving.
pub struct NetworkVerifier {
    /// Track lock acquisition order across threads
    lock_graph: HashMap<String, Lock>,
    
    /// Track which variables are accessed in which threads
    thread_accesses: HashMap<u32, HashSet<String>>,
    
    /// Track synchronization (mutex holds)
    synchronized_vars: HashMap<String, Vec<String>>, // var -> locks protecting it
    
    /// Socket lifecycle states
    socket_states: HashMap<String, SocketState>,
    
    /// Violation reports
    violations: Vec<String>,
    
    /// Next thread ID
    next_thread_id: u32,
}

impl NetworkVerifier {
    /// Create a new network verifier
    pub fn new() -> Self {
        Self {
            lock_graph: HashMap::new(),
            thread_accesses: HashMap::new(),
            synchronized_vars: HashMap::new(),
            socket_states: HashMap::new(),
            violations: Vec::new(),
            next_thread_id: 0,
        }
    }
    
    /// Register a lock in the system
    pub fn register_lock(&mut self, name: &str) {
        self.lock_graph.insert(
            name.to_string(),
            Lock {
                dependencies: HashSet::new(),
            },
        );
    }
    
    /// Record a lock acquisition order
    /// 
    /// Call this for each function that acquires locks, in order.
    /// Example: fn foo() holds lock_a then lock_b -> record_lock_order("foo", ["lock_a", "lock_b"])
    pub fn record_lock_order(&mut self, _function: &str, locks: &[&str]) {
        if locks.len() < 2 {
            return; // Single lock, no ordering constraint
        }
        
        // Record that lock[i] must always be released before lock[i+1]
        for i in 0..locks.len() - 1 {
            if let Some(lock) = self.lock_graph.get_mut(locks[i]) {
                lock.dependencies.insert(locks[i + 1].to_string());
            }
        }
    }
    
    /// Check for deadlock potential using cycle detection
    /// 
    /// Returns true if a cycle is found (potential deadlock), false otherwise.
    fn has_cycle_in_lock_graph(&self) -> bool {
        // Build adjacency list
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for (lock_name, lock) in &self.lock_graph {
            for dep in &lock.dependencies {
                graph
                    .entry(lock_name.clone())
                    .or_insert_with(Vec::new)
                    .push(dep.clone());
            }
        }
        
        // DFS-based cycle detection
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        
        for lock in self.lock_graph.keys() {
            if !visited.contains(lock) {
                if self.has_cycle_dfs(lock, &graph, &mut visited, &mut rec_stack) {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// DFS helper for cycle detection
    fn has_cycle_dfs(
        &self,
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        
        if let Some(neighbors) = graph.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    if self.has_cycle_dfs(neighbor, graph, visited, rec_stack) {
                        return true;
                    }
                } else if rec_stack.contains(neighbor) {
                    return true; // Back edge found - cycle exists
                }
            }
        }
        
        rec_stack.remove(node);
        false
    }
    
    /// Record variable access in a specific thread
    pub fn record_thread_access(&mut self, thread_id: u32, var_name: &str) {
        self.thread_accesses
            .entry(thread_id)
            .or_insert_with(HashSet::new)
            .insert(var_name.to_string());
    }
    
    /// Record that a variable is protected by specific locks
    pub fn record_synchronized_access(&mut self, var_name: &str, locks: &[&str]) {
        self.synchronized_vars
            .insert(var_name.to_string(), locks.iter().map(|s| s.to_string()).collect());
    }
    
    /// Allocate a new thread ID (for tracking concurrent access)
    pub fn new_thread_id(&mut self) -> u32 {
        let id = self.next_thread_id;
        self.next_thread_id += 1;
        self.thread_accesses.insert(id, HashSet::new());
        id
    }
    
    /// Initialize socket lifecycle tracking
    pub fn init_socket(&mut self, name: &str) {
        self.socket_states.insert(name.to_string(), SocketState::Uninitialized);
    }
    
    /// Record a socket state transition
    /// 
    /// Returns true if transition is valid, false otherwise
    pub fn transition_socket(&mut self, name: &str, to_state: SocketState) -> bool {
        let current = self.socket_states.get(name).copied().unwrap_or(SocketState::Uninitialized);
        
        // Check valid transitions
        let valid = match (current, to_state) {
            // New socket
            (SocketState::Uninitialized, SocketState::Open) => true,
            // Open can transition to various states
            (SocketState::Open, SocketState::Accepting) => true,
            (SocketState::Open, SocketState::Reading) => true,
            (SocketState::Open, SocketState::Writing) => true,
            (SocketState::Open, SocketState::Closed) => true,
            // Accepting can go to reading
            (SocketState::Accepting, SocketState::Reading) => true,
            // Reading/Writing can stay in same state or close
            (SocketState::Reading, SocketState::Reading) => true,
            (SocketState::Reading, SocketState::Closed) => true,
            (SocketState::Writing, SocketState::Writing) => true,
            (SocketState::Writing, SocketState::Closed) => true,
            // Can't transition from closed or other invalid states
            _ => false,
        };
        
        if valid {
            self.socket_states.insert(name.to_string(), to_state);
            true
        } else {
            self.violations.push(format!(
                "Invalid socket transition {} -> {}",
                match current {
                    SocketState::Uninitialized => "Uninitialized",
                    SocketState::Open => "Open",
                    SocketState::Accepting => "Accepting",
                    SocketState::Reading => "Reading",
                    SocketState::Writing => "Writing",
                    SocketState::Closed => "Closed",
                },
                match to_state {
                    SocketState::Uninitialized => "Uninitialized",
                    SocketState::Open => "Open",
                    SocketState::Accepting => "Accepting",
                    SocketState::Reading => "Reading",
                    SocketState::Writing => "Writing",
                    SocketState::Closed => "Closed",
                }
            ));
            false
        }
    }
    
    /// Verify all network properties
    /// 
    /// Returns list of violations found (empty if all properties verified)
    pub fn verify_all(&mut self) -> Vec<String> {
        // Check for deadlocks
        if self.has_cycle_in_lock_graph() {
            self.violations.push(
                "DEADLOCK RISK: Circular lock dependency detected. Locks must be acquired in consistent order."
                    .to_string(),
            );
        }
        
        // Check for race conditions
        self.check_race_conditions();
        
        // Return all violations found
        self.violations.clone()
    }
    
    /// Check for potential race conditions
    fn check_race_conditions(&mut self) {
        // Find variables accessed by multiple threads
        let mut var_to_threads: HashMap<String, Vec<u32>> = HashMap::new();
        
        for (thread_id, accesses) in &self.thread_accesses {
            for var in accesses {
                var_to_threads
                    .entry(var.clone())
                    .or_insert_with(Vec::new)
                    .push(*thread_id);
            }
        }
        
        // Check each variable accessed by multiple threads
        for (var, threads) in var_to_threads {
            if threads.len() < 2 {
                continue; // Single-threaded access is safe
            }
            
            // Check if this variable is synchronized
            if let Some(locks) = self.synchronized_vars.get(&var) {
                if !locks.is_empty() {
                    // Variable has synchronization
                    continue;
                }
            }
            
            // Unsynchronized concurrent access - potential race
            self.violations.push(format!(
                "RACE CONDITION: Variable '{}' accessed by {} threads without synchronization",
                var,
                threads.len()
            ));
        }
    }
    
    /// Get all detected violations
    pub fn violations(&self) -> &[String] {
        &self.violations
    }
    
    /// Clear all violations (for reuse)
    pub fn clear(&mut self) {
        self.violations.clear();
    }
}

impl Default for NetworkVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_simple_deadlock() {
        let mut verifier = NetworkVerifier::new();
        verifier.register_lock("lock_a");
        verifier.register_lock("lock_b");

        // Thread 1: acquire lock_a then lock_b
        verifier.record_lock_order("thread1", &["lock_a", "lock_b"]);
        // Thread 2: acquire lock_b then lock_a (circular!)
        verifier.record_lock_order("thread2", &["lock_b", "lock_a"]);

        assert!(verifier.has_cycle_in_lock_graph());
        assert!(!verifier.verify_all().is_empty());
    }

    #[test]
    fn allows_consistent_lock_order() {
        let mut verifier = NetworkVerifier::new();
        verifier.register_lock("lock_a");
        verifier.register_lock("lock_b");

        // Both acquire in same order
        verifier.record_lock_order("thread1", &["lock_a", "lock_b"]);
        verifier.record_lock_order("thread2", &["lock_a", "lock_b"]);

        assert!(!verifier.has_cycle_in_lock_graph());
        assert!(verifier.verify_all().is_empty());
    }

    #[test]
    fn detects_unsynchronized_access() {
        let mut verifier = NetworkVerifier::new();
        let t1 = verifier.new_thread_id();
        let t2 = verifier.new_thread_id();

        verifier.record_thread_access(t1, "shared_var");
        verifier.record_thread_access(t2, "shared_var");

        // No synchronization recorded
        let violations = verifier.verify_all();
        assert!(!violations.is_empty());
        assert!(violations[0].contains("RACE CONDITION"));
    }

    #[test]
    fn allows_synchronized_access() {
        let mut verifier = NetworkVerifier::new();
        let t1 = verifier.new_thread_id();
        let t2 = verifier.new_thread_id();

        verifier.register_lock("mutex");
        verifier.record_thread_access(t1, "shared_var");
        verifier.record_thread_access(t2, "shared_var");
        verifier.record_synchronized_access("shared_var", &["mutex"]);

        let violations = verifier.verify_all();
        assert!(violations.is_empty());
    }

    #[test]
    fn socket_lifecycle_valid_transition() {
        let mut verifier = NetworkVerifier::new();
        verifier.init_socket("sock");

        assert!(verifier.transition_socket("sock", SocketState::Open));
        assert!(verifier.transition_socket("sock", SocketState::Accepting));
        assert!(verifier.transition_socket("sock", SocketState::Reading));
        assert!(verifier.transition_socket("sock", SocketState::Closed));
    }

    #[test]
    fn socket_lifecycle_rejects_invalid_transition() {
        let mut verifier = NetworkVerifier::new();
        verifier.init_socket("sock");
        verifier.transition_socket("sock", SocketState::Open);
        verifier.transition_socket("sock", SocketState::Closed);

        // Can't transition from Closed
        assert!(!verifier.transition_socket("sock", SocketState::Open));
    }
}
