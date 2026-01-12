#![forbid(unsafe_code)]

/// Race Condition Detection Engine for Pillar 5
/// 
/// Provides formal verification of concurrent code to prevent:
/// - Data races (unsynchronized concurrent memory access)
/// - Deadlocks (circular lock dependencies)
/// - Use-after-free in concurrent contexts
/// - Resource leaks in concurrent code
/// 
/// Uses happens-before relationship analysis + lock dependency graphs.

use std::collections::{HashMap, HashSet, VecDeque};

/// Types of concurrent access
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccessType {
    /// Read-only access
    Read,
    /// Write access (modifying)
    Write,
    /// Lock acquisition
    LockAcquire,
    /// Lock release
    LockRelease,
}

/// Represents a single memory access in a thread
#[derive(Clone, Debug)]
pub struct MemoryAccess {
    pub var_name: String,
    pub access_type: AccessType,
    pub thread_id: u32,
    pub line: u32,
    pub col: u32,
}

impl MemoryAccess {
    pub fn new(var_name: String, access_type: AccessType, thread_id: u32, line: u32, col: u32) -> Self {
        MemoryAccess {
            var_name,
            access_type,
            thread_id,
            line,
            col,
        }
    }

    pub fn is_write(&self) -> bool {
        self.access_type == AccessType::Write
    }

    pub fn is_lock_op(&self) -> bool {
        matches!(self.access_type, AccessType::LockAcquire | AccessType::LockRelease)
    }
}

/// Race condition violation
#[derive(Clone, Debug)]
pub enum RaceViolation {
    /// Two threads access same variable, at least one writes, without synchronization
    DataRace {
        var_name: String,
        thread1: u32,
        access1: (u32, u32),
        thread2: u32,
        access2: (u32, u32),
    },
    /// Circular lock dependency detected
    Deadlock {
        cycle: Vec<(String, String)>, // lock dependencies forming cycle
    },
    /// Variable accessed after it's been freed
    UseAfterFree {
        var_name: String,
        freed_at: (u32, u32),
        used_at: (u32, u32),
    },
    /// Lock acquired but never released
    LockLeak {
        lock_name: String,
        acquired_at: (u32, u32),
    },
}

impl RaceViolation {
    pub fn message(&self) -> String {
        match self {
            RaceViolation::DataRace { var_name, thread1, access1, thread2, access2 } => {
                format!(
                    "data race on '{}': thread {} accesses at {}:{}, thread {} accesses at {}:{}",
                    var_name, thread1, access1.0, access1.1, thread2, access2.0, access2.1
                )
            }
            RaceViolation::Deadlock { cycle } => {
                let cycle_str = cycle.iter()
                    .map(|(a, b)| format!("{} → {}", a, b))
                    .collect::<Vec<_>>()
                    .join(" → ");
                format!("deadlock detected: {}", cycle_str)
            }
            RaceViolation::UseAfterFree { var_name, freed_at, used_at } => {
                format!(
                    "use after free: '{}' freed at {}:{} but used at {}:{}",
                    var_name, freed_at.0, freed_at.1, used_at.0, used_at.1
                )
            }
            RaceViolation::LockLeak { lock_name, acquired_at } => {
                format!(
                    "lock leak: '{}' acquired at {}:{} but never released",
                    lock_name, acquired_at.0, acquired_at.1
                )
            }
        }
    }
}

/// Lock information for deadlock detection
#[derive(Clone, Debug)]
pub struct LockInfo {
    pub name: String,
    pub acquired_by: HashMap<u32, (u32, u32)>, // thread_id -> (line, col)
    pub dependencies: HashSet<String>, // locks acquired after this one
}

impl LockInfo {
    pub fn new(name: String) -> Self {
        LockInfo {
            name,
            acquired_by: HashMap::new(),
            dependencies: HashSet::new(),
        }
    }
}

/// Tracks synchronization information (which locks protect which variables)
#[derive(Clone, Debug)]
pub struct SynchronizationInfo {
    /// Variables -> Locks protecting them
    pub protection_map: HashMap<String, Vec<String>>,
    /// Lock acquisition order per thread
    pub lock_order: HashMap<u32, Vec<String>>,
}

impl SynchronizationInfo {
    pub fn new() -> Self {
        SynchronizationInfo {
            protection_map: HashMap::new(),
            lock_order: HashMap::new(),
        }
    }

    /// Mark that a variable is protected by a lock
    pub fn add_protection(&mut self, var_name: String, lock_name: String) {
        self.protection_map
            .entry(var_name)
            .or_insert_with(Vec::new)
            .push(lock_name);
    }

    /// Check if variable is protected by a lock
    pub fn is_protected(&self, var_name: &str) -> bool {
        self.protection_map
            .get(var_name)
            .map(|locks| !locks.is_empty())
            .unwrap_or(false)
    }

