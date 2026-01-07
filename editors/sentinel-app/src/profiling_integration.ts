/// Sentinel Profiling Dashboard Integration
/// 
/// This module connects the profiling_dashboard.tsx React component
/// with the LSP telemetry stream, displaying real-time proof performance metrics.
///
/// Integration points:
/// - LSP telemetry events (proof start/complete)
/// - Per-function latency tracking
/// - P50/P95/P99 percentile calculation
/// - Performance recommendations
/// - Trend visualization

export interface VerificationTelemetry {
  timestamp: number;
  function: string;
  file: string;
  phases: {
    parse_ms: number;
    semantic_ms: number;
    normalize_ms: number;
    z3_ms: number;
  };
  total_ms: number;
  result: "verified" | "failed" | "timeout";
  cached: boolean;
}

export interface PerformanceMetrics {
  p50_ms: number;
  p95_ms: number;
  p99_ms: number;
  mean_ms: number;
  min_ms: number;
  max_ms: number;
  count: number;
  cached_ratio: number;
}

export interface PerformanceRecommendation {
  level: "info" | "warning" | "critical";
  message: string;
  suggestion: string;
  target: string;
}

export class ProfilingDashboardIntegration {
  private telemetryHistory: VerificationTelemetry[] = [];
  private container: HTMLElement | null = null;
  private maxHistorySize = 50;

  constructor() {}

  /// Initialize dashboard in DOM
  initialize(containerId: string): void {
    const container = document.getElementById(containerId);
    if (!container) {
      console.error(`Profiling dashboard container not found: ${containerId}`);
      return;
    }
    this.container = container;
    this.render();
  }

  /// Add telemetry event from LSP
  addTelemetryEvent(event: VerificationTelemetry): void {
    this.telemetryHistory.push({
      ...event,
      timestamp: Date.now(),
    });

    // Keep history bounded
    if (this.telemetryHistory.length > this.maxHistorySize) {
      this.telemetryHistory.shift();
    }

    this.render();
  }

  /// Calculate performance metrics from history
  private calculateMetrics(): PerformanceMetrics {
    if (this.telemetryHistory.length === 0) {
      return {
        p50_ms: 0,
        p95_ms: 0,
        p99_ms: 0,
        mean_ms: 0,
        min_ms: 0,
        max_ms: 0,
        count: 0,
        cached_ratio: 0,
      };
    }

    const latencies = this.telemetryHistory.map((e) => e.total_ms).sort((a, b) => a - b);
    const cachedCount = this.telemetryHistory.filter((e) => e.cached).length;

    const percentile = (arr: number[], p: number) => {
      const index = Math.ceil((p / 100) * arr.length) - 1;
      return arr[Math.max(0, index)] || 0;
    };

    return {
      p50_ms: percentile(latencies, 50),
      p95_ms: percentile(latencies, 95),
      p99_ms: percentile(latencies, 99),
      mean_ms: latencies.reduce((a, b) => a + b, 0) / latencies.length,
      min_ms: Math.min(...latencies),
      max_ms: Math.max(...latencies),
      count: latencies.length,
      cached_ratio: cachedCount / latencies.length,
    };
  }

  /// Generate performance recommendations
  private generateRecommendations(metrics: PerformanceMetrics): PerformanceRecommendation[] {
    const recommendations: PerformanceRecommendation[] = [];

    // Check P95 latency target
    if (metrics.p95_ms > 200) {
      recommendations.push({
        level: "critical",
        message: `P95 latency is ${metrics.p95_ms.toFixed(0)}ms (target: <200ms)`,
        suggestion: "Enable fast profile: #[verify_profile = \"fast\"] or add intermediate assertions",
        target: "p95_latency",
      });
    }

    // Check cache effectiveness
    if (metrics.cached_ratio < 0.3) {
      recommendations.push({
        level: "warning",
        message: `Cache hit rate is ${(metrics.cached_ratio * 100).toFixed(0)}% (target: >50%)`,
        suggestion: "Review file dependencies; consider breaking into smaller functions",
        target: "cache_effectiveness",
      });
    }

    // Check outliers
    if (metrics.max_ms > metrics.p95_ms * 3) {
      recommendations.push({
        level: "warning",
        message: `Outlier detected: ${metrics.max_ms.toFixed(0)}ms (P95: ${metrics.p95_ms.toFixed(0)}ms)`,
        suggestion: "Check for solver timeouts or complex quantifiers in slow functions",
        target: "outliers",
      });
    }

    // Positive feedback
    if (metrics.p95_ms <= 200 && metrics.cached_ratio >= 0.5) {
      recommendations.push({
        level: "info",
        message: "✓ Performance targets met",
        suggestion: "Continue with current verification strategy",
        target: "performance_ok",
      });
    }

    return recommendations;
  }

