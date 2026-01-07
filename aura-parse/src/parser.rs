#![forbid(unsafe_code)]

use std::mem;

use aura_ast::{
    span_between, AssignStmt, BinOp, Block, CallArg, CellDef, ExternCell, Expr, ExprKind, FlowBlock,
    FlowOp, Ident, IfStmt, ImportStmt, LayoutBlock, MatchArm, MatchStmt, Param, Pattern, Program,
    PropStmt, RangeConstraint, RenderBlock, Span, Stmt, StrandDef, TraitDef, TypeAlias, TypeArg,
    TypeRef, UnaryOp, WhileStmt, EnumDef, EnumFieldDef, EnumVariantDef, RecordDef, RecordFieldDef,
    TypeParam, MacroDef, MacroCall, Spanned,
};
use aura_lex::{Token, TokenKind};

use crate::error::ParseError;
use crate::ParseConfig;

use std::collections::HashMap;

pub struct Parser<'a> {
    tokens: &'a [Token],
    idx: usize,
    config: ParseConfig,
}

#[derive(Clone, Debug)]
struct MacroTemplate {
    params: Vec<String>,
    body: Block,
}

fn expand_macros(program: Program) -> Result<Program, ParseError> {
    let mut macros: HashMap<String, MacroTemplate> = HashMap::new();
    let mut stmts_no_defs: Vec<Stmt> = Vec::with_capacity(program.stmts.len());

    // Collect macro defs first (top-level only).
    for stmt in program.stmts {
        match stmt {
            Stmt::MacroDef(def) => {
                let name = def.name.node.clone();
                if macros.contains_key(&name) {
                    return Err(ParseError {
                        message: format!("duplicate macro definition '{name}'"),
                        span: def.span,
                    });
                }
                let params = def.params.into_iter().map(|p| p.node).collect();
                macros.insert(name, MacroTemplate { params, body: def.body });
            }
            other => stmts_no_defs.push(other),
        }
    }

    let mut gensym_counter: u64 = 0;
    let mut out: Vec<Stmt> = Vec::with_capacity(stmts_no_defs.len());
    for stmt in stmts_no_defs {
        out.extend(expand_stmt(stmt, &macros, &mut gensym_counter)?);
    }

    Ok(Program { stmts: out })
}

fn expand_block(block: Block, macros: &HashMap<String, MacroTemplate>, gensym_counter: &mut u64) -> Result<Block, ParseError> {
    let mut out: Vec<Stmt> = Vec::with_capacity(block.stmts.len());
    for s in block.stmts {
        out.extend(expand_stmt(s, macros, gensym_counter)?);
    }
    Ok(Block {
        span: block.span,
        stmts: out,
        yield_expr: block.yield_expr,
    })
}

fn expand_stmt(
    stmt: Stmt,
    macros: &HashMap<String, MacroTemplate>,
    gensym_counter: &mut u64,
) -> Result<Vec<Stmt>, ParseError> {
    match stmt {
        Stmt::MacroDef(def) => Err(ParseError {
            message: "macro definitions are only supported at top-level (MVP)".to_string(),
            span: def.span,
        }),
        Stmt::MacroCall(call) => {
            let name = call.name.node.clone();
            let Some(tpl) = macros.get(&name).cloned() else {
                return Err(ParseError {
                    message: format!("unknown macro '{name}'"),
                    span: call.span,
                });
            };

            if call.args.len() != tpl.params.len() {
                return Err(ParseError {
                    message: format!(
                        "wrong number of args for macro '{name}': expected {}, got {}",
                        tpl.params.len(),
                        call.args.len()
                    ),
                    span: call.span,
                });
            }

            let mut subst: HashMap<String, Expr> = HashMap::new();
            for (p, a) in tpl.params.iter().zip(call.args.iter()) {
                subst.insert(p.clone(), a.clone());
            }

            // Hygiene: macro-local binder names get a gensym suffix.
            let mut rename: HashMap<String, String> = HashMap::new();
            collect_binder_idents_in_block(&tpl.body, &mut rename, gensym_counter);

            let expanded = rewrite_block(&tpl.body, &subst, &rename);
            let expanded = expand_block(expanded, macros, gensym_counter)?;

            let mut out: Vec<Stmt> = expanded.stmts;
            if let Some(y) = expanded.yield_expr {
                out.push(Stmt::ExprStmt(y));
            }
            Ok(out)
        }

        Stmt::CellDef(mut c) => {
            c.body = expand_block(c.body, macros, gensym_counter)?;
            Ok(vec![Stmt::CellDef(c)])
        }
        Stmt::FlowBlock(mut fb) => {
            fb.body = expand_block(fb.body, macros, gensym_counter)?;
            Ok(vec![Stmt::FlowBlock(fb)])
        }
        Stmt::UnsafeBlock(mut u) => {
            u.body = expand_block(u.body, macros, gensym_counter)?;
            Ok(vec![Stmt::UnsafeBlock(u)])
        }
        Stmt::Layout(mut l) => {
            l.body = expand_block(l.body, macros, gensym_counter)?;
            Ok(vec![Stmt::Layout(l)])
        }
        Stmt::Render(mut r) => {
            r.body = expand_block(r.body, macros, gensym_counter)?;
            Ok(vec![Stmt::Render(r)])
        }
        Stmt::If(mut i) => {
            i.then_block = expand_block(i.then_block, macros, gensym_counter)?;
            if let Some(e) = i.else_block.take() {
                i.else_block = Some(expand_block(e, macros, gensym_counter)?);
            }
            Ok(vec![Stmt::If(i)])
        }
        Stmt::Match(mut m) => {
            for arm in &mut m.arms {
                arm.body = expand_block(arm.body.clone(), macros, gensym_counter)?;
            }
            Ok(vec![Stmt::Match(m)])
        }
        Stmt::While(mut w) => {
            w.body = expand_block(w.body, macros, gensym_counter)?;
            Ok(vec![Stmt::While(w)])
        }

        other => Ok(vec![other]),
    }
}

fn collect_binder_idents_in_block(block: &Block, rename: &mut HashMap<String, String>, gensym_counter: &mut u64) {
    for s in &block.stmts {
        match s {
            Stmt::StrandDef(sd) => {
                let n = sd.name.node.clone();
                rename.entry(n.clone()).or_insert_with(|| {
                    *gensym_counter += 1;
                    format!("{n}__m{}", *gensym_counter)
                });
            }
            Stmt::CellDef(cd) => {
                // Avoid capturing cell-local bindings if someone uses macro inside macro.
                let n = cd.name.node.clone();
                rename.entry(n.clone()).or_insert_with(|| {
                    *gensym_counter += 1;
                    format!("{n}__m{}", *gensym_counter)
                });
            }
            Stmt::FlowBlock(fb) => {
                let n = fb.name.node.clone();
                rename.entry(n.clone()).or_insert_with(|| {
                    *gensym_counter += 1;
                    format!("{n}__m{}", *gensym_counter)
                });
            }
            Stmt::Assign(a) => {
                let n = a.target.node.clone();
                rename.entry(n.clone()).or_insert_with(|| {
                    *gensym_counter += 1;
                    format!("{n}__m{}", *gensym_counter)
                });
            }
            Stmt::If(i) => {
                collect_binder_idents_in_block(&i.then_block, rename, gensym_counter);
                if let Some(e) = &i.else_block {
                    collect_binder_idents_in_block(e, rename, gensym_counter);
                }
            }
            Stmt::Match(m) => {
                for arm in &m.arms {
                    collect_binder_idents_in_block(&arm.body, rename, gensym_counter);
                }
            }
            Stmt::While(w) => {
                collect_binder_idents_in_block(&w.body, rename, gensym_counter);
            }
            Stmt::UnsafeBlock(u) => {
                collect_binder_idents_in_block(&u.body, rename, gensym_counter);
            }
            Stmt::Layout(l) => collect_binder_idents_in_block(&l.body, rename, gensym_counter),
            Stmt::Render(r) => collect_binder_idents_in_block(&r.body, rename, gensym_counter),
            Stmt::MacroDef(_) | Stmt::MacroCall(_) => {}
            _ => {}
        }
    }
}

fn rewrite_block(block: &Block, subst: &HashMap<String, Expr>, rename: &HashMap<String, String>) -> Block {
    Block {
        span: block.span,
        stmts: block
            .stmts
            .iter()
            .map(|s| rewrite_stmt(s, subst, rename))
            .collect(),
        yield_expr: block.yield_expr.as_ref().map(|e| rewrite_expr(e, subst, rename)),
    }
}

