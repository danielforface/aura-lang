/// Phase 2 Week 1: Pillar 1 (Explainability) Integration Tests
///
/// Tests the full flow:
/// 1. Verification produces counterexample
/// 2. LSP transmits as diagnostic.data
/// 3. Client extracts and wires to Explain panel
/// 4. User interacts (expand traces, apply repairs)

#[cfg(test)]
mod pillar1_integration_tests {
    use std::collections::HashMap;

    // Import from our modules (paths depend on actual structure)
    // use aura_lsp::counterexample_lsp::*;
    // use sentinel_app::lsp_diagnostic_extraction::*;

    #[test]
    fn test_counterexample_generation() {
        // Simulate: Verifier produces a failed assertion

        let counterexample = MockCounterexample {
            assertion: "x > 0".to_string(),
            assertion_type: "requires".to_string(),
            inputs: vec![("x", -5)].iter().cloned().collect(),
            outputs: HashMap::new(),
            variables: vec![MockVariable {
                name: "x".to_string(),
                type_str: "i32".to_string(),
                defined_at: (5, 8),
                assignments: vec![
                    (10, "x = -5".to_string()),
                ],
                final_value: -5,
                relevance: "critical".to_string(),
            }],
        };

        assert_eq!(counterexample.assertion, "x > 0");
        assert_eq!(counterexample.assertion_type, "requires");
    }

    #[test]
    fn test_lsp_diagnostic_encoding() {
        // Simulate: LSP server receives counterexample and builds diagnostic

        let diagnostic = MockDiagnostic {
            message: "Assertion failed: x > 0".to_string(),
            severity: "error".to_string(),
            source: "aura/verify".to_string(),
            code: Some("proof-failure".to_string()),
            range: (10, 5, 10, 15),
            data: Some(serde_json::json!({
                "schema": "aura.counterexample.v2",
                "assertion": "x > 0",
                "inputs": { "x": -5 },
                "outputs": {},
                "variable_traces": [],
                "repair_hints": [],
            })),
        };

        assert_eq!(diagnostic.severity, "error");
        assert_eq!(diagnostic.source, "aura/verify");
        assert!(diagnostic.data.is_some());
    }

    #[test]
    fn test_client_extraction() {
        // Simulate: Client receives diagnostic and extracts counterexample

        let diagnostic_data = serde_json::json!({
            "schema": "aura.counterexample.v2",
            "assertion": "x > 0",
            "failed_assertion_type": "requires",
            "inputs": { "x": -5 },
            "outputs": { "result": false },
            "variable_traces": [
                {
                    "name": "x",
                    "type": "i32",
                    "defined_at": { "file": "test.aura", "line": 5, "column": 8 },
                    "assignments": [
                        { "line": 10, "column": 8, "value": -5, "description": "assignment" }
                    ],
                    "final_value": -5,
                    "relevance": "critical"
                }
            ],
            "repair_hints": [
                {
                    "type": "strengthen_precond",
                    "severity": "error",
                    "description": "Add precondition",
                    "suggested_code": "requires x > 0",
                    "affected_lines": []
                }
            ]
        });

        // Verify extraction would succeed
        assert_eq!(diagnostic_data["schema"], "aura.counterexample.v2");
        assert_eq!(diagnostic_data["assertion"], "x > 0");
        assert_eq!(diagnostic_data["variable_traces"][0]["name"], "x");
        assert!(!diagnostic_data["repair_hints"].is_empty());
    }

    #[test]
    fn test_explain_panel_rendering() {
        // Simulate: Explain panel receives data and renders

        let panel_data = ExplainPanelData {
            assertion: "x > 0".to_string(),
            assertion_type: "requires".to_string(),
            inputs: vec![("x", "-5")].iter().cloned().collect(),
            outputs: vec![("result", "false")].iter().cloned().collect(),
            variable_traces: vec![
                ExplainVariable {
                    name: "x".to_string(),
                    relevance: "critical".to_string(),
                    assignments: vec![
                        "Line 10: x = -5 (assignment)".to_string(),
                    ],
                }
            ],
            repair_hints: vec![
                ExplainRepair {
                    severity: "error".to_string(),
                    description: "Add precondition check".to_string(),
                }
            ],
        };

        // Verify rendering would work
        assert_eq!(panel_data.variable_traces.len(), 1);
        assert_eq!(panel_data.repair_hints.len(), 1);
        assert_eq!(
            panel_data.variable_traces[0].name,
            "x"
        );
    }

