/// LSP Diagnostic Extraction & Explain Panel Wiring
/// 
/// This module extracts counterexample data from LSP diagnostics
/// and bridges it to the Explain panel for interactive visualization.
///
/// Flow:
/// 1. LSP emits Diagnostic with counterexample.v2 schema
/// 2. extract_counterexample() parses the data
/// 3. updateExplainPanel() renders in Sentinel UI
/// 4. User interacts (expand, repair, navigate)

import { escapeHtml } from "./lib/html";
import { updateExplainPanel, showExplainPanel } from "./explain_integration";

export interface CounterexampleV2 {
  schema: "aura.counterexample.v2";
  assertion: string;
  failed_assertion_type: "requires" | "ensures" | "assert";
  inputs: Record<string, any>;
  outputs: Record<string, any>;
  variable_traces: VariableTrace[];
  repair_hints: RepairHint[];
  unsat_core?: string[];
  failure_explanation?: string;
}

export interface VariableTrace {
  name: string;
  type: string;
  defined_at: SourceLocation;
  assignments: Assignment[];
  final_value: any;
  relevance: "critical" | "supporting" | "background";
}

export interface Assignment {
  line: number;
  column: number;
  value: any;
  description: string;
}

export interface SourceLocation {
  file: string;
  line: number;
  column: number;
}

export interface RepairHint {
  type: "strengthen_precond" | "weaken_postcond" | "add_invariant" | "relax_assertion";
  severity: "error" | "warning" | "info";
  description: string;
  suggested_code?: string;
  affected_lines: number[];
}

/// Extract counterexample from LSP diagnostic
export function extractCounterexample(diagnostic: any): CounterexampleV2 | null {
  if (!diagnostic.data?.counterexample) {
    return null;
  }

  const cx = diagnostic.data.counterexample;

  // Validate schema version
  if (cx.schema !== "aura.counterexample.v2") {
    console.warn(`Unknown counterexample schema: ${cx.schema}`);
    return null;
  }

  return {
    schema: cx.schema,
    assertion: cx.assertion || diagnostic.message,
    failed_assertion_type: cx.failed_assertion_type || "assert",
    inputs: cx.inputs || {},
    outputs: cx.outputs || {},
    variable_traces: cx.variable_traces || [],
    repair_hints: cx.repair_hints || [],
    unsat_core: cx.unsat_core,
    failure_explanation: cx.failure_explanation,
  };
}

/// Rank variable traces by relevance
export function rankVariableTraces(traces: VariableTrace[]): VariableTrace[] {
  const relevanceOrder = { critical: 0, supporting: 1, background: 2 };
  return traces.sort((a, b) => {
    const aOrder = relevanceOrder[a.relevance] || 2;
    const bOrder = relevanceOrder[b.relevance] || 2;
    return aOrder - bOrder;
  });
}

/// Generate repair hints from proof failure analysis
export function generateRepairHints(cx: CounterexampleV2): RepairHint[] {
  const hints: RepairHint[] = [];

  const failureType = cx.failed_assertion_type;

  if (failureType === "requires") {
    // Input violated precondition
    hints.push({
      type: "strengthen_precond",
      severity: "error",
      description: "The caller passed invalid input. Add precondition check.",
      suggested_code: `requires ${cx.assertion}`,
      affected_lines: [],
    });
  } else if (failureType === "ensures") {
    // Output violated postcondition
    hints.push({
      type: "weaken_postcond",
      severity: "warning",
      description: "The function cannot guarantee this output. Relax the postcondition.",
      suggested_code: `ensures ${cx.assertion}`,
      affected_lines: [],
    });
  } else if (failureType === "assert") {
    // Assertion failed - could be multiple causes
    hints.push(
      {
        type: "strengthen_precond",
        severity: "error",
        description: "Add input constraints to prevent this case.",
        suggested_code: `requires condition`,
        affected_lines: [],
      },
      {
        type: "add_invariant",
        severity: "warning",
        description: "Add loop/data invariant to track values.",
        suggested_code: `invariant condition`,
        affected_lines: [],
      },
      {
        type: "relax_assertion",
        severity: "info",
        description: "Make the assertion less strict if overconstrained.",
        suggested_code: `assert weaker_condition`,
        affected_lines: [],
      }
    );
  }

  return hints;
}

