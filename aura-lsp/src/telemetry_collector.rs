/// Phase 2 Week 2: Pillar 2 (Performance) Telemetry Integration
///
/// This module bridges the verifier's performance metrics to the profiling dashboard
/// in real-time. Enables live monitoring of P50/P95/P99 latencies and recommendations.
///
/// Flow:
/// 1. Verifier emits VerificationTelemetry events (latencies, cache stats, etc.)
/// 2. LSP collects telemetry in a time-windowed buffer
/// 3. Client receives telemetry stream via LSP
/// 4. Dashboard updates metrics in real-time
/// 5. Client computes recommendations based on patterns

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Telemetry event from verifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationTelemetry {
    pub timestamp: u64,
    pub file: String,
    pub line: usize,
    pub verification_type: String, // "function", "assertion", "loop_invariant"
    pub total_latency_ms: f64,
    pub parse_latency_ms: f64,
    pub semantic_latency_ms: f64,
    pub normalize_latency_ms: f64,
    pub z3_latency_ms: f64,
    pub other_latency_ms: f64,
    pub z3_solver_config: String,
    pub result: String, // "proved", "unproved", "unknown", "timeout"
    pub cache_hit: bool,
    pub cache_latency_ms: Option<f64>,
    pub num_clauses: usize,
    pub num_variables: usize,
}

/// Aggregated metrics for the dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub sample_count: usize,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
    pub cache_hit_rate: f64,
    pub prove_rate: f64,
    pub avg_z3_latency_ms: f64,
    pub avg_parse_latency_ms: f64,
    pub avg_semantic_latency_ms: f64,
    pub avg_normalize_latency_ms: f64,
}

/// Phase breakdown for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseBreakdown {
    pub parse_percent: f64,
    pub semantic_percent: f64,
    pub normalize_percent: f64,
    pub z3_percent: f64,
    pub other_percent: f64,
}

/// Performance recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRecommendation {
    pub id: String,
    pub severity: String, // "critical", "warning", "info"
    pub category: String, // "z3-tuning", "cache", "algorithm", "input"
    pub title: String,
    pub description: String,
    pub suggested_action: String,
    pub confidence: f64, // 0.0 - 1.0
    pub potential_speedup: f64, // milliseconds
}

/// Telemetry buffer and aggregator
pub struct TelemetryCollector {
    samples: VecDeque<VerificationTelemetry>,
    max_samples: usize,
}

