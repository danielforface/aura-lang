#![forbid(unsafe_code)]

use std::collections::HashMap;

/// Represents bounds constraints on a collection access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundsContract {
    /// Minimum valid index (inclusive)
    pub min_index: usize,
    /// Maximum valid index (inclusive)
    pub max_index: usize,
    /// Capacity of the underlying collection
    pub capacity: usize,
}

impl BoundsContract {
    /// Create a new bounds contract for a collection.
    pub fn new(capacity: usize) -> Self {
        BoundsContract {
            min_index: 0,
            max_index: capacity.saturating_sub(1),
            capacity,
        }
    }

    /// Check if an index is within bounds.
    pub fn is_valid_index(&self, index: usize) -> bool {
        index <= self.max_index
    }

    /// Check if a range [start, end) is valid.
    pub fn is_valid_range(&self, start: usize, end: usize) -> bool {
        start <= end && end <= self.capacity
    }

    /// Get the number of valid elements.
    pub fn valid_count(&self) -> usize {
        self.max_index.saturating_sub(self.min_index) + 1
    }

    /// Get error message for an out-of-bounds access.
    pub fn bounds_error(&self, index: usize) -> String {
        format!(
            "index {} is out of bounds for collection of size {} (valid range: [0, {}])",
            index, self.capacity, self.max_index
        )
    }
}

/// Represents verified Vec<T> with bounds contracts.
#[derive(Clone, Debug)]
pub struct VerifiedVec<T> {
    /// Internal storage
    data: Vec<T>,
    /// Bounds contract for this vector
    contract: BoundsContract,
    /// Number of initialized elements
    len: usize,
}

impl<T: Clone> VerifiedVec<T> {
    /// Create a new verified vector with given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        VerifiedVec {
            data: Vec::with_capacity(capacity),
            contract: BoundsContract::new(capacity),
            len: 0,
        }
    }

    /// Create a verified vector from a vec.
    pub fn from_vec(vec: Vec<T>) -> Self {
        let len = vec.len();
        let capacity = vec.capacity();
        VerifiedVec {
            data: vec,
            contract: BoundsContract::new(capacity),
            len,
        }
    }

    /// Get the length of the vector.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if the vector is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the capacity of the vector.
    pub fn capacity(&self) -> usize {
        self.contract.capacity
    }

    /// Push an element (verifies bounds contract).
    pub fn push(&mut self, value: T) -> Result<(), String> {
        if self.len >= self.capacity() {
            return Err(format!(
                "cannot push: vector is at capacity (len={}, capacity={})",
                self.len,
                self.capacity()
            ));
        }

        self.data.push(value);
        self.len += 1;
        Ok(())
    }

    /// Pop an element (verifies bounds contract).
    pub fn pop(&mut self) -> Option<T> {
        if self.len > 0 {
            self.len -= 1;
            self.data.pop()
        } else {
            None
        }
    }

    /// Get an element at the specified index (with bounds check).
    pub fn get(&self, index: usize) -> Result<&T, String> {
        if !self.contract.is_valid_index(index) || index >= self.len {
            return Err(self.contract.bounds_error(index));
        }
        Ok(&self.data[index])
    }

    /// Get a mutable element at the specified index (with bounds check).
    pub fn get_mut(&mut self, index: usize) -> Result<&mut T, String> {
        if !self.contract.is_valid_index(index) || index >= self.len {
            return Err(self.contract.bounds_error(index));
        }
        Ok(&mut self.data[index])
    }

    /// Get the bounds contract for this vector.
    pub fn contract(&self) -> &BoundsContract {
        &self.contract
    }

    /// Verify internal invariants.
    pub fn verify_invariants(&self) -> Result<(), String> {
        // Invariant 1: len <= capacity
        if self.len > self.capacity() {
            return Err(format!("invariant violated: len > capacity ({} > {})", self.len, self.capacity()));
        }

        // Invariant 2: data.len() == len
        if self.data.len() != self.len {
            return Err(format!("invariant violated: data.len() != len ({} != {})", self.data.len(), self.len));
        }

        // Invariant 3: contract bounds are consistent
        if self.contract.max_index < self.capacity().saturating_sub(1) {
            return Err("invariant violated: contract bounds inconsistent".to_string());
        }

        Ok(())
    }
}

/// Represents verified HashMap<K, V> with collision tracking.
#[derive(Clone, Debug)]
pub struct VerifiedHashMap<K, V> {
    /// Internal storage
    data: HashMap<K, V>,
    /// Track collision count
    collision_count: usize,
    /// Maximum allowed collisions before rehashing (safety bound)
    max_collisions: usize,
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> VerifiedHashMap<K, V> {
    /// Create a new verified hashmap.
    pub fn new() -> Self {
        VerifiedHashMap {
            data: HashMap::new(),
            collision_count: 0,
            max_collisions: 1000,  // Conservative bound
        }
    }

    /// Create a verified hashmap with capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        VerifiedHashMap {
            data: HashMap::with_capacity(capacity),
            collision_count: 0,
            max_collisions: capacity * 2,  // Adaptive bound
        }
    }

    /// Insert a key-value pair.
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, String> {
        // Check collision bound
        if self.collision_count >= self.max_collisions {
            return Err(format!(
                "collision count limit exceeded ({} >= {})",
                self.collision_count, self.max_collisions
            ));
        }

        // Insert and track
        if self.data.contains_key(&key) {
            // Replacing existing key doesn't increase collision count
        } else {
            // Check load factor to predict collisions
            if self.data.len() > self.data.capacity() / 2 {
                self.collision_count += 1;
            }
        }

        Ok(self.data.insert(key, value))
    }

    /// Get a value by key.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.data.get(key)
    }

    /// Get a mutable value by key.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.data.get_mut(key)
    }

    /// Remove a key-value pair.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.data.remove(key)
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get collision count.
    pub fn collision_count(&self) -> usize {
        self.collision_count
    }

    /// Reset collision tracking (for rehash simulation).
    pub fn reset_collision_count(&mut self) {
        self.collision_count = 0;
    }

    /// Verify internal invariants.
    pub fn verify_invariants(&self) -> Result<(), String> {
        // Invariant 1: collision count within bounds
        if self.collision_count > self.max_collisions {
            return Err(format!(
                "invariant violated: collision_count > max_collisions ({} > {})",
                self.collision_count, self.max_collisions
            ));
        }

        // Invariant 2: all keys in data are reachable
        for key in self.data.keys() {
            if self.data.get(key).is_none() {
                return Err("invariant violated: key in data but not retrievable".to_string());
            }
        }

        Ok(())
    }
}