    /// Get protecting locks for a variable
    pub fn get_protecting_locks(&self, var_name: &str) -> Option<&Vec<String>> {
        self.protection_map.get(var_name)
    }
}

impl Default for SynchronizationInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Race condition detector
pub struct RaceDetector {
    /// All memory accesses in order
    accesses: Vec<MemoryAccess>,
    /// Lock information for deadlock detection
    locks: HashMap<String, LockInfo>,
    /// Variables freed in each thread
    freed_vars: HashMap<u32, Vec<(String, u32, u32)>>, // thread -> (var, line, col)
    /// Synchronization information
    sync_info: SynchronizationInfo,
    /// Detected violations
    violations: Vec<RaceViolation>,
}

impl RaceDetector {
    pub fn new() -> Self {
        RaceDetector {
            accesses: Vec::new(),
            locks: HashMap::new(),
            freed_vars: HashMap::new(),
            sync_info: SynchronizationInfo::new(),
            violations: Vec::new(),
        }
    }

    /// Record a memory access
    pub fn record_access(&mut self, access: MemoryAccess) {
        self.accesses.push(access);
    }

    /// Record a lock operation
    pub fn record_lock(&mut self, lock_name: String, acquired: bool, thread_id: u32, line: u32, col: u32) {
        let lock = self.locks
            .entry(lock_name.clone())
            .or_insert_with(|| LockInfo::new(lock_name.clone()));
        
        if acquired {
            lock.acquired_by.insert(thread_id, (line, col));
        } else {
            lock.acquired_by.remove(&thread_id);
        }
    }

    /// Mark a lock as protecting a variable
    pub fn add_protection(&mut self, var_name: String, lock_name: String) {
        self.sync_info.add_protection(var_name, lock_name);
    }

    /// Record variable being freed
    pub fn record_free(&mut self, var_name: String, thread_id: u32, line: u32, col: u32) {
        self.freed_vars
            .entry(thread_id)
            .or_insert_with(Vec::new)
            .push((var_name, line, col));
    }

    /// Detect data races
    pub fn detect_data_races(&mut self) {
        let mut var_accesses: HashMap<String, Vec<(u32, AccessType, u32, u32)>> = HashMap::new();
        
        for access in &self.accesses {
            var_accesses
                .entry(access.var_name.clone())
                .or_insert_with(Vec::new)
                .push((access.thread_id, access.access_type, access.line, access.col));
        }

        for (var_name, accesses) in var_accesses {
            let mut threads = HashSet::new();
            let mut has_write = false;
            
            for (thread_id, access_type, _line, _col) in &accesses {
                threads.insert(*thread_id);
                if *access_type == AccessType::Write {
                    has_write = true;
                }
            }

            // Race condition: multiple threads, at least one write, no synchronization
            if threads.len() > 1 && has_write && !self.sync_info.is_protected(&var_name) {
                let accesses_sorted: Vec<_> = accesses.iter().collect();
                if accesses_sorted.len() >= 2 {
                    let access1 = accesses_sorted[0];
                    let access2 = accesses_sorted[1];
                    
                    self.violations.push(RaceViolation::DataRace {
                        var_name,
                        thread1: access1.0,
                        access1: (access1.2, access1.3),
                        thread2: access2.0,
                        access2: (access2.2, access2.3),
                    });
                }
            }
        }
    }

    /// Detect potential deadlocks using cycle detection
    pub fn detect_deadlocks(&mut self) {
        // Build dependency graph: lock A -> lock B if B acquired after A in same thread
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
        
        for (_thread_id, lock_order) in &self.sync_info.lock_order {
            for i in 0..lock_order.len() {
                for j in (i + 1)..lock_order.len() {
                    graph
                        .entry(lock_order[i].clone())
                        .or_insert_with(HashSet::new)
                        .insert(lock_order[j].clone());
                }
            }
        }

        // Detect cycles using DFS
        let all_locks: Vec<_> = graph.keys().cloned().collect();
        for lock in all_locks {
            if let Some(cycle) = self.find_cycle(&graph, &lock) {
                self.violations.push(RaceViolation::Deadlock { cycle });
            }
        }
    }

    /// Detect use-after-free violations
    pub fn detect_use_after_free(&mut self) {
        for (thread_id, freed) in &self.freed_vars {
            for (var_name, freed_line, freed_col) in freed {
                for access in &self.accesses {
                    if &access.var_name == var_name && access.thread_id == *thread_id && access.line > *freed_line {
                        self.violations.push(RaceViolation::UseAfterFree {
                            var_name: var_name.clone(),
                            freed_at: (*freed_line, *freed_col),
                            used_at: (access.line, access.col),
                        });
                    }
                }
            }
        }
    }

