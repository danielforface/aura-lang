/// Linear Type Enforcement Diagnostics
/// 
/// Provides detailed error messages and diagnostics for linear type violations,
/// including move site tracking, source location information, and helpful suggestions.

use crate::ownership_enforcement::ViolationKind;

/// Diagnostic severity level
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Error that must be fixed
    Error,
    /// Warning that should be addressed
    Warning,
    /// Informational hint
    Info,
}

impl Severity {
    pub fn display(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

/// Source location information
#[derive(Clone, Debug)]
pub struct Location {
    /// Line number (1-based)
    pub line: u32,
    /// Column number (1-based)
    pub col: u32,
    /// File path
    pub file: String,
}

impl Location {
    pub fn new(file: String, line: u32, col: u32) -> Self {
        Location { file, line, col }
    }
    
    pub fn display(&self) -> String {
        format!("{}:{}:{}", self.file, self.line, self.col)
    }
}

/// Related location information (e.g., move site)
#[derive(Clone, Debug)]
pub struct RelatedLocation {
    pub location: Location,
    pub message: String,
}

/// Detailed diagnostic for a linear type violation
#[derive(Clone, Debug)]
pub struct LinearTypeDiagnostic {
    /// Primary location of the error
    pub location: Location,
    /// Severity level
    pub severity: Severity,
    /// Error kind/category
    pub error_kind: ViolationKind,
    /// Brief error message
    pub message: String,
    /// Detailed explanation
    pub details: Option<String>,
    /// Related locations (move site, definition, etc.)
    pub related: Vec<RelatedLocation>,
    /// Suggested fix
    pub suggestion: Option<String>,
    /// Code snippet showing the problem
    pub code_snippet: Option<CodeSnippet>,
}

/// Code snippet to display in diagnostics
#[derive(Clone, Debug)]
pub struct CodeSnippet {
    /// Source code lines
    pub lines: Vec<String>,
    /// Line number of first line in snippet (1-based)
    pub start_line: u32,
    /// Column ranges to highlight (per line)
    pub highlights: Vec<(u32, u32, u32)>, // (line_offset, start_col, end_col)
}

impl CodeSnippet {
    pub fn new(lines: Vec<String>, start_line: u32) -> Self {
        CodeSnippet {
            lines,
            start_line,
            highlights: Vec::new(),
        }
    }
    
    pub fn with_highlight(mut self, line_offset: u32, start_col: u32, end_col: u32) -> Self {
        self.highlights.push((line_offset, start_col, end_col));
        self
    }
    
    pub fn display(&self) -> String {
        let mut result = String::new();
        for (idx, line) in self.lines.iter().enumerate() {
            let line_num = self.start_line + idx as u32;
            result.push_str(&format!("{:>4} | {}\n", line_num, line));
            
            // Add highlight underline if this line has highlights
            if let Some(highlight) = self.highlights.iter().find(|(off, _, _)| *off as usize == idx) {
                let spaces = " ".repeat(4 + 3 + highlight.1 as usize);
                let underline = "^".repeat((highlight.2 - highlight.1) as usize);
                result.push_str(&format!("     | {}{}\n", spaces, underline));
            }
        }
        result
    }
}

impl LinearTypeDiagnostic {
    /// Create a new linear type diagnostic
    pub fn new(
        location: Location,
        error_kind: ViolationKind,
        message: String,
    ) -> Self {
        LinearTypeDiagnostic {
            location,
            severity: Severity::Error,
            error_kind,
            message,
            details: None,
            related: Vec::new(),
            suggestion: None,
            code_snippet: None,
        }
    }
    
    /// Set the severity level
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
    
    /// Add detailed explanation
    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }
    
    /// Add a related location
    pub fn with_related(mut self, location: Location, message: String) -> Self {
        self.related.push(RelatedLocation { location, message });
        self
    }
    
    /// Add a suggestion for fixing
    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }
    
    /// Add a code snippet
    pub fn with_snippet(mut self, snippet: CodeSnippet) -> Self {
        self.code_snippet = Some(snippet);
        self
    }
    
    /// Format the diagnostic as a string
    pub fn display(&self) -> String {
        let mut output = String::new();
        
        // Header: severity + location + error code
        output.push_str(&format!(
            "{}: {}: {}\n",
            self.severity.display().to_uppercase(),
            self.location.display(),
            self.message
        ));
        
        // Code snippet if available
        if let Some(snippet) = &self.code_snippet {
            output.push_str("\n");
            output.push_str(&snippet.display());
        }
        
        // Details
        if let Some(details) = &self.details {
            output.push_str(&format!("\nDetails:\n  {}\n", details));
        }
        
        // Related locations
        if !self.related.is_empty() {
            output.push_str("\nRelated locations:\n");
            for rel in &self.related {
                output.push_str(&format!("  {} {}\n", rel.location.display(), rel.message));
            }
        }
        
        // Suggestion
        if let Some(suggestion) = &self.suggestion {
            output.push_str(&format!("\nSuggestion:\n  {}\n", suggestion));
        }
        
        output
    }
}

