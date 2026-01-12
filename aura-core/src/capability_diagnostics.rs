#![forbid(unsafe_code)]

/// Capability Enforcement Diagnostics
/// 
/// Provides detailed error reporting for capability violations with:
/// - Source location information
/// - Remediation suggestions
/// - Diagnostic history/context
/// - LSP-compatible output format

use std::collections::VecDeque;
use crate::capability_enforcement::{CapabilityKind, CapabilityViolation};

/// Severity level of a diagnostic
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilitySeverity {
    Error,
    Warning,
    Info,
}

impl CapabilitySeverity {
    pub fn display(&self) -> &'static str {
        match self {
            CapabilitySeverity::Error => "error",
            CapabilitySeverity::Warning => "warning",
            CapabilitySeverity::Info => "info",
        }
    }
}

/// Source location information
#[derive(Clone, Debug)]
pub struct CapabilityLocation {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

impl CapabilityLocation {
    pub fn new(file: String, line: u32, col: u32) -> Self {
        CapabilityLocation { file, line, col }
    }

    pub fn display(&self) -> String {
        format!("{}:{}:{}", self.file, self.line, self.col)
    }
}

/// Related location for error context
#[derive(Clone, Debug)]
pub struct RelatedLocation {
    pub location: CapabilityLocation,
    pub message: String,
}

impl RelatedLocation {
    pub fn new(file: String, line: u32, col: u32, message: String) -> Self {
        RelatedLocation {
            location: CapabilityLocation::new(file, line, col),
            message,
        }
    }
}

/// Comprehensive capability violation diagnostic
#[derive(Clone, Debug)]
pub struct CapabilityDiagnostic {
    /// Primary location of violation
    pub location: CapabilityLocation,
    /// Violation kind
    pub violation: CapabilityViolation,
    /// Severity
    pub severity: CapabilitySeverity,
    /// Main error message
    pub message: String,
    /// Detailed explanation
    pub details: Option<String>,
    /// Related locations (e.g., where capability was defined)
    pub related_locations: Vec<RelatedLocation>,
    /// Suggested fix
    pub suggestion: Option<String>,
    /// Code snippet with highlights
    pub code_snippet: Option<CodeSnippet>,
}

impl CapabilityDiagnostic {
    pub fn new(
        location: CapabilityLocation,
        violation: CapabilityViolation,
        message: String,
    ) -> Self {
        CapabilityDiagnostic {
            location,
            violation,
            severity: CapabilitySeverity::Error,
            message,
            details: None,
            related_locations: Vec::new(),
            suggestion: None,
            code_snippet: None,
        }
    }

    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_severity(mut self, severity: CapabilitySeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_related(mut self, related: RelatedLocation) -> Self {
        self.related_locations.push(related);
        self
    }

    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }

    pub fn with_code_snippet(mut self, snippet: CodeSnippet) -> Self {
        self.code_snippet = Some(snippet);
        self
    }

    pub fn full_message(&self) -> String {
        let mut msg = format!(
            "{}: {}",
            self.severity.display(),
            self.message
        );

        if let Some(details) = &self.details {
            msg.push_str(&format!("\n  Details: {}", details));
        }

        if !self.related_locations.is_empty() {
            msg.push_str("\n  Related locations:");
            for related in &self.related_locations {
                msg.push_str(&format!("\n    - {} at {}",
                    related.message,
                    related.location.display()
                ));
            }
        }

        if let Some(suggestion) = &self.suggestion {
            msg.push_str(&format!("\n  Suggestion: {}", suggestion));
        }

        msg
    }
}

/// Code snippet for diagnostic context
#[derive(Clone, Debug)]
pub struct CodeSnippet {
    pub source_line: String,
    pub line_number: u32,
    pub highlight_start: u32,
    pub highlight_end: u32,
}

impl CodeSnippet {
    pub fn new(
        source_line: String,
        line_number: u32,
        highlight_start: u32,
        highlight_end: u32,
    ) -> Self {
        CodeSnippet {
            source_line,
            line_number,
            highlight_start,
            highlight_end,
        }
    }

    pub fn display(&self) -> String {
        let mut output = format!("  {} | {}\n", self.line_number, self.source_line);
        output.push_str("    | ");
        for i in 0..self.source_line.len() {
            if i >= self.highlight_start as usize && i < self.highlight_end as usize {
                output.push('^');
            } else {
                output.push(' ');
            }
        }
        output
    }
}

/// Factory for creating common capability diagnostics
pub struct CapabilityDiagnosticFactory;

impl CapabilityDiagnosticFactory {
    /// Create diagnostic for use after consumption
    pub fn use_after_consumption(
        file: String,
        line: u32,
        col: u32,
        var_name: &str,
        kind: CapabilityKind,
        consumed_at_line: u32,
        consumed_at_col: u32,
    ) -> CapabilityDiagnostic {
        let location = CapabilityLocation::new(file.clone(), line, col);
        let violation = CapabilityViolation::UseAfterConsumption {
            var_name: var_name.to_string(),
            consumed_at: (consumed_at_line, consumed_at_col),
        };

        let message = format!(
            "capability '{}' ({}) used after consumption",
            var_name,
            kind.display()
        );

        let mut diag = CapabilityDiagnostic::new(location, violation, message);

        diag = diag.with_details(format!(
            "The {} capability '{}' was already consumed (closed/finalized) at {}:{}. \
             It cannot be used again after that point.",
            kind.display(),
            var_name,
            consumed_at_line,
            consumed_at_col
        ));

        diag = diag.with_related(RelatedLocation::new(
            file.clone(),
            consumed_at_line,
            consumed_at_col,
            format!("'{}' was consumed here", var_name),
        ));

        diag = diag.with_suggestion(format!(
            "Either:\n  \
             1. Use '{}' before the consumption point\n  \
             2. Create a new {} capability instead\n  \
             3. Restructure your code to avoid using after consumption",
            var_name,
            kind.display()
        ));

        diag
    }