impl TelemetryCollector {
    pub fn new(max_samples: usize) -> Self {
        TelemetryCollector {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn add_sample(&mut self, telemetry: VerificationTelemetry) {
        self.samples.push_back(telemetry);
        if self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }

    pub fn get_aggregated_metrics(&self) -> Option<AggregatedMetrics> {
        if self.samples.is_empty() {
            return None;
        }

        let mut latencies: Vec<f64> = self
            .samples
            .iter()
            .map(|t| t.total_latency_ms)
            .collect();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let sample_count = latencies.len() as f64;
        let p50_idx = ((sample_count * 0.50) as usize).min(latencies.len() - 1);
        let p95_idx = ((sample_count * 0.95) as usize).min(latencies.len() - 1);
        let p99_idx = ((sample_count * 0.99) as usize).min(latencies.len() - 1);

        let cache_hits = self.samples.iter().filter(|t| t.cache_hit).count();
        let proved = self
            .samples
            .iter()
            .filter(|t| t.result == "proved")
            .count();

        Some(AggregatedMetrics {
            sample_count: self.samples.len(),
            p50_ms: latencies[p50_idx],
            p95_ms: latencies[p95_idx],
            p99_ms: latencies[p99_idx],
            min_ms: latencies[0],
            max_ms: latencies[latencies.len() - 1],
            mean_ms: latencies.iter().sum::<f64>() / sample_count,
            cache_hit_rate: (cache_hits as f64) / sample_count,
            prove_rate: (proved as f64) / sample_count,
            avg_z3_latency_ms: self.samples.iter().map(|t| t.z3_latency_ms).sum::<f64>() / sample_count,
            avg_parse_latency_ms: self.samples.iter().map(|t| t.parse_latency_ms).sum::<f64>() / sample_count,
            avg_semantic_latency_ms: self.samples.iter().map(|t| t.semantic_latency_ms).sum::<f64>() / sample_count,
            avg_normalize_latency_ms: self.samples.iter().map(|t| t.normalize_latency_ms).sum::<f64>() / sample_count,
        })
    }

    pub fn get_phase_breakdown(&self) -> PhaseBreakdown {
        if self.samples.is_empty() {
            return PhaseBreakdown {
                parse_percent: 0.0,
                semantic_percent: 0.0,
                normalize_percent: 0.0,
                z3_percent: 0.0,
                other_percent: 0.0,
            };
        }

        let total_parse: f64 = self.samples.iter().map(|t| t.parse_latency_ms).sum();
        let total_semantic: f64 = self.samples.iter().map(|t| t.semantic_latency_ms).sum();
        let total_normalize: f64 = self.samples.iter().map(|t| t.normalize_latency_ms).sum();
        let total_z3: f64 = self.samples.iter().map(|t| t.z3_latency_ms).sum();
        let total_other: f64 = self.samples.iter().map(|t| t.other_latency_ms).sum();

        let total = total_parse + total_semantic + total_normalize + total_z3 + total_other;
        if total == 0.0 {
            return PhaseBreakdown {
                parse_percent: 0.0,
                semantic_percent: 0.0,
                normalize_percent: 0.0,
                z3_percent: 0.0,
                other_percent: 0.0,
            };
        }

        PhaseBreakdown {
            parse_percent: (total_parse / total) * 100.0,
            semantic_percent: (total_semantic / total) * 100.0,
            normalize_percent: (total_normalize / total) * 100.0,
            z3_percent: (total_z3 / total) * 100.0,
            other_percent: (total_other / total) * 100.0,
        }
    }

    pub fn generate_recommendations(&self) -> Vec<PerformanceRecommendation> {
        let mut recommendations = Vec::new();

        if let Some(metrics) = self.get_aggregated_metrics() {
            // Check P95 latency target
            if metrics.p95_ms > 200.0 {
                if metrics.avg_z3_latency_ms > metrics.mean_ms * 0.5 {
                    recommendations.push(PerformanceRecommendation {
                        id: "z3-tuning-aggressive".to_string(),
                        severity: "critical".to_string(),
                        category: "z3-tuning".to_string(),
                        title: "Z3 Solver is Slow".to_string(),
                        description: format!(
                            "Z3 accounts for {:.0}% of verification time. Consider using 'fast' profile.",
                            (metrics.avg_z3_latency_ms / metrics.mean_ms) * 100.0
                        ),
                        suggested_action: "Set 'solver-profile = fast' in aura.toml or use -Z fast flag".to_string(),
                        confidence: 0.9,
                        potential_speedup: metrics.avg_z3_latency_ms * 0.3,
                    });
                }
            }

            // Check cache hit rate
            if metrics.cache_hit_rate < 0.3 {
                recommendations.push(PerformanceRecommendation {
                    id: "cache-tuning".to_string(),
                    severity: "warning".to_string(),
                    category: "cache".to_string(),
                    title: "Cache Hit Rate is Low".to_string(),
                    description: format!(
                        "Only {:.0}% of verifications hit the cache. Check if similar assertions repeat.",
                        metrics.cache_hit_rate * 100.0
                    ),
                    suggested_action: "Enable incremental verification or refactor overlapping assertions".to_string(),
                    confidence: 0.7,
                    potential_speedup: metrics.mean_ms * 0.4,
                });
            }

            // Check prove rate
            if metrics.prove_rate < 0.7 {
                recommendations.push(PerformanceRecommendation {
                    id: "assertions-too-strong".to_string(),
                    severity: "info".to_string(),
                    category: "input".to_string(),
                    title: "Many Unproved Assertions".to_string(),
                    description: format!(
                        "Only {:.0}% of assertions are proved. Some may be unprovable or need invariants.",
                        metrics.prove_rate * 100.0
                    ),
                    suggested_action: "Review loop invariants and add stronger preconditions".to_string(),
                    confidence: 0.6,
                    potential_speedup: 0.0, // Not a speed issue
                });
            }
        }

        // Check for slowest samples
        if let Some(sample) = self
            .samples
            .iter()
            .max_by(|a, b| a.total_latency_ms.partial_cmp(&b.total_latency_ms).unwrap_or(std::cmp::Ordering::Equal))
        {
            if sample.total_latency_ms > 500.0 {
                recommendations.push(PerformanceRecommendation {
                    id: "slow-verification".to_string(),
                    severity: "warning".to_string(),
                    category: "algorithm".to_string(),
                    title: "Very Slow Verification Found".to_string(),
                    description: format!(
                        "{}:{} took {:.0}ms to verify",
                        sample.file, sample.line, sample.total_latency_ms
                    ),
                    suggested_action: "Simplify assertion or break into smaller sub-assertions".to_string(),
                    confidence: 0.8,
                    potential_speedup: sample.total_latency_ms * 0.5,
                });
            }
        }

        recommendations
    }
}

// ============= TESTS =============

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sample_telemetry(latency_ms: f64) -> VerificationTelemetry {
        VerificationTelemetry {
            timestamp: 1000,
            file: "test.aura".to_string(),
            line: 10,
            verification_type: "assertion".to_string(),
            total_latency_ms: latency_ms,
            parse_latency_ms: latency_ms * 0.05,
            semantic_latency_ms: latency_ms * 0.1,
            normalize_latency_ms: latency_ms * 0.15,
            z3_latency_ms: latency_ms * 0.6,
            other_latency_ms: latency_ms * 0.1,
            z3_solver_config: "thorough".to_string(),
            result: "proved".to_string(),
            cache_hit: false,
            cache_latency_ms: None,
            num_clauses: 1000,
            num_variables: 500,
        }
    }

    #[test]
    fn test_telemetry_collection() {
        let mut collector = TelemetryCollector::new(10);
        collector.add_sample(make_sample_telemetry(100.0));
        collector.add_sample(make_sample_telemetry(150.0));
        collector.add_sample(make_sample_telemetry(120.0));

        assert_eq!(collector.samples.len(), 3);
    }

    #[test]
    fn test_aggregated_metrics() {
        let mut collector = TelemetryCollector::new(10);
        for latency in &[100.0, 150.0, 120.0, 200.0, 110.0] {
            collector.add_sample(make_sample_telemetry(*latency));
        }

        let metrics = collector.get_aggregated_metrics().unwrap();
        assert_eq!(metrics.sample_count, 5);
        assert_eq!(metrics.min_ms, 100.0);
        assert_eq!(metrics.max_ms, 200.0);
        assert!(metrics.p50_ms >= 100.0 && metrics.p50_ms <= 200.0);
        assert!(metrics.p95_ms >= 100.0 && metrics.p95_ms <= 200.0);
    }

    #[test]
    fn test_phase_breakdown() {
        let mut collector = TelemetryCollector::new(10);
        collector.add_sample(make_sample_telemetry(100.0));

        let breakdown = collector.get_phase_breakdown();
        assert!((breakdown.parse_percent + breakdown.semantic_percent
            + breakdown.normalize_percent
            + breakdown.z3_percent
            + breakdown.other_percent
            - 100.0)
            .abs() < 0.01);
    }

    #[test]
    fn test_recommendations_z3_slow() {
        let mut collector = TelemetryCollector::new(10);
        // Add samples with high Z3 latency
        for _ in 0..20 {
            collector.add_sample(make_sample_telemetry(300.0)); // > 200ms target
        }

        let recommendations = collector.generate_recommendations();
        assert!(!recommendations.is_empty());
        assert!(recommendations
            .iter()
            .any(|r| r.category == "z3-tuning"));
    }

    #[test]
    fn test_recommendations_low_cache_hit() {
        let mut collector = TelemetryCollector::new(100);
        for i in 0..50 {
            let mut sample = make_sample_telemetry(100.0);
            sample.cache_hit = i < 10; // 20% hit rate
            collector.add_sample(sample);
        }

        let recommendations = collector.generate_recommendations();
        assert!(recommendations.iter().any(|r| r.category == "cache"));
    }
}
