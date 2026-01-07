/// Client-side Telemetry Receiver and Dashboard Integration
///
/// Receives VerificationTelemetry from LSP server and:
/// 1. Maintains real-time sample buffer
/// 2. Computes metrics (P50/P95/P99)
/// 3. Generates recommendations
/// 4. Updates dashboard UI in real-time

import { useState, useCallback, useEffect } from "react";

export interface VerificationTelemetry {
  timestamp: number;
  file: string;
  line: number;
  verification_type: string; // "function", "assertion", "loop_invariant"
  total_latency_ms: number;
  parse_latency_ms: number;
  semantic_latency_ms: number;
  normalize_latency_ms: number;
  z3_latency_ms: number;
  other_latency_ms: number;
  z3_solver_config: string;
  result: string; // "proved", "unproved", "unknown", "timeout"
  cache_hit: boolean;
  cache_latency_ms?: number;
  num_clauses: number;
  num_variables: number;
}

export interface AggregatedMetrics {
  sample_count: number;
  p50_ms: number;
  p95_ms: number;
  p99_ms: number;
  min_ms: number;
  max_ms: number;
  mean_ms: number;
  cache_hit_rate: number;
  prove_rate: number;
  avg_z3_latency_ms: number;
  avg_parse_latency_ms: number;
  avg_semantic_latency_ms: number;
  avg_normalize_latency_ms: number;
}

export interface PhaseBreakdown {
  parse_percent: number;
  semantic_percent: number;
  normalize_percent: number;
  z3_percent: number;
  other_percent: number;
}

export interface PerformanceRecommendation {
  id: string;
  severity: "critical" | "warning" | "info";
  category: string;
  title: string;
  description: string;
  suggested_action: string;
  confidence: number;
  potential_speedup: number;
}

export interface TelemetryReceiverState {
  samples: VerificationTelemetry[];
  maxSamples: number;
  metrics: AggregatedMetrics | null;
  phaseBreakdown: PhaseBreakdown | null;
  recommendations: PerformanceRecommendation[];
  lastUpdate: number;
}

export class TelemetryReceiver {
  private state: TelemetryReceiverState;
  private listeners: Set<(state: TelemetryReceiverState) => void>;

  constructor(maxSamples: number = 100) {
    this.state = {
      samples: [],
      maxSamples,
      metrics: null,
      phaseBreakdown: null,
      recommendations: [],
      lastUpdate: 0,
    };
    this.listeners = new Set();
  }

  public addSample(telemetry: VerificationTelemetry): void {
    this.state.samples.push(telemetry);
    if (this.state.samples.length > this.state.maxSamples) {
      this.state.samples.shift();
    }

    this.computeMetrics();
    this.state.lastUpdate = Date.now();
    this.notifyListeners();
  }

  private computeMetrics(): void {
    if (this.state.samples.length === 0) {
      this.state.metrics = null;
      this.state.phaseBreakdown = null;
      this.state.recommendations = [];
      return;
    }

    // Compute percentiles
    const latencies = this.state.samples
      .map((s) => s.total_latency_ms)
      .sort((a, b) => a - b);

    const p50Idx = Math.floor(latencies.length * 0.5);
    const p95Idx = Math.floor(latencies.length * 0.95);
    const p99Idx = Math.floor(latencies.length * 0.99);

    const cacheHits = this.state.samples.filter((s) => s.cache_hit).length;
    const proved = this.state.samples.filter((s) => s.result === "proved").length;
    const avgTotal = latencies.reduce((a, b) => a + b, 0) / latencies.length;

    this.state.metrics = {
      sample_count: this.state.samples.length,
      p50_ms: latencies[p50Idx],
      p95_ms: latencies[p95Idx],
      p99_ms: latencies[p99Idx],
      min_ms: latencies[0],
      max_ms: latencies[latencies.length - 1],
      mean_ms: avgTotal,
      cache_hit_rate: cacheHits / this.state.samples.length,
      prove_rate: proved / this.state.samples.length,
      avg_z3_latency_ms:
        this.state.samples.reduce((sum, s) => sum + s.z3_latency_ms, 0) /
        this.state.samples.length,
      avg_parse_latency_ms:
        this.state.samples.reduce((sum, s) => sum + s.parse_latency_ms, 0) /
        this.state.samples.length,
      avg_semantic_latency_ms:
        this.state.samples.reduce((sum, s) => sum + s.semantic_latency_ms, 0) /
        this.state.samples.length,
      avg_normalize_latency_ms:
        this.state.samples.reduce((sum, s) => sum + s.normalize_latency_ms, 0) /
        this.state.samples.length,
    };

    // Compute phase breakdown
    const totalParse = this.state.samples.reduce((sum, s) => sum + s.parse_latency_ms, 0);
    const totalSemantic = this.state.samples.reduce((sum, s) => sum + s.semantic_latency_ms, 0);
    const totalNormalize = this.state.samples.reduce((sum, s) => sum + s.normalize_latency_ms, 0);
    const totalZ3 = this.state.samples.reduce((sum, s) => sum + s.z3_latency_ms, 0);
    const totalOther = this.state.samples.reduce((sum, s) => sum + s.other_latency_ms, 0);

    const totalTime =
      totalParse + totalSemantic + totalNormalize + totalZ3 + totalOther;

    this.state.phaseBreakdown = {
      parse_percent: (totalParse / totalTime) * 100,
      semantic_percent: (totalSemantic / totalTime) * 100,
      normalize_percent: (totalNormalize / totalTime) * 100,
      z3_percent: (totalZ3 / totalTime) * 100,
      other_percent: (totalOther / totalTime) * 100,
    };

    // Generate recommendations
    this.state.recommendations = this.generateRecommendations();
  }

