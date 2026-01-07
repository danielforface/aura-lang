#![forbid(unsafe_code)]
#![allow(unused_assignments)]

use aura_ast::Span;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("parse error: {message}")]
#[diagnostic(code(aura::parse))]
#[allow(unused_assignments)]
pub struct ParseError {
    pub message: String,
    #[label]
    pub span: Span,
}
