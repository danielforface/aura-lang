/// Memory State Debugger Panel Component
///
/// Displays ownership state, regions, and verified data structures
/// in the VSCode debugger sidebar for Pillar 3 integration

import React, { useState, useEffect } from "react";

export interface OwnershipDisplay {
  variable_name: string;
  type_name: string;
  state: string;
  transitions: StateTransition[];
  borrows: BorrowInfo[];
}

export interface StateTransition {
  from_state: string;
  to_state: string;
  at_line: number;
  at_column: number;
  reason: string;
}

export interface BorrowInfo {
  borrowed_at_line: number;
  borrowed_at_column: number;
  borrow_type: string;
  borrowed_until: BorrowEnd;
}

export interface BorrowEnd {
  type: "AtLine" | "EndOfScope";
  line?: number;
}

export interface RegionMemoryDisplay {
  region_name: string;
  total_allocated: number;
  allocation_count: number;
  allocations: string[];
}

export interface UseAfterMoveError {
  variable_name: string;
  moved_at_line: number;
  use_at_line: number;
  use_at_column: number;
  severity: string;
}

export interface MemoryDebuggerProps {
  ownership: Record<string, OwnershipDisplay>;
  regions: Record<string, RegionMemoryDisplay>;
  errors: UseAfterMoveError[];
  onNavigateToLine?: (line: number) => void;
}