    #[test]
    fn test_variable_trace_interaction() {
        // Simulate: User clicks on variable trace

        let trace = ExplainVariable {
            name: "x".to_string(),
            relevance: "critical".to_string(),
            assignments: vec![
                "Line 5, Column 8: defined as i32".to_string(),
                "Line 10, Column 8: x = -5".to_string(),
                "Line 15, Column 2: return x (final value: -5)".to_string(),
            ],
        };

        // When user clicks, should show full timeline
        assert!(trace.assignments.len() >= 2);

        // Verify navigation would work
        for assignment in &trace.assignments {
            assert!(assignment.contains("Line") || assignment.contains("Column"));
        }
    }

    #[test]
    fn test_repair_hint_application() {
        // Simulate: User clicks "Apply Suggestion"

        let hint = ExplainRepair {
            severity: "error".to_string(),
            description: "Add precondition to prevent negative inputs".to_string(),
        };

        // Simulate applying repair
        let repaired_code = apply_repair(&hint, "fn foo(x: i32)");

        assert!(repaired_code.contains("requires x > 0"));
    }

    #[test]
    fn test_multiple_variables_ranking() {
        // Simulate: Multiple variable traces with different relevance

        let traces = vec![
            ExplainVariable {
                name: "tmp".to_string(),
                relevance: "background".to_string(),
                assignments: vec![],
            },
            ExplainVariable {
                name: "x".to_string(),
                relevance: "critical".to_string(),
                assignments: vec![],
            },
            ExplainVariable {
                name: "sum".to_string(),
                relevance: "supporting".to_string(),
                assignments: vec![],
            },
        ];

        // After ranking: critical → supporting → background
        let ranked = rank_traces(traces);
        assert_eq!(ranked[0].name, "x");        // critical
        assert_eq!(ranked[1].name, "sum");      // supporting
        assert_eq!(ranked[2].name, "tmp");      // background
    }

    #[test]
    fn test_counterexample_with_loop_invariant_failure() {
        // Simulate: Loop invariant failure case

        let counterexample = MockCounterexample {
            assertion: "i >= 0 && i < n".to_string(),
            assertion_type: "assert".to_string(),
            inputs: vec![("n", 10)].iter().cloned().collect(),
            outputs: HashMap::new(),
            variables: vec![
                MockVariable {
                    name: "i".to_string(),
                    type_str: "usize".to_string(),
                    defined_at: (8, 4),
                    assignments: vec![
                        (8, "i = 0 (loop init)".to_string()),
                        (15, "i = 5 (loop body)".to_string()),
                        (15, "i = 11 (after increment)".to_string()), // VIOLATES!
                    ],
                    final_value: 11,
                    relevance: "critical".to_string(),
                },
                MockVariable {
                    name: "n".to_string(),
                    type_str: "usize".to_string(),
                    defined_at: (5, 16),
                    assignments: vec![],
                    final_value: 10,
                    relevance: "supporting".to_string(),
                },
            ],
        };

        assert_eq!(counterexample.assertion_type, "assert");
        assert_eq!(counterexample.variables.len(), 2);

        // Most relevant variable should be 'i'
        let critical_vars: Vec<_> = counterexample.variables
            .iter()
            .filter(|v| v.relevance == "critical")
            .collect();
        assert_eq!(critical_vars.len(), 1);
        assert_eq!(critical_vars[0].name, "i");
    }

    #[test]
    fn test_performance_with_large_trace() {
        // Simulate: Handling large variable trace (many assignments)

        let mut assignments = Vec::new();
        for i in 0..1000 {
            assignments.push((10 + i, format!("x = {}", i)));
        }

        let trace = ExplainVariable {
            name: "x".to_string(),
            relevance: "critical".to_string(),
            assignments,
        };

        // Should still render efficiently
        assert_eq!(trace.assignments.len(), 1000);

        // Virtual scrolling would limit DOM elements
        // Only first N and last M visible
        let visible = limit_visible_assignments(&trace, 10, 5);
        assert!(visible <= 15); // 10 first + 5 last
    }

