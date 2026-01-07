// Profiling Infrastructure for Proof Pipeline
//
// Collects telemetry on proof performance: parsing, type-checking, Z3 solving.
// Reports latency metrics to LSP clients (e.g., Sentinel IDE).
//
// Design:
// 1. Phase timing: capture duration of each compilation phase
// 2. Low-overhead: use scoped measurements (RAII guards)
// 3. LSP integration: emit metrics as structured diagnostic data
// 4. Persistence: record stats for offline analysis
//
// Target: <200ms (p95) for incremental file (1000-line) verification

use std::time::{Duration, Instant};
use std::fmt;

/// A single timed proof phase
#[derive(Debug, Clone)]
pub struct PhaseMetric {
    pub name: String,
    pub duration_ms: u128,
    pub timestamp: u128,  // milliseconds since phase start
}

impl PhaseMetric {
    pub fn new(name: String, duration_ms: u128, timestamp: u128) -> Self {
        PhaseMetric {
            name,
            duration_ms,
            timestamp,
        }
    }

    pub fn display(&self) -> String {
        format!("{:<20} {:>6}ms @ {:>6}ms", self.name, self.duration_ms, self.timestamp)
    }
}

/// Complete proof metrics for a single file/proof run
#[derive(Debug, Clone)]
pub struct ProofMetrics {
    pub file_path: String,
    pub total_duration_ms: u128,
    pub parse_ms: Option<u128>,
    pub semantic_analysis_ms: Option<u128>,
    pub normalization_ms: Option<u128>,
    pub z3_solve_ms: Option<u128>,
    pub other_ms: Option<u128>,
    pub smt_calls: usize,
    pub smt_symbols: usize,
    pub proof_status: String,  // "passed", "failed", "timeout", etc.
    pub phases: Vec<PhaseMetric>,
    pub cache_hit: bool,
    pub timestamp_ms: u128,
}

impl ProofMetrics {
    /// Create a new metrics collector
    pub fn new(file_path: String) -> Self {
        let now = Self::now_ms();
        ProofMetrics {
            file_path,
            total_duration_ms: 0,
            parse_ms: None,
            semantic_analysis_ms: None,
            normalization_ms: None,
            z3_solve_ms: None,
            other_ms: None,
            smt_calls: 0,
            smt_symbols: 0,
            proof_status: "pending".to_string(),
            phases: Vec::new(),
            cache_hit: false,
            timestamp_ms: now,
        }
    }

