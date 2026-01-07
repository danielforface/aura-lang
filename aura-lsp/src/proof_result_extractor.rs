/// Proof Result Extraction & Mapping
/// 
/// Extracts proof verification results and maps them to debugger test cases
/// for differential testing in the CI gate

use std::collections::HashMap;

/// Raw proof result from verification engine
#[derive(Debug, Clone)]
pub struct RawProofResult {
    pub proof_id: String,
    pub program_text: String,
    pub verification_result: VerificationOutcome,
    pub witness_data: Option<WitnessData>,
    pub execution_trace: Option<ExecutionTrace>,
    pub error_info: Option<ErrorInfo>,
}

/// Verification outcome
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationOutcome {
    Verified,
    CounterexampleFound,
    Timeout,
    Error(String),
    Incomplete,
}

/// Witness data from proof
#[derive(Debug, Clone)]
pub struct WitnessData {
    pub values: HashMap<String, ProofValue>,
    pub constraints: Vec<Constraint>,
    pub timestamp: u64,
}

/// Individual proof value
#[derive(Debug, Clone, PartialEq)]
pub enum ProofValue {
    Integer(i64),
    Boolean(bool),
    String(String),
    Array(Vec<ProofValue>),
    Struct(HashMap<String, ProofValue>),
}

impl ProofValue {
    pub fn to_string_display(&self) -> String {
        match self {
            ProofValue::Integer(i) => i.to_string(),
            ProofValue::Boolean(b) => b.to_string(),
            ProofValue::String(s) => s.clone(),
            ProofValue::Array(arr) => {
                format!(
                    "[{}]",
                    arr.iter()
                        .map(|v| v.to_string_display())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            ProofValue::Struct(s) => {
                let fields: Vec<_> = s
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v.to_string_display()))
                    .collect();
                format!("{{{}}}", fields.join(", "))
            }
        }
    }
}

/// Constraint from proof
#[derive(Debug, Clone)]
pub struct Constraint {
    pub variable: String,
    pub operator: String,
    pub value: ProofValue,
}

/// Execution trace from proof
#[derive(Debug, Clone)]
pub struct ExecutionTrace {
    pub steps: Vec<TraceStep>,
    pub entry_point: String,
    pub exit_point: String,
}

/// Individual execution step
#[derive(Debug, Clone)]
pub struct TraceStep {
    pub line: usize,
    pub function: String,
    pub operation: String,
    pub state_before: HashMap<String, ProofValue>,
    pub state_after: HashMap<String, ProofValue>,
}

/// Error information
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub error_type: String,
    pub message: String,
    pub location: Option<SourceLocation>,
}

/// Source code location
#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

/// Mapped test case for debugger
#[derive(Debug, Clone)]
pub struct MappedDebuggerTest {
    pub test_name: String,
    pub program: String,
    pub breakpoint: String,
    pub expected_variables: HashMap<String, String>,
    pub expected_output: String,
    pub debugger_commands: Vec<String>,
    pub validation_rules: Vec<ValidationRule>,
}

/// Validation rule for test
#[derive(Debug, Clone)]
pub struct ValidationRule {
    pub variable: String,
    pub check_type: ValidationType,
    pub expected_value: String,
}

/// Validation check type
#[derive(Debug, Clone)]
pub enum ValidationType {
    Equals,
    NotEquals,
    Greater,
    Less,
    Contains,
    StartsWith,
}

/// Proof result extractor
pub struct ProofResultExtractor;

impl ProofResultExtractor {
    /// Extract and map proof results to debugger tests
    pub fn extract_and_map(raw_result: &RawProofResult) -> Result<MappedDebuggerTest, String> {
        // Validate raw result
        Self::validate_raw_result(raw_result)?;

        // Extract witness data
        let variables = if let Some(witness) = &raw_result.witness_data {
            Self::extract_witness_variables(witness)
        } else {
            HashMap::new()
        };

        // Generate test name
        let test_name = Self::generate_test_name(&raw_result.proof_id);

        // Generate debugger commands
        let commands = Self::generate_debugger_commands(&variables);

        // Generate validation rules
        let rules = Self::generate_validation_rules(&variables);

        // Extract expected output
        let output = Self::extract_expected_output(raw_result, &variables);

        Ok(MappedDebuggerTest {
            test_name,
            program: raw_result.program_text.clone(),
            breakpoint: Self::extract_breakpoint(raw_result),
            expected_variables: variables,
            expected_output: output,
            debugger_commands: commands,
            validation_rules: rules,
        })
    }

    fn validate_raw_result(result: &RawProofResult) -> Result<(), String> {
        if result.proof_id.is_empty() {
            return Err("Empty proof ID".to_string());
        }

        if result.program_text.is_empty() {
            return Err("Empty program text".to_string());
        }

        Ok(())
    }

