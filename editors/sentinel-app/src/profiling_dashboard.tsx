import React, { useState, useCallback, useEffect } from 'react';
import './profiling_dashboard.css';

/**
 * ProfilingMetrics represents telemetry from a single proof run.
 */
interface ProfilingMetrics {
  filePath: string;
  totalDurationMs: number;
  parseMs?: number;
  semanticAnalysisMs?: number;
  normalizationMs?: number;
  z3SolveMs?: number;
  otherMs?: number;
  smtCalls: number;
  smtSymbols: number;
  proofStatus: 'passed' | 'failed' | 'timeout' | 'error';
  timestamp: number;  // Unix timestamp in ms
}

/**
 * LatencyPercentiles tracks P50, P95, P99 latencies.
 */
interface LatencyPercentiles {
  p50: number;
  p95: number;
  p99: number;
  samples: number;
}

/**
 * ProfilingDashboard displays telemetry and performance metrics for Aura verification.
 * 
 * Features:
 * - Real-time latency tracking (P50, P95, P99)
 * - Phase breakdown chart (parse, semantic, Z3, etc.)
 * - Cache hit rate visualization
 * - Performance tuning recommendations
 * - Historical trend analysis
 */
export const ProfilingDashboard: React.FC<{
  metrics?: ProfilingMetrics[];
  onTuningChange?: (config: Partial<TuningConfig>) => void;
}> = ({ metrics = [], onTuningChange }) => {
  const [selectedMetric, setSelectedMetric] = useState<ProfilingMetrics | null>(null);
  const [showTrendChart, setShowTrendChart] = useState(true);
  const [percentiles, setPercentiles] = useState<LatencyPercentiles | null>(null);

  // Calculate percentiles from metrics
  useEffect(() => {
    if (metrics.length > 0) {
      const durations = metrics.map(m => m.totalDurationMs).sort((a, b) => a - b);
      const p50Idx = Math.floor(durations.length * 0.5);
      const p95Idx = Math.floor(durations.length * 0.95);
      const p99Idx = Math.floor(durations.length * 0.99);

      setPercentiles({
        p50: durations[p50Idx],
        p95: durations[p95Idx],
        p99: durations[p99Idx],
        samples: durations.length,
      });
    }
  }, [metrics]);

  const handleSelectMetric = useCallback((metric: ProfilingMetrics) => {
    setSelectedMetric(metric);
  }, []);

  const calculatePhaseBreakdown = (metric: ProfilingMetrics) => {
    const phases = [
      { name: 'Parse', ms: metric.parseMs || 0 },
      { name: 'Semantic', ms: metric.semanticAnalysisMs || 0 },
      { name: 'Normalize', ms: metric.normalizationMs || 0 },
      { name: 'Z3 Solve', ms: metric.z3SolveMs || 0 },
      { name: 'Other', ms: metric.otherMs || 0 },
    ];
    return phases.filter(p => p.ms > 0);
  };

  const recommendTuning = (): string[] => {
    if (!percentiles) return [];
    const recs: string[] = [];

    if (percentiles.p95 > 200) {
      recs.push('⚠️ P95 > 200ms: Increase cache size and reduce Z3 timeout');
    }
    if (percentiles.p99 > 500) {
      recs.push('⚠️ P99 > 500ms: Enable parallel verification and solver tuning');
    }
    if (selectedMetric?.z3SolveMs && selectedMetric.z3SolveMs > selectedMetric.totalDurationMs * 0.7) {
      recs.push('💡 Z3 solver time > 70%: Consider pre-population of symbols');
    }
    if (percentiles.p95 < 100) {
      recs.push('✅ P95 < 100ms: Performance is excellent');
    }

    return recs;
  };

  return (
    <div className="profiling-dashboard">
      {/* Header */}
      <div className="dashboard-header">
        <h2>Proof Pipeline Telemetry</h2>
        <div className="header-controls">
          <button
            className={`toggle-btn ${showTrendChart ? 'active' : ''}`}
            onClick={() => setShowTrendChart(!showTrendChart)}
          >
            {showTrendChart ? 'Hide' : 'Show'} Trend
          </button>
        </div>
      </div>

      {/* Latency Summary */}
      {percentiles && (
        <div className="latency-summary">
          <div className="summary-card">
            <div className="metric-label">P50 (Median)</div>
            <div className="metric-value">{percentiles.p50}ms</div>
            <div className="metric-description">50% of proofs complete within this time</div>
          </div>

          <div className="summary-card">
            <div className="metric-label">P95 (95th Percentile)</div>
            <div className={`metric-value ${percentiles.p95 > 200 ? 'warning' : 'ok'}`}>
              {percentiles.p95}ms
            </div>
            <div className="metric-description">Target: &lt;200ms for daily-driver UX</div>
          </div>

          <div className="summary-card">
            <div className="metric-label">P99 (99th Percentile)</div>
            <div className={`metric-value ${percentiles.p99 > 500 ? 'critical' : 'ok'}`}>
              {percentiles.p99}ms
            </div>
            <div className="metric-description">Worst 1% of proofs</div>
          </div>

          <div className="summary-card">
            <div className="metric-label">Samples</div>
            <div className="metric-value">{percentiles.samples}</div>
            <div className="metric-description">Proof runs analyzed</div>
          </div>
        </div>
      )}

      {/* Recommendations */}
      {recommendTuning().length > 0 && (
        <div className="recommendations-section">
          <h3>🔧 Performance Recommendations</h3>
          <div className="recommendations-list">
            {recommendTuning().map((rec, idx) => (
              <div key={idx} className="recommendation-item">
                {rec}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Metrics List */}
      <div className="metrics-section">
        <h3>Recent Verification Runs</h3>
        <div className="metrics-list">
          {metrics.slice(-10).reverse().map((metric, idx) => (
            <div
              key={idx}
              className={`metric-row ${selectedMetric === metric ? 'selected' : ''}`}
              onClick={() => handleSelectMetric(metric)}
            >
              <div className="metric-file">
                {metric.filePath.split('/').pop()}
              </div>
              <div className="metric-status">
                <span className={`status-badge status-${metric.proofStatus}`}>
                  {metric.proofStatus.toUpperCase()}
                </span>
              </div>
              <div className="metric-time">{metric.totalDurationMs}ms</div>
              <div className="metric-smt">
                {metric.smtCalls} SMT calls, {metric.smtSymbols} symbols
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Phase Breakdown */}
      {selectedMetric && (
        <div className="phase-breakdown-section">
          <h3>Phase Breakdown: {selectedMetric.filePath}</h3>
          
          <div className="phase-chart">
            <div className="phase-bars">
              {calculatePhaseBreakdown(selectedMetric).map((phase) => (
                <div key={phase.name} className="phase-item">
                  <div className="phase-label">{phase.name}</div>
                  <div className="phase-bar-container">
                    <div
                      className="phase-bar"
                      style={{
                        width: `${(phase.ms / selectedMetric.totalDurationMs) * 100}%`,
                      }}
                    />
                  </div>
                  <div className="phase-value">{phase.ms}ms</div>
                </div>
              ))}
            </div>
          </div>

          <div className="phase-details">
            <div className="detail-item">
              <span className="detail-label">Total Time:</span>
              <span className="detail-value">{selectedMetric.totalDurationMs}ms</span>
            </div>
            <div className="detail-item">
              <span className="detail-label">SMT Calls:</span>
              <span className="detail-value">{selectedMetric.smtCalls}</span>
            </div>
            <div className="detail-item">
              <span className="detail-label">SMT Symbols:</span>
              <span className="detail-value">{selectedMetric.smtSymbols}</span>
            </div>
            <div className="detail-item">
              <span className="detail-label">Status:</span>
              <span className={`detail-value status-${selectedMetric.proofStatus}`}>
                {selectedMetric.proofStatus}
              </span>
            </div>
          </div>
        </div>
      )}

      {/* Trend Chart */}
      {showTrendChart && metrics.length > 1 && (
        <div className="trend-section">
          <h3>Latency Trend (Last {Math.min(50, metrics.length)} Runs)</h3>
          <div className="trend-chart">
            <svg viewBox="0 0 800 200" preserveAspectRatio="xMidYMid meet">
              {/* Grid lines */}
              <line x1="40" y1="20" x2="40" y2="160" stroke="#444" strokeWidth="1" />
              <line x1="40" y1="160" x2="780" y2="160" stroke="#444" strokeWidth="1" />

              {/* Y-axis labels (milliseconds) */}
              <text x="10" y="25" fontSize="12" fill="#999">
                Max
              </text>
              <text x="10" y="165" fontSize="12" fill="#999">
                0ms
              </text>

              {/* Plot lines */}
              {metrics.slice(-50).map((m, idx) => {
                const x = 40 + (idx / Math.max(1, metrics.length - 1)) * 740;
                const maxMs = Math.max(...metrics.map(m => m.totalDurationMs));
                const y = 160 - (m.totalDurationMs / maxMs) * 140;

                return (
                  <circle
                    key={idx}
                    cx={x}
                    cy={y}
                    r="3"
                    fill={m.proofStatus === 'passed' ? '#4ec9b0' : '#f44747'}
                    opacity="0.7"
                  />
                );
              })}
            </svg>
          </div>
        </div>
      )}
    </div>
  );
};

/**
 * TuningConfig represents the adaptive performance tuning parameters.
 */
interface TuningConfig {
  cacheMaxEntries: number;
  z3TimeoutMs: number;
  enableParallel: boolean;
  parallelThreads: number;
}

export default ProfilingDashboard;