    /// Get current time in milliseconds
    fn now_ms() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis()
    }

    /// Record total duration
    pub fn set_total_duration(&mut self, ms: u128) {
        self.total_duration_ms = ms;
    }

    /// Record parse phase duration
    pub fn set_parse_time(&mut self, ms: u128) {
        self.parse_ms = Some(ms);
    }

    /// Record semantic analysis phase duration
    pub fn set_semantic_analysis_time(&mut self, ms: u128) {
        self.semantic_analysis_ms = Some(ms);
    }

    /// Record normalization phase duration
    pub fn set_normalization_time(&mut self, ms: u128) {
        self.normalization_ms = Some(ms);
    }

    /// Record Z3 solving phase duration
    pub fn set_z3_solve_time(&mut self, ms: u128) {
        self.z3_solve_ms = Some(ms);
    }

    /// Record other operations (misc overhead)
    pub fn set_other_time(&mut self, ms: u128) {
        self.other_ms = Some(ms);
    }

    /// Record SMT solver calls
    pub fn set_smt_calls(&mut self, count: usize) {
        self.smt_calls = count;
    }

    /// Record SMT solver symbols created
    pub fn set_smt_symbols(&mut self, count: usize) {
        self.smt_symbols = count;
    }

    /// Set proof result status
    pub fn set_proof_status(&mut self, status: String) {
        self.proof_status = status;
    }

    /// Set cache hit flag
    pub fn set_cache_hit(&mut self, hit: bool) {
        self.cache_hit = hit;
    }

    /// Add a phase metric
    pub fn add_phase(&mut self, name: String, duration_ms: u128, timestamp: u128) {
        self.phases.push(PhaseMetric::new(name, duration_ms, timestamp));
    }

    /// Check if we exceeded latency budget
    pub fn exceeds_budget(&self, budget_ms: u128) -> bool {
        self.total_duration_ms > budget_ms
    }

    /// Get a summary string for logging/debugging
    pub fn summary(&self) -> String {
        let mut result = format!(
            "Proof: {} | Total: {}ms | Status: {}",
            self.file_path, self.total_duration_ms, self.proof_status
        );

        if let Some(ms) = self.parse_ms {
            result.push_str(&format!(" | Parse: {}ms", ms));
        }
        if let Some(ms) = self.semantic_analysis_ms {
            result.push_str(&format!(" | Sema: {}ms", ms));
        }
        if let Some(ms) = self.z3_solve_ms {
            result.push_str(&format!(" | Z3: {}ms", ms));
        }

        result.push_str(&format!(" | SMT Calls: {} | Cache Hit: {}", self.smt_calls, self.cache_hit));

        result
    }

    /// Get detailed breakdown
    pub fn detailed_breakdown(&self) -> String {
        let mut result = format!(
            "=== Proof Profiling: {} ===\n",
            self.file_path
        );
        result.push_str(&format!("Total Duration:     {:>8}ms\n", self.total_duration_ms));
        result.push_str(&format!("Status:             {:>8}\n", self.proof_status));
        result.push_str(&format!("Cache Hit:          {:>8}\n", self.cache_hit));
        result.push_str("\nPhases:\n");

        if let Some(ms) = self.parse_ms {
            result.push_str(&format!("  Parse:            {:>8}ms\n", ms));
        }
        if let Some(ms) = self.semantic_analysis_ms {
            result.push_str(&format!("  Semantic:         {:>8}ms\n", ms));
        }
        if let Some(ms) = self.normalization_ms {
            result.push_str(&format!("  Normalization:    {:>8}ms\n", ms));
        }
        if let Some(ms) = self.z3_solve_ms {
            result.push_str(&format!("  Z3 Solving:       {:>8}ms\n", ms));
        }
        if let Some(ms) = self.other_ms {
            result.push_str(&format!("  Other:            {:>8}ms\n", ms));
        }

        result.push_str("\nSolver Metrics:\n");
        result.push_str(&format!("  SMT Calls:        {:>8}\n", self.smt_calls));
        result.push_str(&format!("  SMT Symbols:      {:>8}\n", self.smt_symbols));

        result.push_str("\nBreakdown:\n");
        for phase in &self.phases {
            result.push_str(&format!("  {}\n", phase.display()));
        }

        result
    }
}

impl fmt::Display for ProofMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Timer utility for measuring phases (RAII scope guard)
pub struct ScopedTimer {
    name: String,
    start: Instant,
}

impl ScopedTimer {
    /// Start a new timer for a named phase
    pub fn start(name: &str) -> Self {
        ScopedTimer {
            name: name.to_string(),
            start: Instant::now(),
        }
    }

    /// Get elapsed milliseconds without stopping
    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }

    /// Stop timer and return elapsed time
    pub fn stop(self) -> u128 {
        self.elapsed_ms()
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        // Timer is dropped; caller should have called stop() or saved elapsed_ms()
    }
}

/// Aggregated statistics for profiling analysis
#[derive(Debug, Clone)]
pub struct ProfilingStats {
    pub total_runs: usize,
    pub total_time_ms: u128,
    pub average_ms: u128,
    pub min_ms: u128,
    pub max_ms: u128,
    pub p50_ms: u128,
    pub p95_ms: u128,
    pub p99_ms: u128,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

impl ProfilingStats {
    /// Compute stats from a list of measurements
    pub fn from_metrics(metrics: &[ProofMetrics]) -> Self {
        if metrics.is_empty() {
            return ProfilingStats {
                total_runs: 0,
                total_time_ms: 0,
                average_ms: 0,
                min_ms: 0,
                max_ms: 0,
                p50_ms: 0,
                p95_ms: 0,
                p99_ms: 0,
                cache_hits: 0,
                cache_misses: 0,
            };
        }

        let mut durations: Vec<u128> = metrics.iter().map(|m| m.total_duration_ms).collect();
        durations.sort_unstable();

        let total_time: u128 = durations.iter().sum();
        let average = total_time / metrics.len() as u128;

        let cache_hits = metrics.iter().filter(|m| m.cache_hit).count();
        let cache_misses = metrics.len() - cache_hits;

        let percentile = |p: f64| -> u128 {
            let idx = ((durations.len() as f64 - 1.0) * p / 100.0).round() as usize;
            durations[idx.min(durations.len() - 1)]
        };

        ProfilingStats {
            total_runs: metrics.len(),
            total_time_ms: total_time,
            average_ms: average,
            min_ms: durations[0],
            max_ms: durations[durations.len() - 1],
            p50_ms: percentile(50.0),
            p95_ms: percentile(95.0),
            p99_ms: percentile(99.0),
            cache_hits,
            cache_misses,
        }
    }

