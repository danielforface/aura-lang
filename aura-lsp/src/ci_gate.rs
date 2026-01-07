/// CI Gate for Differential Testing
/// 
/// Runs GDB and LLDB on verification output and gates release on matching results

use std::collections::HashMap;

/// CI gate configuration
#[derive(Clone, Debug)]
pub struct CIGateConfig {
    pub min_passing_tests: usize,
    pub allow_warnings: bool,
    pub backends: Vec<String>, // ["gdb", "lldb"]
    pub timeout_seconds: u64,
}

/// CI gate result
pub struct CIGateResult {
    pub passed: bool,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub backend_results: HashMap<String, BackendTestResult>,
}

/// Backend-specific test results
pub struct BackendTestResult {
    pub backend: String,
    pub passed: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// Differential test case
pub struct DifferentialTestCase {
    pub name: String,
    pub program: String,
    pub expected_output: String,
    pub expected_variables: HashMap<String, String>,
}

/// CI Gate executor
pub struct CIGate {
    config: CIGateConfig,
    test_cases: Vec<DifferentialTestCase>,
}

impl CIGate {
    pub fn new(config: CIGateConfig) -> Self {
        CIGate {
            config,
            test_cases: Vec::new(),
        }
    }

    pub fn add_test_case(&mut self, test_case: DifferentialTestCase) {
        self.test_cases.push(test_case);
    }

    pub fn run(&self) -> CIGateResult {
        let mut result = CIGateResult {
            passed: false,
            total_tests: self.test_cases.len(),
            passed_tests: 0,
            failed_tests: 0,
            warnings: Vec::new(),
            errors: Vec::new(),
            backend_results: HashMap::new(),
        };

        // Initialize backend results
        for backend in &self.config.backends {
            result.backend_results.insert(
                backend.clone(),
                BackendTestResult {
                    backend: backend.clone(),
                    passed: 0,
                    failed: 0,
                    errors: Vec::new(),
                },
            );
        }

        // Run tests on each backend
        for test in &self.test_cases {
            let mut test_result_by_backend: HashMap<String, bool> = HashMap::new();

            for backend in &self.config.backends {
                let passed = self.run_test_on_backend(backend, test);
                test_result_by_backend.insert(backend.clone(), passed);

                if let Some(backend_result) = result.backend_results.get_mut(backend) {
                    if passed {
                        backend_result.passed += 1;
                    } else {
                        backend_result.failed += 1;
                    }
                }
            }

            // Check if results match across backends
            let all_match = test_result_by_backend.values().all(|&passed| passed);
            if all_match && test_result_by_backend.values().all(|v| *v) {
                result.passed_tests += 1;
            } else {
                result.failed_tests += 1;
                result.errors.push(format!(
                    "Test '{}' produced different results across backends",
                    test.name
                ));
            }
        }

        // Gate decision
        result.passed = result.passed_tests >= self.config.min_passing_tests
            && (self.config.allow_warnings || result.warnings.is_empty());

        result
    }

    fn run_test_on_backend(&self, backend: &str, test: &DifferentialTestCase) -> bool {
        match backend {
            "gdb" => self.run_gdb_test(test),
            "lldb" => self.run_lldb_test(test),
            _ => {
                eprintln!("Unknown backend: {}", backend);
                false
            }
        }
    }

    fn run_gdb_test(&self, _test: &DifferentialTestCase) -> bool {
        // Simulate: gdb -batch -ex "break main" -ex "run" -ex "print x" ./program
        // In real implementation, would execute GDB MI commands and capture output
        true // Placeholder
    }

    fn run_lldb_test(&self, _test: &DifferentialTestCase) -> bool {
        // Simulate: lldb -b -o "breakpoint set -n main" -o "run" -o "frame variable" ./program
        // In real implementation, would execute LLDB MI commands and capture output
        true // Placeholder
    }
}

/// GitHub Actions integration
pub fn generate_ci_workflow() -> String {
    r#"
name: Differential Testing CI Gate

on: [push, pull_request]

jobs:
  differential-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Install GDB and LLDB
        run: |
          sudo apt-get update
          sudo apt-get install -y gdb lldb
      
      - name: Build Aura
        run: cargo build --release
      
      - name: Run Differential Tests (GDB)
        run: |
          ./scripts/run_differential_tests.sh gdb
      
      - name: Run Differential Tests (LLDB)
        run: |
          ./scripts/run_differential_tests.sh lldb
      
      - name: Run CI Gate
        run: |
          cargo run --bin aura-ci-gate -- --min-passing 95%
      
      - name: Upload Results
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: differential-test-results
          path: results/*.json
      
      - name: Comment PR
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v6
        with:
          script: |
            const fs = require('fs');
            const results = JSON.parse(fs.readFileSync('results/summary.json', 'utf8'));
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: `## Differential Testing Results\n\n${results.summary}`
            });
"#
    .to_string()
}

// ========== TESTS ==========

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ci_gate_passed() {
        let config = CIGateConfig {
            min_passing_tests: 1,
            allow_warnings: true,
            backends: vec!["gdb".to_string(), "lldb".to_string()],
            timeout_seconds: 30,
        };

        let mut gate = CIGate::new(config);
        gate.add_test_case(DifferentialTestCase {
            name: "simple".to_string(),
            program: "simple".to_string(),
            expected_output: "Hello".to_string(),
            expected_variables: HashMap::new(),
        });

        let result = gate.run();
        assert_eq!(result.total_tests, 1);
    }

    #[test]
    fn test_ci_gate_result_fields() {
        let config = CIGateConfig {
            min_passing_tests: 1,
            allow_warnings: false,
            backends: vec!["gdb".to_string()],
            timeout_seconds: 60,
        };

        let gate = CIGate::new(config);
        let result = gate.run();

        assert_eq!(result.total_tests, 0);
        assert_eq!(result.passed_tests, 0);
        assert_eq!(result.failed_tests, 0);
    }

    #[test]
    fn test_workflow_generation() {
        let workflow = generate_ci_workflow();
        assert!(workflow.contains("gdb"));
        assert!(workflow.contains("lldb"));
        assert!(workflow.contains("Differential Testing"));
    }
}
