#![forbid(unsafe_code)]

// Lumina Sentinel geometry verification.
//
// This is intentionally conservative and currently targets the UI subset emitted by the AVM UI tree
// builder (calls with trailing blocks + Prop/Render/Expr children).

#[cfg(feature = "z3")]
use aura_ast::{CallArg, Expr, ExprKind, Ident, Program, Span, Stmt};

#[cfg(feature = "z3")]
use crate::solver::{DiagnosticMetadata, RelatedInfo, VerifyError};

#[cfg(feature = "z3")]
use aura_nexus::NexusContext;

#[cfg(feature = "z3")]
use z3::{
    ast::{Ast, Int},
    Context, SatResult, Solver,
};

#[cfg(feature = "z3")]
const SCREEN_W: i64 = 1920;
#[cfg(feature = "z3")]
const SCREEN_H: i64 = 1080;

#[cfg(feature = "z3")]
#[derive(Clone, Debug)]
struct UiGeomNode {
    kind: String,
    span: Span,
    props: Vec<(String, String, Span)>,
    children: Vec<UiGeomNode>,
}

#[cfg(feature = "z3")]
impl UiGeomNode {
    fn prop(&self, k: &str) -> Option<(&str, Span)> {
        self.props
            .iter()
            .find(|(kk, _, _)| kk == k)
            .map(|(_, v, sp)| (v.as_str(), *sp))
    }

    fn prop_i64(&self, k: &str) -> Option<(i64, Span)> {
        let (v, sp) = self.prop(k)?;
        v.parse::<i64>().ok().map(|n| (n, sp))
    }
}

#[cfg(feature = "z3")]
fn program_requests_lumina(program: &Program) -> bool {
    for stmt in &program.stmts {
        let Stmt::Import(i) = stmt else { continue };
        let last = i.path.last().map(|s| s.node.as_str()).unwrap_or("");
        if last == "aura_lumina" {
            return true;
        }
        if i.path.len() == 2 && i.path[0].node == "aura" && i.path[1].node == "lumina" {
            return true;
        }
    }
    false
}

#[cfg(feature = "z3")]
fn is_ui_call(name: &str, has_trailing: bool) -> bool {
    if has_trailing {
        return true;
    }
    matches!(name, "App" | "VStack" | "HStack" | "Text" | "Button")
}

