#![forbid(unsafe_code)]

mod error;
mod fmt;
mod parser;
pub mod pattern_compiler;

use aura_lex::Lexer;
use miette::IntoDiagnostic;
use std::collections::BTreeSet;

pub use error::ParseError;
pub use fmt::{format_expr, format_program};
pub use parser::Parser;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseConfig {
    /// Language edition (e.g. "2026").
    pub edition: Option<String>,
    /// Enabled unstable features (stringly-typed for forward compatibility).
    pub features: BTreeSet<String>,
}

impl ParseConfig {
    pub fn has_feature(&self, name: &str) -> bool {
        self.features.iter().any(|f| f.eq_ignore_ascii_case(name))
    }
}

pub fn parse_source(src: &str) -> miette::Result<aura_ast::Program> {
    let tokens = Lexer::new(src).lex().into_diagnostic()?;
    let mut parser = Parser::new(&tokens);
    parser.parse_program().into_diagnostic()
}

pub fn parse_source_with_config(src: &str, config: &ParseConfig) -> miette::Result<aura_ast::Program> {
    let tokens = Lexer::new(src).lex().into_diagnostic()?;
    let mut parser = Parser::new_with_config(&tokens, config);
    parser.parse_program().into_diagnostic()
}

/// Parse a source file while attempting to recover from errors.
///
/// Returns a best-effort AST and a list of encountered `ParseError`s.
pub fn parse_source_with_recovery(src: &str) -> miette::Result<(aura_ast::Program, Vec<ParseError>)> {
    let tokens = Lexer::new(src).lex().into_diagnostic()?;
    let mut parser = Parser::new(&tokens);
    Ok(parser.parse_program_with_recovery())
}

pub fn parse_source_with_recovery_config(
    src: &str,
    config: &ParseConfig,
) -> miette::Result<(aura_ast::Program, Vec<ParseError>)> {
    let tokens = Lexer::new(src).lex().into_diagnostic()?;
    let mut parser = Parser::new_with_config(&tokens, config);
    Ok(parser.parse_program_with_recovery())
}

pub fn parse_expr(src: &str) -> miette::Result<aura_ast::Expr> {
    let tokens = Lexer::new(src).lex().into_diagnostic()?;
    let mut parser = Parser::new(&tokens);
    parser.parse_expr_eof().into_diagnostic()
}

pub fn parse_expr_with_config(src: &str, config: &ParseConfig) -> miette::Result<aura_ast::Expr> {
    let tokens = Lexer::new(src).lex().into_diagnostic()?;
    let mut parser = Parser::new_with_config(&tokens, config);
    parser.parse_expr_eof().into_diagnostic()
}