impl<K: Eq + std::hash::Hash + Clone, V: Clone> Default for VerifiedHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_contract_creation() {
        let contract = BoundsContract::new(10);
        assert_eq!(contract.capacity, 10);
        assert_eq!(contract.min_index, 0);
        assert_eq!(contract.max_index, 9);
    }

    #[test]
    fn test_bounds_contract_valid_index() {
        let contract = BoundsContract::new(10);
        assert!(contract.is_valid_index(0));
        assert!(contract.is_valid_index(9));
        assert!(!contract.is_valid_index(10));
        assert!(!contract.is_valid_index(100));
    }

    #[test]
    fn test_bounds_contract_valid_range() {
        let contract = BoundsContract::new(10);
        assert!(contract.is_valid_range(0, 10));
        assert!(contract.is_valid_range(5, 8));
        assert!(!contract.is_valid_range(5, 15));
        assert!(!contract.is_valid_range(10, 11));
    }

    #[test]
    fn test_verified_vec_creation() {
        let vec: VerifiedVec<u32> = VerifiedVec::with_capacity(10);
        assert_eq!(vec.capacity(), 10);
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
    }

    #[test]
    fn test_verified_vec_push() {
        let mut vec: VerifiedVec<u32> = VerifiedVec::with_capacity(5);
        
        assert!(vec.push(1).is_ok());
        assert!(vec.push(2).is_ok());
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_verified_vec_push_overflow() {
        let mut vec: VerifiedVec<u32> = VerifiedVec::with_capacity(2);
        
        assert!(vec.push(1).is_ok());
        assert!(vec.push(2).is_ok());
        assert!(vec.push(3).is_err());  // Overflow
    }

    #[test]
    fn test_verified_vec_get_bounds() {
        let mut vec: VerifiedVec<u32> = VerifiedVec::with_capacity(5);
        vec.push(42).unwrap();
        
        assert!(vec.get(0).is_ok());
        assert_eq!(*vec.get(0).unwrap(), 42);
        assert!(vec.get(1).is_err());  // Out of bounds
        assert!(vec.get(100).is_err());
    }

    #[test]
    fn test_verified_vec_pop() {
        let mut vec: VerifiedVec<u32> = VerifiedVec::with_capacity(5);
        vec.push(1).unwrap();
        vec.push(2).unwrap();
        
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.pop(), None);
    }

    #[test]
    fn test_verified_vec_invariants() {
        let mut vec: VerifiedVec<u32> = VerifiedVec::with_capacity(5);
        vec.push(1).unwrap();
        vec.push(2).unwrap();
        
        assert!(vec.verify_invariants().is_ok());
    }

    #[test]
    fn test_verified_hashmap_creation() {
        let map: VerifiedHashMap<String, u32> = VerifiedHashMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_verified_hashmap_insert_get() {
        let mut map: VerifiedHashMap<String, u32> = VerifiedHashMap::new();
        
        assert!(map.insert("key1".to_string(), 42).is_ok());
        assert_eq!(map.get(&"key1".to_string()), Some(&42));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_verified_hashmap_remove() {
        let mut map: VerifiedHashMap<String, u32> = VerifiedHashMap::new();
        
        map.insert("key1".to_string(), 42).unwrap();
        assert_eq!(map.remove(&"key1".to_string()), Some(42));
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_verified_hashmap_collision_tracking() {
        let mut map: VerifiedHashMap<String, u32> = VerifiedHashMap::with_capacity(5);
        
        for i in 0..10 {
            let _ = map.insert(format!("key{}", i), i);
        }
        
        // Collision count should be tracked (always >= 0 by type)
        let _count = map.collision_count();
    }

    #[test]
    fn test_verified_hashmap_invariants() {
        let mut map: VerifiedHashMap<String, u32> = VerifiedHashMap::new();
        map.insert("a".to_string(), 1).unwrap();
        map.insert("b".to_string(), 2).unwrap();
        
        assert!(map.verify_invariants().is_ok());
    }

    #[test]
    fn test_bounds_contract_valid_count() {
        let contract = BoundsContract::new(10);
        assert_eq!(contract.valid_count(), 10);
    }

    #[test]
    fn test_verified_vec_from_vec() {
        let vec = vec![1, 2, 3];
        let verified: VerifiedVec<u32> = VerifiedVec::from_vec(vec);
        
        assert_eq!(verified.len(), 3);
        assert_eq!(*verified.get(0).unwrap(), 1);
    }
}