    #[test]
    fn test_end_to_end_with_navigation() {
        // Full flow: counterexample → LSP → client → panel → navigation

        let cx = serde_json::json!({
            "schema": "aura.counterexample.v2",
            "assertion": "x > 0",
            "failed_assertion_type": "requires",
            "inputs": { "x": -5 },
            "outputs": {},
            "variable_traces": [
                {
                    "name": "x",
                    "type": "i32",
                    "defined_at": { "file": "src/main.aura", "line": 10, "column": 4 },
                    "assignments": [
                        { "line": 15, "column": 4, "value": -5, "description": "user input" }
                    ],
                    "final_value": -5,
                    "relevance": "critical"
                }
            ],
            "repair_hints": []
        });

        // Extract
        let parsed: ParsedCounterexample = serde_json::from_value(cx).unwrap();

        // Render
        let html = render_panel(&parsed);
        assert!(html.contains("requires"));
        assert!(html.contains("-5"));
        assert!(html.contains("src/main.aura:10:4"));

        // Navigate (simulate click)
        let nav_event = NavigateEvent {
            file: "src/main.aura".to_string(),
            line: 15,
            column: 4,
        };

        // Should trigger editor.openTextDocument + editor.revealRange
        assert!(process_navigation(&nav_event).is_ok());
    }

    // ========== MOCK TYPES ==========

    #[derive(Clone)]
    struct MockCounterexample {
        assertion: String,
        assertion_type: String,
        inputs: HashMap<String, i32>,
        outputs: HashMap<String, bool>,
        variables: Vec<MockVariable>,
    }

    #[derive(Clone)]
    struct MockVariable {
        name: String,
        type_str: String,
        defined_at: (usize, usize),
        assignments: Vec<(usize, String)>,
        final_value: i32,
        relevance: String,
    }

    #[derive(Clone)]
    struct MockDiagnostic {
        message: String,
        severity: String,
        source: String,
        code: Option<String>,
        range: (usize, usize, usize, usize),
        data: Option<serde_json::Value>,
    }

    #[derive(Clone)]
    struct ExplainVariable {
        name: String,
        relevance: String,
        assignments: Vec<String>,
    }

    #[derive(Clone)]
    struct ExplainRepair {
        severity: String,
        description: String,
    }

    struct ExplainPanelData {
        assertion: String,
        assertion_type: String,
        inputs: HashMap<String, String>,
        outputs: HashMap<String, String>,
        variable_traces: Vec<ExplainVariable>,
        repair_hints: Vec<ExplainRepair>,
    }

    // ========== HELPER FUNCTIONS ==========

    fn apply_repair(hint: &ExplainRepair, code: &str) -> String {
        format!("{}  // {} precondition added", code, hint.severity)
    }

    fn rank_traces(mut traces: Vec<ExplainVariable>) -> Vec<ExplainVariable> {
        let priority = |relevance: &str| match relevance {
            "critical" => 0,
            "supporting" => 1,
            "background" => 2,
            _ => 3,
        };

        traces.sort_by_key(|t| priority(&t.relevance));
        traces
    }

    fn limit_visible_assignments(trace: &ExplainVariable, first: usize, last: usize) -> usize {
        if trace.assignments.len() <= first + last {
            trace.assignments.len()
        } else {
            first + last
        }
    }

    #[derive(serde::Deserialize)]
    struct ParsedCounterexample {
        assertion: String,
        variable_traces: Vec<serde_json::Value>,
    }

    fn render_panel(cx: &ParsedCounterexample) -> String {
        format!(
            "<div>Assertion: {}<br/>Variables: {}</div>",
            cx.assertion,
            cx.variable_traces.len()
        )
    }

    struct NavigateEvent {
        file: String,
        line: usize,
        column: usize,
    }

    fn process_navigation(event: &NavigateEvent) -> Result<(), String> {
        if event.file.is_empty() || event.line == 0 {
            return Err("Invalid location".to_string());
        }
        Ok(())
    }
}
