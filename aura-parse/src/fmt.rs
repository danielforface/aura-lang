#![forbid(unsafe_code)]

use aura_ast::{
    BinOp, Block, CallArg, CellDef, Expr, ExprKind, FlowBlock, FlowOp, Ident, IfStmt, LayoutBlock,
    MatchStmt, Pattern, Program, RenderBlock, Span, Stmt, TypeArg, TypeRef, UnaryOp, WhileStmt,
};

const INDENT: &str = "    ";

pub fn format_program(program: &Program) -> String {
    let mut out = String::new();
    let mut first = true;
    for stmt in &program.stmts {
        if !first {
            out.push('\n');
        }
        first = false;
        fmt_stmt(&mut out, 0, stmt);
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

pub fn format_expr(expr: &Expr) -> String {
    let mut out = String::new();
    fmt_expr(&mut out, expr, Prec::Lowest);
    out
}

fn fmt_stmt(out: &mut String, indent: usize, stmt: &Stmt) {
    match stmt {
        Stmt::Import(s) => {
            indent_line(out, indent);
            out.push_str("import ");
            fmt_import_path(out, &s.path);
            out.push('\n');
        }
        Stmt::MacroDef(s) => {
            indent_line(out, indent);
            out.push_str("macro ");
            out.push_str(&s.name.node);
            out.push('(');
            for (i, p) in s.params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&p.node);
            }
            out.push_str("):\n");
            fmt_block_indent(out, indent + 1, &s.body);
        }
        Stmt::MacroCall(s) => {
            indent_line(out, indent);
            out.push_str(&s.name.node);
            out.push_str("!(");
            for (i, a) in s.args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                fmt_expr(out, a, Prec::Lowest);
            }
            out.push_str(")\n");
        }
        Stmt::TypeAlias(s) => {
            indent_line(out, indent);
            out.push_str("type ");
            out.push_str(&s.name.node);

            if !s.params.is_empty() {
                out.push('<');
                for (i, p) in s.params.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&p.name.node);
                    if let Some(b) = &p.bound {
                        out.push_str(": ");
                        out.push_str(&b.node);
                    }
                }
                out.push('>');
            }

            out.push_str(" = ");
            fmt_type_ref(out, &s.target);
            out.push('\n');
        }
        Stmt::TraitDef(s) => {
            indent_line(out, indent);
            out.push_str("trait ");
            out.push_str(&s.name.node);
            out.push('\n');
        }
        Stmt::RecordDef(s) => {
            indent_line(out, indent);
            out.push_str("type ");
            out.push_str(&s.name.node);
            if !s.params.is_empty() {
                out.push('<');
                for (i, p) in s.params.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&p.name.node);
                    if let Some(b) = &p.bound {
                        out.push_str(": ");
                        out.push_str(&b.node);
                    }
                }
                out.push('>');
            }
            out.push_str(" = record { ");
            for (i, f) in s.fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&f.name.node);
                out.push_str(": ");
                fmt_type_ref(out, &f.ty);
                if let Some(d) = &f.default {
                    out.push_str(" = ");
                    fmt_expr(out, d, Prec::Lowest);
                }
            }
            out.push_str(" }\n");
        }
        Stmt::EnumDef(s) => {
            indent_line(out, indent);
            out.push_str("type ");
            out.push_str(&s.name.node);
            if !s.params.is_empty() {
                out.push('<');
                for (i, p) in s.params.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&p.name.node);
                    if let Some(b) = &p.bound {
                        out.push_str(": ");
                        out.push_str(&b.node);
                    }
                }
                out.push('>');
            }
            out.push_str(" = enum { ");
            for (i, v) in s.variants.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&v.name.node);
                if !v.fields.is_empty() {
                    out.push('(');
                    for (j, f) in v.fields.iter().enumerate() {
                        if j > 0 {
                            out.push_str(", ");
                        }
                        out.push_str(&f.name.node);
                        out.push_str(": ");
                        fmt_type_ref(out, &f.ty);
                    }
                    out.push(')');
                }
            }
            out.push_str(" }\n");
        }
        Stmt::StrandDef(s) => {
            indent_line(out, indent);
            out.push_str("val ");
            if s.mutable {
                out.push_str("mut ");
            }
            out.push_str(&s.name.node);
            if let Some(ty) = &s.ty {
                out.push_str(": ");
                fmt_type_ref(out, ty);
                if let Some(w) = &s.where_clause {
                    out.push_str(" where ");
                    fmt_expr(out, w, Prec::Lowest);
                }
            }
            out.push_str(" = ");
            fmt_expr(out, &s.expr, Prec::Lowest);
            out.push('\n');
        }
        Stmt::ExternCell(s) => {
            indent_line(out, indent);
            if s.trusted {
                out.push_str("trusted ");
            }
            out.push_str("extern cell ");
            out.push_str(&s.name.node.replace('.', "::"));
            out.push('(');
            fmt_params(out, &s.params);
            out.push(')');
            out.push_str(": ");
            fmt_type_ref(out, &s.ret);
            out.push('\n');
        }
        Stmt::UnsafeBlock(s) => {
            indent_line(out, indent);
            out.push_str("unsafe:\n");
            fmt_block_indent(out, indent + 1, &s.body);
        }
        Stmt::CellDef(s) => fmt_cell_def(out, indent, s),
        Stmt::FlowBlock(s) => fmt_flow_block(out, indent, s),
        Stmt::Layout(s) => fmt_layout(out, indent, s),
        Stmt::Render(s) => fmt_render(out, indent, s),
        Stmt::Prop(s) => {
            indent_line(out, indent);
            out.push_str(&s.name.node);
            out.push_str(": ");
            fmt_expr(out, &s.expr, Prec::Lowest);
            out.push('\n');
        }
        Stmt::Assign(s) => {
            indent_line(out, indent);
            out.push_str(&s.target.node);
            out.push_str(" = ");
            fmt_expr(out, &s.expr, Prec::Lowest);
            out.push('\n');
        }
        Stmt::If(s) => fmt_if(out, indent, s),
        Stmt::Match(s) => fmt_match(out, indent, s),
        Stmt::While(s) => fmt_while(out, indent, s),
        Stmt::Requires(s) => {
            indent_line(out, indent);
            out.push_str("requires ");
            fmt_expr(out, &s.expr, Prec::Lowest);
            out.push('\n');
        }
        Stmt::Ensures(s) => {
            indent_line(out, indent);
            out.push_str("ensures ");
            fmt_expr(out, &s.expr, Prec::Lowest);
            out.push('\n');
        }
        Stmt::Assert(s) => {
            indent_line(out, indent);
            out.push_str("assert ");
            fmt_expr(out, &s.expr, Prec::Lowest);
            out.push('\n');
        }
        Stmt::Assume(s) => {
            indent_line(out, indent);
            out.push_str("assume ");
            fmt_expr(out, &s.expr, Prec::Lowest);
            out.push('\n');
        }
        Stmt::ExprStmt(e) => {
            indent_line(out, indent);
            fmt_expr(out, e, Prec::Lowest);
            out.push('\n');
        }
    }
}