  private generateRecommendations(): PerformanceRecommendation[] {
    const recs: PerformanceRecommendation[] = [];

    if (!this.state.metrics) return recs;

    const metrics = this.state.metrics;

    // Check P95 latency target (200ms)
    if (metrics.p95_ms > 200) {
      if (metrics.avg_z3_latency_ms > metrics.mean_ms * 0.5) {
        recs.push({
          id: "z3-tuning-aggressive",
          severity: "critical",
          category: "z3-tuning",
          title: "Z3 Solver is Slow",
          description: `Z3 accounts for ${((metrics.avg_z3_latency_ms / metrics.mean_ms) * 100).toFixed(0)}% of verification time`,
          suggested_action:
            "Set solver-profile = fast in aura.toml or use -Z fast flag",
          confidence: 0.9,
          potential_speedup: metrics.avg_z3_latency_ms * 0.3,
        });
      }
    }

    // Check cache hit rate
    if (metrics.cache_hit_rate < 0.3) {
      recs.push({
        id: "cache-tuning",
        severity: "warning",
        category: "cache",
        title: "Cache Hit Rate is Low",
        description: `Only ${(metrics.cache_hit_rate * 100).toFixed(0)}% of verifications hit the cache`,
        suggested_action:
          "Enable incremental verification or refactor overlapping assertions",
        confidence: 0.7,
        potential_speedup: metrics.mean_ms * 0.4,
      });
    }

    // Check prove rate
    if (metrics.prove_rate < 0.7) {
      recs.push({
        id: "assertions-too-strong",
        severity: "info",
        category: "input",
        title: "Many Unproved Assertions",
        description: `Only ${(metrics.prove_rate * 100).toFixed(0)}% of assertions are proved`,
        suggested_action:
          "Review loop invariants and add stronger preconditions",
        confidence: 0.6,
        potential_speedup: 0,
      });
    }

    // Check for slowest samples
    const slowestSample = this.state.samples.reduce((max, s) =>
      s.total_latency_ms > max.total_latency_ms ? s : max
    );

    if (slowestSample.total_latency_ms > 500) {
      recs.push({
        id: "slow-verification",
        severity: "warning",
        category: "algorithm",
        title: "Very Slow Verification Found",
        description: `${slowestSample.file}:${slowestSample.line} took ${slowestSample.total_latency_ms.toFixed(0)}ms`,
        suggested_action:
          "Simplify assertion or break into smaller sub-assertions",
        confidence: 0.8,
        potential_speedup: slowestSample.total_latency_ms * 0.5,
      });
    }

    return recs;
  }

  public subscribe(listener: (state: TelemetryReceiverState) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notifyListeners(): void {
    this.listeners.forEach((listener) => listener(this.state));
  }

  public getState(): TelemetryReceiverState {
    return this.state;
  }

  public getRecentSamples(count: number): VerificationTelemetry[] {
    return this.state.samples.slice(-count);
  }

  public getMetrics(): AggregatedMetrics | null {
    return this.state.metrics;
  }

  public getPhaseBreakdown(): PhaseBreakdown | null {
    return this.state.phaseBreakdown;
  }

  public getRecommendations(): PerformanceRecommendation[] {
    return this.state.recommendations;
  }

  public clear(): void {
    this.state.samples = [];
    this.state.metrics = null;
    this.state.phaseBreakdown = null;
    this.state.recommendations = [];
    this.notifyListeners();
  }
}

/// React hook for telemetry receiver
export function useTelemetryReceiver(
  receiver: TelemetryReceiver
): TelemetryReceiverState {
  const [state, setState] = useState<TelemetryReceiverState>(receiver.getState());

  useEffect(() => {
    const unsubscribe = receiver.subscribe((newState) => {
      setState(newState);
    });

    return unsubscribe;
  }, [receiver]);

  return state;
}

/// Test helpers
export function createMockTelemetry(latencyMs: number): VerificationTelemetry {
  return {
    timestamp: Date.now(),
    file: "test.aura",
    line: 10,
    verification_type: "assertion",
    total_latency_ms: latencyMs,
    parse_latency_ms: latencyMs * 0.05,
    semantic_latency_ms: latencyMs * 0.1,
    normalize_latency_ms: latencyMs * 0.15,
    z3_latency_ms: latencyMs * 0.6,
    other_latency_ms: latencyMs * 0.1,
    z3_solver_config: "thorough",
    result: "proved",
    cache_hit: false,
    num_clauses: 1000,
    num_variables: 500,
  };
}