  /// Render dashboard
  private render(): void {
    if (!this.container) return;

    const metrics = this.calculateMetrics();
    const recommendations = this.generateRecommendations(metrics);

    const html = `
      <div class="profiling-dashboard">
        ${this.renderMetricsCards(metrics)}
        ${this.renderRecommendations(recommendations)}
        ${this.renderRecentVerifications()}
        ${this.renderPhaseBreakdown()}
        ${this.renderTrendChart()}
      </div>
    `;

    this.container.innerHTML = html;
  }

  /// Render metric cards (P50, P95, P99)
  private renderMetricsCards(metrics: PerformanceMetrics): string {
    const card = (label: string, value: string, status: "ok" | "warn" | "critical") => {
      const statusClass = `metric-status-${status}`;
      return `
        <div class="metric-card ${statusClass}">
          <div class="metric-label">${label}</div>
          <div class="metric-value">${value}</div>
        </div>
      `;
    };

    const p50Status = metrics.p50_ms <= 100 ? "ok" : "warn";
    const p95Status = metrics.p95_ms <= 200 ? "ok" : metrics.p95_ms <= 500 ? "warn" : "critical";
    const p99Status = metrics.p99_ms <= 500 ? "ok" : metrics.p99_ms <= 1000 ? "warn" : "critical";

    return `
      <div class="profiling-metrics">
        <h3>Latency Summary</h3>
        <div class="metrics-grid">
          ${card("P50", `${metrics.p50_ms.toFixed(0)}ms`, p50Status)}
          ${card("P95", `${metrics.p95_ms.toFixed(0)}ms`, p95Status)}
          ${card("P99", `${metrics.p99_ms.toFixed(0)}ms`, p99Status)}
          ${card("Mean", `${metrics.mean_ms.toFixed(0)}ms`, "ok")}
          ${card("Samples", `${metrics.count}`, "ok")}
          ${card("Cached", `${(metrics.cached_ratio * 100).toFixed(0)}%`, metrics.cached_ratio >= 0.5 ? "ok" : "warn")}
        </div>
      </div>
    `;
  }

  /// Render recommendations
  private renderRecommendations(recommendations: PerformanceRecommendation[]): string {
    if (recommendations.length === 0) {
      return "";
    }

    return `
      <div class="profiling-recommendations">
        <h3>Performance Recommendations</h3>
        ${recommendations
          .map(
            (r) => `
          <div class="recommendation recommendation-${r.level}">
            <div class="rec-icon">${r.level === "critical" ? "⚠️" : r.level === "warning" ? "⚡" : "ℹ️"}</div>
            <div class="rec-content">
              <div class="rec-message">${escapeHtml(r.message)}</div>
              <div class="rec-suggestion">${escapeHtml(r.suggestion)}</div>
            </div>
          </div>
        `
          )
          .join("")}
      </div>
    `;
  }

  /// Render recent verification runs
  private renderRecentVerifications(): string {
    const recent = this.telemetryHistory.slice(-10).reverse();

    return `
      <div class="profiling-recent">
        <h3>Recent Verifications</h3>
        <div class="recent-list">
          ${recent
            .map(
              (t) => `
            <div class="recent-item recent-${t.result}">
              <div class="recent-function">${escapeHtml(t.function)}</div>
              <div class="recent-status">${t.result}</div>
              <div class="recent-time">${t.total_ms.toFixed(0)}ms ${t.cached ? "(cached)" : ""}</div>
            </div>
          `
            )
            .join("")}
        </div>
      </div>
    `;
  }