fn fmt_match(out: &mut String, indent: usize, s: &MatchStmt) {
    indent_line(out, indent);
    out.push_str("match ");
    fmt_expr(out, &s.scrutinee, Prec::Lowest);
    out.push_str(":\n");

    for arm in &s.arms {
        indent_line(out, indent + 1);
        fmt_pattern(out, &arm.pat);
        out.push_str(":\n");
        fmt_block_indent(out, indent + 2, &arm.body);
    }
}

fn fmt_pattern(out: &mut String, p: &Pattern) {
    match p {
        Pattern::Wildcard { .. } => out.push('_'),
        Pattern::IntLit { value, .. } => out.push_str(&value.to_string()),
        Pattern::StringLit { value, .. } => fmt_string_lit(out, value),
        Pattern::Ctor {
            ty,
            variant,
            binders,
            ..
        } => {
            out.push_str(&ty.node);
            out.push_str("::");
            out.push_str(&variant.node);
            if !binders.is_empty() {
                out.push('(');
                for (i, b) in binders.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&b.node);
                }
                out.push(')');
            }
        }
    }
}

fn fmt_string_lit(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other if other.is_control() => {
                let cp = other as u32;
                out.push_str(&format!("\\u{{{:x}}}", cp));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

fn fmt_import_path(out: &mut String, path: &[Ident]) {
    for (i, seg) in path.iter().enumerate() {
        if i > 0 {
            out.push_str("::");
        }
        out.push_str(&seg.node);
    }
}

fn fmt_params(out: &mut String, params: &[aura_ast::Param]) {
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        if p.mutable {
            out.push_str("mut ");
        }
        out.push_str(&p.name.node);
        out.push_str(": ");
        fmt_type_ref(out, &p.ty);
    }
}

fn fmt_cell_def(out: &mut String, indent: usize, s: &CellDef) {
    indent_line(out, indent);
    out.push_str("cell ");
    out.push_str(&s.name.node.replace('.', "::"));
    out.push('(');
    fmt_params(out, &s.params);
    out.push(')');

    if let Some(flow) = s.flow {
        out.push(' ');
        out.push_str(match flow {
            FlowOp::Sync => "->",
            FlowOp::Async => "~>",
        });
    }

    out.push_str(":\n");
    fmt_block_indent(out, indent + 1, &s.body);
}

fn fmt_flow_block(out: &mut String, indent: usize, s: &FlowBlock) {
    indent_line(out, indent);
    out.push_str(&s.name.node);
    out.push(' ');
    out.push_str(match s.flow {
        FlowOp::Sync => "->",
        FlowOp::Async => "~>",
    });
    out.push_str(":\n");
    fmt_block_indent(out, indent + 1, &s.body);
}

fn fmt_layout(out: &mut String, indent: usize, s: &LayoutBlock) {
    indent_line(out, indent);
    out.push_str("layout:\n");
    fmt_block_indent(out, indent + 1, &s.body);
}

fn fmt_render(out: &mut String, indent: usize, s: &RenderBlock) {
    indent_line(out, indent);
    out.push_str("render:\n");
    fmt_block_indent(out, indent + 1, &s.body);
}

fn fmt_if(out: &mut String, indent: usize, s: &IfStmt) {
    indent_line(out, indent);
    out.push_str("if ");
    fmt_expr(out, &s.cond, Prec::Lowest);
    out.push_str(":\n");
    fmt_block_indent(out, indent + 1, &s.then_block);

    if let Some(else_block) = &s.else_block {
        indent_line(out, indent);
        out.push_str("else:\n");
        fmt_block_indent(out, indent + 1, else_block);
    }
}

fn fmt_while(out: &mut String, indent: usize, s: &WhileStmt) {
    indent_line(out, indent);
    out.push_str("while ");
    fmt_expr(out, &s.cond, Prec::Lowest);
    if let Some(inv) = &s.invariant {
        out.push_str(" invariant ");
        fmt_expr(out, inv, Prec::Lowest);
    }
    if let Some(dec) = &s.decreases {
        out.push_str(" decreases ");
        fmt_expr(out, dec, Prec::Lowest);
    }
    out.push_str(":\n");
    fmt_block_indent(out, indent + 1, &s.body);
}

fn fmt_block_indent(out: &mut String, indent: usize, block: &Block) {
    for stmt in &block.stmts {
        fmt_stmt(out, indent, stmt);
    }
    if let Some(expr) = &block.yield_expr {
        indent_line(out, indent);
        out.push_str("yield ");
        fmt_expr(out, expr, Prec::Lowest);
        out.push('\n');
    }
}

fn fmt_type_ref(out: &mut String, t: &TypeRef) {
    out.push_str(&t.name.node);
    if !t.args.is_empty() {
        out.push('<');
        for (i, a) in t.args.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            match a {
                TypeArg::Type(inner) => fmt_type_ref(out, inner),
                TypeArg::Shape(dims) => {
                    out.push('[');
                    for (j, d) in dims.iter().enumerate() {
                        if j > 0 {
                            out.push_str(", ");
                        }
                        out.push_str(&d.to_string());
                    }
                    out.push(']');
                }
            }
        }
        out.push('>');
    }

    if let Some(r) = &t.range {
        out.push('[');
        fmt_expr(out, &r.lo, Prec::Lowest);
        out.push_str("..");
        fmt_expr(out, &r.hi, Prec::Lowest);
        out.push(']');
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Lowest,
    Flow,
    Or,
    And,
    Cmp,
    Add,
    Mul,
    Unary,
    Postfix,
    Primary,
}

fn bin_prec(op: &BinOp) -> Prec {
    match op {
        BinOp::Or => Prec::Or,
        BinOp::And => Prec::And,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => Prec::Cmp,
        BinOp::Add | BinOp::Sub => Prec::Add,
        BinOp::Mul | BinOp::Div => Prec::Mul,
    }
}

fn needs_parens(parent: Prec, child: Prec) -> bool {
    child < parent
}

fn fmt_expr(out: &mut String, expr: &Expr, parent_prec: Prec) {
    match &expr.kind {
        ExprKind::Ident(id) => out.push_str(&id.node),
        ExprKind::IntLit(n) => out.push_str(&n.to_string()),
        ExprKind::StringLit(s) => {
            out.push('"');
            for ch in s.chars() {
                match ch {
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    other if other.is_control() => {
                        let cp = other as u32;
                        out.push_str(&format!("\\u{{{:x}}}", cp));
                    }
                    other => out.push(other),
                }
            }
            out.push('"');
        }
        ExprKind::StyleLit { fields } => {
            out.push_str("Style {");
            if !fields.is_empty() {
                out.push(' ');
            }
            for (i, (k, v)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&k.node);
                out.push_str(": ");
                fmt_expr(out, v, Prec::Lowest);
            }
            if !fields.is_empty() {
                out.push(' ');
            }
            out.push('}');
        }
        ExprKind::RecordLit { name, fields } => {
            out.push_str(&name.node);
            out.push_str(" {");
            if !fields.is_empty() {
                out.push(' ');
            }
            for (i, (k, v)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&k.node);
                out.push_str(": ");
                fmt_expr(out, v, Prec::Lowest);
            }
            if !fields.is_empty() {
                out.push(' ');
            }
            out.push('}');
        }
        ExprKind::Unary { op, expr: inner } => {
            let my = Prec::Unary;
            let parens = needs_parens(parent_prec, my);
            if parens {
                out.push('(');
            }
            match op {
                UnaryOp::Neg => out.push('-'),
                UnaryOp::Not => out.push('!'),
            }
            fmt_expr(out, inner, my);
            if parens {
                out.push(')');
            }
        }
        ExprKind::Binary { left, op, right } => {
            let my = bin_prec(op);
            let parens = needs_parens(parent_prec, my);
            if parens {
                out.push('(');
            }
            fmt_expr(out, left, my);
            out.push(' ');
            out.push_str(match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Lt => "<",
                BinOp::Gt => ">",
                BinOp::Le => "<=",
                BinOp::Ge => ">=",
                BinOp::And => "&&",
                BinOp::Or => "||",
            });
            out.push(' ');
            fmt_expr(out, right, my);
            if parens {
                out.push(')');
            }
        }
        ExprKind::ForAll { binders, body } => {
            out.push_str("forall ");
            fmt_quant_binders(out, binders);
            out.push_str(": ");
            fmt_expr(out, body, Prec::Lowest);
        }
        ExprKind::Exists { binders, body } => {
            out.push_str("exists ");
            fmt_quant_binders(out, binders);
            out.push_str(": ");
            fmt_expr(out, body, Prec::Lowest);
        }
        ExprKind::Member { base, member } => {
            let my = Prec::Postfix;
            let parens = needs_parens(parent_prec, my);
            if parens {
                out.push('(');
            }
            fmt_expr(out, base, my);
            out.push_str("::");
            out.push_str(&member.node);
            if parens {
                out.push(')');
            }
        }
        ExprKind::Call {
            callee,
            args,
            trailing,
        } => {
            let my = Prec::Postfix;
            let parens = needs_parens(parent_prec, my);
            if parens {
                out.push('(');
            }
            fmt_expr(out, callee, my);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                match a {
                    CallArg::Positional(e) => fmt_expr(out, e, Prec::Lowest),
                    CallArg::Named { name, value } => {
                        out.push_str(&name.node);
                        out.push_str(": ");
                        fmt_expr(out, value, Prec::Lowest);
                    }
                }
            }
            out.push(')');
            if let Some(b) = trailing {
                out.push(' ');
                fmt_brace_block(out, 0, b);
            }
            if parens {
                out.push(')');
            }
        }
        ExprKind::Lambda { op, body } => {
            let my = Prec::Flow;
            let parens = needs_parens(parent_prec, my);
            if parens {
                out.push('(');
            }
            out.push_str(match op {
                FlowOp::Sync => "->",
                FlowOp::Async => "~>",
            });
            out.push(' ');
            fmt_inline_block(out, 0, body);
            if parens {
                out.push(')');
            }
        }
        ExprKind::Flow { left, op, right } => {
            let my = Prec::Flow;
            let parens = needs_parens(parent_prec, my);
            if parens {
                out.push('(');
            }
            fmt_expr(out, left, my);
            out.push(' ');
            out.push_str(match op {
                FlowOp::Sync => "->",
                FlowOp::Async => "~>",
            });
            out.push(' ');
            fmt_expr(out, right, my);
            if parens {
                out.push(')');
            }
        }
    }
}