    /// Create diagnostic for resource leak
    pub fn resource_leak(
        file: String,
        line: u32,
        col: u32,
        var_name: &str,
        kind: CapabilityKind,
        defined_at_line: u32,
        defined_at_col: u32,
    ) -> CapabilityDiagnostic {
        let location = CapabilityLocation::new(file.clone(), line, col);
        let violation = CapabilityViolation::ResourceLeak {
            var_name: var_name.to_string(),
            current_state: crate::capability_enforcement::CapabilityState::InUse,
        };

        let message = format!(
            "resource leak: {} capability '{}' not consumed before scope end",
            kind.display(),
            var_name
        );

        let mut diag = CapabilityDiagnostic::new(location, violation, message);
        diag = diag.with_severity(CapabilitySeverity::Error);

        diag = diag.with_details(format!(
            "The {} capability '{}' was created at {}:{} but never consumed. \
             This will leak resources. All exclusive capabilities must be explicitly \
             closed/finalized before the scope ends.",
            kind.display(),
            var_name,
            defined_at_line,
            defined_at_col
        ));

        diag = diag.with_related(RelatedLocation::new(
            file.clone(),
            defined_at_line,
            defined_at_col,
            format!("'{}' was defined here", var_name),
        ));

        diag = diag.with_suggestion(format!(
            "You must consume the {} capability '{}':\n  \
             1. If it's a socket: close it (socket.close())\n  \
             2. If it's a tensor: release it (tensor.release())\n  \
             3. If it's a region: deallocate it (region.dealloc())\n  \
             4. Or pass it to a function that will consume it",
            kind.display(),
            var_name
        ));

        diag
    }

    /// Create diagnostic for concurrent access without synchronization
    pub fn concurrent_use_without_sync(
        file: String,
        line: u32,
        col: u32,
        var_name: &str,
        kind: CapabilityKind,
        first_access_line: u32,
        first_access_col: u32,
        second_access_line: u32,
        second_access_col: u32,
    ) -> CapabilityDiagnostic {
        let location = CapabilityLocation::new(file.clone(), line, col);
        let violation = CapabilityViolation::ConcurrentUseWithoutSync {
            var_name: var_name.to_string(),
            first_access: (first_access_line, first_access_col),
            second_access: (second_access_line, second_access_col),
        };

        let message = format!(
            "capability '{}' ({}) used concurrently without synchronization",
            var_name,
            kind.display()
        );

        let mut diag = CapabilityDiagnostic::new(location, violation, message);
        diag = diag.with_severity(CapabilitySeverity::Error);

        diag = diag.with_details(format!(
            "The {} capability '{}' is being accessed by multiple concurrent threads \
             without proper synchronization. Exclusive capabilities cannot be safely \
             shared across threads.",
            kind.display(),
            var_name
        ));

        diag = diag.with_related(RelatedLocation::new(
            file.clone(),
            first_access_line,
            first_access_col,
            format!("First access to '{}' here", var_name),
        ));

        diag = diag.with_related(RelatedLocation::new(
            file.clone(),
            second_access_line,
            second_access_col,
            format!("Second concurrent access to '{}' here", var_name),
        ));

        diag = diag.with_suggestion(format!(
            "To safely share the {} capability '{}':\n  \
             1. Wrap it in a Mutex<> or RwLock<>\n  \
             2. Mark it with @shared annotation\n  \
             3. Use thread-safe synchronization primitives",
            kind.display(),
            var_name
        ));

        diag
    }

