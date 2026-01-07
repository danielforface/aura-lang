/// Enhanced Profiling Dashboard Component for Phase 2 Week 2
///
/// Real-time visualization of performance metrics with:
/// - Live P50/P95/P99 latencies
/// - Phase breakdown stacked bar chart
/// - Performance recommendations
/// - Recent verification history
/// - Trend visualization

import React, { useState, useCallback } from "react";
import {
  TelemetryReceiver,
  useTelemetryReceiver,
  AggregatedMetrics,
  PhaseBreakdown,
  PerformanceRecommendation,
  VerificationTelemetry,
} from "./telemetry_receiver";

export interface ProfilingDashboardProps {
  receiver: TelemetryReceiver;
  onSelectVerification?: (telemetry: VerificationTelemetry) => void;
  onApplyRecommendation?: (recommendation: PerformanceRecommendation) => void;
}

export const ProfilingDashboard: React.FC<ProfilingDashboardProps> = ({
  receiver,
  onSelectVerification,
  onApplyRecommendation,
}) => {
  const telemetryState = useTelemetryReceiver(receiver);
  const [expandedSection, setExpandedSection] = useState<string | null>("metrics");

  if (!telemetryState.metrics) {
    return (
      <div className="profiling-dashboard empty">
        <div className="empty-state">
          <p>No verification data yet. Run some verifications to see metrics.</p>
        </div>
      </div>
    );
  }

  const metrics = telemetryState.metrics;
  const breakdown = telemetryState.phaseBreakdown;
  const recommendations = telemetryState.recommendations;

  // ========== RENDER METRICS CARDS ==========

  const renderMetricsCard = (label: string, value: number, unit: string, threshold?: number) => {
    const isWarning = threshold && value > threshold;
    const statusClass = isWarning ? "warning" : "ok";

    return (
      <div key={label} className={`metric-card ${statusClass}`}>
        <div className="metric-label">{label}</div>
        <div className="metric-value">
          {value.toFixed(1)}{unit}
        </div>
        {threshold && (
          <div className="metric-threshold">
            Target: {threshold.toFixed(1)}{unit}
          </div>
        )}
      </div>
    );
  };

  // ========== RENDER PHASE BREAKDOWN ==========

  const renderPhaseBreakdown = () => {
    if (!breakdown) return null;

    const phases = [
      { name: "Parse", percent: breakdown.parse_percent, color: "#4ec9b0" },
      { name: "Semantic", percent: breakdown.semantic_percent, color: "#569cd6" },
      { name: "Normalize", percent: breakdown.normalize_percent, color: "#646695" },
      { name: "Z3", percent: breakdown.z3_percent, color: "#ce9178" },
      { name: "Other", percent: breakdown.other_percent, color: "#808080" },
    ];

    return (
      <div className="phase-breakdown">
        <div className="breakdown-bar">
          {phases.map((phase) => (
            <div
              key={phase.name}
              className="phase-segment"
              style={{
                width: `${phase.percent}%`,
                backgroundColor: phase.color,
                opacity: 0.8,
              }}
              title={`${phase.name}: ${phase.percent.toFixed(1)}%`}
            />
          ))}
        </div>
        <div className="breakdown-legend">
          {phases.map((phase) => (
            <div key={phase.name} className="legend-item">
              <div
                className="legend-color"
                style={{ backgroundColor: phase.color }}
              />
              <span className="legend-label">
                {phase.name} ({phase.percent.toFixed(0)}%)
              </span>
            </div>
          ))}
        </div>
      </div>
    );
  };

  // ========== RENDER RECOMMENDATIONS ==========

  const renderRecommendations = () => {
    if (recommendations.length === 0) {
      return <div className="no-recommendations">All metrics are within normal range ✓</div>;
    }

    const severityIcon: Record<string, string> = {
      critical: "❌",
      warning: "⚠️",
      info: "ℹ️",
    };

    return (
      <div className="recommendations-list">
        {recommendations.map((rec, idx) => (
          <div
            key={rec.id}
            className={`recommendation-card severity-${rec.severity}`}
          >
            <div className="rec-header">
              <span className="severity-icon">{severityIcon[rec.severity]}</span>
              <span className="rec-title">{rec.title}</span>
              <span className="confidence">
                {(rec.confidence * 100).toFixed(0)}% sure
              </span>
            </div>
            <div className="rec-description">{rec.description}</div>
            <div className="rec-action">{rec.suggested_action}</div>
            {rec.potential_speedup > 0 && (
              <div className="rec-speedup">
                Potential speedup: {rec.potential_speedup.toFixed(0)}ms
              </div>
            )}
            <button
              className="apply-rec-button"
              onClick={() => onApplyRecommendation?.(rec)}
            >
              Apply Recommendation
            </button>
          </div>
        ))}
      </div>
    );
  };

  // ========== RENDER RECENT VERIFICATIONS ==========

  const renderRecentVerifications = () => {
    const recent = telemetryState.samples.slice(-10).reverse();

    return (
      <div className="recent-verifications">
        <table className="verification-table">
          <thead>
            <tr>
              <th>File</th>
              <th>Line</th>
              <th>Type</th>
              <th>Latency</th>
              <th>Result</th>
              <th>Cache</th>
            </tr>
          </thead>
          <tbody>
            {recent.map((sample, idx) => (
              <tr
                key={idx}
                className={`row-${sample.result}`}
                onClick={() => onSelectVerification?.(sample)}
              >
                <td className="file-cell">{sample.file}</td>
                <td className="line-cell">{sample.line}</td>
                <td className="type-cell">{sample.verification_type}</td>
                <td className="latency-cell">
                  {sample.total_latency_ms.toFixed(1)}ms
                </td>
                <td className={`result-cell result-${sample.result}`}>
                  {sample.result}
                </td>
                <td className="cache-cell">
                  {sample.cache_hit ? "✓" : "✗"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    );
  };

  // ========== RENDER TREND CHART ==========

  const renderTrendChart = () => {
    const samples = telemetryState.samples;
    if (samples.length < 2) return null;

    // Simplified ASCII sparkline
    const latencies = samples.map((s) => s.total_latency_ms);
    const min = Math.min(...latencies);
    const max = Math.max(...latencies);
    const range = max - min || 1;

    const sparkline = latencies
      .slice(-20) // Last 20 samples
      .map((lat) => {
        const normalized = (lat - min) / range;
        const bars = "▁▂▃▄▅▆▇█";
        return bars[Math.floor(normalized * (bars.length - 1))];
      })
      .join("");

    return (
      <div className="trend-chart">
        <div className="trend-label">Recent Trend (Last 20 Verifications)</div>
        <div className="trend-sparkline">{sparkline}</div>
        <div className="trend-scale">
          <span className="trend-min">{min.toFixed(0)}ms</span>
          <span className="trend-max">{max.toFixed(0)}ms</span>
        </div>
      </div>
    );
  };

  // ========== MAIN RENDER ==========

  return (
    <div className="profiling-dashboard">
      {/* Header */}
      <div className="dashboard-header">
        <h2>Performance Dashboard</h2>
        <span className="sample-count">
          {metrics.sample_count} verifications
        </span>
      </div>

      {/* Metrics Overview */}
      <section className="dashboard-section">
        <h3
          className="section-title"
          onClick={() =>
            setExpandedSection(
              expandedSection === "metrics" ? null : "metrics"
            )
          }
        >
          📊 Metrics Overview
        </h3>
        {expandedSection === "metrics" && (
          <div className="section-content metrics-grid">
            {renderMetricsCard("P50", metrics.p50_ms, "ms")}
            {renderMetricsCard("P95", metrics.p95_ms, "ms", 200)}
            {renderMetricsCard("P99", metrics.p99_ms, "ms", 250)}
            {renderMetricsCard("Min", metrics.min_ms, "ms")}
            {renderMetricsCard("Max", metrics.max_ms, "ms")}
            {renderMetricsCard("Mean", metrics.mean_ms, "ms")}
            {renderMetricsCard("Cache Hit Rate", metrics.cache_hit_rate * 100, "%")}
            {renderMetricsCard("Proof Rate", metrics.prove_rate * 100, "%")}
          </div>
        )}
      </section>

      {/* Phase Breakdown */}
      <section className="dashboard-section">
        <h3
          className="section-title"
          onClick={() =>
            setExpandedSection(
              expandedSection === "breakdown" ? null : "breakdown"
            )
          }
        >
          ⏱️ Phase Breakdown
        </h3>
        {expandedSection === "breakdown" && (
          <div className="section-content">
            {renderPhaseBreakdown()}
            <div className="phase-details">
              <div className="phase-detail">
                <span>Average Parse Time:</span>
                <code>{metrics.avg_parse_latency_ms.toFixed(2)}ms</code>
              </div>
              <div className="phase-detail">
                <span>Average Semantic Time:</span>
                <code>{metrics.avg_semantic_latency_ms.toFixed(2)}ms</code>
              </div>
              <div className="phase-detail">
                <span>Average Normalize Time:</span>
                <code>{metrics.avg_normalize_latency_ms.toFixed(2)}ms</code>
              </div>
              <div className="phase-detail">
                <span>Average Z3 Time:</span>
                <code>{metrics.avg_z3_latency_ms.toFixed(2)}ms</code>
              </div>
            </div>
          </div>
        )}
      </section>

      {/* Recommendations */}
      <section className="dashboard-section">
        <h3
          className="section-title"
          onClick={() =>
            setExpandedSection(
              expandedSection === "recommendations" ? null : "recommendations"
            )
          }
        >
          💡 Recommendations ({recommendations.length})
        </h3>
        {expandedSection === "recommendations" && (
          <div className="section-content">{renderRecommendations()}</div>
        )}
      </section>

      {/* Recent Verifications */}
      <section className="dashboard-section">
        <h3
          className="section-title"
          onClick={() =>
            setExpandedSection(
              expandedSection === "recent" ? null : "recent"
            )
          }
        >
          📝 Recent Verifications
        </h3>
        {expandedSection === "recent" && (
          <div className="section-content">{renderRecentVerifications()}</div>
        )}
      </section>

      {/* Trend Chart */}
      <section className="dashboard-section">
        <h3
          className="section-title"
          onClick={() =>
            setExpandedSection(
              expandedSection === "trend" ? null : "trend"
            )
          }
        >
          📈 Trend
        </h3>
        {expandedSection === "trend" && (
          <div className="section-content">{renderTrendChart()}</div>
        )}
      </section>
    </div>
  );
};

export default ProfilingDashboard;
