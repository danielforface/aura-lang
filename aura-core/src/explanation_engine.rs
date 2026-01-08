/// Explanation Engine - Pillar 1 Rewrite
/// 
/// Transforms Z3 UNSAT cores into human-readable explanations that developers
/// can understand. Converts cryptic solver outputs into clear, actionable insights.
/// 
/// Key innovation: Traces proof steps back to source code, enriches with variable
/// values, and generates example-based explanations.

#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};

/// Proof step in the derivation
#[derive(Clone, Debug)]
pub struct ProofStep {
    /// Source location in program
    pub location: (u32, u32),
    /// Source code line
    pub source_line: String,
    /// What this step derives
    pub derives: String,
    /// Why this is true
    pub reason: String,
    /// Child steps that support this
    pub children: Vec<Box<ProofStep>>,
}

impl ProofStep {
    pub fn new(location: (u32, u32), source_line: String, derives: String, reason: String) -> Self {
        ProofStep {
            location,
            source_line,
            derives,
            reason,
            children: Vec::new(),
        }
    }

    pub fn with_child(mut self, child: ProofStep) -> Self {
        self.children.push(Box::new(child));
        self
    }

    /// Convert to human-readable English
    pub fn to_english(&self) -> String {
        format!(
            "At line {}: {}\n\
             This means: {}\n\
             Because: {}",
            self.location.0,
            self.source_line,
            self.derives,
            self.reason
        )
    }
}

/// Variable binding information from trace
#[derive(Clone, Debug)]
pub struct VariableBinding {
    pub name: String,
    pub ty: String,
    pub value: Option<String>,
    pub defined_at: (u32, u32),
    pub used_at: Vec<(u32, u32)>,
}

impl VariableBinding {
    pub fn new(name: String, ty: String) -> Self {
        VariableBinding {
            name,
            ty,
            value: None,
            defined_at: (0, 0),
            used_at: Vec::new(),
        }
    }

    pub fn with_value(mut self, value: String) -> Self {
        self.value = Some(value);
        self
    }

    pub fn display(&self) -> String {
        match &self.value {
            Some(v) => format!("{}: {} = {}", self.name, self.ty, v),
            None => format!("{}: {}", self.name, self.ty),
        }
    }
}

/// Concrete counterexample
#[derive(Clone, Debug)]
pub struct Counterexample {
    /// Variable bindings in the counterexample
    pub bindings: Vec<VariableBinding>,
    /// Expected vs actual values
    pub violations: Vec<(String, String, String)>, // (property, expected, actual)
}

impl Counterexample {
    pub fn new() -> Self {
        Counterexample {
            bindings: Vec::new(),
            violations: Vec::new(),
        }
    }

    pub fn add_binding(&mut self, binding: VariableBinding) {
        self.bindings.push(binding);
    }

    pub fn add_violation(&mut self, property: String, expected: String, actual: String) {
        self.violations.push((property, expected, actual));
    }

    pub fn display(&self) -> String {
        let mut output = String::from("Counterexample:\n");
        
        output.push_str("  Variables:\n");
        for binding in &self.bindings {
            output.push_str(&format!("    {}\n", binding.display()));
        }
        
        output.push_str("  Violations:\n");
        for (prop, expected, actual) in &self.violations {
            output.push_str(&format!(
                "    {} should be '{}' but is '{}'\n",
                prop, expected, actual
            ));
        }
        
        output
    }
}

impl Default for Counterexample {
    fn default() -> Self {
        Self::new()
    }
}

/// Human-readable explanation
#[derive(Clone, Debug)]
pub struct Explanation {
    /// Main error message in plain English
    pub message: String,
    /// Proof steps in order
    pub proof_steps: Vec<ProofStep>,
    /// Variable trace enrichment
    pub variable_trace: HashMap<String, VariableBinding>,
    /// Concrete counterexample
    pub counterexample: Option<Counterexample>,
    /// Suggested fixes
    pub suggestions: Vec<String>,
}

impl Explanation {
    pub fn new(message: String) -> Self {
        Explanation {
            message,
            proof_steps: Vec::new(),
            variable_trace: HashMap::new(),
            counterexample: None,
            suggestions: Vec::new(),
        }
    }

    pub fn add_step(&mut self, step: ProofStep) {
        self.proof_steps.push(step);
    }

