// Sentinel Explain Panel: Interactive Counterexample Viewer
//
// Renders typed counterexamples from proof failures in an expandable tree view.
// Shows:
// - Typed values (primitives, records, enums, arrays)
// - Variable traces (definition + assignment locations)
// - UNSAT core highlighting (variables in failing assertions)
// - Repair hints (suggested fixes for failing assertions)
//
// Integration: Called when proof fails with Diagnostic.data.counterexample

import React, { useState, useCallback } from 'react';
import { TypedValue } from './typed_value';
import './explain_panel.css';

export interface CounterexamplePayload {
  schema: string;  // 'aura.counterexample.v2'
  assertion: {
    text: string;
    location: SourceLocation;
  };
  failingVariables: VariableTrace[];
  typedValues: Record<string, TypedValue>;
  unsatCoreVariables: string[];
  repairHints: RepairHint[];
}

export interface VariableTrace {
  name: string;
  definedAt: SourceLocation;
  assignmentsAt: SourceLocation[];
  finalValue: string;
  type: string;
}

export interface RepairHint {
  kind: 'strengthen-precondition' | 'weaken-postcondition' | 'add-invariant' | 'relax-assertion';
  text: string;
  suggestedCode: string;
  confidence: 'high' | 'medium' | 'low';
}

export interface SourceLocation {
  file: string;
  line: number;
  column?: number;
}

/**
 * Main Explain Panel Component
 * Displayed when a proof fails
 */
export const ExplainPanel: React.FC<{
  counterexample: CounterexamplePayload;
  onNavigateToSource: (loc: SourceLocation) => void;
}> = ({ counterexample, onNavigateToSource }) => {
  const [expandedVariables, setExpandedVariables] = useState<Set<string>>(new Set());
  const [selectedHint, setSelectedHint] = useState<number | null>(null);

  const toggleVariable = useCallback((varName: string) => {
    setExpandedVariables(prev => {
      const next = new Set(prev);
      if (next.has(varName)) {
        next.delete(varName);
      } else {
        next.add(varName);
      }
      return next;
    });
  }, []);

  return (
    <div className="explain-panel">
      {/* Header: Failed Assertion */}
      <div className="explain-header">
        <h2>❌ Proof Failure</h2>
        <div className="assertion-display">
          <code>{counterexample.assertion.text}</code>
          <button
            className="btn-small"
            onClick={() => onNavigateToSource(counterexample.assertion.location)}
            title="Go to assertion"
          >
            → {counterexample.assertion.location.file}:{counterexample.assertion.location.line}
          </button>
        </div>
      </div>

      {/* Main Content */}
      <div className="explain-content">
        {/* Section 1: Failing Variables */}
        <section className="section variables-section">
          <h3>
            <span className="icon">📋</span>
            Counterexample Values ({counterexample.failingVariables.length})
          </h3>
          <div className="variables-list">
            {counterexample.failingVariables.map((variable, idx) => (
              <VariableCard
                key={idx}
                variable={variable}
                isExpanded={expandedVariables.has(variable.name)}
                onToggle={() => toggleVariable(variable.name)}
                isInUnsatCore={counterexample.unsatCoreVariables.includes(variable.name)}
                typedValue={counterexample.typedValues[variable.name]}
                onNavigate={onNavigateToSource}
              />
            ))}
          </div>
        </section>

        {/* Section 2: Variable Traces */}
        <section className="section traces-section">
          <h3>
            <span className="icon">🔍</span>
            Variable Traces (Definition → Assignment → Use)
          </h3>
          <div className="traces-container">
            {counterexample.failingVariables.map((variable) => (
              <VariableTraceView
                key={variable.name}
                variable={variable}
                onNavigate={onNavigateToSource}
              />
            ))}
          </div>
        </section>

        {/* Section 3: UNSAT Core */}
        {counterexample.unsatCoreVariables.length > 0 && (
          <section className="section unsat-core-section">
            <h3>
              <span className="icon">⚡</span>
              UNSAT Core Variables ({counterexample.unsatCoreVariables.length})
            </h3>
            <p className="explanation">
              These variables appear in proof steps that led to the failure.
            </p>
            <div className="unsat-core-list">
              {counterexample.unsatCoreVariables.map((varName, idx) => (
                <span key={idx} className="unsat-core-variable">
                  {varName}
                </span>
              ))}
            </div>
          </section>
        )}

        {/* Section 4: Repair Hints */}
        {counterexample.repairHints.length > 0 && (
          <section className="section repair-section">
            <h3>
              <span className="icon">💡</span>
              Repair Hints ({counterexample.repairHints.length})
            </h3>
            <div className="hints-list">
              {counterexample.repairHints.map((hint, idx) => (
                <RepairHintCard
                  key={idx}
                  hint={hint}
                  isSelected={selectedHint === idx}
                  onSelect={() => setSelectedHint(selectedHint === idx ? null : idx)}
                />
              ))}
            </div>
          </section>
        )}
      </div>

      {/* Footer: Summary */}
      <div className="explain-footer">
        <div className="summary">
          <p>
            <strong>{counterexample.failingVariables.length}</strong> variables found in counterexample
            {counterexample.unsatCoreVariables.length > 0 &&
              ` • <strong>${counterexample.unsatCoreVariables.length}</strong> in UNSAT core`}
            {counterexample.repairHints.length > 0 &&
              ` • <strong>${counterexample.repairHints.length}</strong> repair hints`}
          </p>
        </div>
      </div>
    </div>
  );
};