    /// Create diagnostic for improper sharing
    pub fn improper_sharing(
        file: String,
        line: u32,
        col: u32,
        var_name: &str,
        kind: CapabilityKind,
    ) -> CapabilityDiagnostic {
        let location = CapabilityLocation::new(file, line, col);
        let violation = CapabilityViolation::ImproperSharing {
            var_name: var_name.to_string(),
            shared_at: (line, col),
        };

        let message = format!(
            "capability '{}' ({}) shared without proper annotation",
            var_name,
            kind.display()
        );

        let mut diag = CapabilityDiagnostic::new(location, violation, message);
        diag = diag.with_severity(CapabilitySeverity::Warning);

        diag = diag.with_details(format!(
            "The {} capability '{}' is being shared across threads or scopes, \
             but it is not marked with a sharing annotation.",
            kind.display(),
            var_name
        ));

        diag = diag.with_suggestion(format!(
            "Add a sharing annotation to '{}' if concurrent access is intentional:\n  \
             @shared val {}: {} = ...",
            var_name,
            var_name,
            kind.display()
        ));

        diag
    }
}

/// Collects and reports capability diagnostics
pub struct CapabilityDiagnosticReporter {
    diagnostics: VecDeque<CapabilityDiagnostic>,
}

impl CapabilityDiagnosticReporter {
    pub fn new() -> Self {
        CapabilityDiagnosticReporter {
            diagnostics: VecDeque::new(),
        }
    }

    pub fn add(&mut self, diag: CapabilityDiagnostic) {
        self.diagnostics.push_back(diag);
    }

    pub fn all(&self) -> Vec<&CapabilityDiagnostic> {
        self.diagnostics.iter().collect()
    }

    pub fn errors(&self) -> Vec<&CapabilityDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == CapabilitySeverity::Error)
            .collect()
    }

    pub fn warnings(&self) -> Vec<&CapabilityDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == CapabilitySeverity::Warning)
            .collect()
    }

    pub fn has_errors(&self) -> bool {
        !self.errors().is_empty()
    }

    pub fn count(&self) -> usize {
        self.diagnostics.len()
    }

    pub fn clear(&mut self) {
        self.diagnostics.clear();
    }

    pub fn report_all(&self) -> String {
        let mut output = String::new();
        for diag in self.all() {
            output.push_str(&diag.full_message());
            output.push('\n');
        }
        output
    }
}

impl Default for CapabilityDiagnosticReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_use_after_consumption_diagnostic() {
        let diag = CapabilityDiagnosticFactory::use_after_consumption(
            "test.aura".to_string(),
            5,
            10,
            "socket",
            CapabilityKind::Socket,
            3,
            5,
        );

        assert_eq!(diag.severity, CapabilitySeverity::Error);
        assert!(diag.message.contains("socket"));
        assert!(diag.full_message().contains("consumed"));
    }

    #[test]
    fn test_resource_leak_diagnostic() {
        let diag = CapabilityDiagnosticFactory::resource_leak(
            "test.aura".to_string(),
            10,
            0,
            "tensor",
            CapabilityKind::Tensor,
            2,
            5,
        );

        assert_eq!(diag.severity, CapabilitySeverity::Error);
        assert!(diag.message.contains("resource leak"));
        assert!(diag.suggestion.is_some());
    }

    #[test]
    fn test_concurrent_use_diagnostic() {
        let diag = CapabilityDiagnosticFactory::concurrent_use_without_sync(
            "test.aura".to_string(),
            8,
            5,
            "shared_socket",
            CapabilityKind::Socket,
            3,
            0,
            7,
            10,
        );

        assert_eq!(diag.severity, CapabilitySeverity::Error);
        assert!(diag.message.contains("concurrent"));
        assert_eq!(diag.related_locations.len(), 2);
    }

    #[test]
    fn test_diagnostic_reporter() {
        let mut reporter = CapabilityDiagnosticReporter::new();
        
        let diag1 = CapabilityDiagnosticFactory::use_after_consumption(
            "test.aura".to_string(),
            5,
            10,
            "socket",
            CapabilityKind::Socket,
            3,
            5,
        );

        let mut diag2 = CapabilityDiagnosticFactory::improper_sharing(
            "test.aura".to_string(),
            10,
            5,
            "tensor",
            CapabilityKind::Tensor,
        );

        reporter.add(diag1);
        reporter.add(diag2);

        assert_eq!(reporter.count(), 2);
        assert_eq!(reporter.errors().len(), 1);
        assert_eq!(reporter.warnings().len(), 1);
    }

    #[test]
    fn test_code_snippet() {
        let snippet = CodeSnippet::new(
            "val socket = Socket::create()".to_string(),
            5,
            4,
            10,
        );

        let display = snippet.display();
        assert!(display.contains("5 |"));
        assert!(display.contains("^"));
    }

    #[test]
    fn test_location_display() {
        let loc = CapabilityLocation::new(
            "test.aura".to_string(),
            5,
            10,
        );

        assert_eq!(loc.display(), "test.aura:5:10");
    }

    #[test]
    fn test_diagnostic_builder_pattern() {
        let location = CapabilityLocation::new("test.aura".to_string(), 5, 10);
        let violation = CapabilityViolation::ResourceLeak {
            var_name: "socket".to_string(),
            current_state: crate::capability_enforcement::CapabilityState::InUse,
        };

        let diag = CapabilityDiagnostic::new(location, violation, "test".to_string())
            .with_severity(CapabilitySeverity::Warning)
            .with_details("details".to_string())
            .with_suggestion("fix".to_string());

        assert_eq!(diag.severity, CapabilitySeverity::Warning);
        assert!(diag.details.is_some());
        assert!(diag.suggestion.is_some());
    }
}
