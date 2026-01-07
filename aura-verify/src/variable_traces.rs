#![forbid(unsafe_code)]

use std::collections::HashMap;

/// Represents a single trace event in a variable's lifecycle.
#[derive(Clone, Debug)]
pub struct TraceEvent {
    /// Type of event: "defined", "assigned", or "final"
    pub event_type: String,
    /// Line number where the event occurs (0-indexed from file start)
    pub line: u32,
    /// Column number (0-indexed)
    pub column: u32,
    /// Filename containing the event
    pub file: String,
    /// Value of the variable at this point
    pub value: String,
    /// Human-readable description of the event
    pub description: String,
}

/// Tracks all assignments and definitions of a single variable.
#[derive(Clone, Debug)]
pub struct VariableTrace {
    /// Variable name
    pub name: String,
    /// Variable type as string (e.g. "u32", "bool", "Record { x: u32, y: u32 }")
    pub var_type: String,
    /// Initial definition location and value
    pub defined_at: Option<TraceEvent>,
    /// All assignment events (in order of occurrence)
    pub assignments: Vec<TraceEvent>,
    /// Final value at counterexample point
    pub final_value: String,
}

impl VariableTrace {
    /// Create a new variable trace for a given variable.
    pub fn new(name: String, var_type: String, final_value: String) -> Self {
        VariableTrace {
            name,
            var_type,
            defined_at: None,
            assignments: Vec::new(),
            final_value,
        }
    }

    /// Add a definition event.
    pub fn add_definition(
        &mut self,
        file: String,
        line: u32,
        column: u32,
        value: String,
    ) {
        self.defined_at = Some(TraceEvent {
            event_type: "defined".to_string(),
            line,
            column,
            file,
            value: value.clone(),
            description: format!(
                "{} defined with initial value {}",
                self.name, value
            ),
        });
    }

    /// Add an assignment event.
    pub fn add_assignment(&mut self, file: String, line: u32, column: u32, value: String) {
        self.assignments.push(TraceEvent {
            event_type: "assigned".to_string(),
            line,
            column,
            file,
            value: value.clone(),
            description: format!("{} assigned to {}", self.name, value),
        });
    }

    /// Get a timeline summary for display in UI.
    pub fn timeline_summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(def) = &self.defined_at {
            parts.push(format!(
                "Defined at {}:{} ({})",
                def.file, def.line, def.value
            ));
        }

        if !self.assignments.is_empty() {
            let last_assign = &self.assignments[self.assignments.len() - 1];
            parts.push(format!(
                "Reassigned {} time(s), last at {}:{} ({})",
                self.assignments.len(),
                last_assign.file,
                last_assign.line,
                last_assign.value
            ));
        }

        parts.push(format!("Final value: {}", self.final_value));

        parts.join(" → ")
    }
}

/// Collects variable traces for counterexample analysis.
pub struct TraceCollector {
    traces: HashMap<String, VariableTrace>,
}

impl TraceCollector {
    /// Create a new trace collector.
    pub fn new() -> Self {
        TraceCollector {
            traces: HashMap::new(),
        }
    }

    /// Register a variable with its type and initial value.
    pub fn track_variable(&mut self, name: String, var_type: String, final_value: String) {
        self.traces
            .insert(name.clone(), VariableTrace::new(name, var_type, final_value));
    }

    /// Record a variable definition event.
    pub fn record_definition(
        &mut self,
        name: &str,
        file: String,
        line: u32,
        column: u32,
        value: String,
    ) {
        if let Some(trace) = self.traces.get_mut(name) {
            trace.add_definition(file, line, column, value);
        }
    }

    /// Record a variable assignment event.
    pub fn record_assignment(
        &mut self,
        name: &str,
        file: String,
        line: u32,
        column: u32,
        value: String,
    ) {
        if let Some(trace) = self.traces.get_mut(name) {
            trace.add_assignment(file, line, column, value);
        }
    }

    /// Get all collected traces.
    pub fn traces(&self) -> Vec<VariableTrace> {
        self.traces.values().cloned().collect()
    }

    /// Get traces for specific variables (by name).
    pub fn traces_for_variables(&self, names: &[&str]) -> Vec<VariableTrace> {
        names
            .iter()
            .filter_map(|name| self.traces.get(*name).cloned())
            .collect()
    }

    /// Get a summary of all traces.
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Variable Trace Summary ({} variables):", self.traces.len()));
        
