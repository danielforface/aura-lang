#![forbid(unsafe_code)]

use aura_ast::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // Keywords
    KwImport,
    KwVal,
    KwCell,
    KwExtern,
    KwMacro,
    KwType,
    KwTrait,
    KwWhere,
    KwEnum,
    KwRecord,
    KwYield,
    KwMut,
    KwIf,
    KwElse,
    KwMatch,
    KwWhile,
    KwInvariant,
    KwDecreases,
    KwRequires,
    KwEnsures,
    KwAssert,
    KwAssume,
    KwForall,
    KwExists,
    KwLayout,
    KwRender,
    KwUnsafe,
    KwTrusted,

    // Operators / punctuation
    Arrow,
    TildeArrow,
    ColonColon,
    Colon,
    Eq,
    EqEq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,

    Plus,
    Minus,
    Star,
    Slash,

    AndAnd,
    OrOr,
    Bang,
    Dot,
    DotDot,
    Comma,

    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    Newline,
    Indent,
    Dedent,
    Eof,

    // Literals / identifiers
    Ident(String),
    Int(u64),
    String(String),
}
