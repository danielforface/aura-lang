#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Performance tuning parameters that can be dynamically adjusted.
#[derive(Clone, Debug)]
pub struct TuningConfig {
    /// Maximum cache entries before eviction
    pub cache_max_entries: usize,
    /// Cache eviction ratio (0.0-1.0)
    pub cache_eviction_ratio: f64,
    /// Z3 solver timeout (milliseconds)
    pub z3_timeout_ms: u32,
    /// Enable incremental solving (vs. fresh solver per goal)
    pub z3_incremental: bool,
    /// Z3 symbol pre-population enable flag
    pub z3_prepopulate: bool,
    /// Maximum file size for full verification (bytes); above this use sampling
    pub file_size_threshold_bytes: usize,
    /// Enable parallel verification
    pub enable_parallel: bool,
    /// Number of parallel threads
    pub parallel_threads: usize,
}

impl Default for TuningConfig {
    fn default() -> Self {
        TuningConfig {
            cache_max_entries: 10000,
            cache_eviction_ratio: 0.3,
            z3_timeout_ms: 5000,
            z3_incremental: true,
            z3_prepopulate: true,
            file_size_threshold_bytes: 50000,
            enable_parallel: true,
            parallel_threads: 4,
        }
    }
}

impl TuningConfig {
    /// Create tuning config targeting <200ms P95 latency
    pub fn target_200ms_p95() -> Self {
        TuningConfig {
            cache_max_entries: 20000,
            cache_eviction_ratio: 0.2,
            z3_timeout_ms: 3000,  // Aggressive timeout for responsiveness
            z3_incremental: true,
            z3_prepopulate: true,
            file_size_threshold_bytes: 100000,
            enable_parallel: true,
            parallel_threads: 6,
        }
    }

    /// Conservative tuning (correctness over speed)
    pub fn conservative() -> Self {
        TuningConfig {
            cache_max_entries: 5000,
            cache_eviction_ratio: 0.5,
            z3_timeout_ms: 10000,
            z3_incremental: false,
            z3_prepopulate: false,
            file_size_threshold_bytes: 10000,
            enable_parallel: false,
            parallel_threads: 1,
        }
    }
}

/// Adaptive tuning engine: monitors latency and adjusts parameters.
pub struct AdaptiveTuner {
    config: Arc<Mutex<TuningConfig>>,
    /// Recent latencies (ms), used for P95 calculation
    recent_latencies: Arc<Mutex<Vec<u128>>>,
    /// Window size for recent latencies
    latency_window_size: usize,
}

impl AdaptiveTuner {
    /// Create a new adaptive tuner
    pub fn new(config: TuningConfig) -> Self {
        AdaptiveTuner {
            config: Arc::new(Mutex::new(config)),
            recent_latencies: Arc::new(Mutex::new(Vec::new())),
            latency_window_size: 100,
        }
    }

    /// Get current configuration
    pub fn config(&self) -> TuningConfig {
        self.config.lock().unwrap().clone()
    }

    /// Record a new latency measurement
    pub fn record_latency(&self, latency_ms: u128) {
        let mut latencies = self.recent_latencies.lock().unwrap();
        latencies.push(latency_ms);
        
        // Keep only recent window
        if latencies.len() > self.latency_window_size {
            let excess = latencies.len() - self.latency_window_size;
            latencies.drain(0..excess);
        }
    }

    /// Calculate P95 latency from recent measurements
    pub fn p95_latency(&self) -> Option<u128> {
        let latencies = self.recent_latencies.lock().unwrap();
        if latencies.is_empty() {
            return None;
        }

        let mut sorted = latencies.clone();
        sorted.sort();
        let p95_idx = (sorted.len() as f64 * 0.95) as usize;
        Some(sorted[p95_idx])
    }

    /// Calculate P50 (median) latency
    pub fn p50_latency(&self) -> Option<u128> {
        let latencies = self.recent_latencies.lock().unwrap();
        if latencies.is_empty() {
            return None;
        }

        let mut sorted = latencies.clone();
        sorted.sort();
        let median_idx = sorted.len() / 2;
        Some(sorted[median_idx])
    }

    /// Calculate P99 latency
    pub fn p99_latency(&self) -> Option<u128> {
        let latencies = self.recent_latencies.lock().unwrap();
        if latencies.is_empty() {
            return None;
        }

        let mut sorted = latencies.clone();
        sorted.sort();
        let p99_idx = (sorted.len() as f64 * 0.99) as usize;
        Some(sorted[p99_idx])
    }

    /// Adapt tuning parameters based on observed latencies
    pub fn adapt(&self) {
        if let Some(p95) = self.p95_latency() {
            let mut config = self.config.lock().unwrap();

            // If P95 > 200ms, increase cache size and parallel threads
            if p95 > 200 {
                config.cache_max_entries = (config.cache_max_entries as f64 * 1.2) as usize;
                if config.enable_parallel && config.parallel_threads < 8 {
                    config.parallel_threads += 1;
                }
                // Reduce Z3 timeout to fail faster on hard problems
                config.z3_timeout_ms = (config.z3_timeout_ms as f64 * 0.8) as u32;
            }
            // If P95 < 150ms, we can afford to be more thorough
            else if p95 < 150 {
                config.z3_timeout_ms = (config.z3_timeout_ms as f64 * 1.2) as u32;
            }
        }
    }

    /// Set configuration directly
    pub fn set_config(&self, config: TuningConfig) {
        *self.config.lock().unwrap() = config;
    }

