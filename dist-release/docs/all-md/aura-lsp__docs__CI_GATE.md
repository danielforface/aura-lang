# CI Gate - Differential Testing System

## Overview

The CI Gate is a differential testing system that gates Aura releases by ensuring proof verification results agree across multiple debugging backends (GDB and LLDB). This prevents edge cases and debugger-specific bugs from reaching production.

## Architecture

### Components

1. **CI Gate Core** (`ci_gate.rs`)
   - Configures testing parameters
   - Manages test execution across backends
   - Generates gate decisions

2. **CI Gate Driver** (`ci_gate_driver.rs`)
   - Loads proof results from verification backend
   - Converts proofs to differential test cases
   - Orchestrates the full test pipeline

3. **Differential Test Runner** (`differential_test_runner.rs`)
   - Executes tests on GDB
   - Executes tests on LLDB
   - Compares results for agreement

### Data Flow

```
Proof Verification Results
         ↓
CI Gate Driver (Load & Convert)
         ↓
Differential Test Runner
    ↙          ↘
  GDB          LLDB
    ↘          ↙
Compare Results & Verify Agreement
         ↓
Gate Decision (Pass/Block Release)
```

## Configuration

### CIGateConfig Structure

```rust
pub struct CIGateConfig {
    pub min_passing_tests: usize,      // Minimum tests that must pass
    pub allow_warnings: bool,           // Allow non-critical warnings
    pub backends: Vec<String>,          // ["gdb", "lldb"]
    pub timeout_seconds: u64,           // Test timeout
}
```

### Usage

```bash
# Run CI gate with default settings
cargo run --bin aura-ci-gate

# Custom configuration
cargo run --bin aura-ci-gate -- --min-passing 95% --backends gdb,lldb --timeout 60

# Differential test runner
cargo run --bin differential-test-runner -- tests/config.json
```

## Test Cases

### DifferentialTest Structure

```rust
pub struct DifferentialTest {
    pub name: String,                          // Test identifier
    pub source_file: String,                   // Program to test
    pub breakpoint: String,                    // Where to break
    pub commands: Vec<String>,                 // Debugger commands
    pub expected_variables: HashMap<String, String>,  // Expected values
    pub expected_output: String,               // Expected output
}
```

### Test Comparison

Each test runs on both GDB and LLDB, comparing:

1. **Execution Status**: Both should succeed or fail consistently
2. **Variable Values**: Variables must have identical values
3. **Output**: Program output must match exactly

```rust
pub struct DifferentialTestComparison {
    pub test_name: String,
    pub gdb_result: DebuggerTestResult,
    pub lldb_result: DebuggerTestResult,
}

impl DifferentialTestComparison {
    pub fn agree(&self) -> bool {
        // Results agree if both have same status and variables
    }
}
```

## Gate Decision Logic

The gate passes if:

- ✅ Number of passing tests >= min_passing_tests
- ✅ All backends agree on results (no contradictions)
- ✅ GDB and LLDB produce identical variable values
- ✅ No errors (or warnings allowed, depending on config)

The gate fails if:

- ❌ Backend disagreement detected
- ❌ Insufficient tests passing
- ❌ Warnings present (if disallowed)
- ❌ Timeout exceeded

## Integration with GitHub Actions

### CI Workflow

The system integrates with GitHub Actions for automated testing:

```yaml
name: Differential Testing CI Gate

on: [push, pull_request]

jobs:
  differential-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y gdb lldb
      
      - name: Build Aura
        run: cargo build --release
      
      - name: Run differential tests
        run: |
          cargo run --bin differential-test-runner -- tests/config.json
      
      - name: Run CI gate
        run: |
          cargo run --bin aura-ci-gate -- --min-passing 95%
```

## Test Results Format

### Gate Result

```json
{
  "passed": true,
  "total_tests": 100,
  "passed_tests": 100,
  "failed_tests": 0,
  "warnings": [],
  "errors": [],
  "backend_results": {
    "gdb": {
      "backend": "gdb",
      "passed": 100,
      "failed": 0,
      "errors": []
    },
    "lldb": {
      "backend": "lldb",
      "passed": 100,
      "failed": 0,
      "errors": []
    }
  }
}
```

### Comparison Report

```
Test: proof_001

GDB Result:
  Status: PASS
  Variables: {"x": "10", "y": "20"}
  
LLDB Result:
  Status: PASS
  Variables: {"x": "10", "y": "20"}

Agreement: ✅ YES
```