/// Builder for creating diagnostics with fluent API
pub struct DiagnosticBuilder {
    location: Location,
    error_kind: ViolationKind,
    message: String,
    severity: Severity,
    details: Option<String>,
    related: Vec<RelatedLocation>,
    suggestion: Option<String>,
    code_snippet: Option<CodeSnippet>,
}

impl DiagnosticBuilder {
    /// Create a new diagnostic builder
    pub fn new(file: String, line: u32, col: u32, error_kind: ViolationKind, message: String) -> Self {
        DiagnosticBuilder {
            location: Location::new(file, line, col),
            error_kind,
            message,
            severity: Severity::Error,
            details: None,
            related: Vec::new(),
            suggestion: None,
            code_snippet: None,
        }
    }
    
    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
    
    pub fn details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }
    
    pub fn related(mut self, file: String, line: u32, col: u32, message: String) -> Self {
        self.related.push(RelatedLocation {
            location: Location::new(file, line, col),
            message,
        });
        self
    }
    
    pub fn suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }
    
    pub fn snippet(mut self, snippet: CodeSnippet) -> Self {
        self.code_snippet = Some(snippet);
        self
    }
    
    pub fn build(self) -> LinearTypeDiagnostic {
        let mut diag = LinearTypeDiagnostic::new(self.location, self.error_kind, self.message);
        diag.severity = self.severity;
        diag.details = self.details;
        diag.related = self.related;
        diag.suggestion = self.suggestion;
        diag.code_snippet = self.code_snippet;
        diag
    }
}

/// Diagnostic factory for common linear type errors
pub struct DiagnosticFactory;

impl DiagnosticFactory {
    /// Create diagnostic for use-after-move error
    pub fn use_after_move(
        file: String,
        use_line: u32,
        use_col: u32,
        var_name: &str,
        move_line: u32,
        move_col: u32,
    ) -> LinearTypeDiagnostic {
        let location = Location::new(file.clone(), use_line, use_col);
        let message = format!("cannot use binding '{}' after it was moved", var_name);
        
        let diag = LinearTypeDiagnostic::new(
            location,
            ViolationKind::UseAfterMove,
            message,
        );
        
        let diag = diag.with_related(
            Location::new(file, move_line, move_col),
            format!("'{}' was moved here", var_name),
        );
        
        let diag = diag.with_details(format!(
            "The binding '{}' was moved at line {}:{}, meaning ownership was transferred. \
             After a move, the original binding cannot be used again.",
            var_name, move_line, move_col
        ));
        
        diag.with_suggestion(format!(
            "If you need to use '{}' multiple times, either:\n  \
             1. Clone/copy the value if it's copyable\n  \
             2. Use a reference (&{}) if borrowing is sufficient\n  \
             3. Restructure your code to avoid the move",
            var_name, var_name
        ))
    }
    
    /// Create diagnostic for linear resource not consumed
    pub fn linear_not_consumed(
        file: String,
        line: u32,
        col: u32,
        var_name: &str,
        type_name: &str,
    ) -> LinearTypeDiagnostic {
        let location = Location::new(file, line, col);
        let message = format!(
            "linear resource '{}' of type '{}' was not consumed",
            var_name, type_name
        );
        
        let diag = LinearTypeDiagnostic::new(
            location,
            ViolationKind::UseNotMoved,
            message,
        );
        
        let diag = diag.with_details(format!(
            "Linear types like '{}' must be explicitly consumed (moved, returned, or closed) \
             before the function ends. This resource was not consumed, which may leak resources.",
            type_name
        ));
        
        diag.with_suggestion(format!(
            "You must consume the linear resource '{}':\n  \
             1. Pass it to another function that consumes it\n  \
             2. Return it to the caller\n  \
             3. Explicitly close/finalize it if applicable",
            var_name
        ))
    }
    
    /// Create diagnostic for double move
    pub fn double_move(
        file: String,
        second_move_line: u32,
        second_move_col: u32,
        var_name: &str,
        first_move_line: u32,
        first_move_col: u32,
    ) -> LinearTypeDiagnostic {
        let location = Location::new(file.clone(), second_move_line, second_move_col);
        let message = format!("cannot move binding '{}' a second time", var_name);
        
        let diag = LinearTypeDiagnostic::new(
            location,
            ViolationKind::DoubleMove,
            message,
        );
        
        let diag = diag.with_related(
            Location::new(file, first_move_line, first_move_col),
            format!("'{}' was first moved here", var_name),
        );
        
        let diag = diag.with_details(format!(
            "The binding '{}' can only be moved once. After the first move at line {}:{}, \
             the binding is no longer usable.",
            var_name, first_move_line, first_move_col
        ));
        
        diag.with_suggestion(format!(
            "Remove this second move, or use a reference if you need to access '{}' again",
            var_name
        ))
    }
    