fn rewrite_stmt(stmt: &Stmt, subst: &HashMap<String, Expr>, rename: &HashMap<String, String>) -> Stmt {
    match stmt {
        Stmt::StrandDef(sd) => {
            let name = rewrite_ident(&sd.name, subst, rename);
            let expr = rewrite_expr(&sd.expr, subst, rename);
            let where_clause = sd.where_clause.as_ref().map(|e| rewrite_expr(e, subst, rename));
            Stmt::StrandDef(aura_ast::StrandDef {
                span: sd.span,
                name,
                mutable: sd.mutable,
                ty: sd.ty.clone(),
                where_clause,
                expr,
            })
        }
        Stmt::Assign(a) => {
            let target = rewrite_ident(&a.target, subst, rename);
            let expr = rewrite_expr(&a.expr, subst, rename);
            Stmt::Assign(AssignStmt { span: a.span, target, expr })
        }
        Stmt::Prop(p) => {
            let name = rewrite_ident(&p.name, subst, rename);
            let expr = rewrite_expr(&p.expr, subst, rename);
            Stmt::Prop(PropStmt { span: p.span, name, expr })
        }
        Stmt::If(i) => Stmt::If(aura_ast::IfStmt {
            span: i.span,
            cond: rewrite_expr(&i.cond, subst, rename),
            then_block: rewrite_block(&i.then_block, subst, rename),
            else_block: i.else_block.as_ref().map(|b| rewrite_block(b, subst, rename)),
        }),
        Stmt::Match(m) => Stmt::Match(aura_ast::MatchStmt {
            span: m.span,
            scrutinee: rewrite_expr(&m.scrutinee, subst, rename),
            arms: m
                .arms
                .iter()
                .map(|a| aura_ast::MatchArm {
                    span: a.span,
                    pat: a.pat.clone(),
                    body: rewrite_block(&a.body, subst, rename),
                })
                .collect(),
        }),
        Stmt::While(w) => Stmt::While(aura_ast::WhileStmt {
            span: w.span,
            cond: rewrite_expr(&w.cond, subst, rename),
            invariant: w.invariant.as_ref().map(|e| rewrite_expr(e, subst, rename)),
            decreases: w.decreases.as_ref().map(|e| rewrite_expr(e, subst, rename)),
            body: rewrite_block(&w.body, subst, rename),
        }),
        Stmt::UnsafeBlock(u) => Stmt::UnsafeBlock(aura_ast::UnsafeBlock {
            span: u.span,
            body: rewrite_block(&u.body, subst, rename),
        }),
        Stmt::Layout(l) => Stmt::Layout(aura_ast::LayoutBlock {
            span: l.span,
            body: rewrite_block(&l.body, subst, rename),
        }),
        Stmt::Render(r) => Stmt::Render(aura_ast::RenderBlock {
            span: r.span,
            body: rewrite_block(&r.body, subst, rename),
        }),
        Stmt::Requires(r) => Stmt::Requires(aura_ast::RequiresStmt {
            span: r.span,
            expr: rewrite_expr(&r.expr, subst, rename),
        }),
        Stmt::Ensures(e) => Stmt::Ensures(aura_ast::EnsuresStmt {
            span: e.span,
            expr: rewrite_expr(&e.expr, subst, rename),
        }),
        Stmt::Assert(a) => Stmt::Assert(aura_ast::AssertStmt {
            span: a.span,
            expr: rewrite_expr(&a.expr, subst, rename),
        }),
        Stmt::Assume(a) => Stmt::Assume(aura_ast::AssumeStmt {
            span: a.span,
            expr: rewrite_expr(&a.expr, subst, rename),
        }),
        Stmt::ExprStmt(e) => Stmt::ExprStmt(rewrite_expr(e, subst, rename)),
        // Do not allow macro-in-macro at MVP (avoid needing multi-pass expansion order).
        Stmt::MacroDef(d) => Stmt::MacroDef(d.clone()),
        Stmt::MacroCall(c) => Stmt::MacroCall(c.clone()),
        // Statements that don't contain expressions or binders we care about in the MVP.
        other => other.clone(),
    }
}

fn rewrite_expr(expr: &Expr, subst: &HashMap<String, Expr>, rename: &HashMap<String, String>) -> Expr {
    // Parameter substitution: macro params are referenced by plain identifiers.
    if let ExprKind::Ident(id) = &expr.kind {
        if let Some(repl) = subst.get(&id.node) {
            return repl.clone();
        }
    }

    match &expr.kind {
        ExprKind::Ident(id) => Expr {
            span: expr.span,
            kind: ExprKind::Ident(rewrite_ident(id, subst, rename)),
        },
        ExprKind::IntLit(n) => Expr { span: expr.span, kind: ExprKind::IntLit(*n) },
        ExprKind::StringLit(s) => Expr { span: expr.span, kind: ExprKind::StringLit(s.clone()) },
        ExprKind::StyleLit { fields } => Expr {
            span: expr.span,
            kind: ExprKind::StyleLit {
                fields: fields
                    .iter()
                    .map(|(k, v)| (rewrite_ident(k, subst, rename), rewrite_expr(v, subst, rename)))
                    .collect(),
            },
        },
        ExprKind::RecordLit { name, fields } => Expr {
            span: expr.span,
            kind: ExprKind::RecordLit {
                name: rewrite_ident(name, subst, rename),
                fields: fields
                    .iter()
                    .map(|(k, v)| (rewrite_ident(k, subst, rename), rewrite_expr(v, subst, rename)))
                    .collect(),
            },
        },
        ExprKind::Unary { op, expr: e } => Expr {
            span: expr.span,
            kind: ExprKind::Unary {
                op: *op,
                expr: Box::new(rewrite_expr(e, subst, rename)),
            },
        },
        ExprKind::Binary { left, op, right } => Expr {
            span: expr.span,
            kind: ExprKind::Binary {
                left: Box::new(rewrite_expr(left, subst, rename)),
                op: *op,
                right: Box::new(rewrite_expr(right, subst, rename)),
            },
        },
        ExprKind::Member { base, member } => Expr {
            span: expr.span,
            kind: ExprKind::Member {
                base: Box::new(rewrite_expr(base, subst, rename)),
                member: rewrite_ident(member, subst, rename),
            },
        },
        ExprKind::Call { callee, args, trailing } => Expr {
            span: expr.span,
            kind: ExprKind::Call {
                callee: Box::new(rewrite_expr(callee, subst, rename)),
                args: args
                    .iter()
                    .map(|a| match a {
                        CallArg::Positional(e) => CallArg::Positional(rewrite_expr(e, subst, rename)),
                        CallArg::Named { name, value } => CallArg::Named {
                            name: rewrite_ident(name, subst, rename),
                            value: rewrite_expr(value, subst, rename),
                        },
                    })
                    .collect(),
                trailing: trailing.as_ref().map(|b| Box::new(rewrite_block(b, subst, rename))),
            },
        },
        ExprKind::Lambda { op, body } => Expr {
            span: expr.span,
            kind: ExprKind::Lambda {
                op: *op,
                body: Box::new(rewrite_block(body, subst, rename)),
            },
        },
        ExprKind::Flow { left, op, right } => Expr {
            span: expr.span,
            kind: ExprKind::Flow {
                left: Box::new(rewrite_expr(left, subst, rename)),
                op: *op,
                right: Box::new(rewrite_expr(right, subst, rename)),
            },
        },
        ExprKind::ForAll { binders, body } => Expr {
            span: expr.span,
            kind: ExprKind::ForAll {
                binders: binders.clone(),
                body: Box::new(rewrite_expr(body, subst, rename)),
            },
        },
        ExprKind::Exists { binders, body } => Expr {
            span: expr.span,
            kind: ExprKind::Exists {
                binders: binders.clone(),
                body: Box::new(rewrite_expr(body, subst, rename)),
            },
        },
    }
}

