/// Proof summaries for module-level proof decomposition.
///
/// When a proof succeeds, we can extract a summary capturing:
/// - What was verified (function, contract, invariant)
/// - What assumptions were made (requires/assume statements)
/// - Module-level interface (public functions + their contracts)

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct ProofSummary {
    /// Module or function identifier
    pub id: String,
    /// What was proved (e.g., "all ensures clauses" or "invariant maintained")
    pub claim: String,
    /// Set of assumptions used (e.g., "requires from foo()")
    pub assumptions: Vec<String>,
    /// Proof result (success / failure / timeout)
    pub result: ProofResult,
    /// Proof cost (steps for Z3, time in ms)
    pub cost: ProofCost,
}

#[derive(Clone, Debug)]
pub enum ProofResult {
    /// Proved successfully
    Success,
    /// Failed (counterexample available)
    Failed {
        reason: String,
    },
    /// Timeout or resource limit
    Timeout {
        limit_ms: u64,
    },
}

#[derive(Clone, Debug)]
pub struct ProofCost {
    /// SMT solver steps (Z3 stat count)
    pub smt_steps: u64,
    /// Wall-clock time in milliseconds
    pub time_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ModuleSummaryCache {
    /// Module name -> proof summaries
    pub summaries: BTreeMap<String, Vec<ProofSummary>>,
    /// Cross-module dependencies: module_a calls module_b
    pub dependencies: BTreeMap<String, BTreeSet<String>>,
}

impl ModuleSummaryCache {
    pub fn new() -> Self {
        ModuleSummaryCache {
            summaries: BTreeMap::new(),
            dependencies: BTreeMap::new(),
        }
    }

    /// Add a proof summary for a module.
    pub fn add_summary(&mut self, module: String, summary: ProofSummary) {
        self.summaries
            .entry(module)
            .or_insert_with(Vec::new)
            .push(summary);
    }

    /// Record that module_a depends on module_b (e.g., via call).
    pub fn add_dependency(&mut self, module_a: String, module_b: String) {
        self.dependencies
            .entry(module_a)
            .or_insert_with(BTreeSet::new)
            .insert(module_b);
    }

    /// Get all summaries for a module.
    pub fn get_summaries(&self, module: &str) -> Vec<&ProofSummary> {
        self.summaries
            .get(module)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Check if a module's proofs are still valid given its dependency changes.
    /// Returns true if none of the module's dependencies have been re-proved with different results.
    pub fn is_module_valid(
        &self,
        module: &str,
        dep_proof_results: &BTreeMap<String, ProofResult>,
    ) -> bool {
        let deps = self
            .dependencies
            .get(module)
            .map(|s| s.iter().collect::<Vec<_>>())
            .unwrap_or_default();

        for dep in deps {
            if let Some(current_result) = dep_proof_results.get(dep) {
                // Check if we have a cached summary for this dependency
                if let Some(summaries) = self.summaries.get(dep) {
                    for summary in summaries {
                        // If any summary result differs, module is invalid
                        if !results_match(&summary.result, current_result) {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    /// Invalidate summaries for a module (after re-proof).
    pub fn invalidate_module(&mut self, module: &str) {
        self.summaries.remove(module);

        // Also invalidate all modules that depend on this one (transitive invalidation)
        let dependents: Vec<String> = self
            .dependencies
            .iter()
            .filter(|(_, deps)| deps.contains(&module.to_string()))
            .map(|(m, _)| m.clone())
            .collect();

        for dep in dependents {
            self.invalidate_module(&dep);
        }
    }

    /// Generate a proof summary from successful verification.
    pub fn summary_from_success(
        module: String,
        function: String,
        assumptions: Vec<String>,
        cost: ProofCost,
    ) -> ProofSummary {
        ProofSummary {
            id: format!("{}::{}", module, function),
            claim: format!("All ensures clauses for {}", function),
            assumptions,
            result: ProofResult::Success,
            cost,
        }
    }

    /// Render a summary as human-readable text.
    pub fn format_summary(summary: &ProofSummary) -> String {
        let result_str = match &summary.result {
            ProofResult::Success => "PROVED".to_string(),
            ProofResult::Failed { reason } => format!("FAILED ({})", reason),
            ProofResult::Timeout { limit_ms } => format!("TIMEOUT (>{}ms)", limit_ms),
        };

        let assumptions_str = if summary.assumptions.is_empty() {
            "no assumptions".to_string()
        } else {
            format!("{} assumption(s)", summary.assumptions.len())
        };

        format!(
            "{} [{}] - {} - {}ms, {} SMT steps",
            summary.id, result_str, assumptions_str, summary.cost.time_ms, summary.cost.smt_steps
        )
    }
}

fn results_match(a: &ProofResult, b: &ProofResult) -> bool {
    match (a, b) {
        (ProofResult::Success, ProofResult::Success) => true,
        (ProofResult::Failed { reason: r1 }, ProofResult::Failed { reason: r2 }) => r1 == r2,
        (ProofResult::Timeout { .. }, ProofResult::Timeout { .. }) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_summary_cache() {
        let mut cache = ModuleSummaryCache::new();

        let summary = ProofSummary {
            id: "math::multiply".to_string(),
            claim: "All ensures clauses".to_string(),
            assumptions: vec!["x >= 0".to_string(), "y >= 0".to_string()],
            result: ProofResult::Success,
            cost: ProofCost {
                smt_steps: 1000,
                time_ms: 50,
            },
        };

        cache.add_summary("math".to_string(), summary);
        assert_eq!(cache.get_summaries("math").len(), 1);
        assert_eq!(cache.get_summaries("other").len(), 0);
    }

    #[test]
    fn test_module_dependencies() {
        let mut cache = ModuleSummaryCache::new();
        cache.add_dependency("app".to_string(), "math".to_string());
        cache.add_dependency("app".to_string(), "io".to_string());

        let deps = cache.dependencies.get("app").unwrap();
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"math".to_string()));
    }

    #[test]
    fn test_summary_formatting() {
        let summary = ProofSummary {
            id: "foo::bar".to_string(),
            claim: "Test claim".to_string(),
            assumptions: vec!["x > 0".to_string()],
            result: ProofResult::Success,
            cost: ProofCost {
                smt_steps: 500,
                time_ms: 25,
            },
        };

        let formatted = ModuleSummaryCache::format_summary(&summary);
        assert!(formatted.contains("foo::bar"));
        assert!(formatted.contains("PROVED"));
        assert!(formatted.contains("1 assumption"));
    }
}