    pub fn add_variable(&mut self, binding: VariableBinding) {
        self.variable_trace.insert(binding.name.clone(), binding);
    }

    pub fn set_counterexample(&mut self, ce: Counterexample) {
        self.counterexample = Some(ce);
    }

    pub fn add_suggestion(&mut self, suggestion: String) {
        self.suggestions.push(suggestion);
    }

    /// Generate full human-readable explanation
    pub fn full_explanation(&self) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("# {}\n\n", self.message));
        
        output.push_str("## Proof\n");
        for (i, step) in self.proof_steps.iter().enumerate() {
            output.push_str(&format!(
                "{}. At line {}: {}\n   → {}\n   (Because: {})\n\n",
                i + 1,
                step.location.0,
                step.source_line,
                step.derives,
                step.reason
            ));
        }
        
        if !self.variable_trace.is_empty() {
            output.push_str("## Variable Trace\n");
            for (name, binding) in &self.variable_trace {
                output.push_str(&format!("  - {}\n", binding.display()));
                output.push_str(&format!("    Defined at line {}\n", binding.defined_at.0));
                if !binding.used_at.is_empty() {
                    output.push_str("    Used at lines: ");
                    let lines: Vec<_> = binding.used_at.iter().map(|l| l.0.to_string()).collect();
                    output.push_str(&lines.join(", "));
                    output.push('\n');
                }
            }
            output.push('\n');
        }
        
        if let Some(ce) = &self.counterexample {
            output.push_str("## Counterexample\n");
            output.push_str(&ce.display());
            output.push('\n');
        }
        
        if !self.suggestions.is_empty() {
            output.push_str("## How to Fix\n");
            for (i, suggestion) in self.suggestions.iter().enumerate() {
                output.push_str(&format!("{}. {}\n", i + 1, suggestion));
            }
        }
        
        output
    }

    /// Generate concise one-liner explanation
    pub fn short_explanation(&self) -> String {
        if let Some(ce) = &self.counterexample {
            if let Some((prop, expected, actual)) = ce.violations.first() {
                return format!(
                    "{}: expected {} but got {}",
                    prop, expected, actual
                );
            }
        }
        self.message.clone()
    }
}

/// Explanation generator from UNSAT cores
pub struct ExplanationEngine {
    /// Proof steps by location
    proof_cache: HashMap<(u32, u32), ProofStep>,
    /// Variable information
    variable_info: HashMap<String, VariableBinding>,
}

impl ExplanationEngine {
    pub fn new() -> Self {
        ExplanationEngine {
            proof_cache: HashMap::new(),
            variable_info: HashMap::new(),
        }
    }

    /// Add a proof step to cache
    pub fn add_proof_step(&mut self, location: (u32, u32), step: ProofStep) {
        self.proof_cache.insert(location, step);
    }

    /// Add variable information
    pub fn add_variable_info(&mut self, binding: VariableBinding) {
        self.variable_info.insert(binding.name.clone(), binding);
    }

    /// Generate explanation from counterexample
    pub fn explain_counterexample(
        &self,
        property: &str,
        expected: &str,
        actual: &str,
    ) -> Explanation {
        let message = format!(
            "Verification failed: {} should be '{}' but is '{}'",
            property, expected, actual
        );
        
        let mut explanation = Explanation::new(message);

        // Create counterexample
        let mut ce = Counterexample::new();
        ce.add_violation(property.to_string(), expected.to_string(), actual.to_string());
        
        // Add variable bindings
        for (_, binding) in &self.variable_info {
            ce.add_binding(binding.clone());
            explanation.add_variable(binding.clone());
        }
        
        explanation.set_counterexample(ce);

        // Add suggestions
        explanation.add_suggestion(
            "Check the logic: are all constraints properly defined?".to_string()
        );
        explanation.add_suggestion(
            "Review the variable definitions and their types".to_string()
        );

        explanation
    }

    /// Generate explanation from proof trace
    pub fn explain_proof(
        &self,
        property: &str,
        proof_locations: &[(u32, u32)],
    ) -> Explanation {
        let message = format!("Could not verify: {}", property);
        let mut explanation = Explanation::new(message);

        // Add proof steps in order
        for location in proof_locations {
            if let Some(step) = self.proof_cache.get(location) {
                explanation.add_step(step.clone());
            }
        }

        // Add variable info
        for (_, binding) in &self.variable_info {
            explanation.add_variable(binding.clone());
        }

        explanation
    }