        for trace in self.traces.values() {
            lines.push(format!("  {} ({}): {}", trace.name, trace.var_type, trace.timeline_summary()));
        }

        lines.join("\n")
    }
}

impl Default for TraceCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_trace_creation() {
        let trace = VariableTrace::new(
            "x".to_string(),
            "u32".to_string(),
            "42".to_string(),
        );
        assert_eq!(trace.name, "x");
        assert_eq!(trace.var_type, "u32");
        assert_eq!(trace.final_value, "42");
        assert_eq!(trace.assignments.len(), 0);
    }

    #[test]
    fn test_add_definition() {
        let mut trace = VariableTrace::new(
            "x".to_string(),
            "u32".to_string(),
            "42".to_string(),
        );
        trace.add_definition(
            "test.aura".to_string(),
            10,
            5,
            "0".to_string(),
        );
        
        assert!(trace.defined_at.is_some());
        let def = trace.defined_at.as_ref().unwrap();
        assert_eq!(def.file, "test.aura");
        assert_eq!(def.line, 10);
        assert_eq!(def.value, "0");
    }

    #[test]
    fn test_add_assignments() {
        let mut trace = VariableTrace::new(
            "x".to_string(),
            "u32".to_string(),
            "42".to_string(),
        );
        
        trace.add_assignment("test.aura".to_string(), 15, 8, "10".to_string());
        trace.add_assignment("test.aura".to_string(), 20, 8, "30".to_string());
        
        assert_eq!(trace.assignments.len(), 2);
        assert_eq!(trace.assignments[0].value, "10");
        assert_eq!(trace.assignments[1].value, "30");
    }

    #[test]
    fn test_timeline_summary() {
        let mut trace = VariableTrace::new(
            "x".to_string(),
            "u32".to_string(),
            "42".to_string(),
        );
        
        trace.add_definition("test.aura".to_string(), 10, 5, "0".to_string());
        trace.add_assignment("test.aura".to_string(), 15, 8, "10".to_string());
        trace.add_assignment("test.aura".to_string(), 20, 8, "42".to_string());
        
        let summary = trace.timeline_summary();
        assert!(summary.contains("Defined at"));
        assert!(summary.contains("Reassigned 2 time(s)"));
        assert!(summary.contains("Final value: 42"));
    }

    #[test]
    fn test_trace_collector() {
        let mut collector = TraceCollector::new();
        
        collector.track_variable("x".to_string(), "u32".to_string(), "42".to_string());
        collector.track_variable("y".to_string(), "bool".to_string(), "true".to_string());
        
        collector.record_definition("x", "test.aura".to_string(), 10, 5, "0".to_string());
        collector.record_assignment("x", "test.aura".to_string(), 15, 8, "10".to_string());
        collector.record_assignment("x", "test.aura".to_string(), 20, 8, "42".to_string());
        
        let traces = collector.traces();
        assert_eq!(traces.len(), 2);
        
        let x_trace = &traces[0];
        assert_eq!(x_trace.name, "x");
        assert_eq!(x_trace.assignments.len(), 2);
    }

    #[test]
    fn test_traces_for_specific_variables() {
        let mut collector = TraceCollector::new();
        
        collector.track_variable("x".to_string(), "u32".to_string(), "42".to_string());
        collector.track_variable("y".to_string(), "bool".to_string(), "true".to_string());
        collector.track_variable("z".to_string(), "u32".to_string(), "100".to_string());
        
        let selected = collector.traces_for_variables(&["x", "z"]);
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().any(|t| t.name == "x"));
        assert!(selected.iter().any(|t| t.name == "z"));
        assert!(!selected.iter().any(|t| t.name == "y"));
    }

    #[test]
    fn test_collector_summary() {
        let mut collector = TraceCollector::new();
        
        collector.track_variable("x".to_string(), "u32".to_string(), "42".to_string());
        collector.track_variable("y".to_string(), "bool".to_string(), "true".to_string());
        
        let summary = collector.summary();
        assert!(summary.contains("Variable Trace Summary (2 variables)"));
        assert!(summary.contains("x (u32)"));
        assert!(summary.contains("y (bool)"));
    }

    #[test]
    fn test_empty_trace_no_assignments() {
        let trace = VariableTrace::new(
            "unused".to_string(),
            "u32".to_string(),
            "0".to_string(),
        );
        
        let summary = trace.timeline_summary();
        assert!(!summary.contains("Reassigned"));
        assert!(summary.contains("Final value: 0"));
    }
}
