/// Enhanced Explain Panel Component for Proof Failures
/// 
/// Interactive visualization of counterexamples with:
/// - Assertion details
/// - Variable trace timeline
/// - Repair hint suggestions
/// - Source code navigation
///
/// Built on React hooks for real-time updates from LSP

import React, { useState, useCallback, useMemo } from "react";
import {
  CounterexampleV2,
  VariableTrace,
  RepairHint,
  applyRepairHint,
} from "./lsp_diagnostic_extraction";

export interface ExplainPanelProps {
  diagnostic?: any;
  counterexample?: CounterexampleV2;
  onNavigateToLocation?: (file: string, line: number, column: number) => void;
  onApplyRepair?: (hint: RepairHint) => Promise<void>;
}

interface ExplainPanelState {
  isExpanded: boolean;
  selectedTraceIndex: number | null;
  hoveredTraceIndex: number | null;
  activeRepairIndex: number | null;
  showDetails: boolean;
}

export const ExplainPanel: React.FC<ExplainPanelProps> = ({
  diagnostic,
  counterexample,
  onNavigateToLocation,
  onApplyRepair,
}) => {
  const [state, setState] = useState<ExplainPanelState>({
    isExpanded: true,
    selectedTraceIndex: null,
    hoveredTraceIndex: null,
    activeRepairIndex: null,
    showDetails: false,
  });

  if (!counterexample) {
    return null;
  }

  const cx = counterexample;

  // ========== RENDER ASSERTION ==========

  const renderAssertion = () => (
    <div className="explain-assertion-box">
      <div className="assertion-header">
        <span className="assertion-type-badge">
          {cx.failed_assertion_type.toUpperCase()}
        </span>
        <span className="assertion-text">{cx.assertion}</span>
      </div>
      {cx.failure_explanation && (
        <div className="failure-explanation">{cx.failure_explanation}</div>
      )}
      <div className="input-output-grid">
        <div className="input-column">
          <h4>Inputs</h4>
          <code className="value-tree">{JSON.stringify(cx.inputs, null, 2)}</code>
        </div>
        <div className="output-column">
          <h4>Outputs</h4>
          <code className="value-tree">{JSON.stringify(cx.outputs, null, 2)}</code>
        </div>
      </div>
    </div>
  );

  // ========== RENDER VARIABLE TRACE ==========

  const renderVariableTrace = (trace: VariableTrace, index: number) => {
    const isSelected = state.selectedTraceIndex === index;
    const isHovered = state.hoveredTraceIndex === index;

    const handleTraceClick = () => {
      setState({
        ...state,
        selectedTraceIndex: isSelected ? null : index,
      });
    };

    const handleTraceHover = (hovered: boolean) => {
      setState({
        ...state,
        hoveredTraceIndex: hovered ? index : null,
      });
    };

    return (
      <div
        key={`trace-${index}`}
        className={`explain-trace ${isSelected ? "selected" : ""} ${
          isHovered ? "hovered" : ""
        } relevance-${trace.relevance}`}
        onClick={handleTraceClick}
        onMouseEnter={() => handleTraceHover(true)}
        onMouseLeave={() => handleTraceHover(false)}
      >
        {/* Trace header */}
        <div className="trace-header">
          <span className="trace-name">{trace.name}</span>
          <span className="trace-type">: {trace.type}</span>
          <span className={`relevance-badge ${trace.relevance}`}>
            {trace.relevance}
          </span>
          <span className="final-value">{JSON.stringify(trace.final_value)}</span>
        </div>

        {/* Timeline of assignments */}
        {isSelected && trace.assignments.length > 0 && (
          <div className="trace-timeline">
            <div className="definition-point">
              <span className="line-ref">
                {trace.defined_at.file}:{trace.defined_at.line}
              </span>
              <span className="phase-label">defined</span>
              <button
                className="nav-button"
                onClick={(e) => {
                  e.stopPropagation();
                  onNavigateToLocation?.(
                    trace.defined_at.file,
                    trace.defined_at.line,
                    trace.defined_at.column
                  );
                }}
              >
                → Jump
              </button>
            </div>

            {trace.assignments.map((assignment, assignIdx) => (
              <div key={`assign-${assignIdx}`} className="assignment-point">
                <span className="line-ref">
                  {assignment.line}:{assignment.column}
                </span>
                <span className="phase-label">assign</span>
                <span className="assignment-value">
                  = {JSON.stringify(assignment.value)}
                </span>
                <span className="assignment-desc">{assignment.description}</span>
                <button
                  className="nav-button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onNavigateToLocation?.(
                      trace.defined_at.file,
                      assignment.line,
                      assignment.column
                    );
                  }}
                >
                  → Jump
                </button>
              </div>
            ))}

            <div className="final-point">
              <span className="phase-label">final</span>
              <span className="final-value">= {JSON.stringify(trace.final_value)}</span>
            </div>
          </div>
        )}
      </div>
    );
  };

  // ========== RENDER REPAIR HINT ==========

  const renderRepairHint = (hint: RepairHint, index: number) => {
    const isActive = state.activeRepairIndex === index;
    const severityIcon = {
      error: "❌",
      warning: "⚠️",
      info: "ℹ️",
    }[hint.severity];

    const handleApply = async () => {
      if (onApplyRepair) {
        await onApplyRepair(hint);
      }
      setState({
        ...state,
        activeRepairIndex: null,
      });
    };

    return (
      <div
        key={`hint-${index}`}
        className={`explain-repair-hint severity-${hint.severity}`}
      >
        <div className="hint-header">
          <span className="severity-icon">{severityIcon}</span>
          <span className="hint-type">{hint.type}</span>
          <button
            className="expand-button"
            onClick={() => {
              setState({
                ...state,
                activeRepairIndex: isActive ? null : index,
              });
            }}
          >
            {isActive ? "▼" : "▶"}
          </button>
        </div>

        <div className="hint-description">{hint.description}</div>

        {isActive && hint.suggested_code && (
          <div className="hint-details">
            <div className="suggested-code-box">
              <span className="label">Suggested:</span>
              <code className="suggested-code">{hint.suggested_code}</code>
            </div>
            {hint.affected_lines.length > 0 && (
              <div className="affected-lines">
                <span className="label">Affects lines:</span>
                <span className="line-numbers">
                  {hint.affected_lines.join(", ")}
                </span>
              </div>
            )}
            <button className="apply-button" onClick={handleApply}>
              Apply Suggestion
            </button>
          </div>
        )}
      </div>
    );
  };

  // ========== MAIN RENDER ==========

  return (
    <div className="explain-panel">
      <div className="explain-panel-header">
        <h3>Proof Failure Analysis</h3>
        <span className="diagnostic-location">
          {diagnostic?.range?.start?.line}:
          {diagnostic?.range?.start?.character}
        </span>
      </div>

      {/* Assertion Details */}
      <section className="section assertions-section">
        <h4>Assertion That Failed</h4>
        {renderAssertion()}
      </section>

      {/* Variable Traces */}
      {cx.variable_traces.length > 0 && (
        <section className="section traces-section">
          <h4>Variable Traces ({cx.variable_traces.length})</h4>
          <p className="section-hint">
            Click a variable to see its lifecycle from definition to final value
          </p>
          <div className="traces-container">
            {cx.variable_traces.map((trace, idx) => renderVariableTrace(trace, idx))}
          </div>
        </section>
      )}

      {/* Repair Hints */}
      {cx.repair_hints.length > 0 && (
        <section className="section repairs-section">
          <h4>Repair Suggestions ({cx.repair_hints.length})</h4>
          <p className="section-hint">
            Try one of these approaches to fix the failing assertion
          </p>
          <div className="repairs-container">
            {cx.repair_hints.map((hint, idx) => renderRepairHint(hint, idx))}
          </div>
        </section>
      )}

      {/* UNSAT Core (Advanced) */}
      {cx.unsat_core && state.showDetails && (
        <section className="section advanced-section">
          <h4>UNSAT Core (Advanced)</h4>
          <pre className="unsat-core-display">{cx.unsat_core.join("\n")}</pre>
        </section>
      )}

      {/* Footer */}
      <div className="explain-panel-footer">
        <button
          className="toggle-details-button"
          onClick={() => {
            setState({
              ...state,
              showDetails: !state.showDetails,
            });
          }}
        >
          {state.showDetails ? "Hide" : "Show"} Advanced Details
        </button>
      </div>
    </div>
  );
};

export default ExplainPanel;