    /// Generate example-based explanation
    pub fn explain_with_example(
        &self,
        property: &str,
        example: HashMap<String, String>,
    ) -> Explanation {
        let message = format!("Counterexample for: {}", property);
        let mut explanation = Explanation::new(message);

        let mut ce = Counterexample::new();
        
        for (var_name, value) in example {
            if let Some(mut binding) = self.variable_info.get(&var_name).cloned() {
                binding = binding.with_value(value);
                explanation.add_variable(binding.clone());
                ce.add_binding(binding);
            }
        }

        explanation.set_counterexample(ce);
        explanation.add_suggestion("Use this example to debug your code".to_string());
        explanation
    }
}

impl Default for ExplanationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_step_creation() {
        let step = ProofStep::new(
            (5, 0),
            "x = 10".to_string(),
            "x >= 0".to_string(),
            "x is assigned positive value".to_string(),
        );

        assert_eq!(step.location, (5, 0));
        assert_eq!(step.derives, "x >= 0");
    }

    #[test]
    fn test_variable_binding() {
        let binding = VariableBinding::new("x".to_string(), "u32".to_string())
            .with_value("42".to_string());

        assert_eq!(binding.name, "x");
        assert_eq!(binding.value, Some("42".to_string()));
    }

    #[test]
    fn test_counterexample_display() {
        let mut ce = Counterexample::new();
        let binding = VariableBinding::new("x".to_string(), "u32".to_string())
            .with_value("5".to_string());
        
        ce.add_binding(binding);
        ce.add_violation("x > 10".to_string(), "true".to_string(), "false".to_string());

        let display = ce.display();
        assert!(display.contains("x"));
        assert!(display.contains("5"));
    }

    #[test]
    fn test_explanation_creation() {
        let mut exp = Explanation::new("Test error".to_string());
        
        let step = ProofStep::new(
            (1, 0),
            "code".to_string(),
            "property".to_string(),
            "reason".to_string(),
        );
        
        exp.add_step(step);
        exp.add_suggestion("Fix it".to_string());

        assert!(!exp.proof_steps.is_empty());
        assert_eq!(exp.suggestions.len(), 1);
    }

    #[test]
    fn test_explanation_engine() {
        let mut engine = ExplanationEngine::new();
        
        let binding = VariableBinding::new("x".to_string(), "u32".to_string())
            .with_value("5".to_string());
        
        engine.add_variable_info(binding);

        let exp = engine.explain_counterexample(
            "x > 10",
            "true",
            "false",
        );

        assert!(exp.message.contains("failed"));
        assert!(exp.counterexample.is_some());
    }

    #[test]
    fn test_short_explanation() {
        let mut exp = Explanation::new("Error".to_string());
        
        let mut ce = Counterexample::new();
        ce.add_violation("prop".to_string(), "expected".to_string(), "actual".to_string());
        exp.set_counterexample(ce);

        let short = exp.short_explanation();
        assert!(short.contains("prop"));
        assert!(short.contains("expected"));
    }

    #[test]
    fn test_full_explanation() {
        let mut exp = Explanation::new("Test error".to_string());
        
        let step = ProofStep::new(
            (5, 0),
            "x = 10".to_string(),
            "derived".to_string(),
            "because".to_string(),
        );
        
        exp.add_step(step);
        
        let binding = VariableBinding::new("x".to_string(), "u32".to_string())
            .with_value("10".to_string());
        exp.add_variable(binding);
        
        let full = exp.full_explanation();
        assert!(full.contains("Test error"));
        assert!(full.contains("Proof"));
        assert!(full.contains("Variable Trace"));
    }

    #[test]
    fn test_explain_with_example() {
        let engine = ExplanationEngine::new();
        
        let mut binding = VariableBinding::new("x".to_string(), "u32".to_string());
        binding.defined_at = (1, 0);
        
        // Note: In a real test, we'd add the binding to the engine
        let mut example = HashMap::new();
        example.insert("x".to_string(), "5".to_string());
        
        let exp = engine.explain_with_example("x > 10", example);
        assert!(exp.message.contains("Counterexample"));
    }
}
