#![forbid(unsafe_code)]
#![allow(unused_assignments)]

use aura_ast::Span;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("semantic error: {message}")]
#[diagnostic(code(aura::sema))]
#[allow(unused_assignments)]
pub struct SemanticError {
    pub message: String,
    #[label]
    pub span: Span,
}