    /// Display stats in human-readable format
    pub fn display(&self) -> String {
        if self.total_runs == 0 {
            return "No profiling data".to_string();
        }

        format!(
            "Profiling Stats (runs={})\n\
             Total:   {} ms\n\
             Average: {} ms\n\
             Min:     {} ms\n\
             Max:     {} ms\n\
             P50:     {} ms\n\
             P95:     {} ms (target <200ms)\n\
             P99:     {} ms\n\
             Cache Hits:  {} ({}%)\n\
             Cache Miss:  {} ({}%)",
            self.total_runs,
            self.total_time_ms,
            self.average_ms,
            self.min_ms,
            self.max_ms,
            self.p50_ms,
            self.p95_ms,
            self.p99_ms,
            self.cache_hits,
            (self.cache_hits * 100) / self.total_runs,
            self.cache_misses,
            (self.cache_misses * 100) / self.total_runs,
        )
    }
}

impl fmt::Display for ProfilingStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_metrics_creation() {
        let metrics = ProofMetrics::new("test.aura".to_string());
        assert_eq!(metrics.file_path, "test.aura");
        assert_eq!(metrics.proof_status, "pending");
        assert!(!metrics.cache_hit);
    }

    #[test]
    fn test_proof_metrics_set_times() {
        let mut metrics = ProofMetrics::new("test.aura".to_string());
        metrics.set_parse_time(10);
        metrics.set_semantic_analysis_time(20);
        metrics.set_z3_solve_time(30);
        metrics.set_total_duration(60);

        assert_eq!(metrics.parse_ms, Some(10));
        assert_eq!(metrics.semantic_analysis_ms, Some(20));
        assert_eq!(metrics.z3_solve_ms, Some(30));
        assert_eq!(metrics.total_duration_ms, 60);
    }

    #[test]
    fn test_proof_metrics_exceeds_budget() {
        let mut metrics = ProofMetrics::new("test.aura".to_string());
        metrics.set_total_duration(250);

        assert!(metrics.exceeds_budget(200));
        assert!(!metrics.exceeds_budget(300));
    }

    #[test]
    fn test_profiling_stats_from_metrics() {
        let mut metrics_list = vec![];
        for i in 1..=5 {
            let mut m = ProofMetrics::new(format!("test{}.aura", i));
            m.set_total_duration(i as u128 * 50);
            m.set_cache_hit(i % 2 == 0);
            metrics_list.push(m);
        }

        let stats = ProfilingStats::from_metrics(&metrics_list);
        assert_eq!(stats.total_runs, 5);
        assert_eq!(stats.cache_hits, 2);
        assert_eq!(stats.cache_misses, 3);
        assert_eq!(stats.min_ms, 50);
        assert_eq!(stats.max_ms, 250);
    }

    #[test]
    fn test_scoped_timer() {
        let timer = ScopedTimer::start("test_phase");
        let elapsed = timer.stop();
        assert!(elapsed < 100); // Should be very fast
    }

    #[test]
    fn test_proof_metrics_summary() {
        let mut metrics = ProofMetrics::new("test.aura".to_string());
        metrics.set_parse_time(10);
        metrics.set_total_duration(100);
        metrics.set_proof_status("passed".to_string());
        metrics.set_smt_calls(5);
        metrics.set_cache_hit(true);

        let summary = metrics.summary();
        assert!(summary.contains("test.aura"));
        assert!(summary.contains("100ms"));
        assert!(summary.contains("passed"));
        assert!(summary.contains("Parse: 10ms"));
    }

    #[test]
    fn test_profiling_stats_empty() {
        let stats = ProfilingStats::from_metrics(&[]);
        assert_eq!(stats.total_runs, 0);
        assert_eq!(stats.average_ms, 0);
    }
}