fn rewrite_ident(id: &Ident, _subst: &HashMap<String, Expr>, rename: &HashMap<String, String>) -> Ident {
    if let Some(new) = rename.get(&id.node) {
        Spanned::new(id.span, new.clone())
    } else {
        id.clone()
    }
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            idx: 0,
            config: ParseConfig::default(),
        }
    }

    pub fn new_with_config(tokens: &'a [Token], config: &ParseConfig) -> Self {
        Self {
            tokens,
            idx: 0,
            config: config.clone(),
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut stmts = Vec::new();
        while !self.at(TokenKind::Eof) {
            self.skip_newlines();
            if self.at(TokenKind::Eof) {
                break;
            }
            stmts.push(self.parse_stmt()?);
        }
        let program = Program { stmts };
        if self.config.has_feature("macros") {
            expand_macros(program)
        } else {
            Ok(program)
        }
    }

    /// Parse a program while attempting to recover from errors.
    ///
    /// Recovery strategy (MVP): on a statement parse error, skip tokens until a
    /// reasonable statement boundary (`Newline`, `Dedent`, or `Eof`), then continue.
    ///
    /// This is intended for IDE diagnostics (best-effort), not for producing a
    /// guaranteed-correct AST.
    pub fn parse_program_with_recovery(&mut self) -> (Program, Vec<ParseError>) {
        let mut stmts = Vec::new();
        let mut errors = Vec::new();

        while !self.at(TokenKind::Eof) {
            self.skip_newlines();
            if self.at(TokenKind::Eof) {
                break;
            }

            match self.parse_stmt() {
                Ok(stmt) => stmts.push(stmt),
                Err(err) => {
                    errors.push(err);
                    self.recover_to_stmt_boundary();
                }
            }
        }

        let program = Program { stmts };
        if self.config.has_feature("macros") {
            match expand_macros(program.clone()) {
                Ok(p) => (p, errors),
                Err(e) => {
                    errors.push(e);
                    (program, errors)
                }
            }
        } else {
            (program, errors)
        }
    }

    fn recover_to_stmt_boundary(&mut self) {
        // Avoid infinite loops if we're already at EOF.
        if self.at(TokenKind::Eof) {
            return;
        }

        // Skip until we hit a boundary token.
        while !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Newline) || self.at(TokenKind::Dedent) {
                break;
            }
            self.next();
        }

        // Consume one trailing newline to make progress.
        if self.at(TokenKind::Newline) {
            self.next();
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::KwImport) => Ok(Stmt::Import(self.parse_import_stmt()?)),
            Some(TokenKind::KwMacro) => {
                if !self.config.has_feature("macros") {
                    let span = self.peek_span().unwrap_or_else(|| span_between(0, 0));
                    return Err(ParseError {
                        message: "macro definitions require unstable feature 'macros'".to_string(),
                        span,
                    });
                }
                Ok(Stmt::MacroDef(self.parse_macro_def()?))
            }
            Some(TokenKind::KwTrait) => Ok(Stmt::TraitDef(self.parse_trait_def()?)),
            Some(TokenKind::KwType) => self.parse_type_stmt(),
            Some(TokenKind::KwVal) => Ok(Stmt::StrandDef(self.parse_strand_def()?)),
            Some(TokenKind::KwExtern) | Some(TokenKind::KwTrusted) => {
                Ok(Stmt::ExternCell(self.parse_extern_cell()?))
            }
            Some(TokenKind::KwCell) => Ok(Stmt::CellDef(self.parse_cell_def()?)),
            Some(TokenKind::KwUnsafe) => Ok(Stmt::UnsafeBlock(self.parse_unsafe_block()?)),
            Some(TokenKind::KwLayout) => Ok(Stmt::Layout(self.parse_layout_block()?)),
            Some(TokenKind::KwRender) => Ok(Stmt::Render(self.parse_render_block()?)),
            Some(TokenKind::KwRequires) => {
                let s = self.parse_requires_stmt()?;
                self.expect_stmt_terminator()?;
                Ok(Stmt::Requires(s))
            }
            Some(TokenKind::KwEnsures) => {
                let s = self.parse_ensures_stmt()?;
                self.expect_stmt_terminator()?;
                Ok(Stmt::Ensures(s))
            }
            Some(TokenKind::KwAssert) => {
                let s = self.parse_assert_stmt()?;
                self.expect_stmt_terminator()?;
                Ok(Stmt::Assert(s))
            }
            Some(TokenKind::KwAssume) => {
                let s = self.parse_assume_stmt()?;
                self.expect_stmt_terminator()?;
                Ok(Stmt::Assume(s))
            }
            Some(TokenKind::KwIf) => Ok(Stmt::If(self.parse_if_stmt()?)),
            Some(TokenKind::KwMatch) => Ok(Stmt::Match(self.parse_match_stmt()?)),
            Some(TokenKind::KwWhile) => Ok(Stmt::While(self.parse_while_stmt()?)),
            Some(TokenKind::Ident(_)) => {
                if self.peek_kind_n(1).is_some_and(|k| matches!(k, TokenKind::Bang)) {
                    if !self.config.has_feature("macros") {
                        let span = self.peek_span().unwrap_or_else(|| span_between(0, 0));
                        return Err(ParseError {
                            message: "macro invocations require unstable feature 'macros'".to_string(),
                            span,
                        });
                    }
                    let s = self.parse_macro_call_stmt()?;
                    self.expect_stmt_terminator()?;
                    return Ok(Stmt::MacroCall(s));
                }

                if self.is_flow_block_start() {
                    Ok(Stmt::FlowBlock(self.parse_flow_block()?))
                } else if self.peek_kind_n(1).is_some_and(|k| matches!(k, TokenKind::Colon)) {
                    Ok(Stmt::Prop(self.parse_prop_stmt()?))
                } else if self.peek_kind_n(1).is_some_and(|k| matches!(k, TokenKind::Eq)) {
                    Ok(Stmt::Assign(self.parse_assign_stmt()?))
                } else {
                    let expr = self.parse_expr()?;
                    self.expect_stmt_terminator()?;
                    Ok(Stmt::ExprStmt(expr))
                }
            }
            _ => {
                let span = self.peek_span().unwrap_or_else(|| span_between(0, 0));
                Err(ParseError {
                    message: "expected a statement".to_string(),
                    span,
                })
            }
        }
    }

    fn parse_macro_def(&mut self) -> Result<MacroDef, ParseError> {
        let start = self.expect(TokenKind::KwMacro)?;
        let name = self.parse_qualified_ident()?;

        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !self.at(TokenKind::RParen) {
            loop {
                params.push(self.parse_qualified_ident()?);
                if self.at(TokenKind::Comma) {
                    self.next();
                    continue;
                }
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::Colon)?;
        let body = self.parse_logic_block()?;
        let span = join(start.span, body.span);
        Ok(MacroDef { span, name, params, body })
    }

    fn parse_macro_call_stmt(&mut self) -> Result<MacroCall, ParseError> {
        let name = self.parse_qualified_ident()?;
        let start_span = name.span;
        self.expect(TokenKind::Bang)?;
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        if !self.at(TokenKind::RParen) {
            loop {
                args.push(self.parse_expr()?);
                if self.at(TokenKind::Comma) {
                    self.next();
                    continue;
                }
                break;
            }
        }
        let end = self.expect(TokenKind::RParen)?;
        let span = join(start_span, end.span);
        Ok(MacroCall { span, name, args })
    }

    fn parse_layout_block(&mut self) -> Result<LayoutBlock, ParseError> {
        let start = self.expect(TokenKind::KwLayout)?;
        self.expect(TokenKind::Colon)?;
        let body = self.parse_logic_block()?;
        let span = join(start.span, body.span);
        Ok(LayoutBlock { span, body })
    }

    fn parse_render_block(&mut self) -> Result<RenderBlock, ParseError> {
        let start = self.expect(TokenKind::KwRender)?;
        self.expect(TokenKind::Colon)?;
        let body = if self.at(TokenKind::Newline) {
            self.parse_logic_block()?
        } else {
            let expr = self.parse_expr()?;
            self.expect_stmt_terminator()?;
            Block {
                span: expr.span,
                stmts: Vec::new(),
                yield_expr: Some(expr),
            }
        };
        let span = join(start.span, body.span);
        Ok(RenderBlock { span, body })
    }

    fn parse_requires_stmt(&mut self) -> Result<aura_ast::RequiresStmt, ParseError> {
        let kw = self.expect(TokenKind::KwRequires)?;
        let expr = self.parse_expr()?;
        let span = join(kw.span, expr.span);
        Ok(aura_ast::RequiresStmt { span, expr })
    }

    fn parse_ensures_stmt(&mut self) -> Result<aura_ast::EnsuresStmt, ParseError> {
        let kw = self.expect(TokenKind::KwEnsures)?;
        let expr = self.parse_expr()?;
        let span = join(kw.span, expr.span);
        Ok(aura_ast::EnsuresStmt { span, expr })
    }

    fn parse_assert_stmt(&mut self) -> Result<aura_ast::AssertStmt, ParseError> {
        let kw = self.expect(TokenKind::KwAssert)?;
        let expr = self.parse_expr()?;
        let span = join(kw.span, expr.span);
        Ok(aura_ast::AssertStmt { span, expr })
    }

    fn parse_assume_stmt(&mut self) -> Result<aura_ast::AssumeStmt, ParseError> {
        let kw = self.expect(TokenKind::KwAssume)?;
        let expr = self.parse_expr()?;
        let span = join(kw.span, expr.span);
        Ok(aura_ast::AssumeStmt { span, expr })
    }

    fn parse_prop_stmt(&mut self) -> Result<PropStmt, ParseError> {
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let expr = self.parse_expr()?;
        self.expect_stmt_terminator()?;
        let span = join(name.span, expr.span);
        Ok(PropStmt { span, name, expr })
    }

    #[allow(dead_code)]
    fn parse_type_alias(&mut self) -> Result<TypeAlias, ParseError> {
        let start = self.expect(TokenKind::KwType)?;
        let name = self.expect_ident()?;

        let mut params: Vec<TypeParam> = Vec::new();
        if self.at(TokenKind::Lt) {
            // Generic type parameters: `type Foo<T, U> = ...`
            self.next();

            if self.at(TokenKind::Gt) {
                return Err(ParseError {
                    message: "type parameter list cannot be empty".to_string(),
                    span: self.peek_span().unwrap_or(name.span),
                });
            }

            loop {
                let p_name = self.expect_ident()?;
                let bound = if self.at(TokenKind::Colon) {
                    self.next();
                    Some(self.expect_ident()?)
                } else {
                    None
                };
                let p_span = if let Some(b) = &bound {
                    join(p_name.span, b.span)
                } else {
                    p_name.span
                };
                params.push(TypeParam {
                    span: p_span,
                    name: p_name,
                    bound,
                });
                if self.at(TokenKind::Comma) {
                    self.next();
                    continue;
                }
                break;
            }

            self.expect(TokenKind::Gt)?;
        }

        self.expect(TokenKind::Eq)?;
        let target = self.parse_type_ref()?;
        self.expect_stmt_terminator()?;
        let span = join(start.span, target.span);
        Ok(TypeAlias {
            span,
            name,
            params,
            target,
        })
    }

    fn parse_trait_def(&mut self) -> Result<TraitDef, ParseError> {
        let start = self.expect(TokenKind::KwTrait)?;
        let name = self.expect_ident()?;
        self.expect_stmt_terminator()?;
        let span = join(start.span, name.span);
        Ok(TraitDef { span, name })
    }

    fn parse_type_stmt(&mut self) -> Result<Stmt, ParseError> {
        // Parse a `type` header and then dispatch by body:
        // - `type Name = record { ... }`
        // - `type Name = enum { ... }`
        // - `type Name = TypeRef`
        let start = self.expect(TokenKind::KwType)?;
        let name = self.expect_ident()?;

        let mut params: Vec<TypeParam> = Vec::new();
        if self.at(TokenKind::Lt) {
            self.next();
            if self.at(TokenKind::Gt) {
                return Err(ParseError {
                    message: "type parameter list cannot be empty".to_string(),
                    span: self.peek_span().unwrap_or(name.span),
                });
            }
            loop {
                let p_name = self.expect_ident()?;
                let bound = if self.at(TokenKind::Colon) {
                    self.next();
                    Some(self.expect_ident()?)
                } else {
                    None
                };
                let p_span = if let Some(b) = &bound {
                    join(p_name.span, b.span)
                } else {
                    p_name.span
                };
                params.push(TypeParam {
                    span: p_span,
                    name: p_name,
                    bound,
                });
                if self.at(TokenKind::Comma) {
                    self.next();
                    continue;
                }
                break;
            }
            self.expect(TokenKind::Gt)?;
        }

        self.expect(TokenKind::Eq)?;

        if self.at(TokenKind::KwRecord) {
            let def = self.parse_record_def_after_header(start.span, name, params)?;
            return Ok(Stmt::RecordDef(def));
        }
        if self.at(TokenKind::KwEnum) {
            let def = self.parse_enum_def_after_header(start.span, name, params)?;
            return Ok(Stmt::EnumDef(def));
        }

        // Fallback: type alias
        let target = self.parse_type_ref()?;
        self.expect_stmt_terminator()?;
        let span = join(start.span, target.span);
        Ok(Stmt::TypeAlias(TypeAlias {
            span,
            name,
            params,
            target,
        }))
    }

    fn parse_record_def_after_header(
        &mut self,
        start_span: Span,
        name: Ident,
        params: Vec<TypeParam>,
    ) -> Result<RecordDef, ParseError> {
        let _kw = self.expect(TokenKind::KwRecord)?;
        let lb = self.expect(TokenKind::LBrace)?;

        while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
            self.next();
        }

        let mut fields: Vec<RecordFieldDef> = Vec::new();
        if self.at(TokenKind::RBrace) {
            let rb = self.next().unwrap();
            self.expect_stmt_terminator()?;
            let span = join(start_span, rb.span);
            return Ok(RecordDef {
                span,
                name,
                params,
                fields,
            });
        }

        loop {
            while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                self.next();
            }
            if self.at(TokenKind::RBrace) {
                let rb = self.next().unwrap();
                self.expect_stmt_terminator()?;
                let span = join(start_span, rb.span);
                return Ok(RecordDef {
                    span,
                    name,
                    params,
                    fields,
                });
            }
            if self.at(TokenKind::Eof) {
                return Err(ParseError {
                    message: "unterminated record type; expected '}'".to_string(),
                    span: join(start_span, lb.span),
                });
            }

            let field_name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let field_ty = self.parse_type_ref()?;

            let default = if self.at(TokenKind::Eq) {
                self.next();
                Some(self.parse_expr()?)
            } else {
                None
            };

            let field_span = join(field_name.span, field_ty.span);
            fields.push(RecordFieldDef {
                span: field_span,
                name: field_name,
                ty: field_ty,
                default,
            });

            while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                self.next();
            }

            if self.at(TokenKind::Comma) {
                self.next();
                continue;
            }

            if self.at(TokenKind::RBrace) {
                continue;
            }

            return Err(ParseError {
                message: "expected ',' or '}' in record type".to_string(),
                span: self.peek_span().unwrap_or_else(|| join(start_span, lb.span)),
            });
        }
    }

    fn parse_enum_def_after_header(
        &mut self,
        start_span: Span,
        name: Ident,
        params: Vec<TypeParam>,
    ) -> Result<EnumDef, ParseError> {
        let _kw = self.expect(TokenKind::KwEnum)?;
        let lb = self.expect(TokenKind::LBrace)?;

        while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
            self.next();
        }

        let mut variants: Vec<EnumVariantDef> = Vec::new();
        if self.at(TokenKind::RBrace) {
            let rb = self.next().unwrap();
            self.expect_stmt_terminator()?;
            let span = join(start_span, rb.span);
            return Ok(EnumDef {
                span,
                name,
                params,
                variants,
            });
        }

        loop {
            while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                self.next();
            }
            if self.at(TokenKind::RBrace) {
                let rb = self.next().unwrap();
                self.expect_stmt_terminator()?;
                let span = join(start_span, rb.span);
                return Ok(EnumDef {
                    span,
                    name,
                    params,
                    variants,
                });
            }
            if self.at(TokenKind::Eof) {
                return Err(ParseError {
                    message: "unterminated enum type; expected '}'".to_string(),
                    span: join(start_span, lb.span),
                });
            }

            let v_name = self.expect_ident()?;
            let mut fields: Vec<EnumFieldDef> = Vec::new();

            if self.at(TokenKind::LParen) {
                self.next();
                if !self.at(TokenKind::RParen) {
                    loop {
                        let f_name = self.expect_ident()?;
                        self.expect(TokenKind::Colon)?;
                        let f_ty = self.parse_type_ref()?;
                        let f_span = join(f_name.span, f_ty.span);
                        fields.push(EnumFieldDef {
                            span: f_span,
                            name: f_name,
                            ty: f_ty,
                        });
                        if self.at(TokenKind::Comma) {
                            self.next();
                            continue;
                        }
                        break;
                    }
                }
                self.expect(TokenKind::RParen)?;
            }

            let v_span = v_name.span;
            variants.push(EnumVariantDef {
                span: v_span,
                name: v_name,
                fields,
            });

            while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                self.next();
            }

            if self.at(TokenKind::Comma) {
                self.next();
                continue;
            }

            if self.at(TokenKind::RBrace) {
                continue;
            }

            return Err(ParseError {
                message: "expected ',' or '}' in enum type".to_string(),
                span: self.peek_span().unwrap_or_else(|| join(start_span, lb.span)),
            });
        }
    }

    fn parse_import_stmt(&mut self) -> Result<ImportStmt, ParseError> {
        let start = self.expect(TokenKind::KwImport)?;
        let mut path = Vec::new();
        path.push(self.expect_ident()?);
        while self.at(TokenKind::ColonColon) {
            self.next();
            path.push(self.expect_ident()?);
        }
        self.expect_stmt_terminator()?;
        let end_span = path.last().map(|p| p.span).unwrap_or(start.span);
        let span = join(start.span, end_span);
        Ok(ImportStmt { span, path })
    }

    fn parse_strand_def(&mut self) -> Result<StrandDef, ParseError> {
        let start = self.expect(TokenKind::KwVal)?;
        let mutable = if self.at(TokenKind::KwMut) {
            self.next();
            true
        } else {
            false
        };
        let name = self.expect_ident()?;
        let mut ty = None;
        let mut where_clause = None;
        if self.at(TokenKind::Colon) {
            self.next();
            ty = Some(self.parse_type_ref()?);

            if self.at(TokenKind::KwWhere) {
                self.next();
                where_clause = Some(self.parse_expr()?);
            }
        }
        self.expect(TokenKind::Eq)?;
        let expr = self.parse_expr()?;
        self.expect_stmt_terminator()?;
        let end_span = expr.span;
        let span = join(start.span, end_span);
        Ok(StrandDef {
            span,
            name,
            mutable,
            ty,
            where_clause,
            expr,
        })
    }

    fn parse_cell_def(&mut self) -> Result<CellDef, ParseError> {
        let start = self.expect(TokenKind::KwCell)?;
        let name = self.parse_qualified_ident()?;
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;

        let flow = if self.at(TokenKind::Arrow) || self.at(TokenKind::TildeArrow) {
            Some(self.parse_flow_op()?)
        } else {
            None
        };

        self.expect(TokenKind::Colon)?;
        let body = self.parse_logic_block()?;
        let span = join(start.span, body.span);
        Ok(CellDef {
            span,
            name,
            params,
            flow,
            body,
        })
    }

    fn parse_extern_cell(&mut self) -> Result<ExternCell, ParseError> {
        let trusted = if self.at(TokenKind::KwTrusted) {
            self.next();
            true
        } else {
            false
        };

        let start = self.expect(TokenKind::KwExtern)?;
        self.expect(TokenKind::KwCell)?;
        let name = self.parse_qualified_ident()?;
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::Colon)?;
        let ret = self.parse_type_ref()?;
        self.expect_stmt_terminator()?;
        let span = join(start.span, ret.span);
        Ok(ExternCell {
            span,
            trusted,
            name,
            params,
            ret,
        })
    }

    fn parse_unsafe_block(&mut self) -> Result<aura_ast::UnsafeBlock, ParseError> {
        let start = self.expect(TokenKind::KwUnsafe)?;
        self.expect(TokenKind::Colon)?;
        let body = self.parse_logic_block()?;
        let span = join(start.span, body.span);
        Ok(aura_ast::UnsafeBlock { span, body })
    }

    fn parse_qualified_ident(&mut self) -> Result<Ident, ParseError> {
        let first = self.expect_ident()?;
        let mut full = first.node.clone();
        let mut end = first.span;
        while self.at(TokenKind::ColonColon) || self.at(TokenKind::Dot) {
            self.next();
            let next = self.expect_ident()?;
            end = next.span;
            full.push('.');
            full.push_str(&next.node);
        }
        let span = join(first.span, end);
        Ok(Ident::new(span, full))
    }

    fn parse_flow_block(&mut self) -> Result<FlowBlock, ParseError> {
        let name = self.expect_ident()?;
        let flow = self.parse_flow_op()?;
        self.expect(TokenKind::Colon)?;
        let body = self.parse_logic_block()?;
        let span = join(name.span, body.span);
        Ok(FlowBlock {
            span,
            name,
            flow,
            body,
        })
    }

    fn parse_logic_block(&mut self) -> Result<Block, ParseError> {
        // After ':', require NEWLINE INDENT ... DEDENT
        self.expect(TokenKind::Newline)?;
        let indent_tok = self.expect(TokenKind::Indent)?;

        let mut stmts = Vec::new();
        let mut yield_expr = None;

        loop {
            self.skip_newlines();
            if self.at(TokenKind::Dedent) {
                let dedent = self.next().unwrap();
                let span = join(indent_tok.span, dedent.span);
                return Ok(Block {
                    span,
                    stmts,
                    yield_expr,
                });
            }
            if self.at(TokenKind::Eof) {
                let span = indent_tok.span;
                return Err(ParseError {
                    message: "unterminated block; expected dedent".to_string(),
                    span,
                });
            }

            if self.at(TokenKind::KwYield) {
                if yield_expr.is_some() {
                    return Err(ParseError {
                        message: "multiple yield statements in one block".to_string(),
                        span: self.peek_span().unwrap_or(indent_tok.span),
                    });
                }
                let y = self.next().unwrap();
                let expr = self.parse_expr()?;
                self.expect_stmt_terminator()?;
                yield_expr = Some(expr);
                // Enforce yield is last.
                self.skip_newlines();
                if !self.at(TokenKind::Dedent) {
                    return Err(ParseError {
                        message: "yield must be the last statement in a block".to_string(),
                        span: y.span,
                    });
                }
                continue;
            }

            stmts.push(self.parse_stmt()?);
        }
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        if self.at(TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let mutable = if self.at(TokenKind::KwMut) {
                self.next();
                true
            } else {
                false
            };
            let ty = self.parse_type_ref()?;
            let span = join(name.span, ty.span);
            params.push(Param {
                span,
                name,
                mutable,
                ty,
            });

            if self.at(TokenKind::Comma) {
                self.next();
                if self.at(TokenKind::RParen) {
                    break;
                }
                continue;
            }
            break;
        }
        Ok(params)
    }

    fn parse_assign_stmt(&mut self) -> Result<AssignStmt, ParseError> {
        let target = self.expect_ident()?;
        self.expect(TokenKind::Eq)?;
        let expr = self.parse_expr()?;
        self.expect_stmt_terminator()?;
        let span = join(target.span, expr.span);
        Ok(AssignStmt { span, target, expr })
    }

    fn parse_if_stmt(&mut self) -> Result<IfStmt, ParseError> {
        let start = self.expect(TokenKind::KwIf)?;
        let cond = self.parse_expr()?;
        self.expect(TokenKind::Colon)?;
        let then_block = self.parse_logic_block()?;

        self.skip_newlines();
        let else_block = if self.at(TokenKind::KwElse) {
            self.next();
            self.expect(TokenKind::Colon)?;
            Some(self.parse_logic_block()?)
        } else {
            None
        };

        let end_span = else_block
            .as_ref()
            .map(|b| b.span)
            .unwrap_or(then_block.span);
        let span = join(start.span, end_span);
        Ok(IfStmt {
            span,
            cond,
            then_block,
            else_block,
        })
    }

    fn parse_match_stmt(&mut self) -> Result<MatchStmt, ParseError> {
        let start = self.expect(TokenKind::KwMatch)?;
        let scrutinee = self.parse_expr()?;
        self.expect(TokenKind::Colon)?;

        // match header requires an indented arm list.
        self.expect(TokenKind::Newline)?;
        self.expect(TokenKind::Indent)?;

        let mut arms: Vec<MatchArm> = Vec::new();
        while !self.at(TokenKind::Dedent) && !self.at(TokenKind::Eof) {
            self.skip_newlines();
            if self.at(TokenKind::Dedent) || self.at(TokenKind::Eof) {
                break;
            }

            let pat = self.parse_pattern()?;
            self.expect(TokenKind::Colon)?;
            let body = self.parse_logic_block()?;
            let span = join(pat_span(&pat), body.span);
            arms.push(MatchArm { span, pat, body });
        }

        let end = self.expect(TokenKind::Dedent)?;
        let span = join(start.span, end.span);
        Ok(MatchStmt { span, scrutinee, arms })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        // Avoid borrowing `self` across `next()`.
        if matches!(self.peek_kind(), Some(TokenKind::Ident(name)) if name == "_") {
            let t = self.next().expect("token");
            return Ok(Pattern::Wildcard { span: t.span });
        }

        if matches!(self.peek_kind(), Some(TokenKind::Int(_))) {
            let t = self.next().expect("token");
            let TokenKind::Int(value) = t.kind else {
                unreachable!();
            };
            return Ok(Pattern::IntLit { span: t.span, value });
        }

        if matches!(self.peek_kind(), Some(TokenKind::String(_))) {
            let t = self.next().expect("token");
            let TokenKind::String(value) = t.kind else {
                unreachable!();
            };
            return Ok(Pattern::StringLit { span: t.span, value });
        }

        // Constructor pattern: Type::Variant(x, y)
        if matches!(self.peek_kind(), Some(TokenKind::Ident(_))) {
            let ty = self.expect_ident()?;
            if self.at(TokenKind::ColonColon) || self.at(TokenKind::Dot) {
                self.next();
                let variant = self.expect_ident()?;
                let mut binders: Vec<Ident> = Vec::new();

                if self.at(TokenKind::LParen) {
                    self.next();
                    if !self.at(TokenKind::RParen) {
                        loop {
                            binders.push(self.expect_ident()?);
                            if self.at(TokenKind::Comma) {
                                self.next();
                                continue;
                            }
                            break;
                        }
                    }
                    let rp = self.expect(TokenKind::RParen)?;
                    let span = join(ty.span, rp.span);
                    return Ok(Pattern::Ctor {
                        span,
                        ty,
                        variant,
                        binders,
                    });
                }

                let span = join(ty.span, variant.span);
                return Ok(Pattern::Ctor {
                    span,
                    ty,
                    variant,
                    binders,
                });
            }

            // If it's just an ident (not a ctor), treat as parse error for now.
            return Err(ParseError {
                message: "expected a match pattern ('_', int, string, or Type::Variant)".to_string(),
                span: ty.span,
            });
        }

        let span = self.peek_span().unwrap_or_else(|| span_between(0, 0));
        Err(ParseError {
            message: "expected a match pattern ('_', int, string, or Type::Variant)".to_string(),
            span,
        })
    }

    fn parse_while_stmt(&mut self) -> Result<WhileStmt, ParseError> {
        let start = self.expect(TokenKind::KwWhile)?;
        let cond = self.parse_expr()?;

        let invariant = if self.at(TokenKind::KwInvariant) {
            self.next();
            Some(self.parse_expr()?)
        } else {
            None
        };

        let decreases = if self.at(TokenKind::KwDecreases) {
            self.next();
            Some(self.parse_expr()?)
        } else {
            None
        };

        self.expect(TokenKind::Colon)?;
        let body = self.parse_logic_block()?;
        let span = join(start.span, body.span);
        Ok(WhileStmt {
            span,
            cond,
            invariant,
            decreases,
            body,
        })
    }

    fn parse_type_ref(&mut self) -> Result<TypeRef, ParseError> {
        let name = self.expect_ident()?;
        let mut end = name.span;
        let mut args = Vec::new();
        let mut range = None;

        if self.at(TokenKind::Lt) {
            // Generic type args: Foo<T, [1,2,3]>
            let _lt = self.next().unwrap();
            loop {
                args.push(self.parse_type_arg()?);
                if self.at(TokenKind::Comma) {
                    self.next();
                    continue;
                }
                break;
            }
            let gt = self.expect(TokenKind::Gt)?;
            end = gt.span;
        }

        if self.at(TokenKind::LBracket) {
            let lb = self.next().unwrap();
            let lo = self.parse_expr()?;
            self.expect(TokenKind::DotDot)?;
            let hi = self.parse_expr()?;
            let rb = self.expect(TokenKind::RBracket)?;
            let span = join(lb.span, rb.span);
            range = Some(RangeConstraint { span, lo, hi });
            end = rb.span;
        }

        let span = join(name.span, end);
        Ok(TypeRef {
            span,
            name,
            args,
            range,
        })
    }

    fn parse_type_arg(&mut self) -> Result<TypeArg, ParseError> {
        if self.at(TokenKind::LBracket) {
            // Shape literal: [224, 224, 3]
            self.next();
            let mut dims = Vec::new();

            if !self.at(TokenKind::RBracket) {
                loop {
                    if self.config.features.contains("ctfe") {
                        // CTFE MVP: allow const integer expressions in tensor shapes.
                        // Example: `Tensor<u32, [2 + 3*4, (2+3)*4]>`.
                        let expr = self.parse_expr()?;
                        let n = eval_const_u64(&expr)?;
                        dims.push(n);
                    } else {
                        let tok = self.next().ok_or_else(|| ParseError {
                            message: "unexpected end of input while parsing shape".to_string(),
                            span: self.peek_span().unwrap_or_else(|| span_between(0, 0)),
                        })?;
                        match tok.kind {
                            TokenKind::Int(n) => {
                                dims.push(n);
                                // In non-CTFE mode, shapes are a list of integer literals.
                                // If we see an operator or other token, produce a gated diagnostic.
                                if !self.at(TokenKind::Comma) && !self.at(TokenKind::RBracket) {
                                    return Err(ParseError {
                                        message: "shape dimensions must be integer literals (enable unstable feature 'ctfe' for const expressions)".to_string(),
                                        span: self.peek_span().unwrap_or(tok.span),
                                    });
                                }
                            }
                            _ => {
                                return Err(ParseError {
                                    message: "shape dimensions must be integer literals (enable unstable feature 'ctfe' for const expressions)".to_string(),
                                    span: tok.span,
                                })
                            }
                        }
                    }

                    if self.at(TokenKind::Comma) {
                        self.next();
                        continue;
                    }
                    break;
                }
            }

            self.expect(TokenKind::RBracket)?;
            return Ok(TypeArg::Shape(dims));
        }

        Ok(TypeArg::Type(Box::new(self.parse_type_ref()?)))
    }

    fn parse_flow_op(&mut self) -> Result<FlowOp, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::Arrow) => {
                self.next();
                Ok(FlowOp::Sync)
            }
            Some(TokenKind::TildeArrow) => {
                self.next();
                Ok(FlowOp::Async)
            }
            _ => Err(ParseError {
                message: "expected flow operator (-> or ~>)".to_string(),
                span: self.peek_span().unwrap_or_else(|| span_between(0, 0)),
            }),
        }
    }

    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_flow_expr()
    }

    pub fn parse_expr_eof(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_expr()?;
        // Allow trailing newlines.
        while self.at(TokenKind::Newline) {
            self.next();
        }
        if !self.at(TokenKind::Eof) {
            return Err(ParseError {
                message: "expected end of input".to_string(),
                span: self.peek_span().unwrap_or_else(|| span_between(0, 0)),
            });
        }
        Ok(expr)
    }

    fn parse_flow_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_or_expr()?;
        while self.at(TokenKind::Arrow) || self.at(TokenKind::TildeArrow) {
            let op_tok = self.next().unwrap();
            let op = match op_tok.kind {
                TokenKind::Arrow => FlowOp::Sync,
                TokenKind::TildeArrow => FlowOp::Async,
                _ => unreachable!(),
            };
            let right = self.parse_or_expr()?;
            let span = join(left.span, right.span);
            left = Expr {
                span,
                kind: ExprKind::Flow {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and_expr()?;
        while self.at(TokenKind::OrOr) {
            self.next();
            let right = self.parse_and_expr()?;
            let span = join(left.span, right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op: BinOp::Or,
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_cmp_expr()?;
        while self.at(TokenKind::AndAnd) {
            self.next();
            let right = self.parse_cmp_expr()?;
            let span = join(left.span, right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op: BinOp::And,
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_cmp_expr(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_add_expr()?;
        let op = match self.peek_kind() {
            Some(TokenKind::EqEq) => Some(BinOp::Eq),
            Some(TokenKind::Neq) => Some(BinOp::Ne),
            Some(TokenKind::Lt) => Some(BinOp::Lt),
            Some(TokenKind::Gt) => Some(BinOp::Gt),
            Some(TokenKind::Le) => Some(BinOp::Le),
            Some(TokenKind::Ge) => Some(BinOp::Ge),
            _ => None,
        };

        let Some(op) = op else { return Ok(left) };
        self.next();
        let right = self.parse_add_expr()?;
        let span = join(left.span, right.span);
        let expr = Expr {
            span,
            kind: ExprKind::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            },
        };

        // Syntax stabilization: disallow chained comparisons like `a < b < c`.
        // Require explicit parentheses or boolean operators instead.
        if matches!(
            self.peek_kind(),
            Some(
                TokenKind::EqEq
                    | TokenKind::Neq
                    | TokenKind::Lt
                    | TokenKind::Gt
                    | TokenKind::Le
                    | TokenKind::Ge
            )
        ) {
            let span = self.peek_span().unwrap_or(expr.span);
            return Err(ParseError {
                message: "chained comparisons are not supported; use parentheses or boolean operators".to_string(),
                span,
            });
        }

        Ok(expr)
    }

    fn parse_add_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_mul_expr()?;
        loop {
            let op = match self.peek_kind() {
                Some(TokenKind::Plus) => Some(BinOp::Add),
                Some(TokenKind::Minus) => Some(BinOp::Sub),
                _ => None,
            };
            let Some(op) = op else { break };
            self.next();
            let right = self.parse_mul_expr()?;
            let span = join(left.span, right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_mul_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary_expr()?;
        loop {
            let op = match self.peek_kind() {
                Some(TokenKind::Star) => Some(BinOp::Mul),
                Some(TokenKind::Slash) => Some(BinOp::Div),
                _ => None,
            };
            let Some(op) = op else { break };
            self.next();
            let right = self.parse_unary_expr()?;
            let span = join(left.span, right.span);
            left = Expr {
                span,
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
            };
        }
        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, ParseError> {
        if self.at(TokenKind::Bang) {
            let t = self.next().unwrap();
            let expr = self.parse_unary_expr()?;
            let span = join(t.span, expr.span);
            return Ok(Expr {
                span,
                kind: ExprKind::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                },
            });
        }
        if self.at(TokenKind::Minus) {
            let t = self.next().unwrap();
            let expr = self.parse_unary_expr()?;
            let span = join(t.span, expr.span);
            return Ok(Expr {
                span,
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                },
            });
        }
        self.parse_postfix_expr()
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary_expr()?;
        loop {
            // Record literal: `TypeName { field: value, ... }`
            if self.at(TokenKind::LBrace) {
                if let ExprKind::Ident(name) = &expr.kind {
                    if name.node != "Style" {
                        expr = self.parse_record_lit(expr.span, name.clone())?;
                        continue;
                    }
                }
            }

            if self.at(TokenKind::Dot) || self.at(TokenKind::ColonColon) {
                let _sep = self.next().unwrap();
                let member = self.expect_ident()?;
                let span = join(expr.span, member.span);
                expr = Expr {
                    span,
                    kind: ExprKind::Member {
                        base: Box::new(expr),
                        member,
                    },
                };
                // Keep looping: allow chained member + call.
                continue;
            }

            // Generic args are currently a syntactic allowance only (ignored by the AST).
            // We only treat `<...>` as generics when it is immediately followed by `(`.
            if self.at(TokenKind::Lt) {
                if self.try_parse_generic_args()?.is_some() {
                    continue;
                }
            }

            if self.at(TokenKind::LParen) {
                let lp = self.next().unwrap();
                let args = self.parse_args()?;
                let rp = self.expect(TokenKind::RParen)?;
                let span = join(expr.span, rp.span);
                expr = Expr {
                    span,
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                        trailing: None,
                    },
                };
                let _ = (lp,);

                // Optional trailing brace block: `Foo(...) { ... }`
                if self.at(TokenKind::LBrace) {
                    let body = self.parse_brace_block()?;
                    let span = join(expr.span, body.span);
                    expr = match expr.kind {
                        ExprKind::Call { callee, args, trailing: _ } => Expr {
                            span,
                            kind: ExprKind::Call {
                                callee,
                                args,
                                trailing: Some(Box::new(body)),
                            },
                        },
                        _ => unreachable!(),
                    };
                }
                continue;
            }

            break;
        }
        Ok(expr)
    }

    fn parse_record_lit(&mut self, start_span: Span, name: Ident) -> Result<Expr, ParseError> {
        let lb = self.expect(TokenKind::LBrace)?;

        while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
            self.next();
        }

        let mut fields: Vec<(Ident, Expr)> = Vec::new();
        if self.at(TokenKind::RBrace) {
            let rb = self.next().unwrap();
            let span = join(start_span, rb.span);
            return Ok(Expr {
                span,
                kind: ExprKind::RecordLit { name, fields },
            });
        }

        loop {
            while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                self.next();
            }
            if self.at(TokenKind::RBrace) {
                let rb = self.next().unwrap();
                let span = join(start_span, rb.span);
                return Ok(Expr {
                    span,
                    kind: ExprKind::RecordLit { name, fields },
                });
            }
            if self.at(TokenKind::Eof) {
                return Err(ParseError {
                    message: "unterminated record literal; expected '}'".to_string(),
                    span: join(start_span, lb.span),
                });
            }

            let key = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expr()?;
            fields.push((key, value));

            while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                self.next();
            }

            if self.at(TokenKind::Comma) {
                self.next();
                continue;
            }

            if self.at(TokenKind::RBrace) {
                continue;
            }

            return Err(ParseError {
                message: "expected ',' or '}' in record literal".to_string(),
                span: self.peek_span().unwrap_or_else(|| join(start_span, lb.span)),
            });
        }
    }

    fn parse_args(&mut self) -> Result<Vec<CallArg>, ParseError> {
        let mut args: Vec<CallArg> = Vec::new();
        if self.at(TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            // Named arg: Ident ':' expr
            if matches!(self.peek_kind(), Some(TokenKind::Ident(_)))
                && self.peek_kind_n(1).is_some_and(|k| matches!(k, TokenKind::Colon))
            {
                let name = self.expect_ident()?;
                self.expect(TokenKind::Colon)?;
                let value = self.parse_expr()?;
                args.push(CallArg::Named { name, value });
            } else {
                let expr = self.parse_expr()?;
                args.push(CallArg::Positional(expr));
            }

            if self.at(TokenKind::Comma) {
                self.next();
                if self.at(TokenKind::RParen) {
                    break;
                }
                continue;
            }
            break;
        }
        Ok(args)
    }

    fn parse_brace_block(&mut self) -> Result<Block, ParseError> {
        let lb = self.expect(TokenKind::LBrace)?;

        // Optional newline after '{'
        if self.at(TokenKind::Newline) {
            self.next();
        }

        let mut stmts = Vec::new();
        let mut yield_expr = None;

        loop {
            // Allow indentation tokens inside braces but do not require them.
            while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                self.next();
            }

            if self.at(TokenKind::RBrace) {
                let rb = self.next().unwrap();
                let span = join(lb.span, rb.span);
                return Ok(Block { span, stmts, yield_expr });
            }
            if self.at(TokenKind::Eof) {
                return Err(ParseError {
                    message: "unterminated brace block; expected '}'".to_string(),
                    span: lb.span,
                });
            }

            if self.at(TokenKind::KwYield) {
                if yield_expr.is_some() {
                    return Err(ParseError {
                        message: "multiple yield statements in one block".to_string(),
                        span: self.peek_span().unwrap_or(lb.span),
                    });
                }
                let y = self.next().unwrap();
                let expr = self.parse_expr()?;
                yield_expr = Some(expr);
                // yield must be last before '}' (ignoring whitespace/newlines).
                loop {
                    if self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                        self.next();
                        continue;
                    }
                    break;
                }
                if !self.at(TokenKind::RBrace) {
                    return Err(ParseError {
                        message: "yield must be the last statement in a block".to_string(),
                        span: y.span,
                    });
                }
                continue;
            }

            stmts.push(self.parse_stmt()?);
        }
    }

    fn try_parse_generic_args(&mut self) -> Result<Option<Vec<TypeRef>>, ParseError> {
        if !self.at(TokenKind::Lt) {
            return Ok(None);
        }

        // Lookahead: `< ... > (` with no newline/indent/dedent in the middle.
        // If it doesn't match, this is likely a comparison operator handled at the binary level.
        let mut j = self.idx;
        let mut saw_gt = None;
        while let Some(tok) = self.tokens.get(j) {
            match tok.kind {
                TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent | TokenKind::Eof => {
                    return Ok(None);
                }
                TokenKind::Gt => {
                    saw_gt = Some(j);
                    break;
                }
                _ => {
                    j += 1;
                }
            }
        }
        let Some(gt_idx) = saw_gt else {
            return Ok(None);
        };
        let Some(after_gt) = self.tokens.get(gt_idx + 1) else {
            return Ok(None);
        };
        if !matches!(after_gt.kind, TokenKind::LParen) {
            return Ok(None);
        }

        // Feature gate: call-site generics are not part of the stable surface yet.
        if !self.config.has_feature("callsite-generics") {
            return Err(ParseError {
                message: "call-site generic arguments are gated behind feature 'callsite-generics'".to_string(),
                span: self.peek_span().unwrap_or_else(|| span_between(0, 0)),
            });
        }

        // Consume: < TypeRef (, TypeRef)* >
        let _lt = self.next().unwrap();
        let mut args = Vec::new();
        if self.at(TokenKind::Gt) {
            // Empty generics: <>
            let _ = self.next();
            return Ok(Some(args));
        }

        loop {
            args.push(self.parse_type_ref()?);
            if self.at(TokenKind::Comma) {
                self.next();
                continue;
            }
            break;
        }
        self.expect(TokenKind::Gt)?;
        Ok(Some(args))
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, ParseError> {
        let tok = self
            .next()
            .ok_or_else(|| ParseError {
                message: "unexpected end of input".to_string(),
                span: span_between(0, 0),
            })?;

        match tok.kind {
            TokenKind::KwForall | TokenKind::KwExists => {
                let is_forall = matches!(tok.kind, TokenKind::KwForall);
                let binders = self.parse_quant_binders()?;
                self.expect(TokenKind::Colon)?;
                let body = self.parse_expr()?;
                let span = join(tok.span, body.span);
                let kind = if is_forall {
                    ExprKind::ForAll {
                        binders,
                        body: Box::new(body),
                    }
                } else {
                    ExprKind::Exists {
                        binders,
                        body: Box::new(body),
                    }
                };
                Ok(Expr { span, kind })
            }
            TokenKind::Arrow | TokenKind::TildeArrow => {
                let op = match tok.kind {
                    TokenKind::Arrow => FlowOp::Sync,
                    TokenKind::TildeArrow => FlowOp::Async,
                    _ => unreachable!(),
                };
                let body = self.parse_brace_block()?;
                let span = join(tok.span, body.span);
                Ok(Expr {
                    span,
                    kind: ExprKind::Lambda {
                        op,
                        body: Box::new(body),
                    },
                })
            }
            TokenKind::Ident(name) => {
                let ident: Ident = Ident {
                    span: tok.span,
                    node: name,
                };

                // `Style { key: value, ... }`
                if ident.node == "Style" && self.at(TokenKind::LBrace) {
                    return self.parse_style_lit(tok.span);
                }

                Ok(Expr {
                    span: tok.span,
                    kind: ExprKind::Ident(ident),
                })
            }
            TokenKind::Int(n) => Ok(Expr {
                span: tok.span,
                kind: ExprKind::IntLit(n),
            }),
            TokenKind::String(s) => Ok(Expr {
                span: tok.span,
                kind: ExprKind::StringLit(s),
            }),
            TokenKind::LParen => {
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            _ => Err(ParseError {
                message: "expected an expression".to_string(),
                span: tok.span,
            }),
        }
    }

    fn parse_quant_binders(&mut self) -> Result<Vec<aura_ast::QuantBinder>, ParseError> {
        let lp = self.expect(TokenKind::LParen)?;
        let mut binders: Vec<aura_ast::QuantBinder> = Vec::new();

        if self.at(TokenKind::RParen) {
            let _ = self.next();
            return Ok(binders);
        }

        loop {
            let name = self.expect_ident()?;
            let ty = if self.at(TokenKind::Colon) {
                self.next();
                Some(self.parse_type_ref()?)
            } else {
                None
            };
            let end_span = ty.as_ref().map(|t| t.span).unwrap_or(name.span);
            let span = join(name.span, end_span);
            binders.push(aura_ast::QuantBinder { span, name, ty });

            if self.at(TokenKind::Comma) {
                self.next();
                continue;
            }
            break;
        }

        let rp = self.expect(TokenKind::RParen)?;
        let _ = join(lp.span, rp.span);
        Ok(binders)
    }

    fn parse_style_lit(&mut self, start_span: Span) -> Result<Expr, ParseError> {
        let lb = self.expect(TokenKind::LBrace)?;

        // Allow optional whitespace/newlines after '{'.
        while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
            self.next();
        }

        let mut fields: Vec<(Ident, Expr)> = Vec::new();

        if self.at(TokenKind::RBrace) {
            let rb = self.next().unwrap();
            let span = join(start_span, rb.span);
            return Ok(Expr {
                span,
                kind: ExprKind::StyleLit { fields },
            });
        }

        loop {
            while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                self.next();
            }
            if self.at(TokenKind::RBrace) {
                let rb = self.next().unwrap();
                let span = join(start_span, rb.span);
                return Ok(Expr {
                    span,
                    kind: ExprKind::StyleLit { fields },
                });
            }
            if self.at(TokenKind::Eof) {
                return Err(ParseError {
                    message: "unterminated style literal; expected '}'".to_string(),
                    span: join(start_span, lb.span),
                });
            }

            let key = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expr()?;
            fields.push((key, value));

            while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                self.next();
            }

            if self.at(TokenKind::Comma) {
                self.next();
                // Allow trailing comma before '}'.
                while self.at(TokenKind::Newline) || self.at(TokenKind::Indent) || self.at(TokenKind::Dedent) {
                    self.next();
                }
                if self.at(TokenKind::RBrace) {
                    continue;
                }
                continue;
            }

            // If no comma, we must be at end.
            if self.at(TokenKind::RBrace) {
                continue;
            }

            return Err(ParseError {
                message: "expected ',' or '}' in style literal".to_string(),
                span: self.peek_span().unwrap_or_else(|| join(start_span, lb.span)),
            });
        }
    }

    fn is_flow_block_start(&self) -> bool {
        // Ident (->|~>) :
        let Some(TokenKind::Ident(_)) = self.peek_kind() else {
            return false;
        };
        let Some(op) = self.peek_kind_n(1) else {
            return false;
        };
        let Some(TokenKind::Colon) = self.peek_kind_n(2) else {
            return false;
        };
        matches!(op, TokenKind::Arrow | TokenKind::TildeArrow)
    }

    fn skip_newlines(&mut self) {
        while self.at(TokenKind::Newline) {
            self.next();
        }
    }

    #[allow(dead_code)]
    fn expect_newline_or_eof(&mut self) -> Result<(), ParseError> {
        if self.at(TokenKind::Newline) {
            self.next();
            Ok(())
        } else if self.at(TokenKind::Eof) {
            Ok(())
        } else {
            Err(ParseError {
                message: "expected end of line".to_string(),
                span: self.peek_span().unwrap_or_else(|| span_between(0, 0)),
            })
        }
    }

    fn expect_stmt_terminator(&mut self) -> Result<(), ParseError> {
        if self.at(TokenKind::Newline) {
            self.next();
            Ok(())
        } else if self.at(TokenKind::RBrace) || self.at(TokenKind::Eof) {
            Ok(())
        } else {
            Err(ParseError {
                message: "expected end of line".to_string(),
                span: self.peek_span().unwrap_or_else(|| span_between(0, 0)),
            })
        }
    }

    fn expect_ident(&mut self) -> Result<Ident, ParseError> {
        let tok = self.expect_any()?;
        match tok.kind {
            TokenKind::Ident(name) => Ok(Ident {
                span: tok.span,
                node: name,
            }),
            _ => Err(ParseError {
                message: "expected identifier".to_string(),
                span: tok.span,
            }),
        }
    }

    fn expect(&mut self, expected: TokenKind) -> Result<Token, ParseError> {
        let tok = self.expect_any()?;
        if mem::discriminant(&tok.kind) == mem::discriminant(&expected) {
            Ok(tok)
        } else {
            Err(ParseError {
                message: format!("expected {expected:?}") ,
                span: tok.span,
            })
        }
    }

    fn expect_any(&mut self) -> Result<Token, ParseError> {
        self.next().ok_or_else(|| ParseError {
            message: "unexpected end of input".to_string(),
            span: span_between(0, 0),
        })
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek_kind()
            .is_some_and(|k| mem::discriminant(k) == mem::discriminant(&kind))
    }

    fn next(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.idx)?.clone();
        self.idx += 1;
        Some(tok)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.idx).map(|t| &t.kind)
    }

    fn peek_kind_n(&self, n: usize) -> Option<&TokenKind> {
        self.tokens.get(self.idx + n).map(|t| &t.kind)
    }

    fn peek_span(&self) -> Option<Span> {
        self.tokens.get(self.idx).map(|t| t.span)
    }
}

