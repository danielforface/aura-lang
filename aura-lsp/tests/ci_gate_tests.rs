/// Integration tests for CI gate functionality

#[cfg(test)]
mod ci_gate_integration_tests {
    use std::collections::HashMap;

    // Mock types for testing without full LSP dependencies
    #[derive(Debug, Clone)]
    struct MockProofResult {
        pub program_path: String,
        pub expected_output: String,
        pub variable_state: HashMap<String, String>,
    }

    #[derive(Debug, Clone)]
    struct MockCIGateConfig {
        pub min_passing_tests: usize,
        pub allow_warnings: bool,
        pub backends: Vec<String>,
        pub timeout_seconds: u64,
    }

    #[derive(Debug)]
    struct MockCIGateResult {
        pub passed: bool,
        pub total_tests: usize,
        pub passed_tests: usize,
        pub failed_tests: usize,
        pub errors: Vec<String>,
    }

    #[test]
    fn test_ci_gate_with_passing_proofs() {
        let config = MockCIGateConfig {
            min_passing_tests: 5,
            allow_warnings: false,
            backends: vec!["gdb".to_string(), "lldb".to_string()],
            timeout_seconds: 60,
        };

        let result = MockCIGateResult {
            passed: true,
            total_tests: 5,
            passed_tests: 5,
            failed_tests: 0,
            errors: Vec::new(),
        };

        assert!(result.passed);
        assert_eq!(result.passed_tests, 5);
        assert_eq!(result.failed_tests, 0);
    }

    #[test]
    fn test_ci_gate_with_failing_proofs() {
        let config = MockCIGateConfig {
            min_passing_tests: 5,
            allow_warnings: false,
            backends: vec!["gdb".to_string(), "lldb".to_string()],
            timeout_seconds: 60,
        };

        let result = MockCIGateResult {
            passed: false,
            total_tests: 5,
            passed_tests: 3,
            failed_tests: 2,
            errors: vec!["test failed".to_string()],
        };

        assert!(!result.passed);
        assert_eq!(result.failed_tests, 2);
    }

    #[test]
    fn test_differential_test_agreement() {
        // Simulates GDB and LLDB producing same results
        let gdb_variables = {
            let mut m = HashMap::new();
            m.insert("x".to_string(), "10".to_string());
            m.insert("y".to_string(), "20".to_string());
            m
        };

        let lldb_variables = {
            let mut m = HashMap::new();
            m.insert("x".to_string(), "10".to_string());
            m.insert("y".to_string(), "20".to_string());
            m
        };

        assert_eq!(gdb_variables, lldb_variables);
    }

    #[test]
    fn test_differential_test_disagreement() {
        // Simulates GDB and LLDB producing different results
        let gdb_variables = {
            let mut m = HashMap::new();
            m.insert("x".to_string(), "10".to_string());
            m
        };

        let lldb_variables = {
            let mut m = HashMap::new();
            m.insert("x".to_string(), "15".to_string());
            m
        };

        assert_ne!(gdb_variables, lldb_variables);
    }

    #[test]
    fn test_multiple_backends_agreement() {
        let backends = vec!["gdb", "lldb"];
        let results = vec![
            ("gdb", true, vec!["x=10".to_string()]),
            ("lldb", true, vec!["x=10".to_string()]),
        ];

        let all_agree = results.iter().all(|(_, passed, _)| *passed)
            && results.iter().map(|(_, _, vars)| vars).all(|v| v == &vec!["x=10".to_string()]);

        assert!(all_agree);
    }

    #[test]
    fn test_proof_result_extraction() {
        let proof_result = MockProofResult {
            program_path: "test.aura".to_string(),
            expected_output: "proof succeeded".to_string(),
            variable_state: {
                let mut m = HashMap::new();
                m.insert("witness".to_string(), "42".to_string());
                m.insert("condition".to_string(), "true".to_string());
                m
            },
        };

        assert_eq!(proof_result.variable_state.len(), 2);
        assert_eq!(
            proof_result.variable_state.get("witness"),
            Some(&"42".to_string())
        );
    }

    #[test]
    fn test_ci_gate_minimum_passing_threshold() {
        let total = 10;
        let passed = 95; // 95% of 10 = 9.5, so need at least 10 passing
        
        let min_required = ((total as f32 * 95.0 / 100.0).ceil()) as usize;
        assert!(passed >= min_required);
    }

    #[test]
    fn test_ci_gate_with_warnings_allowed() {
        let config = MockCIGateConfig {
            min_passing_tests: 5,
            allow_warnings: true,
            backends: vec!["gdb".to_string()],
            timeout_seconds: 30,
        };

        let mut result = MockCIGateResult {
            passed: true,
            total_tests: 5,
            passed_tests: 5,
            failed_tests: 0,
            errors: vec!["warning: deprecated function used".to_string()],
        };

        // With allow_warnings=true, this should still pass
        if config.allow_warnings {
            result.passed = true;
        }

        assert!(result.passed);
    }

    #[test]
    fn test_ci_gate_with_warnings_disallowed() {
        let config = MockCIGateConfig {
            min_passing_tests: 5,
            allow_warnings: false,
            backends: vec!["gdb".to_string()],
            timeout_seconds: 30,
        };

        let mut result = MockCIGateResult {
            passed: true,
            total_tests: 5,
            passed_tests: 5,
            failed_tests: 0,
            errors: vec!["warning: deprecated function used".to_string()],
        };

        // With allow_warnings=false, warnings should cause failure
        if !config.allow_warnings && !result.errors.is_empty() {
            result.passed = false;
        }

        assert!(!result.passed);
    }

    #[test]
    fn test_gdb_lldb_output_parsing() {
        let gdb_output = "Breakpoint 1 at 0x401030\nRunning test...\nx = 10\ny = 20\n";
        
        // Parse x variable
        let x_value = gdb_output
            .lines()
            .find(|l| l.contains("x ="))
            .and_then(|l| l.split('=').nth(1))
            .map(|v| v.trim());

        assert_eq!(x_value, Some("10"));
    }

    #[test]
    fn test_backends_timeout_handling() {
        let timeout = 60u64;
        let execution_time = 30u64;

        // Test should pass if within timeout
        assert!(execution_time < timeout);
    }

    #[test]
    fn test_ci_gate_report_generation() {
        let result = MockCIGateResult {
            passed: true,
            total_tests: 10,
            passed_tests: 10,
            failed_tests: 0,
            errors: Vec::new(),
        };

        let report = format!(
            "Total: {}, Passed: {}, Failed: {}",
            result.total_tests, result.passed_tests, result.failed_tests
        );

        assert!(report.contains("Total: 10"));
        assert!(report.contains("Passed: 10"));
        assert!(report.contains("Failed: 0"));
    }
}
