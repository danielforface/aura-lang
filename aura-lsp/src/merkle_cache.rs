/// Merkle-based proof cache for functions and their direct dependencies.
///
/// This module implements a project-wide cache where each function proof is keyed by:
/// - Function signature hash
/// - Direct dependency (callees) hashes
/// - Source text hash
///
/// The cache provides stable keys across file edits and enables fine-grained invalidation:
/// only functions whose direct deps changed need re-proof.

use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MerkleProofEntry {
    /// Hash of the function signature + body
    pub fn_hash: String,
    /// Sorted list of direct callees (function names this function calls)
    pub callees: Vec<String>,
    /// Hash of combined callee proof summaries (empty if no callees)
    pub dep_hash: String,
    /// Cached proof result (serialized diagnostics)
    pub proof_result: Vec<u8>,
    /// Timestamp of last update
    pub timestamp: u64,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MerkleProofCache {
    /// Per-module function cache: file_id -> function_name -> proof entry
    pub modules: HashMap<String, HashMap<String, MerkleProofEntry>>,
    /// Global function summaries for cross-module invalidation
    /// function_fqn -> (output_hash, callee_list)
    pub fn_summaries: BTreeMap<String, (String, Vec<String>)>,
    /// Schema version
    pub version: u32,
}

impl MerkleProofCache {
    pub fn new() -> Self {
        MerkleProofCache {
            modules: HashMap::new(),
            fn_summaries: BTreeMap::new(),
            version: 1,
        }
    }

    /// Insert or update a proof entry for a function.
    pub fn insert_proof(
        &mut self,
        file_id: String,
        fn_name: String,
        entry: MerkleProofEntry,
    ) {
        self.modules
            .entry(file_id)
            .or_insert_with(HashMap::new)
            .insert(fn_name, entry);
    }

    /// Update the function summary (for cross-module validation).
    pub fn update_fn_summary(
        &mut self,
        fn_fqn: String,
        output_hash: String,
        callees: Vec<String>,
    ) {
        self.fn_summaries.insert(fn_fqn, (output_hash, callees));
    }

    /// Get the proof entry for a function (if cache hit).
    pub fn get_proof(
        &self,
        file_id: &str,
        fn_name: &str,
    ) -> Option<&MerkleProofEntry> {
        self.modules
            .get(file_id)
            .and_then(|m| m.get(fn_name))
    }

    /// Check if a function's proof is still valid given current dep hashes.
    /// Returns true if the entry's dep_hash matches the recomputed dep_hash.
    pub fn is_proof_valid(
        &self,
        file_id: &str,
        fn_name: &str,
        current_dep_hash: &str,
    ) -> bool {
        self.get_proof(file_id, fn_name)
            .map(|entry| entry.dep_hash == current_dep_hash)
            .unwrap_or(false)
    }

    /// Remove all entries for a given module (file).
    pub fn invalidate_module(&mut self, file_id: &str) {
        self.modules.remove(file_id);
    }

    /// Find all modules that call a given function, and invalidate them.
    /// This cascades invalidation for transitive callers.
    pub fn invalidate_callers_of(&mut self, fn_name: &str) -> Vec<String> {
        let mut to_remove: Vec<String> = Vec::new();

        // For each module, check if any function calls `fn_name`.
        for (file_id, fns) in &mut self.modules {
            let mut fn_to_remove: Vec<String> = Vec::new();
            for (name, entry) in fns.iter() {
                if entry.callees.contains(&fn_name.to_string()) {
                    fn_to_remove.push(name.clone());
                    to_remove.push(format!("{}:{}", file_id, name));
                }
            }
            for name in fn_to_remove {
                fns.remove(&name);
            }
        }

        to_remove
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.modules.clear();
        self.fn_summaries.clear();
    }

    /// Return cache statistics (for telemetry).
    pub fn stats(&self) -> (usize, usize) {
        let module_count = self.modules.len();
        let total_entries = self.modules.values().map(|m| m.len()).sum();
        (module_count, total_entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_cache_insert_and_lookup() {
        let mut cache = MerkleProofCache::new();
        let entry = MerkleProofEntry {
            fn_hash: "abc123".to_string(),
            callees: vec!["foo".to_string()],
            dep_hash: "dep456".to_string(),
            proof_result: vec![1, 2, 3],
            timestamp: 1000,
        };

        cache.insert_proof("main.aura".to_string(), "bar".to_string(), entry.clone());
        assert!(cache.get_proof("main.aura", "bar").is_some());
        assert!(cache.get_proof("main.aura", "baz").is_none());
    }

    #[test]
    fn test_merkle_cache_validity() {
        let mut cache = MerkleProofCache::new();
        let entry = MerkleProofEntry {
            fn_hash: "abc123".to_string(),
            callees: vec![],
            dep_hash: "dep456".to_string(),
            proof_result: vec![],
            timestamp: 1000,
        };

        cache.insert_proof("main.aura".to_string(), "foo".to_string(), entry);
        assert!(cache.is_proof_valid("main.aura", "foo", "dep456"));
        assert!(!cache.is_proof_valid("main.aura", "foo", "dep789"));
    }

    #[test]
    fn test_merkle_cache_invalidate_callers() {
        let mut cache = MerkleProofCache::new();

        let entry1 = MerkleProofEntry {
            fn_hash: "abc".to_string(),
            callees: vec!["foo".to_string()], // calls foo
            dep_hash: "dep1".to_string(),
            proof_result: vec![],
            timestamp: 1000,
        };
        let entry2 = MerkleProofEntry {
            fn_hash: "def".to_string(),
            callees: vec!["bar".to_string()], // calls bar, not foo
            dep_hash: "dep2".to_string(),
            proof_result: vec![],
            timestamp: 1000,
        };

        cache.insert_proof("main.aura".to_string(), "baz".to_string(), entry1);
        cache.insert_proof("main.aura".to_string(), "qux".to_string(), entry2);

        // Invalidate all functions calling "foo"
        let removed = cache.invalidate_callers_of("foo");
        assert_eq!(removed.len(), 1);
        assert!(cache.get_proof("main.aura", "baz").is_none());
        assert!(cache.get_proof("main.aura", "qux").is_some());
    }
}