fn join(a: Span, b: Span) -> Span {
    let a0: usize = a.offset().into();
    let b0: usize = b.offset().into();
    let b1 = b0 + b.len();
    if b0 >= a0 {
        span_between(a0, b1)
    } else {
        let a1 = a0 + a.len();
        span_between(b0, a1)
    }
}

fn eval_const_u64(expr: &Expr) -> Result<u64, ParseError> {
    match &expr.kind {
        ExprKind::IntLit(n) => Ok(*n),
        ExprKind::Unary { op, expr: inner } => match op {
            UnaryOp::Neg => {
                let v = eval_const_u64(inner)?;
                if v == 0 {
                    Ok(0)
                } else {
                    Err(ParseError {
                        message: "const u64 expressions cannot be negative".to_string(),
                        span: expr.span,
                    })
                }
            }
            UnaryOp::Not => Err(ParseError {
                message: "const u64 expressions do not support boolean operators".to_string(),
                span: expr.span,
            }),
        },
        ExprKind::Binary { left, op, right } => {
            let l = eval_const_u64(left)?;
            let r = eval_const_u64(right)?;
            let out = match op {
                BinOp::Add => l.checked_add(r),
                BinOp::Sub => l.checked_sub(r),
                BinOp::Mul => l.checked_mul(r),
                BinOp::Div => {
                    if r == 0 {
                        None
                    } else {
                        Some(l / r)
                    }
                }
                _ => None,
            };
            out.ok_or_else(|| ParseError {
                message: "unsupported or overflowing const u64 expression".to_string(),
                span: expr.span,
            })
        }
        _ => Err(ParseError {
            message: "shape dimensions must be const integer expressions".to_string(),
            span: expr.span,
        }),
    }
}

fn pat_span(p: &Pattern) -> Span {
    match p {
        Pattern::Wildcard { span } => *span,
        Pattern::IntLit { span, .. } => *span,
        Pattern::StringLit { span, .. } => *span,
        Pattern::Ctor { span, .. } => *span,
    }
}