## Proof Result Extraction

The driver extracts proof verification results and converts them to differential test cases:

```rust
pub fn convert_to_test_cases(&self) -> Vec<DifferentialTestCase> {
    self.proof_results
        .iter()
        .enumerate()
        .map(|(idx, proof_result)| {
            DifferentialTestCase {
                name: format!("proof_{}", idx),
                program: proof_result.program_path.clone(),
                expected_output: proof_result.expected_output.clone(),
                expected_variables: proof_result.variable_state.clone(),
            }
        })
        .collect()
}
```

## Debugger Output Parsing

### GDB Parsing

```rust
fn parse_variables(output: &str, expected_vars: &HashMap<String, String>) -> HashMap<String, String> {
    // Look for pattern: variable_name = value
    for var_name in expected_vars.keys() {
        if let Some(line) = output.lines().find(|l| l.contains(&format!("{} =", var_name))) {
            if let Some(value) = line.split('=').nth(1) {
                variables.insert(var_name.clone(), value.trim().to_string());
            }
        }
    }
    variables
}
```

### LLDB Parsing

```rust
fn parse_variables(output: &str, expected_vars: &HashMap<String, String>) -> HashMap<String, String> {
    // Look for pattern: (VariableName) Type = value
    for var_name in expected_vars.keys() {
        if let Some(line) = output.lines().find(|l| l.contains(&format!("({}) ", var_name))) {
            if let Some(value) = line.split('=').nth(1) {
                variables.insert(var_name.clone(), value.trim().to_string());
            }
        }
    }
    variables
}
```

## Report Generation

The gate generates comprehensive reports:

```rust
pub fn generate_report(result: &CIGateResult) -> String {
    format!(
        r#"# CI Gate Report

## Summary
- **Total Tests**: {}
- **Passed**: {}
- **Failed**: {}
- **Gate Status**: {}

## Backend Results
{}

## Recommendation
{}"#,
        result.total_tests,
        result.passed_tests,
        result.failed_tests,
        if result.passed { "✅ PASSED" } else { "❌ FAILED" },
        // ... backend details ...
        // ... recommendation ...
    )
}
```

## Testing

### Unit Tests

Run individual component tests:

```bash
cargo test ci_gate::tests
cargo test ci_gate_driver::tests
cargo test differential_test_runner::tests
```

### Integration Tests

Run full CI gate tests:

```bash
cargo test --test ci_gate_tests
```

### Example Test

```rust
#[test]
fn test_ci_gate_with_passing_proofs() {
    let config = CIGateConfig {
        min_passing_tests: 5,
        allow_warnings: false,
        backends: vec!["gdb".to_string(), "lldb".to_string()],
        timeout_seconds: 60,
    };

    let mut gate = CIGate::new(config);
    // ... add test cases ...
    
    let result = gate.run();
    assert!(result.passed);
    assert_eq!(result.passed_tests, 5);
}
```

## Release Gating Workflow

1. **Push Code** → Triggers GitHub Actions
2. **Run Verification** → Collect proof results
3. **Extract Proofs** → Convert to differential tests
4. **Run Tests** → Execute on GDB and LLDB
5. **Compare Results** → Verify agreement
6. **Gate Decision** → Pass/block release
7. **Generate Report** → Post results to PR

## Future Enhancements

- [ ] Additional debugger backends (VSCode Debugger, QEMU)
- [ ] Parallel test execution
- [ ] Result caching for faster re-runs
- [ ] Historical trend analysis
- [ ] Automated bisection for disagreement debugging
- [ ] Machine learning for anomaly detection

## Troubleshooting

### Backend Disagreement

If GDB and LLDB produce different results:

1. Check debugger versions
2. Enable verbose output for debugging
3. Compare raw debugger commands
4. Check for floating-point precision issues
5. Review proof result extraction logic

### Timeout Issues

If tests exceed timeout:

1. Increase timeout value
2. Check system resource usage
3. Profile individual tests
4. Optimize proof verification

### Test Configuration Issues

If tests fail to load:

1. Verify JSON configuration syntax
2. Check file paths are absolute
3. Ensure debuggers are installed
4. Check debugger accessibility

## References

- [GDB Documentation](https://sourceware.org/gdb/onlinedocs/)
- [LLDB Documentation](https://lldb.llvm.org/use/index.html)
- [LSP Specification](https://microsoft.github.io/language-server-protocol/)