    /// Get tuning statistics as formatted string
    pub fn stats(&self) -> String {
        let mut lines = Vec::new();
        
        lines.push("Performance Tuning Stats:".to_string());
        
        if let Some(p50) = self.p50_latency() {
            lines.push(format!("  P50 (median): {}ms", p50));
        }
        if let Some(p95) = self.p95_latency() {
            lines.push(format!("  P95: {}ms", p95));
        }
        if let Some(p99) = self.p99_latency() {
            lines.push(format!("  P99: {}ms", p99));
        }

        let config = self.config.lock().unwrap();
        lines.push(format!("  Cache max entries: {}", config.cache_max_entries));
        lines.push(format!("  Z3 timeout: {}ms", config.z3_timeout_ms));
        lines.push(format!(
            "  Parallel: {} threads",
            config.parallel_threads
        ));
        lines.push(format!("  Incremental Z3: {}", config.z3_incremental));

        lines.join("\n")
    }
}

impl Default for AdaptiveTuner {
    fn default() -> Self {
        Self::new(TuningConfig::default())
    }
}

/// Z3 Solver tuning: specific strategies for improving solver performance.
pub struct Z3SolverTuning;

impl Z3SolverTuning {
    /// Get recommended Z3 params for faster solving
    pub fn fast_params() -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("smt.arith.solver".to_string(), "2".to_string());  // Fast arithmetic
        params.insert("nlsat.seed".to_string(), "0".to_string());       // Deterministic
        params.insert("sat.random_seed".to_string(), "0".to_string());
        // Use simplification (can speed up or slow down depending on formula)
        params.insert("simplify.max_steps".to_string(), "1000000".to_string());
        params
    }

    /// Get recommended params for correctness (slower)
    pub fn correct_params() -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("smt.arith.solver".to_string(), "0".to_string());  // Default (slower but more robust)
        params.insert("simplify.max_steps".to_string(), "5000000".to_string());
        params
    }

    /// Get params for incremental solving (push/pop)
    pub fn incremental_params() -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("smt.arith.solver".to_string(), "2".to_string());
        // Incremental solving often benefits from caching
        params.insert("memory_high_watermark".to_string(), "1024".to_string());  // 1GB
        params
    }
}

/// Cache performance tracking
#[derive(Clone, Debug)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub current_size: usize,
}

impl CacheStats {
    pub fn new() -> Self {
        CacheStats {
            hits: 0,
            misses: 0,
            evictions: 0,
            current_size: 0,
        }
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "Cache: {} hits, {} misses, {} evictions ({:.1}% hit rate) [{}]",
            self.hits,
            self.misses,
            self.evictions,
            self.hit_rate() * 100.0,
            self.current_size
        )
    }
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tuning_config_default() {
        let config = TuningConfig::default();
        assert_eq!(config.cache_max_entries, 10000);
        assert_eq!(config.z3_timeout_ms, 5000);
        assert!(config.z3_incremental);
    }

    #[test]
    fn test_tuning_config_200ms_p95() {
        let config = TuningConfig::target_200ms_p95();
        assert_eq!(config.cache_max_entries, 20000);
        assert_eq!(config.z3_timeout_ms, 3000);  // More aggressive
        assert!(config.enable_parallel);
        assert_eq!(config.parallel_threads, 6);
    }

    #[test]
    fn test_tuning_config_conservative() {
        let config = TuningConfig::conservative();
        assert_eq!(config.cache_max_entries, 5000);
        assert_eq!(config.z3_timeout_ms, 10000);
        assert!(!config.z3_incremental);
        assert!(!config.enable_parallel);
    }

    #[test]
    fn test_adaptive_tuner_latency_recording() {
        let tuner = AdaptiveTuner::new(TuningConfig::default());
        
        tuner.record_latency(100);
        tuner.record_latency(150);
        tuner.record_latency(200);
        
        let p50 = tuner.p50_latency().unwrap();
        assert_eq!(p50, 150);
    }

    #[test]
    fn test_adaptive_tuner_p95_calculation() {
        let tuner = AdaptiveTuner::new(TuningConfig::default());
        
        // Record 100 latencies from 1ms to 100ms
        for i in 1..=100 {
            tuner.record_latency(i);
        }
        
        let p95 = tuner.p95_latency().unwrap();
        // P95 should be around 95ms
        assert!(p95 >= 90 && p95 <= 100);
    }

    #[test]
    fn test_adaptive_tuner_high_latency_adaptation() {
        let mut config = TuningConfig::default();
        config.cache_max_entries = 10000;
        config.z3_timeout_ms = 5000;
        
        let tuner = AdaptiveTuner::new(config);
        
        // Record high latencies
        for _ in 0..100 {
            tuner.record_latency(250);
        }
        
        let original_cache = tuner.config().cache_max_entries;
        tuner.adapt();
        let new_cache = tuner.config().cache_max_entries;
        
        // Cache should increase when latencies are high
        assert!(new_cache > original_cache);
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let mut stats = CacheStats::new();
        stats.hits = 75;
        stats.misses = 25;
        
        let rate = stats.hit_rate();
        assert!((rate - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_cache_stats_empty() {
        let stats = CacheStats::new();
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_z3_solver_tuning() {
        let fast = Z3SolverTuning::fast_params();
        assert!(fast.contains_key("smt.arith.solver"));
        assert_eq!(fast.get("smt.arith.solver"), Some(&"2".to_string()));
        
        let correct = Z3SolverTuning::correct_params();
        assert_eq!(correct.get("smt.arith.solver"), Some(&"0".to_string()));
        
        let incr = Z3SolverTuning::incremental_params();
        assert!(incr.contains_key("memory_high_watermark"));
    }

    #[test]
    fn test_adaptive_tuner_stats_display() {
        let tuner = AdaptiveTuner::new(TuningConfig::default());
        
        tuner.record_latency(100);
        tuner.record_latency(120);
        
        let stats = tuner.stats();
        assert!(stats.contains("Performance Tuning Stats"));
        assert!(stats.contains("Cache max entries"));
    }
}