/**
 * Individual Variable Card (Expandable)
 */
const VariableCard: React.FC<{
  variable: VariableTrace;
  isExpanded: boolean;
  onToggle: () => void;
  isInUnsatCore: boolean;
  typedValue?: TypedValue;
  onNavigate: (loc: SourceLocation) => void;
}> = ({ variable, isExpanded, onToggle, isInUnsatCore, typedValue, onNavigate }) => {
  return (
    <div className={`variable-card ${isInUnsatCore ? 'in-unsat-core' : ''}`}>
      <div className="variable-header" onClick={onToggle}>
        <span className={`expand-icon ${isExpanded ? 'expanded' : ''}`}>▶</span>
        <span className="variable-name">{variable.name}</span>
        <span className="variable-type">: {variable.type}</span>
        {isInUnsatCore && <span className="badge badge-unsat-core">UNSAT Core</span>}
      </div>

      {isExpanded && (
        <div className="variable-details">
          {/* Final Value */}
          <div className="detail-row">
            <span className="detail-label">Final Value:</span>
            <code className="detail-value">{variable.finalValue}</code>
          </div>

          {/* Typed Value Tree (if available) */}
          {typedValue && (
            <div className="detail-row">
              <span className="detail-label">Typed Breakdown:</span>
              <TypedValueTree value={typedValue} />
            </div>
          )}

          {/* Definition Location */}
          <div className="detail-row">
            <span className="detail-label">Defined at:</span>
            <LocationLink location={variable.definedAt} onNavigate={onNavigate} />
          </div>

          {/* Assignment Locations */}
          {variable.assignmentsAt.length > 0 && (
            <div className="detail-row">
              <span className="detail-label">Assigned at:</span>
              <div className="location-list">
                {variable.assignmentsAt.map((loc, idx) => (
                  <LocationLink key={idx} location={loc} onNavigate={onNavigate} />
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

/**
 * Variable Trace Timeline View
 */
const VariableTraceView: React.FC<{
  variable: VariableTrace;
  onNavigate: (loc: SourceLocation) => void;
}> = ({ variable, onNavigate }) => {
  return (
    <div className="variable-trace">
      <div className="trace-timeline">
        {/* Defined */}
        <div className="trace-event event-define">
          <div className="event-marker">◯</div>
          <div className="event-content">
            <div className="event-type">Defined</div>
            <LocationLink location={variable.definedAt} onNavigate={onNavigate} />
          </div>
        </div>

        {/* Assignments */}
        {variable.assignmentsAt.map((loc, idx) => (
          <React.Fragment key={idx}>
            <div className="trace-connector">↓</div>
            <div className="trace-event event-assign">
              <div className="event-marker">●</div>
              <div className="event-content">
                <div className="event-type">Assigned</div>
                <LocationLink location={loc} onNavigate={onNavigate} />
              </div>
            </div>
          </React.Fragment>
        ))}

        {/* Final Value */}
        <div className="trace-connector">↓</div>
        <div className="trace-event event-final">
          <div className="event-marker">✓</div>
          <div className="event-content">
            <div className="event-type">Final Value</div>
            <code>{variable.finalValue}</code>
          </div>
        </div>
      </div>
    </div>
  );
};

/**
 * Repair Hint Card
 */
const RepairHintCard: React.FC<{
  hint: RepairHint;
  isSelected: boolean;
  onSelect: () => void;
}> = ({ hint, isSelected, onSelect }) => {
  const confidenceColor = {
    high: '#4ec9b0',
    medium: '#dcdcaa',
    low: '#f44747',
  }[hint.confidence];

  const iconMap = {
    'strengthen-precondition': '🔒',
    'weaken-postcondition': '📤',
    'add-invariant': '↔️',
    'relax-assertion': '🎯',
  };

  return (
    <div
      className={`repair-hint-card ${isSelected ? 'selected' : ''}`}
      onClick={onSelect}
    >
      <div className="hint-header">
        <span className="hint-icon">{iconMap[hint.kind]}</span>
        <span className="hint-kind">{hint.kind.replace(/-/g, ' ')}</span>
        <span
          className="hint-confidence"
          style={{ color: confidenceColor }}
        >
          {hint.confidence}
        </span>
      </div>

      <div className="hint-text">{hint.text}</div>

      {isSelected && (
        <div className="hint-code">
          <pre>
            <code>{hint.suggestedCode}</code>
          </pre>
        </div>
      )}
    </div>
  );
};

/**
 * Typed Value Tree (Recursive)
 */
const TypedValueTree: React.FC<{ value: TypedValue; depth?: number }> = ({
  value,
  depth = 0,
}) => {
  const [expanded, setExpanded] = useState(depth === 0);

  if (depth > 5) {
    return <span className="tree-truncated">...</span>;
  }

  const isCompound =
    value.kind === 'Record' ||
    value.kind === 'Enum' ||
    value.kind === 'Array' ||
    value.kind === 'Tuple';

  return (
    <div className="typed-value-tree" style={{ paddingLeft: `${depth * 16}px` }}>
      {isCompound && (
        <span
          className="tree-toggle"
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? '▼' : '▶'}
        </span>
      )}

      <span className={`value-${value.kind.toLowerCase()}`}>
        {value.kind === 'Primitive' && (
          <>
            <span className="type-label">{value.typ}</span>
            <span className="value-literal">{value.value}</span>
          </>
        )}
        {value.kind === 'Record' && (
          <>
            <span className="type-label">{value.name}</span>
            {expanded && value.fields && (
              <div className="record-fields">
                {Object.entries(value.fields).map(([fname, fval]) => (
                  <div key={fname} className="field">
                    <span className="field-name">{fname}</span>:
                    <TypedValueTree value={fval} depth={depth + 1} />
                  </div>
                ))}
              </div>
            )}
          </>
        )}
        {value.kind === 'Enum' && (
          <>
            <span className="type-label">{value.name}</span>::
            <span className="variant-name">{value.variant}</span>
            {value.payload && expanded && (
              <TypedValueTree value={value.payload} depth={depth + 1} />
            )}
          </>
        )}
        {value.kind === 'Array' && (
          <>
            <span className="type-label">[{value.elementType}]</span>
            {expanded && value.elements && (
              <div className="array-elements">
                {value.elements.slice(0, 10).map((elem, idx) => (
                  <div key={idx} className="array-element">
                    [{idx}] = <TypedValueTree value={elem} depth={depth + 1} />
                  </div>
                ))}
                {value.elements.length > 10 && (
                  <div className="array-truncated">
                    ... and {value.elements.length - 10} more elements
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </span>
    </div>
  );
};

/**
 * Source Location Link (Clickable)
 */
const LocationLink: React.FC<{
  location: SourceLocation;
  onNavigate: (loc: SourceLocation) => void;
}> = ({ location, onNavigate }) => {
  return (
    <button
      className="btn-location"
      onClick={() => onNavigate(location)}
      title={`Go to ${location.file}:${location.line}`}
    >
      📍 {location.file}:{location.line}
      {location.column && `:${location.column}`}
    </button>
  );
};

export default ExplainPanel;
