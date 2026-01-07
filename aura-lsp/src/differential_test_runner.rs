/// Differential test runner for GDB and LLDB
/// 
/// Executes the same test cases on both debuggers and compares results

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::io::Write;

/// Test execution result from a single debugger
pub struct DebuggerTestResult {
    pub debugger: String,
    pub test_name: String,
    pub passed: bool,
    pub output: String,
    pub variables: HashMap<String, String>,
    pub errors: Vec<String>,
}

/// Differential test case
pub struct DifferentialTest {
    pub name: String,
    pub source_file: String,
    pub breakpoint: String, // function or line number
    pub commands: Vec<String>, // debugger commands to run
    pub expected_variables: HashMap<String, String>,
    pub expected_output: String,
}

/// GDB-specific runner
pub struct GDBRunner;

impl GDBRunner {
    /// Run test using GDB
    pub fn run_test(test: &DifferentialTest) -> Result<DebuggerTestResult, String> {
        // Build GDB commands
        let mut gdb_commands = format!("break {}\nrun\n", test.breakpoint);
        for cmd in &test.commands {
            gdb_commands.push_str(&format!("{}\n", cmd));
        }
        gdb_commands.push_str("quit\n");

        // Execute GDB
        let mut child = Command::new("gdb")
            .args(&["-batch", "-x", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg(&test.source_file)
            .spawn()
            .map_err(|e| format!("Failed to spawn GDB: {}", e))?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| "Failed to open stdin".to_string())?;
            stdin
                .write_all(gdb_commands.as_bytes())
                .map_err(|e| format!("Failed to write to GDB stdin: {}", e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait for GDB: {}", e))?;

        let output_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        let passed = output.status.success();
        let variables = Self::parse_variables(&output_str, &test.expected_variables);

        Ok(DebuggerTestResult {
            debugger: "gdb".to_string(),
            test_name: test.name.clone(),
            passed,
            output: output_str,
            variables,
            errors: if stderr_str.is_empty() {
                Vec::new()
            } else {
                vec![stderr_str]
            },
        })
    }

    fn parse_variables(
        output: &str,
        expected_vars: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        for var_name in expected_vars.keys() {
            // Look for pattern: variable_name = value
            if let Some(line) = output
                .lines()
                .find(|l| l.contains(&format!("{} =", var_name)))
            {
                if let Some(value) = line.split('=').nth(1) {
                    variables.insert(var_name.clone(), value.trim().to_string());
                }
            }
        }

        variables
    }
}

/// LLDB-specific runner
pub struct LLDBRunner;

impl LLDBRunner {
    /// Run test using LLDB
    pub fn run_test(test: &DifferentialTest) -> Result<DebuggerTestResult, String> {
        // Build LLDB commands
        let mut lldb_commands = format!("breakpoint set -n {}\nrun\n", test.breakpoint);
        for cmd in &test.commands {
            lldb_commands.push_str(&format!("{}\n", cmd));
        }
        lldb_commands.push_str("quit\n");

        // Execute LLDB
        let mut child = Command::new("lldb")
            .args(&["-b", "-s", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg(&test.source_file)
            .spawn()
            .map_err(|e| format!("Failed to spawn LLDB: {}", e))?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or_else(|| "Failed to open stdin".to_string())?;
            stdin
                .write_all(lldb_commands.as_bytes())
                .map_err(|e| format!("Failed to write to LLDB stdin: {}", e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait for LLDB: {}", e))?;

        let output_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        let passed = output.status.success();
        let variables = Self::parse_variables(&output_str, &test.expected_variables);

        Ok(DebuggerTestResult {
            debugger: "lldb".to_string(),
            test_name: test.name.clone(),
            passed,
            output: output_str,
            variables,
            errors: if stderr_str.is_empty() {
                Vec::new()
            } else {
                vec![stderr_str]
            },
        })
    }

    fn parse_variables(
        output: &str,
        expected_vars: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        for var_name in expected_vars.keys() {
            // Look for pattern: (VariableName) Type = value
            if let Some(line) = output
                .lines()
                .find(|l| l.contains(&format!("({}) ", var_name)))
            {
                if let Some(value) = line.split('=').nth(1) {
                    variables.insert(var_name.clone(), value.trim().to_string());
                }
            }
        }

        variables
    }
}

/// Differential test runner - compares results between debuggers
pub struct DifferentialTestRunner {
    tests: Vec<DifferentialTest>,
}

impl DifferentialTestRunner {
    pub fn new() -> Self {
        DifferentialTestRunner {
            tests: Vec::new(),
        }
    }

    pub fn add_test(&mut self, test: DifferentialTest) {
        self.tests.push(test);
    }

    /// Run all tests on both GDB and LLDB
    pub fn run_all(&self) -> Vec<DifferentialTestComparison> {
        self.tests
            .iter()
            .map(|test| {
                let gdb_result = GDBRunner::run_test(test).unwrap_or(DebuggerTestResult {
                    debugger: "gdb".to_string(),
                    test_name: test.name.clone(),
                    passed: false,
                    output: String::new(),
                    variables: HashMap::new(),
                    errors: vec!["Failed to run".to_string()],
                });

                let lldb_result = LLDBRunner::run_test(test).unwrap_or(DebuggerTestResult {
                    debugger: "lldb".to_string(),
                    test_name: test.name.clone(),
                    passed: false,
                    output: String::new(),
                    variables: HashMap::new(),
                    errors: vec!["Failed to run".to_string()],
                });

                DifferentialTestComparison {
                    test_name: test.name.clone(),
                    gdb_result,
                    lldb_result,
                }
            })
            .collect()
    }

    /// Check if results match across debuggers
    pub fn all_match(&self, comparison: &DifferentialTestComparison) -> bool {
        // Both should succeed
        if !comparison.gdb_result.passed || !comparison.lldb_result.passed {
            return false;
        }

        // Variables should match
        for (key, gdb_val) in &comparison.gdb_result.variables {
            if let Some(lldb_val) = comparison.lldb_result.variables.get(key) {
                if gdb_val != lldb_val {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

/// Comparison result between two debuggers
pub struct DifferentialTestComparison {
    pub test_name: String,
    pub gdb_result: DebuggerTestResult,
    pub lldb_result: DebuggerTestResult,
}

impl DifferentialTestComparison {
    /// Check if results agree
    pub fn agree(&self) -> bool {
        self.gdb_result.passed == self.lldb_result.passed
            && self.gdb_result.variables == self.lldb_result.variables
    }

    /// Generate comparison report
    pub fn report(&self) -> String {
        format!(
            r#"Test: {}

GDB Result:
  Status: {}
  Variables: {:?}
  
LLDB Result:
  Status: {}
  Variables: {:?}

Agreement: {}
"#,
            self.test_name,
            if self.gdb_result.passed { "PASS" } else { "FAIL" },
            self.gdb_result.variables,
            if self.lldb_result.passed { "PASS" } else { "FAIL" },
            self.lldb_result.variables,
            if self.agree() { "✅ YES" } else { "❌ NO" }
        )
    }
}

// ========== TESTS ==========

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_differential_test_creation() {
        let mut test_runner = DifferentialTestRunner::new();

        test_runner.add_test(DifferentialTest {
            name: "simple_add".to_string(),
            source_file: "test.c".to_string(),
            breakpoint: "main".to_string(),
            commands: vec!["print x".to_string()],
            expected_variables: {
                let mut m = HashMap::new();
                m.insert("x".to_string(), "10".to_string());
                m
            },
            expected_output: String::new(),
        });

        assert_eq!(test_runner.tests.len(), 1);
    }

    #[test]
    fn test_parse_gdb_variables() {
        let output = "Breakpoint 1 at 0x401030\nx = 10\n";
        let mut expected = HashMap::new();
        expected.insert("x".to_string(), "10".to_string());

        let result = GDBRunner::parse_variables(output, &expected);
        assert_eq!(result.get("x"), Some(&"10".to_string()));
    }

    #[test]
    fn test_comparison_agree() {
        let comparison = DifferentialTestComparison {
            test_name: "test".to_string(),
            gdb_result: DebuggerTestResult {
                debugger: "gdb".to_string(),
                test_name: "test".to_string(),
                passed: true,
                output: String::new(),
                variables: {
                    let mut m = HashMap::new();
                    m.insert("x".to_string(), "10".to_string());
                    m
                },
                errors: Vec::new(),
            },
            lldb_result: DebuggerTestResult {
                debugger: "lldb".to_string(),
                test_name: "test".to_string(),
                passed: true,
                output: String::new(),
                variables: {
                    let mut m = HashMap::new();
                    m.insert("x".to_string(), "10".to_string());
                    m
                },
                errors: Vec::new(),
            },
        };

        assert!(comparison.agree());
    }
}