  /// Render phase breakdown chart
  private renderPhaseBreakdown(): string {
    if (this.telemetryHistory.length === 0) {
      return "";
    }

    // Average across all samples
    const avgPhases = {
      parse_ms: 0,
      semantic_ms: 0,
      normalize_ms: 0,
      z3_ms: 0,
    };

    for (const event of this.telemetryHistory) {
      avgPhases.parse_ms += event.phases.parse_ms;
      avgPhases.semantic_ms += event.phases.semantic_ms;
      avgPhases.normalize_ms += event.phases.normalize_ms;
      avgPhases.z3_ms += event.phases.z3_ms;
    }

    const count = this.telemetryHistory.length;
    avgPhases.parse_ms /= count;
    avgPhases.semantic_ms /= count;
    avgPhases.normalize_ms /= count;
    avgPhases.z3_ms /= count;

    const total = Object.values(avgPhases).reduce((a, b) => a + b, 0);

    return `
      <div class="profiling-phases">
        <h3>Phase Breakdown (Average)</h3>
        <div class="phase-bars">
          <div class="phase-bar">
            <div class="phase parse" style="width: ${(avgPhases.parse_ms / total * 100).toFixed(1)}%"></div>
            <div class="phase semantic" style="width: ${(avgPhases.semantic_ms / total * 100).toFixed(1)}%"></div>
            <div class="phase normalize" style="width: ${(avgPhases.normalize_ms / total * 100).toFixed(1)}%"></div>
            <div class="phase z3" style="width: ${(avgPhases.z3_ms / total * 100).toFixed(1)}%"></div>
          </div>
        </div>
        <div class="phase-legend">
          <span class="parse">Parse</span>
          <span class="semantic">Semantic</span>
          <span class="normalize">Normalize</span>
          <span class="z3">Z3</span>
        </div>
      </div>
    `;
  }

  /// Render trend chart (last 20 samples)
  private renderTrendChart(): string {
    const recent = this.telemetryHistory.slice(-20);
    if (recent.length === 0) {
      return "";
    }

    const maxLatency = Math.max(...recent.map((e) => e.total_ms));
    const threshold = 200;

    const points = recent
      .map((e, i) => {
        const x = (i / (recent.length - 1)) * 100;
        const y = 100 - (e.total_ms / maxLatency) * 100;
        const pointClass = e.total_ms > threshold ? "over-target" : "under-target";
        return `<circle cx="${x}" cy="${y}" r="3" class="trend-point ${pointClass}" data-ms="${e.total_ms.toFixed(0)}"/>`;
      })
      .join("");

    const targetLine = 100 - (threshold / maxLatency) * 100;

    return `
      <div class="profiling-trend">
        <h3>Latency Trend (Last 20)</h3>
        <svg viewBox="0 0 100 100" class="trend-chart">
          <line x1="0" y1="${targetLine}" x2="100" y2="${targetLine}" class="target-line" stroke-dasharray="2,2" />
          ${points}
        </svg>
        <div class="trend-note">Target: <code>200ms</code></div>
      </div>
    `;
  }
}

/// Utility: escape HTML
function escapeHtml(text: string): string {
  const map: Record<string, string> = {
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#039;",
  };
  return text.replace(/[&<>"']/g, (m) => map[m]);
}

/// Global instance
let profilingDashboardInstance: ProfilingDashboardIntegration | null = null;

/// Initialize profiling dashboard globally
export function initializeProfilingDashboard(): void {
  profilingDashboardInstance = new ProfilingDashboardIntegration();
  profilingDashboardInstance.initialize("profiling-dashboard-container");
}

/// Add telemetry event from LSP
export function addTelemetryEvent(event: VerificationTelemetry): void {
  if (!profilingDashboardInstance) {
    initializeProfilingDashboard();
  }
  profilingDashboardInstance?.addTelemetryEvent(event);
}
