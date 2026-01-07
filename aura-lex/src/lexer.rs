#![forbid(unsafe_code)]
#![allow(unused_assignments)]

use aura_ast::{span_between, Span};
use logos::Logos;
use miette::Diagnostic;
use thiserror::Error;

use crate::token::{Token, TokenKind};

#[derive(Debug, Error, Diagnostic)]
#[error("lex error: {message}")]
#[diagnostic(code(aura::lex))]
#[allow(unused_assignments)]
pub struct LexError {
    pub message: String,
    #[label]
    pub span: Span,
}

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \f\r]+")]
enum RawToken {
    #[token("import")]
    KwImport,
    #[token("val")]
    KwVal,
    #[token("extern")]
    KwExtern,
    #[token("macro")]
    KwMacro,
    #[token("cell")]
    KwCell,
    #[token("type")]
    KwType,
    #[token("trait")]
    KwTrait,
    #[token("where")]
    KwWhere,
    #[token("enum")]
    KwEnum,
    #[token("record")]
    KwRecord,
    #[token("yield")]
    KwYield,
    #[token("mut")]
    KwMut,
    #[token("if")]
    KwIf,
    #[token("else")]
    KwElse,
    #[token("match")]
    KwMatch,
    #[token("while")]
    KwWhile,
    #[token("invariant")]
    KwInvariant,
    #[token("decreases")]
    KwDecreases,
    #[token("requires")]
    KwRequires,
    #[token("ensures")]
    KwEnsures,
    #[token("assert")]
    KwAssert,
    #[token("assume")]
    KwAssume,
    #[token("forall")]
    KwForall,
    #[token("exists")]
    KwExists,
    #[token("layout")]
    KwLayout,
    #[token("render")]
    KwRender,

    #[token("unsafe")]
    KwUnsafe,
    #[token("trusted")]
    KwTrusted,

    #[token("->")]
    Arrow,
    #[token("~>")]
    TildeArrow,

    #[token("::")]
    ColonColon,

    #[token("==")]
    EqEq,
    #[token("!=")]
    Neq,
    #[token("<=")]
    Le,
    #[token(">=")]
    Ge,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,

    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("!")]
    Bang,

    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,

    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,

    #[token(":")]
    Colon,
    #[token("=")]
    Eq,
    #[token(",")]
    Comma,

    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,

    #[regex(r"0b[01_]+", |lex| parse_int_prefixed(lex.slice(), 2, 2))]
    #[regex(r"0o[0-7_]+", |lex| parse_int_prefixed(lex.slice(), 8, 2))]
    #[regex(r"0x[0-9a-fA-F_]+", |lex| parse_int_prefixed(lex.slice(), 16, 2))]
    #[regex(r"[0-9][0-9_]*", |lex| parse_int_decimal(lex.slice()))]
    Int(Option<u64>),

    // String literals: "..." with a limited, strict set of escapes.
    // Supported: \n, \t, \r, \", \\, and \u{HEX} (1-6 hex digits)
    #[regex(r#"\"([^\"\\]|\\.)*\""#, parse_string)]
    String(Option<String>),

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
}

fn parse_int_decimal(s: &str) -> Option<u64> {
    let digits = strip_underscores(s)?;
    digits.parse::<u64>().ok()
}

fn parse_int_prefixed(s: &str, radix: u32, prefix_len: usize) -> Option<u64> {
    let rest = s.get(prefix_len..)?;
    let digits = strip_underscores(rest)?;
    u64::from_str_radix(&digits, radix).ok()
}

fn strip_underscores(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    if s.starts_with('_') || s.ends_with('_') || s.contains("__") {
        return None;
    }
    Some(s.replace('_', ""))
}

fn parse_string(lex: &mut logos::Lexer<RawToken>) -> Option<String> {
    let s = lex.slice();
    let inner = &s[1..s.len().saturating_sub(1)];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }

        let Some(esc) = chars.next() else {
            return None;
        };

        match esc {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'u' => {
                // Expect: \u{HEX}
                if chars.next() != Some('{') {
                    return None;
                }
                let mut hex = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch == '}' {
                        break;
                    }
                    hex.push(ch);
                    chars.next();
                    if hex.len() > 6 {
                        return None;
                    }
                }
                if chars.next() != Some('}') {
                    return None;
                }
                if hex.is_empty() {
                    return None;
                }
                let cp = u32::from_str_radix(&hex, 16).ok()?;
                let ch = char::from_u32(cp)?;
                out.push(ch);
            }
            _ => return None,
        }
    }

    Some(out)
}