#[cfg(feature = "z3")]
fn literal_to_string(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::IntLit(n) => Some(n.to_string()),
        ExprKind::StringLit(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(feature = "z3")]
fn style_fields_to_props(fields: &[(Ident, Expr)]) -> Vec<(String, String, Span)> {
    let mut out = Vec::new();
    for (k, v) in fields {
        if let Some(s) = literal_to_string(v) {
            out.push((k.node.clone(), s, v.span));
        }
    }
    out
}

#[cfg(feature = "z3")]
fn ui_from_expr(expr: &Expr) -> Option<UiGeomNode> {
    let ExprKind::Call { callee, args, trailing } = &expr.kind else {
        return None;
    };

    let callee_name = match &callee.kind {
        ExprKind::Ident(id) => id.node.clone(),
        _ => return None,
    };

    if !is_ui_call(&callee_name, trailing.is_some()) {
        return None;
    }

    let mut props: Vec<(String, String, Span)> = Vec::new();

    for (idx, a) in args.iter().enumerate() {
        match a {
            CallArg::Positional(e) => {
                if let Some(v) = literal_to_string(e) {
                    props.push((format!("_{idx}"), v, e.span));
                }
            }
            CallArg::Named { name, value } => {
                if matches!(value.kind, ExprKind::Lambda { .. }) {
                    props.push((name.node.clone(), "<callback>".to_string(), value.span));
                } else if let ExprKind::StyleLit { fields } = &value.kind {
                    if name.node == "style" {
                        props.extend(style_fields_to_props(fields));
                    }
                } else if let Some(v) = literal_to_string(value) {
                    props.push((name.node.clone(), v, value.span));
                }
            }
        }
    }

    let mut node = UiGeomNode {
        kind: callee_name,
        span: expr.span,
        props,
        children: Vec::new(),
    };

    if let Some(tb) = trailing {
        for s in &tb.stmts {
            match s {
                Stmt::Prop(p) => {
                    if matches!(p.expr.kind, ExprKind::Lambda { .. }) {
                        node.props
                            .push((p.name.node.clone(), "<callback>".to_string(), p.expr.span));
                    } else if let ExprKind::StyleLit { fields } = &p.expr.kind {
                        if p.name.node == "style" {
                            node.props.extend(style_fields_to_props(fields));
                        }
                    } else if let Some(v) = literal_to_string(&p.expr) {
                        node.props.push((p.name.node.clone(), v, p.expr.span));
                    }
                }
                Stmt::ExprStmt(e) => {
                    if let Some(child) = ui_from_expr(e) {
                        node.children.push(child);
                    }
                }
                Stmt::Render(r) => {
                    // `render:` blocks can contain either a single yielded UI expr or one/more UI expr statements.
                    for child in ui_roots_from_block(&r.body) {
                        node.children.push(child);
                    }
                }
                _ => {}
            }
        }

        if let Some(y) = &tb.yield_expr {
            if let Some(child) = ui_from_expr(y) {
                node.children.push(child);
            }
        }
    }

    Some(node)
}

#[cfg(feature = "z3")]
fn ui_roots_from_block(b: &aura_ast::Block) -> Vec<UiGeomNode> {
    let mut out: Vec<UiGeomNode> = Vec::new();

    for s in &b.stmts {
        match s {
            Stmt::ExprStmt(e) => {
                if let Some(n) = ui_from_expr(e) {
                    out.push(n);
                }
            }
            Stmt::Render(r) => {
                // Recurse: render blocks often contain UI expr statements rather than a yield expr.
                out.extend(ui_roots_from_block(&r.body));
            }
            _ => {}
        }
    }

    if let Some(y) = &b.yield_expr {
        if let Some(n) = ui_from_expr(y) {
            out.push(n);
        }
    }

    out
}

#[cfg(feature = "z3")]
#[derive(Clone)]
struct Vars<'ctx> {
    x: Int<'ctx>,
    y: Int<'ctx>,
    w: Int<'ctx>,
    h: Int<'ctx>,
}

#[cfg(feature = "z3")]
fn mk_vars<'ctx>(ctx: &'ctx Context, name: &str) -> Vars<'ctx> {
    Vars {
        x: Int::new_const(ctx, format!("{name}_x")),
        y: Int::new_const(ctx, format!("{name}_y")),
        w: Int::new_const(ctx, format!("{name}_w")),
        h: Int::new_const(ctx, format!("{name}_h")),
    }
}

#[cfg(feature = "z3")]
fn push_common_constraints<'ctx>(solver: &Solver<'ctx>, v: &Vars<'ctx>) {
    solver.assert(&v.x.ge(&Int::from_i64(v.x.get_ctx(), 0)));
    solver.assert(&v.y.ge(&Int::from_i64(v.y.get_ctx(), 0)));
    solver.assert(&v.w.ge(&Int::from_i64(v.w.get_ctx(), 0)));
    solver.assert(&v.h.ge(&Int::from_i64(v.h.get_ctx(), 0)));

    let sw = Int::from_i64(v.x.get_ctx(), SCREEN_W);
    let sh = Int::from_i64(v.x.get_ctx(), SCREEN_H);

    let xw = Int::add(v.x.get_ctx(), &[&v.x, &v.w]);
    let yh = Int::add(v.x.get_ctx(), &[&v.y, &v.h]);
    solver.assert(&xw.le(&sw));
    solver.assert(&yh.le(&sh));
}