    fn extract_witness_variables(witness: &WitnessData) -> HashMap<String, String> {
        witness
            .values
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string_display()))
            .collect()
    }

    fn generate_test_name(proof_id: &str) -> String {
        format!("test_{}", proof_id.replace("-", "_").to_lowercase())
    }

    fn extract_breakpoint(result: &RawProofResult) -> String {
        // Try to extract main function or first function
        if let Some(pos) = result.program_text.find("fn ") {
            let remainder = &result.program_text[pos + 3..];
            if let Some(end) = remainder.find('(') {
                return remainder[..end].trim().to_string();
            }
        }

        "main".to_string()
    }

    fn generate_debugger_commands(variables: &HashMap<String, String>) -> Vec<String> {
        let mut commands = vec![
            "break main".to_string(),
            "run".to_string(),
        ];

        // Add print commands for each variable
        for var_name in variables.keys() {
            commands.push(format!("print {}", var_name));
        }

        commands.push("quit".to_string());
        commands
    }

    fn generate_validation_rules(variables: &HashMap<String, String>) -> Vec<ValidationRule> {
        variables
            .iter()
            .map(|(name, value)| ValidationRule {
                variable: name.clone(),
                check_type: ValidationType::Equals,
                expected_value: value.clone(),
            })
            .collect()
    }

    fn extract_expected_output(
        result: &RawProofResult,
        variables: &HashMap<String, String>,
    ) -> String {
        let mut output = String::new();

        // Add verification status
        match result.verification_result {
            VerificationOutcome::Verified => output.push_str("Verification: OK\n"),
            VerificationOutcome::CounterexampleFound => output.push_str("Verification: COUNTEREXAMPLE\n"),
            VerificationOutcome::Timeout => output.push_str("Verification: TIMEOUT\n"),
            VerificationOutcome::Error(ref e) => output.push_str(&format!("Verification: ERROR - {}\n", e)),
            VerificationOutcome::Incomplete => output.push_str("Verification: INCOMPLETE\n"),
        }

        // Add variables to output
        if !variables.is_empty() {
            output.push_str("Variables:\n");
            for (name, value) in variables.iter() {
                output.push_str(&format!("  {} = {}\n", name, value));
            }
        }

        output
    }
}

/// Proof result mapper
pub struct ProofResultMapper;

impl ProofResultMapper {
    /// Map multiple proof results to test cases
    pub fn map_results(
        raw_results: &[RawProofResult],
    ) -> Result<Vec<MappedDebuggerTest>, String> {
        raw_results
            .iter()
            .map(ProofResultExtractor::extract_and_map)
            .collect()
    }

    /// Create validation suite from proofs
    pub fn create_validation_suite(
        results: &[MappedDebuggerTest],
    ) -> ValidationSuite {
        ValidationSuite {
            tests: results.to_vec(),
            total_tests: results.len(),
            total_validations: results
                .iter()
                .map(|t| t.validation_rules.len())
                .sum(),
        }
    }

    /// Generate differential test pairs
    pub fn generate_differential_pairs(
        results: &[MappedDebuggerTest],
    ) -> Vec<DifferentialTestPair> {
        results
            .iter()
            .map(|test| DifferentialTestPair {
                test_name: test.test_name.clone(),
                gdb_test: test.clone(),
                lldb_test: test.clone(),
            })
            .collect()
    }
}

/// Validation test suite
#[derive(Debug, Clone)]
pub struct ValidationSuite {
    pub tests: Vec<MappedDebuggerTest>,
    pub total_tests: usize,
    pub total_validations: usize,
}

/// Differential test pair (same test on GDB and LLDB)
#[derive(Debug, Clone)]
pub struct DifferentialTestPair {
    pub test_name: String,
    pub gdb_test: MappedDebuggerTest,
    pub lldb_test: MappedDebuggerTest,
}

// ==================== TESTS ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_witness_variables() {
        let mut values = HashMap::new();
        values.insert("x".to_string(), ProofValue::Integer(42));
        values.insert("y".to_string(), ProofValue::Boolean(true));

        let witness = WitnessData {
            values,
            constraints: vec![],
            timestamp: 0,
        };

        let extracted = ProofResultExtractor::extract_witness_variables(&witness);
        assert_eq!(extracted.get("x"), Some(&"42".to_string()));
        assert_eq!(extracted.get("y"), Some(&"true".to_string()));
    }

    #[test]
    fn test_proof_value_display() {
        assert_eq!(ProofValue::Integer(42).to_string_display(), "42");
        assert_eq!(ProofValue::Boolean(true).to_string_display(), "true");
        assert_eq!(ProofValue::String("test".to_string()).to_string_display(), "test");
    }

    #[test]
    fn test_extract_and_map() {
        let result = RawProofResult {
            proof_id: "test_proof".to_string(),
            program_text: "fn main() { let x = 42; }".to_string(),
            verification_result: VerificationOutcome::Verified,
            witness_data: None,
            execution_trace: None,
            error_info: None,
        };

        let mapped = ProofResultExtractor::extract_and_map(&result);
        assert!(mapped.is_ok());

        let test = mapped.unwrap();
        assert!(test.test_name.contains("test_proof"));
    }

    #[test]
    fn test_validation_suite_creation() {
        let tests = vec![];
        let suite = ProofResultMapper::create_validation_suite(&tests);

        assert_eq!(suite.total_tests, 0);
    }
}