pub struct Lexer<'a> {
    src: &'a str,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self { src }
    }

    pub fn lex(&self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        let mut indent_stack: Vec<usize> = vec![0];

        // Track absolute byte offsets.
        let mut line_start = 0usize;

        for line in self.src.split_inclusive('\n') {
            let line_len = line.len();
            let line_end = line_start + line_len;

            // Strip trailing newline for indentation + raw lexing.
            let mut content = line;
            let had_newline = content.ends_with('\n');
            if had_newline {
                content = &content[..content.len() - 1];
            }

            // Skip completely empty/whitespace-only lines (but still advance line_start).
            if content.trim().is_empty() {
                line_start = line_end;
                continue;
            }

            // Reject tabs anywhere (simpler/safer indentation rules).
            if content.as_bytes().iter().any(|b| *b == b'\t') {
                return Err(LexError {
                    message: "tabs are not allowed; use spaces".to_string(),
                    span: span_between(line_start, line_end),
                });
            }

            let leading_spaces = content
                .as_bytes()
                .iter()
                .take_while(|b| **b == b' ')
                .count();

            let current_indent = *indent_stack.last().unwrap_or(&0);
            if leading_spaces > current_indent {
                indent_stack.push(leading_spaces);
                tokens.push(Token {
                    kind: TokenKind::Indent,
                    span: span_between(line_start, line_start + leading_spaces),
                });
            } else if leading_spaces < current_indent {
                while let Some(&top) = indent_stack.last() {
                    if leading_spaces == top {
                        break;
                    }
                    indent_stack.pop();
                    tokens.push(Token {
                        kind: TokenKind::Dedent,
                        span: span_between(line_start, line_start + leading_spaces),
                    });
                }
                if *indent_stack.last().unwrap_or(&usize::MAX) != leading_spaces {
                    return Err(LexError {
                        message: "inconsistent indentation".to_string(),
                        span: span_between(line_start, line_end),
                    });
                }
            }

            // Naive comment stripping (outside of string literals): strip at the earliest
            // occurrence of '#' or '//' (prototype behavior).
            let mut code = &content[leading_spaces..];
            let hash = code.find('#');
            let slash_slash = code.find("//");
            let cut = match (hash, slash_slash) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
            if let Some(idx) = cut {
                code = &code[..idx];
            }
            if code.trim().is_empty() {
                // Line was only comment.
                line_start = line_end;
                continue;
            }

            let mut lex = RawToken::lexer(code);
            while let Some(raw) = lex.next() {
                let span_in_line = lex.span();
                let abs_start = line_start + leading_spaces + span_in_line.start;
                let abs_end = line_start + leading_spaces + span_in_line.end;

                let kind = match raw {
                    Ok(RawToken::KwImport) => TokenKind::KwImport,
                    Ok(RawToken::KwVal) => TokenKind::KwVal,
                    Ok(RawToken::KwExtern) => TokenKind::KwExtern,
                    Ok(RawToken::KwMacro) => TokenKind::KwMacro,
                    Ok(RawToken::KwCell) => TokenKind::KwCell,
                    Ok(RawToken::KwType) => TokenKind::KwType,
                    Ok(RawToken::KwTrait) => TokenKind::KwTrait,
                    Ok(RawToken::KwWhere) => TokenKind::KwWhere,
                    Ok(RawToken::KwEnum) => TokenKind::KwEnum,
                    Ok(RawToken::KwRecord) => TokenKind::KwRecord,
                    Ok(RawToken::KwYield) => TokenKind::KwYield,
                    Ok(RawToken::KwMut) => TokenKind::KwMut,
                    Ok(RawToken::KwIf) => TokenKind::KwIf,
                    Ok(RawToken::KwElse) => TokenKind::KwElse,
                    Ok(RawToken::KwMatch) => TokenKind::KwMatch,
                    Ok(RawToken::KwWhile) => TokenKind::KwWhile,
                    Ok(RawToken::KwInvariant) => TokenKind::KwInvariant,
                    Ok(RawToken::KwDecreases) => TokenKind::KwDecreases,
                    Ok(RawToken::KwRequires) => TokenKind::KwRequires,
                    Ok(RawToken::KwEnsures) => TokenKind::KwEnsures,
                    Ok(RawToken::KwAssert) => TokenKind::KwAssert,
                    Ok(RawToken::KwAssume) => TokenKind::KwAssume,
                    Ok(RawToken::KwForall) => TokenKind::KwForall,
                    Ok(RawToken::KwExists) => TokenKind::KwExists,
                    Ok(RawToken::KwLayout) => TokenKind::KwLayout,
                    Ok(RawToken::KwRender) => TokenKind::KwRender,
                    Ok(RawToken::KwUnsafe) => TokenKind::KwUnsafe,
                    Ok(RawToken::KwTrusted) => TokenKind::KwTrusted,

                    Ok(RawToken::Arrow) => TokenKind::Arrow,
                    Ok(RawToken::TildeArrow) => TokenKind::TildeArrow,

                    Ok(RawToken::ColonColon) => TokenKind::ColonColon,

                    Ok(RawToken::EqEq) => TokenKind::EqEq,
                    Ok(RawToken::Neq) => TokenKind::Neq,
                    Ok(RawToken::Le) => TokenKind::Le,
                    Ok(RawToken::Ge) => TokenKind::Ge,
                    Ok(RawToken::Lt) => TokenKind::Lt,
                    Ok(RawToken::Gt) => TokenKind::Gt,

                    Ok(RawToken::AndAnd) => TokenKind::AndAnd,
                    Ok(RawToken::OrOr) => TokenKind::OrOr,
                    Ok(RawToken::Bang) => TokenKind::Bang,

                    Ok(RawToken::Plus) => TokenKind::Plus,
                    Ok(RawToken::Minus) => TokenKind::Minus,
                    Ok(RawToken::Star) => TokenKind::Star,
                    Ok(RawToken::Slash) => TokenKind::Slash,

                    Ok(RawToken::DotDot) => TokenKind::DotDot,
                    Ok(RawToken::Dot) => TokenKind::Dot,

                    Ok(RawToken::Colon) => TokenKind::Colon,
                    Ok(RawToken::Eq) => TokenKind::Eq,
                    Ok(RawToken::Comma) => TokenKind::Comma,

                    Ok(RawToken::LParen) => TokenKind::LParen,
                    Ok(RawToken::RParen) => TokenKind::RParen,
                    Ok(RawToken::LBrace) => TokenKind::LBrace,
                    Ok(RawToken::RBrace) => TokenKind::RBrace,
                    Ok(RawToken::LBracket) => TokenKind::LBracket,
                    Ok(RawToken::RBracket) => TokenKind::RBracket,

                    Ok(RawToken::Ident(s)) => TokenKind::Ident(s),
                    Ok(RawToken::Int(Some(n))) => TokenKind::Int(n),
                    Ok(RawToken::Int(None)) => {
                        return Err(LexError {
                            message: "invalid integer literal".to_string(),
                            span: span_between(abs_start, abs_end),
                        });
                    }
                    Ok(RawToken::String(Some(s))) => TokenKind::String(s),
                    Ok(RawToken::String(None)) => {
                        return Err(LexError {
                            message: "invalid string literal".to_string(),
                            span: span_between(abs_start, abs_end),
                        });
                    }

                    Err(_) => {
                        return Err(LexError {
                            message: "unexpected token".to_string(),
                            span: span_between(abs_start, abs_end),
                        });
                    }
                };

                tokens.push(Token {
                    kind,
                    span: span_between(abs_start, abs_end),
                });
            }

            // End of logical line.
            tokens.push(Token {
                kind: TokenKind::Newline,
                span: span_between(line_end, line_end),
            });

            line_start = line_end;
        }

        // Close open indents.
        while indent_stack.len() > 1 {
            indent_stack.pop();
            tokens.push(Token {
                kind: TokenKind::Dedent,
                span: span_between(self.src.len(), self.src.len()),
            });
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            span: span_between(self.src.len(), self.src.len()),
        });

        Ok(tokens)
    }
}