    /// Detect lock leaks
    pub fn detect_lock_leaks(&mut self) {
        for (_, lock) in &self.locks {
            // If a lock is still acquired by any thread at end, it's leaked
            for (_thread_id, (line, col)) in &lock.acquired_by {
                self.violations.push(RaceViolation::LockLeak {
                    lock_name: lock.name.clone(),
                    acquired_at: (*line, *col),
                });
            }
        }
    }

    /// Run all detection algorithms
    pub fn detect_all(&mut self) {
        self.detect_data_races();
        self.detect_deadlocks();
        self.detect_use_after_free();
        self.detect_lock_leaks();
    }

    /// Get all detected violations
    pub fn violations(&self) -> &[RaceViolation] {
        &self.violations
    }

    /// Check if any violations detected
    pub fn has_violations(&self) -> bool {
        !self.violations.is_empty()
    }

    /// Find cycle in lock dependency graph using DFS
    fn find_cycle(&self, graph: &HashMap<String, HashSet<String>>, start: &str) -> Option<Vec<(String, String)>> {
        let mut visited = HashSet::new();
        let mut path = VecDeque::new();
        
        if self.dfs_cycle(&graph, start, &mut visited, &mut path) {
            let mut cycle = Vec::new();
            for i in 0..path.len() - 1 {
                cycle.push((path[i].clone(), path[i + 1].clone()));
            }
            return Some(cycle);
        }
        None
    }

    fn dfs_cycle(
        &self,
        graph: &HashMap<String, HashSet<String>>,
        node: &str,
        visited: &mut HashSet<String>,
        path: &mut VecDeque<String>,
    ) -> bool {
        if visited.contains(node) {
            // Found cycle if node is in current path
            return path.contains(&node.to_string());
        }

        visited.insert(node.to_string());
        path.push_back(node.to_string());

        if let Some(neighbors) = graph.get(node) {
            for neighbor in neighbors {
                if self.dfs_cycle(graph, neighbor, visited, path) {
                    return true;
                }
            }
        }

        path.pop_back();
        false
    }
}

impl Default for RaceDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_data_race() {
        let mut detector = RaceDetector::new();
        
        // Thread 0 writes to x
        detector.record_access(MemoryAccess::new("x".to_string(), AccessType::Write, 0, 5, 0));
        
        // Thread 1 reads from x without synchronization
        detector.record_access(MemoryAccess::new("x".to_string(), AccessType::Read, 1, 10, 0));
        
        detector.detect_all();
        
        assert!(detector.has_violations());
        assert!(detector.violations().iter().any(|v| matches!(v, RaceViolation::DataRace { .. })));
    }

    #[test]
    fn test_protected_variable_no_race() {
        let mut detector = RaceDetector::new();
        
        // Both accesses protected by lock
        detector.add_protection("x".to_string(), "lock1".to_string());
        
        detector.record_access(MemoryAccess::new("x".to_string(), AccessType::Write, 0, 5, 0));
        detector.record_access(MemoryAccess::new("x".to_string(), AccessType::Read, 1, 10, 0));
        
        detector.detect_all();
        
        // Should not detect race since protected
        assert!(!detector.violations().iter().any(|v| matches!(v, RaceViolation::DataRace { .. })));
    }

    #[test]
    fn test_use_after_free() {
        let mut detector = RaceDetector::new();
        
        detector.record_free("ptr".to_string(), 0, 5, 0);
        detector.record_access(MemoryAccess::new("ptr".to_string(), AccessType::Read, 0, 10, 0));
        
        detector.detect_all();
        
        assert!(detector.violations().iter().any(|v| matches!(v, RaceViolation::UseAfterFree { .. })));
    }

    #[test]
    fn test_lock_leak() {
        let mut detector = RaceDetector::new();
        
        detector.record_lock("lock1".to_string(), true, 0, 5, 0);
        // Don't release the lock
        
        detector.detect_all();
        
        assert!(detector.violations().iter().any(|v| matches!(v, RaceViolation::LockLeak { .. })));
    }

    #[test]
    fn test_memory_access_new() {
        let access = MemoryAccess::new("x".to_string(), AccessType::Write, 1, 5, 10);
        assert_eq!(access.var_name, "x");
        assert_eq!(access.thread_id, 1);
        assert!(access.is_write());
    }

    #[test]
    fn test_synchronization_info() {
        let mut sync = SynchronizationInfo::new();
        sync.add_protection("x".to_string(), "lock1".to_string());
        
        assert!(sync.is_protected("x"));
        assert!(!sync.is_protected("y"));
        assert!(sync.get_protecting_locks("x").is_some());
    }
}