#[cfg(feature = "z3")]
fn constrain_node<'ctx>(
    ctx: &'ctx Context,
    solver: &Solver<'ctx>,
    node: &UiGeomNode,
    name: &str,
    v: &Vars<'ctx>,
    parent: Option<&Vars<'ctx>>,
) {
    push_common_constraints(solver, v);

    if let Some(p) = parent {
        // Keep child inside parent bounds.
        solver.assert(&v.x.ge(&p.x));
        solver.assert(&v.y.ge(&p.y));

        let child_right = Int::add(ctx, &[&v.x, &v.w]);
        let child_bottom = Int::add(ctx, &[&v.y, &v.h]);
        let parent_right = Int::add(ctx, &[&p.x, &p.w]);
        let parent_bottom = Int::add(ctx, &[&p.y, &p.h]);
        solver.assert(&child_right.le(&parent_right));
        solver.assert(&child_bottom.le(&parent_bottom));
    }

    // Size constraints from props.
    if let Some((w, _sp)) = node.prop_i64("width") {
        solver.assert(&v.w._eq(&Int::from_i64(ctx, w)));
    }
    if let Some((h, _sp)) = node.prop_i64("height") {
        solver.assert(&v.h._eq(&Int::from_i64(ctx, h)));
    }

    // Root fills the screen.
    if parent.is_none() {
        solver.assert(&v.x._eq(&Int::from_i64(ctx, 0)));
        solver.assert(&v.y._eq(&Int::from_i64(ctx, 0)));
        solver.assert(&v.w._eq(&Int::from_i64(ctx, SCREEN_W)));
        solver.assert(&v.h._eq(&Int::from_i64(ctx, SCREEN_H)));
    }

    // Parent-child layout constraints for VStack.
    if node.kind == "VStack" {
        let spacing = node.prop_i64("spacing").map(|(n, _)| n).unwrap_or(0);
        let spacing_i = Int::from_i64(ctx, spacing);

        // Ensure children are stacked top-to-bottom.
        let mut prev: Option<Vars<'ctx>> = None;
        for (i, child) in node.children.iter().enumerate() {
            let cv = mk_vars(ctx, &format!("{name}_c{i}"));
            constrain_node(ctx, solver, child, &format!("{name}_c{i}"), &cv, Some(v));

            if let Some(pv) = &prev {
                // y_next >= y_prev + h_prev + spacing
                let rhs = Int::add(ctx, &[&pv.y, &pv.h, &spacing_i]);
                solver.assert(&cv.y.ge(&rhs));
            } else {
                solver.assert(&cv.y.ge(&v.y));
            }

            // Align to parent's x by default.
            solver.assert(&cv.x._eq(&v.x));

            prev = Some(cv);
        }

        return;
    }

    // Parent-child layout constraints for HStack.
    if node.kind == "HStack" {
        let spacing = node.prop_i64("spacing").map(|(n, _)| n).unwrap_or(0);
        let spacing_i = Int::from_i64(ctx, spacing);

        let mut prev: Option<Vars<'ctx>> = None;
        for (i, child) in node.children.iter().enumerate() {
            let cv = mk_vars(ctx, &format!("{name}_c{i}"));
            constrain_node(ctx, solver, child, &format!("{name}_c{i}"), &cv, Some(v));

            if let Some(pv) = &prev {
                // x_next >= x_prev + w_prev + spacing
                let rhs = Int::add(ctx, &[&pv.x, &pv.w, &spacing_i]);
                solver.assert(&cv.x.ge(&rhs));
            } else {
                solver.assert(&cv.x.ge(&v.x));
            }

            // Align to parent's y by default.
            solver.assert(&cv.y._eq(&v.y));

            prev = Some(cv);
        }

        return;
    }

    // Default: recurse without additional layout constraints.
    for (i, child) in node.children.iter().enumerate() {
        let cv = mk_vars(ctx, &format!("{name}_c{i}"));
        constrain_node(ctx, solver, child, &format!("{name}_c{i}"), &cv, Some(v));
    }
}

#[cfg(feature = "z3")]
fn find_obvious_overflow(node: &UiGeomNode) -> Option<(Span, String)> {
    if let Some((w, sp)) = node.prop_i64("width") {
        if w > SCREEN_W {
            return Some((
                sp,
                format!(
                    "'{}' width={} exceeds screen width {}",
                    node.kind, w, SCREEN_W
                ),
            ));
        }
    }
    if let Some((h, sp)) = node.prop_i64("height") {
        if h > SCREEN_H {
            return Some((
                sp,
                format!(
                    "'{}' height={} exceeds screen height {}",
                    node.kind, h, SCREEN_H
                ),
            ));
        }
    }
    for c in &node.children {
        if let Some(x) = find_obvious_overflow(c) {
            return Some(x);
        }
    }
    None
}