/// Bridge LSP diagnostic to Explain panel
export function handleProofFailure(diagnostic: any): void {
  // Extract counterexample
  const cx = extractCounterexample(diagnostic);
  if (!cx) {
    console.log("No counterexample in diagnostic");
    return;
  }

  // Rank variables by relevance
  const rankedTraces = rankVariableTraces(cx.variable_traces);

  // Generate repair hints
  const hints = generateRepairHints(cx);

  // Prepare payload for Explain panel
  const payload = {
    message: diagnostic.message,
    line: diagnostic.range?.start?.line || 0,
    column: diagnostic.range?.start?.character || 0,
    file: diagnostic.source || "unknown",
    data: {
      counterexample: {
        inputs: cx.inputs,
        outputs: cx.outputs,
        assertion: cx.assertion,
        assertion_type: cx.failed_assertion_type,
        failure_explanation: cx.failure_explanation,
      },
      variableTraces: rankedTraces,
      repairHints: hints,
    },
  };

  // Update Explain panel
  updateExplainPanel(payload);

  // Show panel to user
  showExplainPanel();

  // Log for debugging
  console.log("Proof failure:", {
    message: diagnostic.message,
    traces: rankedTraces.length,
    hints: hints.length,
  });
}

/// Apply repair hint to source code
export async function applyRepairHint(hint: RepairHint, diagnostic: any): Promise<void> {
  // This would typically call applyWorkspaceEdit to modify the source
  console.log(`Applying repair: ${hint.type}`);
  console.log(`Suggested: ${hint.suggested_code}`);

  // In real implementation:
  // 1. Create TextEdit for the change
  // 2. Send WorkspaceEdit to LSP
  // 3. Re-run verifier
  // 4. Update Explain panel with new result
}

// ============ INTEGRATION WITH MAIN.TS ============

/// Hook into main.ts diagnostic rendering
export function setupExplainPanelForDiagnostics(): void {
  // This would be called during Sentinel initialization
  console.log("Explain panel diagnostic integration initialized");

  // Listen for proof failures (in main.ts):
  // When a diagnostic with severity === "error" and source === "aura/verify":
  //   → Call handleProofFailure(diagnostic)
}

// ============ TEST HELPERS ============

export function createMockCounterexample(
  assertion: string,
  inputs: Record<string, any>,
  outputs: Record<string, any>
): CounterexampleV2 {
  return {
    schema: "aura.counterexample.v2",
    assertion,
    failed_assertion_type: "assert",
    inputs,
    outputs,
    variable_traces: [
      {
        name: "x",
        type: "i32",
        defined_at: { file: "test.aura", line: 5, column: 8 },
        assignments: [
          { line: 8, column: 8, value: 1, description: "initial assignment" },
          { line: 10, column: 8, value: 2, description: "first update" },
        ],
        final_value: 2,
        relevance: "critical",
      },
    ],
    repair_hints: [],
  };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_extract_counterexample() {
    let diagnostic = serde_json::json!({
      "message": "Assertion failed: x > 0",
      "range": {
        "start": { "line": 10, "character": 5 }
      },
      "data": {
        "counterexample": {
          "schema": "aura.counterexample.v2",
          "assertion": "x > 0",
          "failed_assertion_type": "assert",
          "inputs": { "x": -5 },
          "outputs": { "x": -5 },
          "variable_traces": [],
          "repair_hints": []
        }
      }
    });

    let cx = extractCounterexample(diagnostic.as_object().unwrap());
    assert!(cx.is_some());
    assert_eq!(cx.unwrap().assertion, "x > 0");
  }

  #[test]
  fn test_generate_repair_hints() {
    let cx = createMockCounterexample("x > 0", { "x": -5 }, { "x": -5 });
    let hints = generateRepairHints(&cx);

    assert!(!hints.is_empty());
    assert!(hints.iter().any(|h| h.type == "strengthen_precond"));
  }

  #[test]
  fn test_rank_variable_traces() {
    let traces = vec![
      VariableTrace {
        name: "x".to_string(),
        relevance: "background",
        // ... other fields
      },
      VariableTrace {
        name: "y".to_string(),
        relevance: "critical",
        // ... other fields
      },
    ];

    let ranked = rankVariableTraces(traces);
    assert_eq!(ranked[0].name, "y"); // critical first
    assert_eq!(ranked[1].name, "x"); // background last
  }
}