fn fmt_inline_block(out: &mut String, indent: usize, block: &Block) {
    // Inline blocks are rare today (lambda bodies). Keep a compact brace form.
    fmt_brace_block(out, indent, block);
}

fn fmt_brace_block(out: &mut String, indent: usize, block: &Block) {
    out.push('{');
    out.push('\n');
    for stmt in &block.stmts {
        fmt_stmt(out, indent + 1, stmt);
    }
    if let Some(expr) = &block.yield_expr {
        indent_line(out, indent + 1);
        out.push_str("yield ");
        fmt_expr(out, expr, Prec::Lowest);
        out.push('\n');
    }
    indent_line(out, indent);
    out.push('}');
}

fn indent_line(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str(INDENT);
    }
}

fn fmt_quant_binders(out: &mut String, binders: &[aura_ast::QuantBinder]) {
    out.push('(');
    for (i, b) in binders.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&b.name.node);
        if let Some(ty) = &b.ty {
            out.push_str(": ");
            fmt_type_ref(out, ty);
        }
    }
    out.push(')');
}

#[allow(dead_code)]
fn _span(_s: Span) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_idempotent(src: &str) {
        let program = crate::parse_source(src).unwrap();
        let formatted1 = format_program(&program);
        let program2 = crate::parse_source(&formatted1).unwrap();
        let formatted2 = format_program(&program2);
        assert_eq!(formatted1, formatted2);
    }

    #[test]
    fn fmt_roundtrip_basic() {
        is_idempotent(
            "import aura::io\n\nval x: u32 = 1 + 2 * 3\ncell main():\n    io::println(text: \"hi\\n\")\n",
        );
    }

    #[test]
    fn fmt_roundtrip_generic_type_alias_syntax() {
        is_idempotent(
            "type Box<T> = Tensor<T>\n\nval x: Box<u32> = tensor::new(len: 3)\n",
        );
    }
 }