export const MemoryDebuggerPanel: React.FC<MemoryDebuggerProps> = ({
  ownership,
  regions,
  errors,
  onNavigateToLine,
}) => {
  const [expandedVar, setExpandedVar] = useState<string | null>(null);
  const [expandedRegion, setExpandedRegion] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | "alive" | "moved" | "borrowed">(
    "all"
  );

  // ========== FILTER OWNERSHIP VARIABLES ==========

  const filteredOwnership = Object.entries(ownership).filter(([_, display]) => {
    if (filter === "all") return true;
    if (filter === "alive") return display.state.includes("Alive");
    if (filter === "moved") return display.state.includes("Moved");
    if (filter === "borrowed") return display.state.includes("Borrowed");
    return true;
  });

  // ========== RENDER OWNERSHIP STATE ==========

  const renderOwnershipVariable = (
    name: string,
    display: OwnershipDisplay
  ) => {
    const isExpanded = expandedVar === name;
    const stateIcon =
      display.state.includes("Alive") && !display.state.includes("Borrowed")
        ? "🟢"
        : display.state.includes("Moved")
        ? "❌"
        : display.state.includes("Borrowed")
        ? "📋"
        : "⚪";

    return (
      <div key={name} className="ownership-variable">
        <div
          className="var-header"
          onClick={() => setExpandedVar(isExpanded ? null : name)}
        >
          <span className="state-icon">{stateIcon}</span>
          <span className="var-name">{name}</span>
          <span className="var-type">: {display.type_name}</span>
          <span className="expand-indicator">{isExpanded ? "▼" : "▶"}</span>
        </div>

        {isExpanded && (
          <div className="var-details">
            {/* State History */}
            {display.transitions.length > 0 && (
              <div className="transitions-section">
                <h4>State History</h4>
                <div className="transitions-list">
                  {display.transitions.map((transition, idx) => (
                    <div key={idx} className="transition">
                      <span className="from-state">{transition.from_state}</span>
                      <span className="arrow">→</span>
                      <span className="to-state">{transition.to_state}</span>
                      <span className="at-location">
                        Line {transition.at_line}
                      </span>
                      <span className="reason">{transition.reason}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Borrows */}
            {display.borrows.length > 0 && (
              <div className="borrows-section">
                <h4>Borrows</h4>
                <div className="borrows-list">
                  {display.borrows.map((borrow, idx) => (
                    <div key={idx} className="borrow">
                      <span className="borrow-type">{borrow.borrow_type}</span>
                      <span className="borrow-location">
                        @ Line {borrow.borrowed_at_line}
                      </span>
                      {borrow.borrowed_until.type === "AtLine" && (
                        <span className="borrow-until">
                          until Line {borrow.borrowed_until.line}
                        </span>
                      )}
                      {borrow.borrowed_until.type === "EndOfScope" && (
                        <span className="borrow-until">until end of scope</span>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    );
  };

  // ========== RENDER REGIONS ==========

  const renderRegion = (name: string, region: RegionMemoryDisplay) => {
    const isExpanded = expandedRegion === name;

    return (
      <div key={name} className="region-card">
        <div
          className="region-header"
          onClick={() => setExpandedRegion(isExpanded ? null : name)}
        >
          <span className="region-name">{name}</span>
          <span className="memory-badge">
            {region.total_allocated}B / {region.allocation_count} allocs
          </span>
          <span className="expand-indicator">{isExpanded ? "▼" : "▶"}</span>
        </div>

        {isExpanded && (
          <div className="region-details">
            <div className="allocation-list">
              {region.allocations.map((alloc, idx) => (
                <div key={idx} className="allocation-item">
                  <code>{alloc}</code>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    );
  };

  // ========== RENDER ERRORS ==========

  const renderErrors = () => {
    if (errors.length === 0) {
      return <div className="no-errors">✓ No memory safety errors detected</div>;
    }

    return (
      <div className="errors-list">
        {errors.map((error, idx) => (
          <div key={idx} className={`error-card severity-${error.severity}`}>
            <div className="error-header">
              <span className="error-icon">❌</span>
              <span className="error-title">Use-After-Move</span>
              <span className="error-var">{error.variable_name}</span>
            </div>
            <div className="error-details">
              <p>
                Variable moved at <strong>Line {error.moved_at_line}</strong>
              </p>
              <p>
                Used again at{" "}
                <strong>
                  Line {error.use_at_line}:{error.use_at_column}
                </strong>
              </p>
            </div>
            <button
              className="jump-button"
              onClick={() => onNavigateToLine?.(error.use_at_line)}
            >
              Go to Error
            </button>
          </div>
        ))}
      </div>
    );
  };

  // ========== MAIN RENDER ==========

  return (
    <div className="memory-debugger-panel">
      {/* Header */}
      <div className="panel-header">
        <h3>🔒 Memory Safety</h3>
        <span className="var-count">
          {filteredOwnership.length} variable(s)
        </span>
      </div>

      {/* Error Section */}
      {errors.length > 0 && (
        <section className="panel-section errors-section">
          <h4>⚠️ Memory Errors</h4>
          {renderErrors()}
        </section>
      )}

      {/* Filter Buttons */}
      <div className="filter-buttons">
        {(["all", "alive", "moved", "borrowed"] as const).map((f) => (
          <button
            key={f}
            className={`filter-btn ${filter === f ? "active" : ""}`}
            onClick={() => setFilter(f)}
          >
            {f === "all" && "All"}
            {f === "alive" && "🟢 Alive"}
            {f === "moved" && "❌ Moved"}
            {f === "borrowed" && "📋 Borrowed"}
          </button>
        ))}
      </div>

      {/* Ownership Variables */}
      <section className="panel-section ownership-section">
        <h4>Ownership State</h4>
        {filteredOwnership.length === 0 ? (
          <div className="empty-message">No variables match filter</div>
        ) : (
          <div className="ownership-list">
            {filteredOwnership.map(([name, display]) =>
              renderOwnershipVariable(name, display)
            )}
          </div>
        )}
      </section>

      {/* Regions */}
      {Object.keys(regions).length > 0 && (
        <section className="panel-section regions-section">
          <h4>Memory Regions</h4>
          <div className="regions-list">
            {Object.entries(regions).map(([name, region]) =>
              renderRegion(name, region)
            )}
          </div>
        </section>
      )}
    </div>
  );
};

export default MemoryDebuggerPanel;