    /// Create diagnostic for move after borrow
    pub fn move_after_borrow(
        file: String,
        move_line: u32,
        move_col: u32,
        var_name: &str,
        borrow_line: u32,
        borrow_col: u32,
    ) -> LinearTypeDiagnostic {
        let location = Location::new(file.clone(), move_line, move_col);
        let message = format!("cannot move binding '{}' while it's borrowed", var_name);
        
        let diag = LinearTypeDiagnostic::new(
            location,
            ViolationKind::MoveAfterBorrow,
            message,
        );
        
        let diag = diag.with_related(
            Location::new(file, borrow_line, borrow_col),
            format!("'{}' was borrowed here", var_name),
        );
        
        let diag = diag.with_details(format!(
            "The binding '{}' has an active borrow at line {}:{}. \
             You cannot move a value while there are outstanding references to it.",
            var_name, borrow_line, borrow_col
        ));
        
        diag.with_suggestion(
            "Ensure all borrowing of this value is complete before attempting to move it".to_string()
        )
    }
    
    /// Create diagnostic for borrow after move
    pub fn borrow_after_move(
        file: String,
        borrow_line: u32,
        borrow_col: u32,
        var_name: &str,
        move_line: u32,
        move_col: u32,
    ) -> LinearTypeDiagnostic {
        let location = Location::new(file.clone(), borrow_line, borrow_col);
        let message = format!("cannot borrow binding '{}' after it was moved", var_name);
        
        let diag = LinearTypeDiagnostic::new(
            location,
            ViolationKind::BorrowAfterMove,
            message,
        );
        
        let diag = diag.with_related(
            Location::new(file, move_line, move_col),
            format!("'{}' was moved here", var_name),
        );
        
        let diag = diag.with_details(format!(
            "Once a binding is moved at line {}:{}, the original binding is invalidated \
             and cannot be borrowed or used in any way.",
            move_line, move_col
        ));
        
        diag.with_suggestion(format!(
            "Either avoid moving '{}', or restructure your code to borrow it before the move",
            var_name
        ))
    }
}

/// Diagnostic collector/reporter
pub struct DiagnosticReporter {
    diagnostics: Vec<LinearTypeDiagnostic>,
}

impl DiagnosticReporter {
    pub fn new() -> Self {
        DiagnosticReporter {
            diagnostics: Vec::new(),
        }
    }
    
    /// Add a diagnostic
    pub fn add(&mut self, diagnostic: LinearTypeDiagnostic) {
        self.diagnostics.push(diagnostic);
    }
    
    /// Get all diagnostics
    pub fn diagnostics(&self) -> &[LinearTypeDiagnostic] {
        &self.diagnostics
    }
    
    /// Get only error diagnostics
    pub fn errors(&self) -> Vec<&LinearTypeDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect()
    }
    
    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
    
    /// Report all diagnostics as formatted strings
    pub fn report(&self) -> String {
        let mut output = String::new();
        for diag in &self.diagnostics {
            output.push_str(&diag.display());
            output.push('\n');
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location() {
        let loc = Location::new("test.aura".to_string(), 10, 5);
        assert_eq!(loc.display(), "test.aura:10:5");
    }

    #[test]
    fn test_diagnostic_creation() {
        let diag = LinearTypeDiagnostic::new(
            Location::new("test.aura".to_string(), 5, 0),
            ViolationKind::UseAfterMove,
            "use after move".to_string(),
        );
        assert_eq!(diag.severity, Severity::Error);
        assert!(diag.display().contains("ERROR"));
    }

    #[test]
    fn test_diagnostic_builder() {
        let diag = DiagnosticBuilder::new(
            "test.aura".to_string(),
            5,
            0,
            ViolationKind::UseAfterMove,
            "test error".to_string(),
        )
        .with_suggestion("fix this".to_string())
        .build();
        
        assert_eq!(diag.severity, Severity::Error);
        assert!(diag.suggestion.is_some());
    }

    #[test]
    fn test_use_after_move_diagnostic() {
        let diag = DiagnosticFactory::use_after_move(
            "test.aura".to_string(),
            10,
            5,
            "x",
            5,
            0,
        );
        assert_eq!(diag.error_kind, ViolationKind::UseAfterMove);
        assert!(!diag.related.is_empty());
    }

    #[test]
    fn test_code_snippet() {
        let lines = vec![
            "let x = model;".to_string(),
            "consume(x);".to_string(),
            "use(x);".to_string(),
        ];
        let snippet = CodeSnippet::new(lines, 1).with_highlight(2, 4, 5);
        let displayed = snippet.display();
        assert!(displayed.contains("3 | use(x);"));
    }

    #[test]
    fn test_diagnostic_reporter() {
        let mut reporter = DiagnosticReporter::new();
        reporter.add(LinearTypeDiagnostic::new(
            Location::new("test.aura".to_string(), 1, 0),
            ViolationKind::UseAfterMove,
            "error".to_string(),
        ));
        
        assert!(reporter.has_errors());
        assert_eq!(reporter.errors().len(), 1);
    }
}