#[cfg(feature = "z3")]
pub fn verify_lumina_geometry(
    program: &Program,
    ctx: &Context,
    _nexus: &mut NexusContext,
) -> Result<(), VerifyError> {
    if !program_requests_lumina(program) {
        return Ok(());
    }

    let mut roots: Vec<UiGeomNode> = Vec::new();
    for stmt in &program.stmts {
        match stmt {
            Stmt::Layout(lb) => roots.extend(ui_roots_from_block(&lb.body)),
            Stmt::Render(rb) => roots.extend(ui_roots_from_block(&rb.body)),
            Stmt::CellDef(c) => {
                // Also scan inside cell bodies.
                roots.extend(ui_roots_from_block(&c.body));
            }
            _ => {}
        }
    }

    if roots.is_empty() {
        return Ok(());
    }

    for (idx, root) in roots.iter().enumerate() {
        if let Some((span, why)) = find_obvious_overflow(root) {
            return Err(VerifyError {
                message: format!(
                    "Lumina Sentinel: geometry overflow on {}x{}: {}",
                    SCREEN_W, SCREEN_H, why
                ),
                span,
                model: None,
                meta: Some(DiagnosticMetadata {
                    model: None,
                    bindings: Vec::new(),
                    typed_bindings: Vec::new(),
                    related: vec![RelatedInfo {
                        span,
                        message: why,
                    }],
                    unsat_core: Vec::new(),
                    hints: Vec::new(),
                    suggestions: Vec::new(),
                }),
            });
        }

        let solver = Solver::new(ctx);
        let v = mk_vars(ctx, &format!("ui_root_{idx}"));
        constrain_node(ctx, &solver, root, &format!("ui_root_{idx}"), &v, None);

        match solver.check() {
            SatResult::Sat => {}
            SatResult::Unsat | SatResult::Unknown => {
                return Err(VerifyError {
                    message: format!(
                        "Lumina Sentinel: geometry constraints are unsatisfiable for screen {}x{}",
                        SCREEN_W, SCREEN_H
                    ),
                    span: root.span,
                    model: None,
                    meta: Some(DiagnosticMetadata {
                        model: None,
                        bindings: Vec::new(),
                        typed_bindings: Vec::new(),
                        related: vec![RelatedInfo {
                            span: root.span,
                            message: "UI layout may overflow the screen".to_string(),
                        }],
                        unsat_core: Vec::new(),
                        hints: Vec::new(),
                        suggestions: Vec::new(),
                    }),
                });
            }
        }
    }

    Ok(())
}

#[cfg(feature = "z3")]
fn color_luminance_1000(name: &str) -> Option<i64> {
    // Rough relative luminance on a 0..1000 scale for a few named colors.
    // (Enough to support the early Lumina Style invariants.)
    match name {
        "Black" => Some(0),
        "White" => Some(1000),
        "Red" => Some(212),
        "Green" => Some(715),
        "Blue" => Some(72),
        "Gold" => Some(815),
        _ => None,
    }
}

