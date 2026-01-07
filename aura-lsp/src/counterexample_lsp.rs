/// LSP Server Counterexample Transmission
/// 
/// This module handles the LSP server side: takes verification results
/// with counterexamples and transmits them to the client as LSP diagnostics.
///
/// Protocol:
/// 1. Verifier produces VerificationResult { counterexample, ... }
/// 2. LSP builds Diagnostic with counterexample.v2 in data field
/// 3. LSP publishes textDocument/publishDiagnostics
/// 4. Client receives and wires to Explain panel

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Counterexample model (verification output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCounterexample {
    pub assertion: String,
    pub assertion_type: String, // "requires", "ensures", "assert"
    pub inputs: HashMap<String, serde_json::Value>,
    pub outputs: HashMap<String, serde_json::Value>,
    pub variable_traces: Vec<VariableTraceData>,
    pub unsat_core: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableTraceData {
    pub name: String,
    pub type_str: String,
    pub defined_at: LocationData,
    pub assignments: Vec<AssignmentData>,
    pub final_value: serde_json::Value,
    pub relevance: String, // "critical", "supporting", "background"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationData {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentData {
    pub line: usize,
    pub column: usize,
    pub value: serde_json::Value,
    pub description: String,
}

/// LSP Diagnostic data field (containing counterexample.v2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterexampleV2 {
    pub schema: String,                    // "aura.counterexample.v2"
    pub assertion: String,
    pub failed_assertion_type: String,
    pub inputs: HashMap<String, serde_json::Value>,
    pub outputs: HashMap<String, serde_json::Value>,
    pub variable_traces: Vec<VariableTraceData>,
    pub repair_hints: Vec<RepairHintData>,
    pub unsat_core: Option<Vec<String>>,
    pub failure_explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairHintData {
    pub hint_type: String, // "strengthen_precond", etc
    pub severity: String,  // "error", "warning", "info"
    pub description: String,
    pub suggested_code: Option<String>,
    pub affected_lines: Vec<usize>,
}

/// Convert verification counterexample to LSP diagnostic data
pub fn counterexample_to_lsp_diagnostic_data(
    cx: &VerificationCounterexample,
    _file: &str,
    _line: usize,
    _column: usize,
) -> CounterexampleV2 {
    let repair_hints = generate_repair_hints(cx);
    let failure_explanation = explain_failure(cx);

    CounterexampleV2 {
        schema: "aura.counterexample.v2".to_string(),
        assertion: cx.assertion.clone(),
        failed_assertion_type: cx.assertion_type.clone(),
        inputs: cx.inputs.clone(),
        outputs: cx.outputs.clone(),
        variable_traces: cx.variable_traces.clone(),
        repair_hints,
        unsat_core: cx.unsat_core.clone(),
        failure_explanation,
    }
}

/// Generate repair hints from counterexample
pub fn generate_repair_hints(cx: &VerificationCounterexample) -> Vec<RepairHintData> {
    let mut hints = Vec::new();

    match cx.assertion_type.as_str() {
        "requires" => {
            // Input violated precondition
            hints.push(RepairHintData {
                hint_type: "strengthen_precond".to_string(),
                severity: "error".to_string(),
                description: "The caller passed invalid input. Add precondition to prevent this input.".to_string(),
                suggested_code: Some(format!("requires {}", cx.assertion)),
                affected_lines: vec![],
            });
        }
        "ensures" => {
            // Output violated postcondition
            hints.push(RepairHintData {
                hint_type: "weaken_postcond".to_string(),
                severity: "warning".to_string(),
                description: "The function cannot guarantee this output. Consider relaxing the postcondition or fixing the implementation.".to_string(),
                suggested_code: Some(format!("ensures {}", cx.assertion)),
                affected_lines: vec![],
            });
        }
        "assert" => {
            // Assertion failed - multiple approaches
            hints.push(RepairHintData {
                hint_type: "strengthen_precond".to_string(),
                severity: "error".to_string(),
                description: "Add input constraints to prevent this case from occurring.".to_string(),
                suggested_code: Some("requires <condition>".to_string()),
                affected_lines: vec![],
            });

            hints.push(RepairHintData {
                hint_type: "add_invariant".to_string(),
                severity: "warning".to_string(),
                description: "Add a loop or data invariant to track values through the computation.".to_string(),
                suggested_code: Some("invariant <condition>".to_string()),
                affected_lines: vec![],
            });

            hints.push(RepairHintData {
                hint_type: "relax_assertion".to_string(),
                severity: "info".to_string(),
                description: "Make the assertion less strict if it is overconstrained.".to_string(),
                suggested_code: Some(format!("assert {}", cx.assertion)),
                affected_lines: vec![],
            });
        }
        _ => {}
    }

    hints
}

/// Explain the failure in natural language
pub fn explain_failure(cx: &VerificationCounterexample) -> Option<String> {
    let assertion_type = &cx.assertion_type;
    let num_traces = cx.variable_traces.len();

    let explanation = match assertion_type.as_str() {
        "requires" => {
            format!(
                "The function expects precondition '{}' to hold, but it failed. \
                 Check the {} variable(s) involved to understand why.",
                cx.assertion, num_traces
            )
        }
        "ensures" => {
            format!(
                "The function failed to guarantee postcondition '{}'. \
                 Inspect the {} variables to see how the output violated the contract.",
                cx.assertion, num_traces
            )
        }
        "assert" => {
            format!(
                "The assertion '{}' was violated at this location. \
                 Examine the {} variable traces to understand the execution path.",
                cx.assertion, num_traces
            )
        }
        _ => return None,
    };

    Some(explanation)
}

// ============= LSP DIAGNOSTIC BUILDER =============

/// Build LSP Diagnostic from verification failure
pub fn build_lsp_diagnostic(
    cx: &VerificationCounterexample,
    message: String,
    file: &str,
    line: usize,
    column: usize,
) -> serde_json::Value {
    let diagnostic_data = counterexample_to_lsp_diagnostic_data(cx, file, line, column);

    serde_json::json!({
        "range": {
            "start": { "line": line, "character": column },
            "end": { "line": line, "character": column + 10 }
        },
        "severity": 1, // Error
        "message": message,
        "source": "aura/verify",
        "code": "proof-failure",
        "data": {
            "counterexample": diagnostic_data
        }
    })
}

// ============= TESTS =============

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_counterexample() -> VerificationCounterexample {
        let mut inputs = HashMap::new();
        inputs.insert("x".to_string(), serde_json::json!(-5));

        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), serde_json::json!(false));

        VerificationCounterexample {
            assertion: "x > 0".to_string(),
            assertion_type: "requires".to_string(),
            inputs,
            outputs,
            variable_traces: vec![],
            unsat_core: None,
        }
    }

    #[test]
    fn test_counterexample_to_lsp() {
        let cx = make_test_counterexample();
        let lsp_data = counterexample_to_lsp_diagnostic_data(&cx, "test.aura", 10, 5);

        assert_eq!(lsp_data.schema, "aura.counterexample.v2");
        assert_eq!(lsp_data.assertion, "x > 0");
        assert!(!lsp_data.repair_hints.is_empty());
    }

    #[test]
    fn test_generate_repair_hints_requires() {
        let cx = make_test_counterexample();
        let hints = generate_repair_hints(&cx);

        assert!(!hints.is_empty());
        assert_eq!(hints[0].hint_type, "strengthen_precond");
        assert_eq!(hints[0].severity, "error");
    }

    #[test]
    fn test_explain_failure() {
        let cx = make_test_counterexample();
        let explanation = explain_failure(&cx);

        assert!(explanation.is_some());
        assert!(explanation.unwrap().contains("precondition"));
    }

    #[test]
    fn test_build_lsp_diagnostic() {
        let cx = make_test_counterexample();
        let diagnostic = build_lsp_diagnostic(
            &cx,
            "Assertion failed: x > 0".to_string(),
            "test.aura",
            10,
            5,
        );

        assert_eq!(diagnostic["message"], "Assertion failed: x > 0");
        assert_eq!(diagnostic["source"], "aura/verify");
        assert!(diagnostic["data"]["counterexample"].is_object());
    }
}
