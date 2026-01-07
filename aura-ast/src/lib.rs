#![forbid(unsafe_code)]

use miette::SourceSpan;

pub type Span = SourceSpan;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spanned<T> {
    pub span: Span,
    pub node: T,
}

impl<T> Spanned<T> {
    pub fn new(span: Span, node: T) -> Self {
        Self { span, node }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            span: self.span,
            node: f(self.node),
        }
    }
}

pub fn span(start: usize, len: usize) -> Span {
    SourceSpan::new(start.into(), len)
}

pub fn span_between(start: usize, end: usize) -> Span {
    debug_assert!(end >= start);
    span(start, end - start)
}

pub type Ident = Spanned<String>;

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Import(ImportStmt),
    MacroDef(MacroDef),
    TypeAlias(TypeAlias),
    TraitDef(TraitDef),
    RecordDef(RecordDef),
    EnumDef(EnumDef),
    StrandDef(StrandDef),
    CellDef(CellDef),
    ExternCell(ExternCell),
    UnsafeBlock(UnsafeBlock),
    Layout(LayoutBlock),
    Render(RenderBlock),
    Prop(PropStmt),
    Assign(AssignStmt),
    If(IfStmt),
    Match(MatchStmt),
    While(WhileStmt),
    Requires(RequiresStmt),
    Ensures(EnsuresStmt),
    Assert(AssertStmt),
    Assume(AssumeStmt),
    MacroCall(MacroCall),
    FlowBlock(FlowBlock),
    ExprStmt(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MacroDef {
    pub span: Span,
    pub name: Ident,
    pub params: Vec<Ident>,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MacroCall {
    pub span: Span,
    pub name: Ident,
    pub args: Vec<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnsafeBlock {
    pub span: Span,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequiresStmt {
    pub span: Span,
    pub expr: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnsuresStmt {
    pub span: Span,
    pub expr: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssertStmt {
    pub span: Span,
    pub expr: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssumeStmt {
    pub span: Span,
    pub expr: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchStmt {
    pub span: Span,
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub span: Span,
    pub pat: Pattern,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    Wildcard { span: Span },
    IntLit { span: Span, value: u64 },
    StringLit { span: Span, value: String },
    Ctor {
        span: Span,
        ty: Ident,
        variant: Ident,
        binders: Vec<Ident>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropStmt {
    pub span: Span,
    pub name: Ident,
    pub expr: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutBlock {
    pub span: Span,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderBlock {
    pub span: Span,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportStmt {
    pub span: Span,
    pub path: Vec<Ident>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternCell {
    pub span: Span,
    pub trusted: bool,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: TypeRef,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeAlias {
    pub span: Span,
    pub name: Ident,
    pub params: Vec<TypeParam>,
    pub target: TypeRef,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeParam {
    pub span: Span,
    pub name: Ident,
    pub bound: Option<Ident>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TraitDef {
    pub span: Span,
    pub name: Ident,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordDef {
    pub span: Span,
    pub name: Ident,
    pub params: Vec<TypeParam>,
    pub fields: Vec<RecordFieldDef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordFieldDef {
    pub span: Span,
    pub name: Ident,
    pub ty: TypeRef,
    pub default: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumDef {
    pub span: Span,
    pub name: Ident,
    pub params: Vec<TypeParam>,
    pub variants: Vec<EnumVariantDef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumVariantDef {
    pub span: Span,
    pub name: Ident,
    pub fields: Vec<EnumFieldDef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumFieldDef {
    pub span: Span,
    pub name: Ident,
    pub ty: TypeRef,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrandDef {
    pub span: Span,
    pub name: Ident,
    pub mutable: bool,
    pub ty: Option<TypeRef>,
    pub where_clause: Option<Expr>,
    pub expr: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellDef {
    pub span: Span,
    pub name: Ident,
    pub params: Vec<Param>,
    pub flow: Option<FlowOp>,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlowBlock {
    pub span: Span,
    pub name: Ident,
    pub flow: FlowOp,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub span: Span,
    pub stmts: Vec<Stmt>,
    pub yield_expr: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub span: Span,
    pub name: Ident,
    pub mutable: bool,
    pub ty: TypeRef,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssignStmt {
    pub span: Span,
    pub target: Ident,
    pub expr: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfStmt {
    pub span: Span,
    pub cond: Expr,
    pub then_block: Block,
    pub else_block: Option<Block>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WhileStmt {
    pub span: Span,
    pub cond: Expr,
    pub invariant: Option<Expr>,
    pub decreases: Option<Expr>,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuantBinder {
    pub span: Span,
    pub name: Ident,
    pub ty: Option<TypeRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeRef {
    pub span: Span,
    pub name: Ident,
    pub args: Vec<TypeArg>,
    pub range: Option<RangeConstraint>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeArg {
    Type(Box<TypeRef>),
    Shape(Vec<u64>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RangeConstraint {
    pub span: Span,
    pub lo: Expr,
    pub hi: Expr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowOp {
    Sync,
    Async,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Ident(Ident),
    IntLit(u64),
    StringLit(String),
    /// `Style { key: value, ... }`
    StyleLit {
        fields: Vec<(Ident, Expr)>,
    },
    /// `TypeName { field: value, ... }`
    RecordLit {
        name: Ident,
        fields: Vec<(Ident, Expr)>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    Member {
        base: Box<Expr>,
        member: Ident,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
        trailing: Option<Box<Block>>,
    },
    Lambda {
        op: FlowOp,
        body: Box<Block>,
    },
    Flow {
        left: Box<Expr>,
        op: FlowOp,
        right: Box<Expr>,
    },

    ForAll {
        binders: Vec<QuantBinder>,
        body: Box<Expr>,
    },
    Exists {
        binders: Vec<QuantBinder>,
        body: Box<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum CallArg {
    Positional(Expr),
    Named { name: Ident, value: Expr },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,

    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,

    And,
    Or,
}