#[cfg(feature = "z3")]
fn verify_node_aesthetics(node: &UiGeomNode, ctx: &Context) -> Result<(), VerifyError> {
    // Radius bounds.
    if let Some((r, sp)) = node.prop_i64("radius") {
        let solver = Solver::new(ctx);
        let rv = Int::from_i64(ctx, r);
        solver.assert(&rv.ge(&Int::from_i64(ctx, 0)));
        solver.assert(&rv.le(&Int::from_i64(ctx, 64)));
        if !matches!(solver.check(), SatResult::Sat) {
            return Err(VerifyError {
                message: "Lumina Style: radius must be within 0..64".to_string(),
                span: sp,
                model: None,
                meta: Some(DiagnosticMetadata {
                    model: None,
                    bindings: Vec::new(),
                    typed_bindings: Vec::new(),
                    related: vec![RelatedInfo {
                        span: sp,
                        message: "radius out of bounds".to_string(),
                    }],
                    unsat_core: Vec::new(),
                    hints: Vec::new(),
                    suggestions: Vec::new(),
                }),
            });
        }
    }

    // Padding bounds.
    if let Some((p, sp)) = node.prop_i64("padding") {
        let solver = Solver::new(ctx);
        let pv = Int::from_i64(ctx, p);
        solver.assert(&pv.ge(&Int::from_i64(ctx, 0)));
        solver.assert(&pv.le(&Int::from_i64(ctx, 128)));
        if !matches!(solver.check(), SatResult::Sat) {
            return Err(VerifyError {
                message: "Lumina Style: padding must be within 0..128".to_string(),
                span: sp,
                model: None,
                meta: Some(DiagnosticMetadata {
                    model: None,
                    bindings: Vec::new(),
                    typed_bindings: Vec::new(),
                    related: vec![RelatedInfo {
                        span: sp,
                        message: "padding out of bounds".to_string(),
                    }],
                    unsat_core: Vec::new(),
                    hints: Vec::new(),
                    suggestions: Vec::new(),
                }),
            });
        }
    }

    // Contrast: require `fg` vs `bg` (when both present) to meet 4.5:1.
    let fg = node.prop("fg");
    let bg = node.prop("bg");
    if let (Some((fg_name, fg_sp)), Some((bg_name, _bg_sp))) = (fg, bg) {
        if let (Some(fg_l), Some(bg_l)) = (color_luminance_1000(fg_name), color_luminance_1000(bg_name)) {
            let solver = Solver::new(ctx);
            let fg_i = Int::from_i64(ctx, fg_l);
            let bg_i = Int::from_i64(ctx, bg_l);

            let fg_ge_bg = fg_i.ge(&bg_i);
            let l_max = fg_ge_bg.ite(&fg_i, &bg_i);
            let l_min = fg_ge_bg.ite(&bg_i, &fg_i);

            // Integerized check for: (Lmax+50)/(Lmin+50) >= 4.5
            // => 10*(Lmax+50) >= 45*(Lmin+50)
            let fifty = Int::from_i64(ctx, 50);
            let ten = Int::from_i64(ctx, 10);
            let forty_five = Int::from_i64(ctx, 45);

            let lhs = Int::mul(ctx, &[&ten, &Int::add(ctx, &[&l_max, &fifty])]);
            let rhs = Int::mul(ctx, &[&forty_five, &Int::add(ctx, &[&l_min, &fifty])]);
            solver.assert(&lhs.ge(&rhs));

            if !matches!(solver.check(), SatResult::Sat) {
                return Err(VerifyError {
                    message: format!(
                        "Lumina Style: insufficient contrast between fg='{}' and bg='{}' (requires >= 4.5:1)",
                        fg_name, bg_name
                    ),
                    span: fg_sp,
                    model: None,
                    meta: Some(DiagnosticMetadata {
                        model: None,
                        bindings: Vec::new(),
                        typed_bindings: Vec::new(),
                        related: vec![RelatedInfo {
                            span: fg_sp,
                            message: "increase contrast between text and background".to_string(),
                        }],
                        unsat_core: Vec::new(),
                        hints: Vec::new(),
                        suggestions: Vec::new(),
                    }),
                });
            }
        }
    }

    for c in &node.children {
        verify_node_aesthetics(c, ctx)?;
    }
    Ok(())
}

#[cfg(feature = "z3")]
pub fn verify_lumina_aesthetics(
    program: &Program,
    ctx: &Context,
    nexus: &mut NexusContext,
) -> Result<(), VerifyError> {
    let _ = nexus;
    if !program_requests_lumina(program) {
        return Ok(());
    }

    let mut roots: Vec<UiGeomNode> = Vec::new();
    for stmt in &program.stmts {
        match stmt {
            Stmt::Layout(lb) => roots.extend(ui_roots_from_block(&lb.body)),
            Stmt::Render(rb) => roots.extend(ui_roots_from_block(&rb.body)),
            Stmt::CellDef(c) => roots.extend(ui_roots_from_block(&c.body)),
            _ => {}
        }
    }

    for root in &roots {
        verify_node_aesthetics(root, ctx)?;
    }

    Ok(())
}
