#![forbid(unsafe_code)]

use std::collections::HashMap;

use aura_ast::{CallArg, Expr, ExprKind, Program, Stmt, StrandDef, TypeAlias, TypeArg, TypeRef};

use aura_parse::format_expr;

use crate::solver::{DiagnosticMetadata, Prover, RelatedInfo, SmtProfile, TypedBinding, VerifyError};

#[derive(Clone, Copy, Debug)]
struct RangeTy {
    lo: u64,
    hi: u64,
}

pub fn verify_program<P: Prover>(program: &Program, prover: &mut P) -> Result<(), VerifyError> {
    let mut aliases: HashMap<String, RangeTy> = HashMap::new();
    for stmt in &program.stmts {
        if let Stmt::TypeAlias(ta) = stmt {
            if let Some(r) = alias_to_range(ta) {
                aliases.insert(ta.name.node.clone(), r);
            }
        }
    }

    for stmt in &program.stmts {
        visit_stmt(stmt, &aliases, prover)?;
    }
    Ok(())
}

#[cfg(feature = "z3")]
use aura_nexus::{record_proof, NexusContext, NexusDiagnostic, NexusRelated, ProofNote, Z3Call, Z3CallEnv, Z3PluginDispatch};

#[cfg(feature = "z3")]
use std::collections::BTreeSet;

#[cfg(feature = "z3")]
use z3::{
    ast::{Ast, Bool, Dynamic, Int},
    Model, Params, SatResult, Solver,
};

#[cfg(feature = "z3")]
#[derive(Clone, Debug, Default)]
struct ProveEvidence {
    unsat_core: Vec<String>,
    interpolant: Option<String>,
    core_related: Vec<RelatedInfo>,
}

#[cfg(feature = "z3")]
fn extract_bindings_from_model_text(model: &str) -> Vec<(String, String)> {
    // Best-effort model parser: scrape `name -> value` pairs from Z3's text form.
    // This is intentionally loose: it only exists to improve counterexample diagnostics.
    model
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (name, value) = line.split_once("->")?;
            Some((name.trim().to_string(), value.trim().trim_end_matches(',').to_string()))
        })
        .collect()
    }

#[cfg(feature = "z3")]
fn typed_bindings_from_model(st: &SymState<'static>, model: &Model<'static>) -> Vec<TypedBinding> {
    let mut names = st.sorts.keys().cloned().collect::<Vec<_>>();
    names.sort();

    let mut out: Vec<TypedBinding> = Vec::new();
    for name in names {
        let sort = match st.sorts.get(&name) {
            Some(s) => s,
            None => continue,
        };

        match sort {
            Sort::Int => {
                let Some(v) = st.ints.get(&name) else { continue };
                let val = model.eval(v, true);
                let value = val
                    .as_ref()
                    .and_then(|x| x.as_u64())
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| {
                        val.map(|x| x.to_string())
                            .unwrap_or_else(|| "<unknown>".to_string())
                    });

                let aura_type = if let Some((lo, hi)) = st.ranges.get(&name).copied() {
                    format!("u32[{lo}..{hi}]")
                } else {
                    "u32".to_string()
                };

                out.push(TypedBinding {
                    name,
                    aura_type,
                    value,
                });
            }
            Sort::Bool => {
                let Some(v) = st.bools.get(&name) else { continue };
                let val = model.eval(v, true);
                let value = val
                    .as_ref()
                    .and_then(|x| x.as_bool())
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| {
                        val.map(|x| x.to_string())
                            .unwrap_or_else(|| "<unknown>".to_string())
                    });

                out.push(TypedBinding {
                    name,
                    aura_type: "bool".to_string(),
                    value,
                });
            }
        }
    }
    out
}


#[cfg(feature = "z3")]
#[derive(Clone, Debug)]
pub enum VerificationStatus {
    Success,
}

#[cfg(feature = "z3")]
#[cfg(feature = "z3")]
#[derive(Clone, Debug)]
pub struct VerificationReport {
    pub status: VerificationStatus,
    pub proofs: Vec<ProofNote>,
}

#[cfg(feature = "z3")]
pub fn verify_program_z3(
    program: &Program,
    prover: &mut crate::solver::z3_prover::Z3Prover,
    plugins: &impl Z3PluginDispatch,
    nexus: &mut NexusContext,
) -> Result<(), VerifyError> {
    verify_program_z3_profile(program, prover, plugins, nexus, SmtProfile::Fast)
}

#[cfg(feature = "z3")]
pub fn verify_program_z3_profile(
    program: &Program,
    prover: &mut crate::solver::z3_prover::Z3Prover,
    plugins: &impl Z3PluginDispatch,
    nexus: &mut NexusContext,
    profile: SmtProfile,
) -> Result<(), VerifyError> {
    let mut engine = Z3Engine::new_with_profile(prover, plugins, profile);
    engine.verify_program(program, nexus)
}
#[cfg(feature = "z3")]
pub fn verify_program_z3_report(
    program: &Program,
    prover: &mut crate::solver::z3_prover::Z3Prover,
    plugins: &impl Z3PluginDispatch,
    nexus: &mut NexusContext,
) -> Result<VerificationReport, VerifyError> {
    verify_program_z3(program, prover, plugins, nexus)?;
    let proofs = aura_nexus::drain_proofs(nexus);
    Ok(VerificationReport {
        status: VerificationStatus::Success,
        proofs,
    })
}

#[cfg(feature = "z3")]
pub fn verify_stmt_z3(
    stmt: &Stmt,
    prover: &mut crate::solver::z3_prover::Z3Prover,
    plugins: &impl Z3PluginDispatch,
    nexus: &mut NexusContext,
) -> Result<(), VerifyError> {
    let mut engine = Z3Engine::new_with_profile(prover, plugins, SmtProfile::Fast);
    engine.visit_top_stmt(stmt, nexus)
}

#[cfg(feature = "z3")]
struct Z3Engine<'p, 'plug, P> {
    ctx: &'static z3::Context,
    prover: &'p mut crate::solver::z3_prover::Z3Prover,
    plugins: &'plug P,
    opts: VerifyOptions,
}

#[cfg(feature = "z3")]
#[derive(Clone, Copy, Debug)]
struct VerifyOptions {
    profile: SmtProfile,
    timeout_ms: u32,
    allow_quantifiers: bool,
    max_quant_binders: usize,

    /// Enable a long-lived Z3 solver with push/pop to keep the solver warm.
    ///
    /// Controlled by env var `AURA_Z3_INCREMENTAL=1`.
    incremental_solver: bool,
}

#[cfg(feature = "z3")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sort {
    Bool,
    Int,
}

#[cfg(feature = "z3")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EvalMode {
    Runtime,
    Spec,
}

#[cfg(feature = "z3")]
impl<'p, 'plug, P> Z3Engine<'p, 'plug, P>
where
    P: Z3PluginDispatch,
{
    fn new_with_profile(
        prover: &'p mut crate::solver::z3_prover::Z3Prover,
        plugins: &'plug P,
        profile: SmtProfile,
    ) -> Self {
        let (timeout_ms, allow_quantifiers) = match profile {
            SmtProfile::Fast => (50, false),
            SmtProfile::Ci => (250, false),
            SmtProfile::Thorough => (2_000, true),
        };
        Self {
            ctx: prover.ctx_static(),
            prover,
            plugins,
            opts: VerifyOptions {
                profile,
                timeout_ms,
                allow_quantifiers,
                max_quant_binders: 4,
                incremental_solver: std::env::var("AURA_Z3_INCREMENTAL")
                    .ok()
                    .as_deref()
                    == Some("1"),
            },
        }
    }

    fn ctx(&self) -> &'static z3::Context {
        self.ctx
    }

    fn verify_program(&mut self, program: &Program, nexus: &mut NexusContext) -> Result<(), VerifyError> {
        for stmt in &program.stmts {
            self.visit_top_stmt(stmt, nexus)?;
        }
        Ok(())
    }

    fn visit_top_stmt(&mut self, stmt: &Stmt, nexus: &mut NexusContext) -> Result<(), VerifyError> {
        match stmt {
            Stmt::Import(_) | Stmt::TypeAlias(_) | Stmt::ExternCell(_) => Ok(()),
            Stmt::CellDef(cell) => {
                let mut st = SymState::new(self.ctx());
                // Treat params as symbolic values.
                for p in &cell.params {
                    st.define_int(&p.name.node, p.name.span)?;

                    if let Some(dims) = tensor_shape_from_type_ref(Some(&p.ty)) {
                        let v = st.ints.get(&p.name.node).cloned().expect("int");
                        st.note_tensor_shape(&v, &dims);
                    }
                }

                // Contracts: only honor `requires`/`ensures` at the top level of the cell body.
                let mut requires: Vec<Expr> = Vec::new();
                let mut ensures: Vec<Expr> = Vec::new();
                let mut rest: Vec<Stmt> = Vec::new();
                for s in &cell.body.stmts {
                    match s {
                        Stmt::Requires(r) => requires.push(r.expr.clone()),
                        Stmt::Ensures(e) => ensures.push(e.expr.clone()),
                        other => rest.push(other.clone()),
                    }
                }

                let body = aura_ast::Block {
                    span: cell.body.span,
                    stmts: rest,
                    yield_expr: cell.body.yield_expr.clone(),
                };

                for r in &requires {
                    let b = self.eval_bool_spec(r, &mut st, nexus)?;
                    st.note_origin_constraint("requires", r.span, &b);
                    st.constraints.push(b);
                }

                let y = self.check_block_ret(&body, &mut st, nexus)?;
                if let Some(v) = y {
                    match v {
                        Value::Int(i) => st.bind_int("result", i, cell.span),
                        Value::Bool(b) => st.bind_bool("result", b, cell.span),
                    }
                }

                for e in &ensures {
                    let ok = self.eval_bool_spec(e, &mut st, nexus)?;
                    self.prove_implied(
                        Some(&st),
                        &st.constraints,
                        &ok.not(),
                        e.span,
                        "postcondition failed",
                        nexus,
                    )?;
                }

                Ok(())
            }
            Stmt::FlowBlock(fb) => {
                let mut st = SymState::new(self.ctx());
                self.check_block(&fb.body, &mut st, nexus)
            }
            Stmt::Layout(lb) => {
                let mut st = SymState::new(self.ctx());
                self.check_block(&lb.body, &mut st, nexus)
            }
            Stmt::Render(rb) => {
                let mut st = SymState::new(self.ctx());
                self.check_block(&rb.body, &mut st, nexus)
            }
            _ => Ok(()),
        }
    }

    fn check_block(
        &mut self,
        block: &aura_ast::Block,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<(), VerifyError> {
        let _ = self.check_block_ret(block, st, nexus)?;
        Ok(())
    }

    fn check_block_ret(
        &mut self,
        block: &aura_ast::Block,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<Option<Value<'static>>, VerifyError> {
        for s in &block.stmts {
            self.check_stmt(s, st, nexus)?;
        }
        if let Some(y) = &block.yield_expr {
            let v = self.eval_any(y, st, nexus)?;
            return Ok(Some(v));
        }
        Ok(None)
    }

    fn check_stmt(
        &mut self,
        stmt: &Stmt,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<(), VerifyError> {
        match stmt {
            Stmt::Import(_) | Stmt::TypeAlias(_) | Stmt::ExternCell(_) => Ok(()),
            Stmt::StrandDef(sd) => {
                let v = self.eval_any(&sd.expr, st, nexus)?;
                match v {
                    Value::Int(i) => {
                        st.bind_int(&sd.name.node, i, sd.name.span);

                        if let Some(dims) = tensor_shape_from_type_ref(sd.ty.as_ref()) {
                            let v = st.ints.get(&sd.name.node).cloned().expect("int");
                            st.note_tensor_shape(&v, &dims);
                        }

                        if let Some((lo, hi)) = range_from_type_ref(sd.ty.as_ref()) {
                            st.note_range(&sd.name.node, lo, hi);
                            let rhs = st.ints.get(&sd.name.node).cloned().expect("int");
                            self.prove_u32_in_range(
                                st,
                                &rhs,
                                lo,
                                hi,
                                sd.name.span,
                                "range-annotated val may be out of bounds",
                                nexus,
                            )?;
                            st.assert_u32_in_range(&rhs, lo, hi);
                        }
                    }
                    Value::Bool(b) => st.bind_bool(&sd.name.node, b, sd.name.span),
                }
                Ok(())
            }
            Stmt::Assign(a) => {
                let v = self.eval_any(&a.expr, st, nexus)?;
                match v {
                    Value::Int(i) => {
                        if let Some((lo, hi)) = st.ranges.get(&a.target.node).copied() {
                            self.prove_u32_in_range(
                                st,
                                &i,
                                lo,
                                hi,
                                a.span,
                                "assignment may violate range",
                                nexus,
                            )?;
                        }
                        st.bind_int(&a.target.node, i, a.target.span);
                        if let Some((lo, hi)) = st.ranges.get(&a.target.node).copied() {
                            let rhs = st.ints.get(&a.target.node).cloned().expect("int");
                            st.assert_u32_in_range(&rhs, lo, hi);
                        }
                    }
                    Value::Bool(b) => st.bind_bool(&a.target.node, b, a.target.span),
                }
                Ok(())
            }
            Stmt::ExprStmt(e) => {
                let _ = self.eval_any(e, st, nexus)?;
                Ok(())
            }
            Stmt::Requires(r) => {
                let b = self.eval_bool_spec(&r.expr, st, nexus)?;
                st.note_origin_constraint(
                    "requires",
                    r.span,
                    &b,
                );
                st.constraints.push(b);
                Ok(())
            }
            Stmt::Assume(a) => {
                let b = self.eval_bool_spec(&a.expr, st, nexus)?;
                st.note_origin_constraint(
                    "assume",
                    a.span,
                    &b,
                );
                st.constraints.push(b);
                Ok(())
            }
            Stmt::Ensures(e) => {
                let ok = self.eval_bool_spec(&e.expr, st, nexus)?;
                self.prove_implied(
                    Some(st),
                    &st.constraints,
                    &ok.not(),
                    e.span,
                    "postcondition failed",
                    nexus,
                )
            }
            Stmt::Assert(a) => {
                let ok = self.eval_bool(&a.expr, st, nexus)?;
                self.prove_implied(
                    Some(st),
                    &st.constraints,
                    &ok.not(),
                    a.span,
                    "assertion failed",
                    nexus,
                )
            }
            Stmt::If(i) => {
                let c = self.eval_bool(&i.cond, st, nexus)?;

                let mut st_then = st.clone();
                st_then.constraints.push(c.clone());
                self.check_block(&i.then_block, &mut st_then, nexus)?;

                let mut st_else = st.clone();
                st_else.constraints.push(c.not());
                if let Some(eb) = &i.else_block {
                    self.check_block(eb, &mut st_else, nexus)?;
                }

                // Conservative join: havoc vars mutated in either branch.
                let mut mutated: BTreeSet<String> = BTreeSet::new();
                collect_mutated_vars(&i.then_block, &mut mutated);
                if let Some(eb) = &i.else_block {
                    collect_mutated_vars(eb, &mut mutated);
                }
                for v in mutated {
                    if st.sorts.get(&v) == Some(&Sort::Int) {
                        let fresh = st.fresh_int(&format!("{v}_if"));
                        st.bind_int(&v, fresh, i.span);
                        if let Some((lo, hi)) = st.ranges.get(&v).copied() {
                            let rhs = st.ints.get(&v).cloned().expect("int");
                            st.assert_u32_in_range(&rhs, lo, hi);
                        }
                    }
                    if st.sorts.get(&v) == Some(&Sort::Bool) {
                        st.bind_bool(&v, Bool::new_const(self.ctx(), format!("{v}_if")), i.span);
                    }
                }

                Ok(())
            }
            Stmt::While(w) => {
                let _ = self.eval_bool(&w.cond, st, nexus)?;

                let inferred;
                let inv_expr = match &w.invariant {
                    Some(e) => e,
                    None => {
                        inferred = self.synthesize_invariant(w, st, nexus)?;
                        &inferred
                    }
                };

                self.check_while_with_invariant(w, st, inv_expr, nexus)
            }
            Stmt::FlowBlock(fb) => self.check_block(&fb.body, st, nexus),
            Stmt::Layout(lb) => self.check_block(&lb.body, st, nexus),
            Stmt::Render(rb) => self.check_block(&rb.body, st, nexus),
            _ => Ok(()),
        }
    }

    fn check_while_with_invariant(
        &mut self,
        w: &aura_ast::WhileStmt,
        st: &mut SymState<'static>,
        inv_expr: &Expr,
        nexus: &mut NexusContext,
    ) -> Result<(), VerifyError> {
        let inv0 = self.eval_bool_spec(inv_expr, st, nexus)?;
        self.prove_implied(
            Some(st),
            &st.constraints,
            &inv0.not(),
            inv_expr.span,
            "loop invariant base case failed",
            nexus,
        )?;

        let cond0 = self.eval_bool(&w.cond, st, nexus)?;

        // Inductive step: assume inv && cond, execute body, prove inv holds.
        let mut step = st.clone_for_step();
        step.constraints.push(inv0.clone());
        step.constraints.push(cond0.clone());

        if let Some(dec_expr) = &w.decreases {
            let d0 = self.eval_int_spec(dec_expr, &mut step, nexus)?;
            let zero = Int::from_i64(self.ctx(), 0);
            let nonneg0 = d0.ge(&zero);
            self.prove_implied(
                Some(&step),
                &step.constraints,
                &nonneg0.not(),
                dec_expr.span,
                "decreases measure may be negative",
                nexus,
            )?;

            self.check_block(&w.body, &mut step, nexus)?;

            let d1 = self.eval_int_spec(dec_expr, &mut step, nexus)?;
            let nonneg1 = d1.ge(&zero);
            self.prove_implied(
                Some(&step),
                &step.constraints,
                &nonneg1.not(),
                dec_expr.span,
                "decreases measure may be negative",
                nexus,
            )?;

            let dec_ok = d1.lt(&d0);
            self.prove_implied(
                Some(&step),
                &step.constraints,
                &dec_ok.not(),
                dec_expr.span,
                "loop termination check failed (decreases did not decrease)",
                nexus,
            )?;
        } else {
            self.check_block(&w.body, &mut step, nexus)?;
        }
        let inv1 = self.eval_bool_spec(inv_expr, &mut step, nexus)?;
        self.prove_implied(
            Some(&step),
            &step.constraints,
            &inv1.not(),
            inv_expr.span,
            "loop invariant inductive step failed",
            nexus,
        )?;

        // After-loop approximation: assume inv && !cond.
        st.constraints.push(inv0);
        st.constraints.push(cond0.not());

        // Conservative havoc of mutated vars.
        let mut mutated: BTreeSet<String> = BTreeSet::new();
        collect_mutated_vars(&w.body, &mut mutated);
        for v in mutated {
            if st.sorts.get(&v) == Some(&Sort::Int) {
                st.bind_int(&v, Int::new_const(self.ctx(), format!("{v}_after")), w.span);
            }
            if st.sorts.get(&v) == Some(&Sort::Bool) {
                st.bind_bool(&v, Bool::new_const(self.ctx(), format!("{v}_after")), w.span);
            }
        }

        Ok(())
    }

    fn synthesize_invariant(
        &mut self,
        w: &aura_ast::WhileStmt,
        st: &SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<Expr, VerifyError> {
        let mut candidates: Vec<Expr> = Vec::new();

        if let Some(tpl) = infer_invariant_from_cond(&w.cond, &w.body) {
            candidates.push(tpl);
        }

        // Always include a tautology fallback so loops can still verify when
        // the loop condition alone is sufficient for safety proofs.
        candidates.push(mk_true_expr(w.span));

        for cand in candidates {
            if self.try_invariant(w, st, &cand, nexus).is_ok() {
                return Ok(cand);
            }
        }

        // Synthesis UX: provide a couple of candidate templates in the diagnostic.
        let mut suggestions: Vec<String> = Vec::new();
        if let Some(tpl) = infer_invariant_from_cond(&w.cond, &w.body) {
            suggestions.push(format!(
                "Try adding: invariant {}",
                format_expr(&tpl)
            ));
        }
        suggestions.push("Try adding: invariant true (for a minimal invariant)
and/or strengthen it with bounds on mutated variables.".to_string());

        Err(VerifyError {
            message: "inductive verifier: could not infer a safe invariant; please add `invariant <expr>`".to_string(),
            span: w.span,
            model: None,
            meta: Some(DiagnosticMetadata {
                model: None,
                bindings: Vec::new(),
                typed_bindings: Vec::new(),
                related: Vec::new(),
                unsat_core: Vec::new(),
                hints: vec![
                    "Loop invariant synthesis is conservative; explicit invariants are often required.".to_string(),
                ],
                suggestions,
            }),
        })
    }

    fn try_invariant(
        &mut self,
        w: &aura_ast::WhileStmt,
        st: &SymState<'static>,
        inv_expr: &Expr,
        nexus: &mut NexusContext,
    ) -> Result<(), VerifyError> {
        // Validate candidate invariant without mutating the caller state.
        let mut st0 = st.clone();
        let inv0 = self.eval_bool_spec(inv_expr, &mut st0, nexus)?;
        self.prove_implied(
            Some(&st0),
            &st0.constraints,
            &inv0.not(),
            inv_expr.span,
            "loop invariant base case failed",
            nexus,
        )?;

        let cond0 = self.eval_bool(&w.cond, &mut st0, nexus)?;

        let mut step = st0.clone_for_step();
        step.constraints.push(inv0.clone());
        step.constraints.push(cond0.clone());
        self.check_block(&w.body, &mut step, nexus)?;
        let inv1 = self.eval_bool_spec(inv_expr, &mut step, nexus)?;
        self.prove_implied(
            Some(&step),
            &step.constraints,
            &inv1.not(),
            inv_expr.span,
            "loop invariant inductive step failed",
            nexus,
        )?;

        Ok(())
    }

    fn prove_implied(
        &mut self,
        st: Option<&SymState<'static>>,
        assumptions: &[Bool<'static>],
        negated_goal: &Bool<'static>,
        span: aura_ast::Span,
        message: &str,
        nexus: &mut NexusContext,
    ) -> Result<(), VerifyError> {
        let evidence = self.prove_implied_with_evidence(st, assumptions, negated_goal, span, message)?;

        // Successful proof: record a proof note so editor UX can render reasoning traces.
        // (Even if we only have partial evidence, this is valuable for explainability.)
        let related = evidence
            .core_related
            .into_iter()
            .map(|r| NexusRelated {
                span: r.span,
                message: r.message,
            })
            .collect::<Vec<_>>();

        record_proof(
            nexus,
            ProofNote {
                plugin: "aura-verify".to_string(),
                span,
                message: format!("Verified: {message}"),
                smt: None,
                related,
                kind: "verify.proved",
                mask: None,
                range: None,
                unsat_core: evidence.unsat_core,
                interpolant: evidence.interpolant,
            },
        );

        Ok(())
    }

    fn prove_implied_with_evidence(
        &mut self,
        st: Option<&SymState<'static>>,
        assumptions: &[Bool<'static>],
        negated_goal: &Bool<'static>,
        span: aura_ast::Span,
        message: &str,
    ) -> Result<ProveEvidence, VerifyError> {
        let ctx = self.ctx();

        // Check UNSAT of: assumptions AND negated_goal
        //
        // If incremental solving is enabled, we keep a long-lived solver and use push/pop
        // plus check-sat-assuming so we don't re-initialize Z3 for each obligation.
        let mut params = Params::new(ctx);
        params.set_u32("timeout", self.opts.timeout_ms);
        // Determinism: ensure Z3 doesn't use random seeds that vary by run.
        // This is especially important for CI reproducibility.
        params.set_u32("smt.random_seed", 0);
        params.set_u32("sat.random_seed", 0);
        // Best-effort: enable UNSAT core extraction.
        params.set_bool("unsat_core", true);

        let nonce = if self.opts.incremental_solver {
            // Important: grab the nonce before borrowing the warm solver.
            self.prover.next_nonce()
        } else {
            0
        };

        let mut local_solver: Option<Solver<'static>> = if self.opts.incremental_solver {
            None
        } else {
            Some(Solver::new(ctx))
        };
        let solver: &mut Solver<'static> = if self.opts.incremental_solver {
            // Warm solver lives in the prover so the LSP can reuse it across verify runs.
            self.prover.warm_solver()
        } else {
            local_solver.as_mut().expect("solver")
        };

        solver.set_params(&params);

        if self.opts.incremental_solver {
            solver.push();
        }

        // Track assertions so we can ask Z3 for an UNSAT core on success.
        // Note: Z3 will return the tracking literals (or assumption literals), so we
        // maintain a map back to readable SMT snippets and (best-effort) source spans.
        let mut label_to_smt: HashMap<String, String> = HashMap::new();
        let mut label_to_ast: HashMap<String, Bool<'static>> = HashMap::new();
        let mut label_to_origin: HashMap<String, RelatedInfo> = HashMap::new();
        let mut assumption_lits: Vec<Bool<'static>> = Vec::new();

        for (i, a) in assumptions.iter().enumerate() {
            let label = format!("assump_{nonce}_{i}");
            let track = Bool::new_const(ctx, label.clone());
            label_to_smt.insert(label.clone(), a.to_string());
            label_to_ast.insert(label.clone(), a.clone());

            // Prefer check-sat-assuming by guarding each fact with a boolean literal.
            // We still enable unsat_core so Z3 can return the minimal set of assumptions.
            solver.assert(&track.implies(a));
            assumption_lits.push(track.clone());

            if let Some(st) = st {
                if let Some((span0, kind)) = st.origin_constraints.get(&a.to_string()) {
                    label_to_origin.insert(
                        label.clone(),
                        RelatedInfo {
                            span: *span0,
                            message: format!("{kind} contributes to UNSAT core"),
                        },
                    );
                }
            }
        }

        let goal_label = format!("negated_goal_{nonce}");
        let goal_track = Bool::new_const(ctx, goal_label.clone());
        label_to_smt.insert(goal_label.clone(), negated_goal.to_string());
        label_to_ast.insert(goal_label.clone(), negated_goal.clone());

        solver.assert(&goal_track.implies(negated_goal));
        assumption_lits.push(goal_track.clone());
        label_to_origin.insert(
            goal_label.clone(),
            RelatedInfo {
                span,
                message: "negated goal (this assertion/ensures) contributes to UNSAT core".to_string(),
            },
        );

        let sat = solver.check_assumptions(&assumption_lits);

        let res = match sat {
            SatResult::Unsat => {
                let core_labels = solver
                    .get_unsat_core()
                    .into_iter()
                    .map(|b| b.to_string())
                    .collect::<Vec<_>>();

                let mut core_smt: Vec<String> = Vec::new();
                let mut core_related: Vec<RelatedInfo> = Vec::new();
                for lbl in &core_labels {
                    if let Some(smt) = label_to_smt.get(lbl) {
                        core_smt.push(smt.clone());
                    } else {
                        core_smt.push(lbl.clone());
                    }

                    if let Some(r) = label_to_origin.get(lbl) {
                        core_related.push(r.clone());
                    }
                }

                // Best-effort derived lemma ("interpolant-like"): conjunction of the core
                // assumptions (excluding the negated goal), but only if it still refutes
                // the negated goal.
                let mut interpolant: Option<String> = None;
                let mut core_assumptions: Vec<Bool<'static>> = Vec::new();
                for lbl in &core_labels {
                    if lbl == &goal_label {
                        continue;
                    }
                    if let Some(ast) = label_to_ast.get(lbl) {
                        core_assumptions.push(ast.clone());
                    }
                }

                if !core_assumptions.is_empty() {
                    let core_refs = core_assumptions.iter().collect::<Vec<_>>();
                    let lemma = Bool::and(ctx, &core_refs);

                    let check = Solver::new(ctx);
                    check.set_params(&params);
                    check.assert(&lemma);
                    check.assert(negated_goal);
                    if check.check() == SatResult::Unsat {
                        interpolant = Some(lemma.to_string());
                    }
                }

                Ok(ProveEvidence {
                    unsat_core: core_smt,
                    interpolant,
                    core_related,
                })
            }
            SatResult::Sat => {
                let model = solver.get_model();
                let model_text = model.as_ref().map(|m| m.to_string());
                let typed_bindings = match (st, model.as_ref()) {
                    (Some(st), Some(m)) => typed_bindings_from_model(st, m),
                    _ => Vec::new(),
                };

                let bindings = if !typed_bindings.is_empty() {
                    typed_bindings
                        .iter()
                        .map(|b| (b.name.clone(), b.value.clone()))
                        .collect::<Vec<_>>()
                } else {
                    model_text
                        .as_deref()
                        .map(extract_bindings_from_model_text)
                        .unwrap_or_default()
                };

                let mut related: Vec<RelatedInfo> = Vec::new();
                if let Some(st) = st {
                    for (name, value) in &bindings {
                        if let Some(def_span) = st.origins.get(name).copied() {
                            related.push(RelatedInfo {
                                span: def_span,
                                message: format!("{name} defined here (counterexample: {name} = {value})"),
                            });
                        }
                        if let Some(assign_span) = st.last_assign.get(name).copied() {
                            related.push(RelatedInfo {
                                span: assign_span,
                                message: format!("{name} last assigned here (counterexample: {name} = {value})"),
                            });
                        }
                    }
                }

                let mut msg = message.to_string();
                if !bindings.is_empty() {
                    let head = bindings
                        .iter()
                        .take(4)
                        .map(|(n, v)| format!("{n} = {v}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    msg = format!("{msg} (failed when {head})");
                }

                let mut hints: Vec<String> = Vec::new();
                let mut suggestions: Vec<String> = Vec::new();

                // Lightweight synthesis: suggest likely repairs.
                if message.contains("loop invariant") {
                    suggestions.push("Add or strengthen `invariant <expr>` on this while loop.".to_string());
                }
                if message.contains("decreases") {
                    suggestions.push("Add `decreases <expr>` and ensure it stays non-negative and strictly decreases.".to_string());
                }
                if message.contains("use-after-consume") {
                    suggestions.push("Avoid using a consumed capability; restructure control flow or re-acquire/clone if supported.".to_string());
                }
                if message.contains("out of bounds") || message.contains("overflow") {
                    suggestions.push("Add an `assert` guarding the index/value range, or strengthen `requires` preconditions.".to_string());
                }

                hints.push(format!("Negated goal (SAT): {negated_goal}"));

                Err(VerifyError {
                    message: msg,
                    span,
                    model: model_text.clone(),
                    meta: Some(DiagnosticMetadata {
                        model: model_text,
                        bindings,
                        typed_bindings,
                        related,
                        unsat_core: Vec::new(),
                        hints,
                        suggestions,
                    }),
                })
            }
            SatResult::Unknown => Err(VerifyError {
                message: format!("{message} (Z3 returned unknown)"),
                span,
                model: None,
                meta: None,
            }),
        };

        if self.opts.incremental_solver {
            solver.pop(1);
        }

        res
    }

    fn prove_u32_in_range(
        &mut self,
        st: &SymState<'static>,
        v: &Int<'static>,
        lo: u64,
        hi: u64,
        span: aura_ast::Span,
        message: &str,
        nexus: &mut NexusContext,
    ) -> Result<(), VerifyError> {
        let lo_i = Int::from_u64(self.ctx(), lo);
        let hi_i = Int::from_u64(self.ctx(), hi);
        let ok = Bool::and(self.ctx(), &[&v.ge(&lo_i), &v.le(&hi_i)]);
        self.prove_implied(Some(st), &st.constraints, &ok.not(), span, message, nexus)
    }

    fn require_alive(
        &self,
        st: &SymState<'static>,
        name: &str,
        use_span: aura_ast::Span,
        nexus: &mut NexusContext,
    ) -> Result<(), VerifyError> {
        if !st.alive.contains_key(name) {
            // If the name isn't tracked as a linear capability, allow it.
            return Ok(());
        }

        let is_alive = st.alive_concrete.get(name).copied().unwrap_or(true);
        if is_alive {
            // Keep a proof note for editor UX, even though this is a concrete check.
            record_proof(
                nexus,
                ProofNote {
                    plugin: "aura-verify".to_string(),
                    span: use_span,
                    message: format!("Verified: '{name}' is available (not consumed)"),
                    smt: st.alive.get(name).map(|b| b.to_string()),
                    related: Vec::new(),
                    kind: "capability.alive",
                    mask: None,
                    range: None,
                    unsat_core: Vec::new(),
                    interpolant: None,
                },
            );
            return Ok(());
        }

        let consumed_span = st
            .consumed_at
            .get(name)
            .copied()
            // Best-effort fallback: if we somehow lost the consume site,
            // still attach a related note so the diagnostic remains explainable.
            .or_else(|| st.last_assign.get(name).copied())
            .or_else(|| st.origins.get(name).copied());

        let mut meta = DiagnosticMetadata {
            model: None,
            bindings: Vec::new(),
            typed_bindings: Vec::new(),
            related: Vec::new(),
            unsat_core: Vec::new(),
            hints: Vec::new(),
            suggestions: vec![
                "Avoid using a consumed capability; restructure control flow or re-acquire/clone if supported."
                    .to_string(),
            ],
        };

        if let Some(consumed_span) = consumed_span {
            meta.related.push(RelatedInfo {
                span: consumed_span,
                message: format!("'{name}' consumed here"),
            });
        }

        Err(VerifyError {
            message: format!("use-after-consume: '{name}' is not available"),
            span: use_span,
            model: None,
            meta: Some(meta),
        })
    }

    fn nexus_diagnostic_to_verify_error(&self, d: NexusDiagnostic) -> VerifyError {
        let related = d
            .related
            .into_iter()
            .map(|r| RelatedInfo {
                span: r.span,
                message: r.message,
            })
            .collect::<Vec<_>>();

        VerifyError {
            message: format!("Sentinel Alert: {}: {}", d.plugin, d.message),
            span: d.span,
            model: d.model.clone(),
            meta: Some(DiagnosticMetadata {
                model: d.model,
                bindings: Vec::new(),
                typed_bindings: Vec::new(),
                related,
                unsat_core: Vec::new(),
                hints: Vec::new(),
                suggestions: Vec::new(),
            }),
        }
    }

    fn verify_error_to_nexus_diagnostic(&self, e: VerifyError) -> NexusDiagnostic {
        let mut d = NexusDiagnostic::new("aura-core", e.span, e.message);
        d.model = e.model;
        if let Some(meta) = e.meta {
            d.related = meta
                .related
                .into_iter()
                .map(|r| NexusRelated {
                    span: r.span,
                    message: r.message,
                })
                .collect::<Vec<_>>();
        }
        d
    }

    fn eval_any(
        &mut self,
        expr: &Expr,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<Value<'static>, VerifyError> {
        self.eval_any_with_mode(expr, st, nexus, EvalMode::Runtime)
    }

    fn eval_any_spec(
        &mut self,
        expr: &Expr,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<Value<'static>, VerifyError> {
        self.eval_any_with_mode(expr, st, nexus, EvalMode::Spec)
    }

    fn eval_any_with_mode(
        &mut self,
        expr: &Expr,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
        mode: EvalMode,
    ) -> Result<Value<'static>, VerifyError> {
        match self.infer_sort(expr, st)? {
            Sort::Bool => Ok(Value::Bool(self.eval_bool_with_mode(expr, st, nexus, mode)?)),
            Sort::Int => Ok(Value::Int(self.eval_int_with_mode(expr, st, nexus, mode)?)),
        }
    }

    fn infer_sort(&self, expr: &Expr, st: &mut SymState<'static>) -> Result<Sort, VerifyError> {
        match &expr.kind {
            ExprKind::IntLit(_) => Ok(Sort::Int),
            ExprKind::StringLit(_) => Ok(Sort::Int),
            ExprKind::StyleLit { .. } => Ok(Sort::Int),
            ExprKind::RecordLit { .. } => Err(VerifyError {
                message: "record literals are not supported in verifier yet".to_string(),
                span: expr.span,
                model: None,
                meta: None,
            }),
            ExprKind::Ident(id) => st
                .sorts
                .get(&id.node)
                .copied()
                .ok_or_else(|| VerifyError {
                    message: format!("unknown identifier '{}' in verifier", id.node),
                    span: id.span,
                    model: None,
                    meta: None,
                }),
            ExprKind::Unary { op, .. } => match op {
                aura_ast::UnaryOp::Neg => Ok(Sort::Int),
                aura_ast::UnaryOp::Not => Ok(Sort::Bool),
            },
            ExprKind::Binary { op, .. } => match op {
                aura_ast::BinOp::Add
                | aura_ast::BinOp::Sub
                | aura_ast::BinOp::Mul
                | aura_ast::BinOp::Div => Ok(Sort::Int),
                aura_ast::BinOp::Eq
                | aura_ast::BinOp::Ne
                | aura_ast::BinOp::Lt
                | aura_ast::BinOp::Gt
                | aura_ast::BinOp::Le
                | aura_ast::BinOp::Ge
                | aura_ast::BinOp::And
                | aura_ast::BinOp::Or => Ok(Sort::Bool),
            },
            ExprKind::Call { callee, .. } => {
                let name = callee_name_and_args(callee, &[]).0;
                match name.as_str() {
                    "tensor.new" | "tensor.len" | "tensor.get" | "tensor.set" => Ok(Sort::Int),
                    "io.println" => Ok(Sort::Int),
                    _ => Ok(Sort::Int),
                }
            }
            ExprKind::Lambda { .. } => Ok(Sort::Int),
            ExprKind::Member { .. } => Err(VerifyError {
                message: "unexpected member expression in verifier".to_string(),
                span: expr.span,
                model: None,
                meta: None,
            }),
            ExprKind::Flow { right, .. } => {
                // Conservative: treat flow result sort as the RHS sort.
                self.infer_sort(right, st)
            }
            ExprKind::ForAll { .. } | ExprKind::Exists { .. } => Ok(Sort::Bool),
        }
    }

    fn eval_bool(
        &mut self,
        expr: &Expr,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<Bool<'static>, VerifyError> {
        self.eval_bool_with_mode(expr, st, nexus, EvalMode::Runtime)
    }

    fn eval_bool_spec(
        &mut self,
        expr: &Expr,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<Bool<'static>, VerifyError> {
        self.eval_bool_with_mode(expr, st, nexus, EvalMode::Spec)
    }

    fn eval_bool_with_mode(
        &mut self,
        expr: &Expr,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
        mode: EvalMode,
    ) -> Result<Bool<'static>, VerifyError> {
        match &expr.kind {
            ExprKind::Unary { op: aura_ast::UnaryOp::Not, expr: inner } => {
                Ok(self.eval_bool_with_mode(inner, st, nexus, mode)?.not())
            }
            ExprKind::Binary { left, op, right } => match op {
                aura_ast::BinOp::And => {
                    let l = self.eval_bool_with_mode(left, st, nexus, mode)?;
                    let r = self.eval_bool_with_mode(right, st, nexus, mode)?;
                    Ok(Bool::and(self.ctx(), &[&l, &r]))
                }
                aura_ast::BinOp::Or => {
                    let l = self.eval_bool_with_mode(left, st, nexus, mode)?;
                    let r = self.eval_bool_with_mode(right, st, nexus, mode)?;
                    Ok(Bool::or(self.ctx(), &[&l, &r]))
                }
                aura_ast::BinOp::Eq
                | aura_ast::BinOp::Ne
                | aura_ast::BinOp::Lt
                | aura_ast::BinOp::Gt
                | aura_ast::BinOp::Le
                | aura_ast::BinOp::Ge => {
                    let l = self.eval_int_with_mode(left, st, nexus, mode)?;
                    let r = self.eval_int_with_mode(right, st, nexus, mode)?;
                    let b = match op {
                        aura_ast::BinOp::Eq => l._eq(&r),
                        aura_ast::BinOp::Ne => l._eq(&r).not(),
                        aura_ast::BinOp::Lt => l.lt(&r),
                        aura_ast::BinOp::Gt => l.gt(&r),
                        aura_ast::BinOp::Le => l.le(&r),
                        aura_ast::BinOp::Ge => l.ge(&r),
                        _ => unreachable!(),
                    };
                    Ok(b)
                }
                other => Err(VerifyError {
                    message: format!("expected boolean operator, got {other:?}"),
                    span: expr.span,
                    model: None,
                    meta: None,
                }),
            },
            ExprKind::Ident(id) => {
                if mode == EvalMode::Runtime {
                    self.require_alive(st, &id.node, id.span, nexus)?;
                }
                st.bools
                    .get(&id.node)
                    .cloned()
                    .ok_or_else(|| VerifyError {
                        message: format!("'{0}' is not a bool in verifier", id.node),
                        span: id.span,
                        model: None,
                        meta: None,
                    })
            }
            ExprKind::ForAll { binders, body } | ExprKind::Exists { binders, body } => {
                if !self.opts.allow_quantifiers {
                    return Err(VerifyError {
                        message: format!(
                            "quantifiers are disabled under SMT profile {:?}; use --smt-profile thorough",
                            self.opts.profile
                        ),
                        span: expr.span,
                        model: None,
                        meta: None,
                    });
                }
                if binders.len() > self.opts.max_quant_binders {
                    return Err(VerifyError {
                        message: format!(
                            "too many quantifier binders (max {})",
                            self.opts.max_quant_binders
                        ),
                        span: expr.span,
                        model: None,
                        meta: None,
                    });
                }

                let mut inner = st.clone();
                let mut bounds: Vec<Dynamic<'static>> = Vec::new();
                for (i, b) in binders.iter().enumerate() {
                    let nm = format!("{}_q{}", b.name.node, i);
                    let is_bool = b
                        .ty
                        .as_ref()
                        .is_some_and(|t| matches!(t.name.node.as_str(), "bool" | "Bool"));
                    if is_bool {
                        let v = Bool::new_const(self.ctx(), nm);
                        inner.bind_bool(&b.name.node, v.clone(), b.span);
                        bounds.push(v.into());
                    } else {
                        let v = Int::new_const(self.ctx(), nm);
                        inner.bind_int(&b.name.node, v.clone(), b.span);
                        bounds.push(v.into());
                    }
                }

                let body_b = self.eval_bool_with_mode(body, &mut inner, nexus, mode)?;
                let bound_refs: Vec<&dyn Ast<'static>> = bounds
                    .iter()
                    .map(|d| d as &dyn Ast<'static>)
                    .collect();

                let q = match &expr.kind {
                    ExprKind::ForAll { .. } => z3::ast::forall_const(self.ctx(), &bound_refs, &[], &body_b),
                    ExprKind::Exists { .. } => z3::ast::exists_const(self.ctx(), &bound_refs, &[], &body_b),
                    _ => unreachable!(),
                };
                Ok(q)
            }
            _ => Err(VerifyError {
                message: "unsupported boolean expression in verifier".to_string(),
                span: expr.span,
                model: None,
                meta: None,
            }),
        }
    }

    fn eval_int(
        &mut self,
        expr: &Expr,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<Int<'static>, VerifyError> {
        self.eval_int_with_mode(expr, st, nexus, EvalMode::Runtime)
    }

    fn eval_int_spec(
        &mut self,
        expr: &Expr,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
    ) -> Result<Int<'static>, VerifyError> {
        self.eval_int_with_mode(expr, st, nexus, EvalMode::Spec)
    }

    fn eval_int_with_mode(
        &mut self,
        expr: &Expr,
        st: &mut SymState<'static>,
        nexus: &mut NexusContext,
        mode: EvalMode,
    ) -> Result<Int<'static>, VerifyError> {
        match &expr.kind {
            ExprKind::IntLit(n) => Ok(Int::from_u64(self.ctx(), *n)),
            ExprKind::StyleLit { .. } => {
                // UI styles are not part of the safety proof today; treat as an uninterpreted value.
                Ok(Int::from_u64(self.ctx(), 0))
            }
            ExprKind::Ident(id) => {
                if mode == EvalMode::Runtime {
                    self.require_alive(st, &id.node, id.span, nexus)?;
                }
                st.ints
                    .get(&id.node)
                    .cloned()
                    .ok_or_else(|| VerifyError {
                        message: format!("'{0}' is not an int in verifier", id.node),
                        span: id.span,
                        model: None,
                        meta: None,
                    })
            }
            ExprKind::Unary {
                op: aura_ast::UnaryOp::Neg,
                expr: inner,
            } => Ok(Int::from_i64(self.ctx(), 0) - self.eval_int_with_mode(inner, st, nexus, mode)?),
            ExprKind::Binary { left, op, right } => match op {
                aura_ast::BinOp::Add => Ok(self.eval_int_with_mode(left, st, nexus, mode)? + self.eval_int_with_mode(right, st, nexus, mode)?),
                aura_ast::BinOp::Sub => Ok(self.eval_int_with_mode(left, st, nexus, mode)? - self.eval_int_with_mode(right, st, nexus, mode)?),
                aura_ast::BinOp::Mul => Ok(self.eval_int_with_mode(left, st, nexus, mode)? * self.eval_int_with_mode(right, st, nexus, mode)?),
                aura_ast::BinOp::Div => Ok(self.eval_int_with_mode(left, st, nexus, mode)? / self.eval_int_with_mode(right, st, nexus, mode)?),
                other => Err(VerifyError {
                    message: format!("expected integer operator, got {other:?}"),
                    span: expr.span,
                    model: None,
                    meta: None,
                }),
            },
            ExprKind::Call { callee, args, .. } => {
                let (name, all_args) = callee_name_and_args(callee, args);
                match name.as_str() {
                    "tensor.new" => {
                        if all_args.len() != 1 {
                            return Err(VerifyError {
                                message: "tensor.new expects 1 arg".to_string(),
                                span: expr.span,
                                model: None,
                                meta: None,
                            });
                        }
                        let len = self.eval_int_with_mode(all_args[0], st, nexus, mode)?;

                        let h = st.fresh_int("tensor");

                        // u32-ish bounds.
                        let lo = Int::from_u64(self.ctx(), 0);
                        let hi = Int::from_u64(self.ctx(), 0xFFFF_FFFF);
                        st.constraints.push(h.ge(&lo));
                        st.constraints.push(h.le(&hi));

                        // Constrain tensor_len(h) == len.
                        let f_len = z3::FuncDecl::new(
                            self.ctx(),
                            "tensor_len",
                            &[&z3::Sort::int(self.ctx())],
                            &z3::Sort::int(self.ctx()),
                        );
                        let len_of_h = f_len.apply(&[&h]).as_int().expect("int");
                        st.constraints.push(len_of_h._eq(&len));
                        Ok(h)
                    }
                    "tensor.len" => {
                        if all_args.len() != 1 {
                            return Err(VerifyError {
                                message: "tensor.len expects 1 arg".to_string(),
                                span: expr.span,
                                model: None,
                                meta: None,
                            });
                        }
                        let t = self.eval_int_with_mode(all_args[0], st, nexus, mode)?;
                        let f = z3::FuncDecl::new(
                            self.ctx(),
                            "tensor_len",
                            &[&z3::Sort::int(self.ctx())],
                            &z3::Sort::int(self.ctx()),
                        );
                        Ok(f.apply(&[&t]).as_int().expect("int"))
                    }
                    "tensor.get" => {
                        if all_args.len() != 2 {
                            return Err(VerifyError {
                                message: "tensor.get expects 2 args".to_string(),
                                span: expr.span,
                                model: None,
                                meta: None,
                            });
                        }
                        let t = self.eval_int_with_mode(all_args[0], st, nexus, mode)?;
                        let idx = self.eval_int_with_mode(all_args[1], st, nexus, mode)?;
                        let f_len = z3::FuncDecl::new(
                            self.ctx(),
                            "tensor_len",
                            &[&z3::Sort::int(self.ctx())],
                            &z3::Sort::int(self.ctx()),
                        );
                        let len = f_len.apply(&[&t]).as_int().expect("int");
                        let ok = idx.lt(&len);

                        // Prove: constraints => idx < len
                        self.prove_implied(
                            Some(st),
                            &st.constraints,
                            &ok.not(),
                            all_args[1].span,
                            "tensor access may be out of bounds",
                            nexus,
                        )?;

                        let f_get = z3::FuncDecl::new(
                            self.ctx(),
                            "tensor_get",
                            &[&z3::Sort::int(self.ctx()), &z3::Sort::int(self.ctx())],
                            &z3::Sort::int(self.ctx()),
                        );
                        Ok(f_get.apply(&[&t, &idx]).as_int().expect("int"))
                    }
                    "tensor.set" => {
                        if all_args.len() != 3 {
                            return Err(VerifyError {
                                message: "tensor.set expects 3 args".to_string(),
                                span: expr.span,
                                model: None,
                                meta: None,
                            });
                        }
                        let t = self.eval_int_with_mode(all_args[0], st, nexus, mode)?;
                        let idx = self.eval_int_with_mode(all_args[1], st, nexus, mode)?;
                        let _val = self.eval_int_with_mode(all_args[2], st, nexus, mode)?;

                        let f_len = z3::FuncDecl::new(
                            self.ctx(),
                            "tensor_len",
                            &[&z3::Sort::int(self.ctx())],
                            &z3::Sort::int(self.ctx()),
                        );
                        let len = f_len.apply(&[&t]).as_int().expect("int");
                        let ok = idx.lt(&len);

                        self.prove_implied(
                            Some(st),
                            &st.constraints,
                            &ok.not(),
                            all_args[1].span,
                            "tensor access may be out of bounds",
                            nexus,
                        )?;

                        Ok(Int::from_u64(self.ctx(), 0))
                    }
                    "io.println" => {
                        // Z3-verified boundary check (prototype): only allow string literals.
                        if all_args.len() != 1 {
                            return Err(VerifyError {
                                message: "io.println expects 1 arg".to_string(),
                                span: expr.span,
                                model: None,
                                meta: None,
                            });
                        }
                        match &all_args[0].kind {
                            ExprKind::StringLit(s) => {
                                const MAX: u64 = 4096;
                                let len = s.as_bytes().len() as u64;

                                let solver = Solver::new(self.ctx());
                                let mut params = Params::new(self.ctx());
                                params.set_u32("timeout", self.opts.timeout_ms);
                                params.set_u32("smt.random_seed", 0);
                                params.set_u32("sat.random_seed", 0);
                                solver.set_params(&params);
                                for a in &st.constraints {
                                    solver.assert(a);
                                }

                                let len_i = Int::from_u64(self.ctx(), len);
                                let max_i = Int::from_u64(self.ctx(), MAX);
                                solver.assert(&len_i.gt(&max_i));
                                match solver.check() {
                                    SatResult::Unsat => Ok(Int::from_u64(self.ctx(), 0)),
                                    SatResult::Sat => Err(VerifyError {
                                        message: format!(
                                            "io.println boundary check failed: string length {len} exceeds MAX={MAX}"
                                        ),
                                        span: all_args[0].span,
                                        model: solver.get_model().map(|m| m.to_string()),
                                        meta: None,
                                    }),
                                    SatResult::Unknown => Err(VerifyError {
                                        message: "io.println boundary check returned unknown".to_string(),
                                        span: all_args[0].span,
                                        model: None,
                                        meta: None,
                                    }),
                                }
                            }
                            _ => Err(VerifyError {
                                message: "io.println expects a string literal".to_string(),
                                span: all_args[0].span,
                                model: None,
                                meta: None,
                            }),
                        }
                    }
                    _other => {
                        // Open-theory hook: let Nexus plugins model unknown calls.
                        let call = Z3Call {
                            name: name.as_str(),
                            args: all_args.as_slice(),
                            span: expr.span,
                        };
                        let mut env = EngineCallEnv {
                            engine: self,
                            st,
                            nexus,
                        };
                        let plugins = env.engine.plugins;
                        if let Some(res) = plugins.try_z3_int_call(&call, &mut env) {
                            return match res {
                                Ok(v) => Ok(v),
                                Err(d) => Err(env.engine.nexus_diagnostic_to_verify_error(d)),
                            };
                        }

                        if name.starts_with("hw.") || name.starts_with("ai.") {
                            return Err(VerifyError {
                                message: format!("no Nexus plugin handled call '{name}'"),
                                span: expr.span,
                                model: None,
                                meta: None,
                            });
                        }

                        // Unknown extern call: model as an uninterpreted fresh u32.
                        let v = st.fresh_int("call");
                        let lo = Int::from_u64(self.ctx(), 0);
                        let hi = Int::from_u64(self.ctx(), 0xFFFF_FFFF);
                        st.constraints.push(v.ge(&lo));
                        st.constraints.push(v.le(&hi));
                        Ok(v)
                    }
                }
            }
            ExprKind::StringLit(_) => {
                // Strings are only supported as arguments to io.println in this verifier pass.
                Ok(Int::from_u64(self.ctx(), 0))
            }
            ExprKind::Lambda { .. } => {
                // Lambdas can appear in UI trees (callbacks). They are not modeled in Z3.
                Ok(Int::from_u64(self.ctx(), 0))
            }
            ExprKind::Flow { left, op: _op, right } => {
                // Match lowering: `left ~> callee(args...)` becomes a call with `left` as arg0.
                let ExprKind::Call { callee, args, .. } = &right.kind else {
                    // Fallback: ignore LHS and evaluate RHS.
                    let _ = self.eval_any_with_mode(left, st, nexus, mode)?;
                    return self.eval_int_with_mode(right, st, nexus, mode);
                };

                let (name, base_args) = callee_name_and_args(callee, args);

                // Open-theory hook: let Nexus plugins model the lowered call.
                let mut arg_exprs: Vec<&Expr> = Vec::with_capacity(base_args.len() + 1);
                arg_exprs.push(left.as_ref());
                arg_exprs.extend(base_args.iter().copied());
                let call = Z3Call {
                    name: name.as_str(),
                    args: arg_exprs.as_slice(),
                    span: expr.span,
                };
                let mut env = EngineCallEnv {
                    engine: self,
                    st,
                    nexus,
                };
                let plugins = env.engine.plugins;
                if let Some(res) = plugins.try_z3_int_call(&call, &mut env) {
                    return match res {
                        Ok(v) => Ok(v),
                        Err(d) => Err(env.engine.nexus_diagnostic_to_verify_error(d)),
                    };
                }

                if name.starts_with("hw.") || name.starts_with("ai.") {
                    return Err(VerifyError {
                        message: format!("no Nexus plugin handled call '{name}'"),
                        span: expr.span,
                        model: None,
                        meta: None,
                    });
                }

                let recv = self.eval_int_with_mode(left, st, nexus, mode)?;

                // Build a value list: [recv] + base_args
                let mut vals: Vec<Int<'static>> = Vec::with_capacity(base_args.len() + 1);
                vals.push(recv);
                for a in base_args {
                    vals.push(self.eval_int_with_mode(a, st, nexus, mode)?);
                }

                // Dispatch through the existing call handlers we care about.
                let out = match name.as_str() {
                    "tensor.set" => {
                        if vals.len() != 3 {
                            return Err(VerifyError { message: "tensor.set expects 3 args".to_string(), span: expr.span, model: None, meta: None });
                        }
                        let t = vals[0].clone();
                        let idx = vals[1].clone();
                        let f_len = z3::FuncDecl::new(self.ctx(), "tensor_len", &[&z3::Sort::int(self.ctx())], &z3::Sort::int(self.ctx()));
                        let len = f_len.apply(&[&t]).as_int().expect("int");
                        let ok = idx.lt(&len);
                        self.prove_implied(
                            Some(st),
                            &st.constraints,
                            &ok.not(),
                            right.span,
                            "tensor access may be out of bounds",
                            nexus,
                        )?;
                        Ok(Int::from_u64(self.ctx(), 0))
                    }
                    "tensor.get" => {
                        if vals.len() != 2 {
                            return Err(VerifyError { message: "tensor.get expects 2 args".to_string(), span: expr.span, model: None, meta: None });
                        }
                        let t = vals[0].clone();
                        let idx = vals[1].clone();
                        let f_len = z3::FuncDecl::new(self.ctx(), "tensor_len", &[&z3::Sort::int(self.ctx())], &z3::Sort::int(self.ctx()));
                        let len = f_len.apply(&[&t]).as_int().expect("int");
                        let ok = idx.lt(&len);
                        self.prove_implied(
                            Some(st),
                            &st.constraints,
                            &ok.not(),
                            right.span,
                            "tensor access may be out of bounds",
                            nexus,
                        )?;
                        let f_get = z3::FuncDecl::new(self.ctx(), "tensor_get", &[&z3::Sort::int(self.ctx()), &z3::Sort::int(self.ctx())], &z3::Sort::int(self.ctx()));
                        Ok(f_get.apply(&[&t, &idx]).as_int().expect("int"))
                    }
                    _ => {
                        // Unknown: fresh value.
                        let v = st.fresh_int("call");
                        let lo = Int::from_u64(self.ctx(), 0);
                        let hi = Int::from_u64(self.ctx(), 0xFFFF_FFFF);
                        st.constraints.push(v.ge(&lo));
                        st.constraints.push(v.le(&hi));
                        Ok(v)
                    }
                };

                // Linear capability transfer: consume the LHS if it's a named value.
                if mode == EvalMode::Runtime {
                    if let ExprKind::Ident(id) = &left.kind {
                        st.consume_if_tracked(&id.node, expr.span);
                    }
                }

                out
            }
            _ => Err(VerifyError {
                message: "unsupported integer expression in verifier".to_string(),
                span: expr.span,
                model: None,
                meta: None,
            }),
        }
    }
}

#[cfg(feature = "z3")]
struct EngineCallEnv<'e, 'p, 'plug, P>
where
    P: Z3PluginDispatch,
{
    engine: &'e mut Z3Engine<'p, 'plug, P>,
    st: &'e mut SymState<'static>,
    nexus: &'e mut NexusContext,
}

#[cfg(feature = "z3")]
impl<'e, 'p, 'plug, P> Z3CallEnv<'static> for EngineCallEnv<'e, 'p, 'plug, P>
where
    P: Z3PluginDispatch,
{
    fn ctx(&self) -> &'static z3::Context {
        self.engine.ctx()
    }

    fn nexus(&mut self) -> &mut NexusContext {
        self.nexus
    }

    fn fresh_int(&mut self, prefix: &str) -> Int<'static> {
        self.st.fresh_int(prefix)
    }

    fn push_constraint(&mut self, c: Bool<'static>) {
        self.st.constraints.push(c);
    }

    fn prove_implied(
        &mut self,
        not_condition: Bool<'static>,
        span: aura_ast::Span,
        message: &str,
    ) -> Result<(), NexusDiagnostic> {
        let negated_goal = not_condition;
        self.engine
            .prove_implied(
                Some(&*self.st),
                &self.st.constraints,
                &negated_goal,
                span,
                message,
                self.nexus,
            )
            .map_err(|e| self.engine.verify_error_to_nexus_diagnostic(e))
    }

    fn eval_int(&mut self, expr: &Expr) -> Result<Int<'static>, NexusDiagnostic> {
        self.engine
            .eval_int(expr, self.st, self.nexus)
            .map_err(|e| self.engine.verify_error_to_nexus_diagnostic(e))
    }

    fn eval_bool(&mut self, expr: &Expr) -> Result<Bool<'static>, NexusDiagnostic> {
        self.engine
            .eval_bool(expr, self.st, self.nexus)
            .map_err(|e| self.engine.verify_error_to_nexus_diagnostic(e))
    }
}

#[cfg(feature = "z3")]
#[derive(Clone)]
struct SymState<'ctx> {
    ctx: &'ctx z3::Context,
    ints: std::collections::HashMap<String, Int<'ctx>>,
    bools: std::collections::HashMap<String, Bool<'ctx>>,
    sorts: std::collections::HashMap<String, Sort>,
    constraints: Vec<Bool<'ctx>>,

    // Best-effort provenance for constraints that come directly from user-visible specs
    // (requires/assume), so we can map UNSAT cores back to source spans.
    origin_constraints: std::collections::HashMap<String, (aura_ast::Span, &'static str)>,
    ranges: std::collections::HashMap<String, (u64, u64)>,

    // Linear capability state: versioned alive flags.
    alive: std::collections::HashMap<String, Bool<'ctx>>,
    alive_concrete: std::collections::HashMap<String, bool>,
    consumed_at: std::collections::HashMap<String, aura_ast::Span>,

    origins: std::collections::HashMap<String, aura_ast::Span>,
    last_assign: std::collections::HashMap<String, aura_ast::Span>,

    // Shape contracts (prototype): keyed by handle's Z3 AST string (stable within a run).
    tensor_shapes_by_handle: std::collections::HashMap<String, Vec<u64>>,
    fresh: u64,
}

#[cfg(feature = "z3")]
impl<'ctx> SymState<'ctx> {
    fn new(ctx: &'ctx z3::Context) -> Self {
        Self {
            ctx,
            ints: std::collections::HashMap::new(),
            bools: std::collections::HashMap::new(),
            sorts: std::collections::HashMap::new(),
            constraints: Vec::new(),
            origin_constraints: std::collections::HashMap::new(),
            ranges: std::collections::HashMap::new(),

            alive: std::collections::HashMap::new(),
            alive_concrete: std::collections::HashMap::new(),
            consumed_at: std::collections::HashMap::new(),

            origins: std::collections::HashMap::new(),
            last_assign: std::collections::HashMap::new(),
            tensor_shapes_by_handle: std::collections::HashMap::new(),
            fresh: 0,
        }
    }

    fn note_origin_constraint(&mut self, kind: &'static str, span: aura_ast::Span, b: &Bool<'ctx>) {
        self.origin_constraints
            .insert(b.to_string(), (span, kind));
    }

    fn clone_for_step(&self) -> Self {
        self.clone()
    }

    fn fresh_int(&mut self, prefix: &str) -> Int<'ctx> {
        let n = self.fresh;
        self.fresh += 1;
        Int::new_const(self.ctx, format!("{prefix}{n}"))
    }

    fn fresh_bool(&mut self, prefix: &str) -> Bool<'ctx> {
        let n = self.fresh;
        self.fresh += 1;
        Bool::new_const(self.ctx, format!("{prefix}{n}"))
    }

    fn set_alive(&mut self, name: &str, is_alive: bool, span: aura_ast::Span) {
        let b = self.fresh_bool(&format!("alive_{name}_"));
        self.constraints
            .push(b._eq(&Bool::from_bool(self.ctx, is_alive)));
        self.alive.insert(name.to_string(), b);
        self.alive_concrete.insert(name.to_string(), is_alive);
        if !is_alive {
            self.consumed_at.insert(name.to_string(), span);
        } else {
            // Re-acquisition / rebind: clear any prior consume site.
            self.consumed_at.remove(name);
        }
    }

    fn consume_if_tracked(&mut self, name: &str, span: aura_ast::Span) {
        if self.alive.contains_key(name) {
            self.set_alive(name, false, span);
        }
    }

    fn note_range(&mut self, name: &str, lo: u64, hi: u64) {
        self.ranges.insert(name.to_string(), (lo, hi));
    }

    fn assert_u32_in_range(&mut self, v: &Int<'ctx>, lo: u64, hi: u64) {
        let lo_i = Int::from_u64(self.ctx, lo);
        let hi_i = Int::from_u64(self.ctx, hi);
        self.constraints.push(v.ge(&lo_i));
        self.constraints.push(v.le(&hi_i));
    }

    fn define_int(&mut self, name: &str, span: aura_ast::Span) -> Result<(), VerifyError> {
        let v = Int::new_const(self.ctx, name);
        self.sorts.insert(name.to_string(), Sort::Int);
        self.ints.insert(name.to_string(), v.clone());
        self.origins.entry(name.to_string()).or_insert(span);
        self.last_assign.insert(name.to_string(), span);
        self.set_alive(name, true, span);
        // u32 bounds (prototype modeling): 0 <= v <= 2^32-1
        let lo = Int::from_u64(self.ctx, 0);
        let hi = Int::from_u64(self.ctx, 0xFFFF_FFFF);
        self.constraints.push(v.ge(&lo));
        self.constraints.push(v.le(&hi));
        Ok(())
    }

    fn bind_int(&mut self, name: &str, v: Int<'ctx>, span: aura_ast::Span) {
        self.sorts.insert(name.to_string(), Sort::Int);
        self.ints.insert(name.to_string(), v);
        self.last_assign.insert(name.to_string(), span);
        self.set_alive(name, true, span);
    }

    fn bind_bool(&mut self, name: &str, v: Bool<'ctx>, span: aura_ast::Span) {
        self.sorts.insert(name.to_string(), Sort::Bool);
        self.bools.insert(name.to_string(), v);
        self.last_assign.insert(name.to_string(), span);
        self.set_alive(name, true, span);
    }

    fn note_tensor_shape(&mut self, tensor: &Int<'ctx>, dims: &[u64]) {
        // Record for diagnostics.
        self.tensor_shapes_by_handle
            .insert(tensor.to_string(), dims.to_vec());

        // Assert tensor_dim(t, i) == dims[i]
        let f_dim = z3::FuncDecl::new(
            self.ctx,
            "tensor_dim",
            &[&z3::Sort::int(self.ctx), &z3::Sort::int(self.ctx)],
            &z3::Sort::int(self.ctx),
        );
        for (i, d) in dims.iter().copied().enumerate() {
            let idx = Int::from_u64(self.ctx, i as u64);
            let dim = f_dim
                .apply(&[tensor, &idx])
                .as_int()
                .expect("int");
            let dv = Int::from_u64(self.ctx, d);
            self.constraints.push(dim._eq(&dv));
        }

        // Optionally relate tensor_len(t) to product(dims).
        let mut prod: u64 = 1;
        for d in dims {
            prod = match prod.checked_mul(*d) {
                Some(v) => v,
                None => return,
            };
        }
        let f_len = z3::FuncDecl::new(
            self.ctx,
            "tensor_len",
            &[&z3::Sort::int(self.ctx)],
            &z3::Sort::int(self.ctx),
        );
        let len = f_len.apply(&[tensor]).as_int().expect("int");
        let p = Int::from_u64(self.ctx, prod);
        self.constraints.push(len._eq(&p));
    }

}

#[cfg(all(test, feature = "z3"))]
mod typed_binding_tests {
    use super::*;

    #[test]
    fn typed_bindings_extract_u32_ranges_and_bools() {
        let mut cfg = z3::Config::new();
        cfg.set_model_generation(true);
        let ctx: &'static z3::Context = Box::leak(Box::new(z3::Context::new(&cfg)));

        let mut st = SymState::new(ctx);

        let x = Int::new_const(ctx, "x");
        st.sorts.insert("x".to_string(), Sort::Int);
        st.ints.insert("x".to_string(), x.clone());
        st.ranges.insert("x".to_string(), (0, 10));

        let b = Bool::new_const(ctx, "b");
        st.sorts.insert("b".to_string(), Sort::Bool);
        st.bools.insert("b".to_string(), b.clone());

        let solver = Solver::new(ctx);
        solver.assert(&x._eq(&Int::from_u64(ctx, 7)));
        solver.assert(&b._eq(&Bool::from_bool(ctx, true)));

        assert_eq!(solver.check(), SatResult::Sat);
        let model = solver.get_model().expect("model");

        let typed = typed_bindings_from_model(&st, &model);
        let mut by_name = std::collections::HashMap::<String, TypedBinding>::new();
        for t in typed {
            by_name.insert(t.name.clone(), t);
        }

        let tx = by_name.get("x").expect("x");
        assert_eq!(tx.aura_type, "u32[0..10]");
        assert_eq!(tx.value, "7");

        let tb = by_name.get("b").expect("b");
        assert_eq!(tb.aura_type, "bool");
        assert_eq!(tb.value, "true");
    }
}

#[cfg(feature = "z3")]
enum Value<'ctx> {
    Int(Int<'ctx>),
    Bool(Bool<'ctx>),
}

#[cfg(feature = "z3")]
fn expr_to_callee_name(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Ident(id) => id.node.clone(),
        ExprKind::Member { base, member } => {
            // Prototype: treat `.len/.get/.set` as tensor instance methods.
            if matches!(member.node.as_str(), "len" | "get" | "set")
                && !matches!(&base.kind, ExprKind::Ident(id) if id.node == "tensor")
            {
                format!("tensor.{}", member.node)
            } else if member.node == "infer"
                && !matches!(&base.kind, ExprKind::Ident(id) if id.node == "ai")
            {
                "ai.infer".to_string()
            } else {
                format!("{}.{}", expr_to_callee_name(base), member.node)
            }
        }
        _ => "<unknown>".to_string(),
    }
}

#[cfg(feature = "z3")]
fn callee_name_and_args<'a>(callee: &'a Expr, args: &'a [CallArg]) -> (String, Vec<&'a Expr>) {
    match &callee.kind {
        ExprKind::Member { base, member }
            if matches!(member.node.as_str(), "len" | "get" | "set")
                && !matches!(&base.kind, ExprKind::Ident(id) if id.node == "tensor") =>
        {
            let mut v = Vec::with_capacity(args.len() + 1);
            v.push(base.as_ref());
            for a in args {
                v.push(call_arg_value(a));
            }
            (format!("tensor.{}", member.node), v)
        }
        ExprKind::Member { base, member }
            if member.node == "infer" && !matches!(&base.kind, ExprKind::Ident(id) if id.node == "ai") =>
        {
            let mut v = Vec::with_capacity(args.len() + 1);
            v.push(base.as_ref());
            for a in args {
                v.push(call_arg_value(a));
            }
            ("ai.infer".to_string(), v)
        }
        _ => {
            let mut v = Vec::with_capacity(args.len());
            for a in args {
                v.push(call_arg_value(a));
            }
            (expr_to_callee_name(callee), v)
        }
    }
}

#[cfg(feature = "z3")]
fn call_arg_value(arg: &CallArg) -> &Expr {
    match arg {
        CallArg::Positional(e) => e,
        CallArg::Named { value, .. } => value,
    }
}

#[cfg(feature = "z3")]
fn tensor_shape_from_type_ref(tr: Option<&aura_ast::TypeRef>) -> Option<Vec<u64>> {
    let tr = tr?;
    if tr.name.node != "Tensor" {
        return None;
    }

    // Tensor<Elem, [d0, d1, ...]>
    if tr.args.len() >= 2 {
        if let TypeArg::Shape(dims) = &tr.args[1] {
            return Some(dims.clone());
        }
    }
    None
}

#[cfg(feature = "z3")]
fn range_from_type_ref(tr: Option<&aura_ast::TypeRef>) -> Option<(u64, u64)> {
    let tr = tr?;
    if tr.name.node != "u32" {
        return None;
    }
    let r = tr.range.as_ref()?;
    let lo = match r.lo.kind {
        aura_ast::ExprKind::IntLit(n) => n,
        _ => return None,
    };
    let hi = match r.hi.kind {
        aura_ast::ExprKind::IntLit(n) => n,
        _ => return None,
    };
    Some((lo, hi))
}

#[cfg(feature = "z3")]
fn mk_true_expr(span: aura_ast::Span) -> Expr {
    let z0 = Expr {
        span,
        kind: ExprKind::IntLit(0),
    };
    Expr {
        span,
        kind: ExprKind::Binary {
            left: Box::new(z0.clone()),
            op: aura_ast::BinOp::Eq,
            right: Box::new(z0),
        },
    }
}

#[cfg(feature = "z3")]
fn infer_invariant_from_cond(cond: &Expr, body: &aura_ast::Block) -> Option<Expr> {
    // If the RHS mentions vars mutated in the loop body, avoid using it.
    let mut mutated: BTreeSet<String> = BTreeSet::new();
    collect_mutated_vars(body, &mut mutated);

    match &cond.kind {
        ExprKind::Binary { left, op, right } => {
            let (new_op, rhs) = match op {
                aura_ast::BinOp::Lt => (aura_ast::BinOp::Le, right.as_ref()),
                aura_ast::BinOp::Le => (aura_ast::BinOp::Le, right.as_ref()),
                aura_ast::BinOp::Gt => (aura_ast::BinOp::Ge, right.as_ref()),
                aura_ast::BinOp::Ge => (aura_ast::BinOp::Ge, right.as_ref()),
                _ => return None,
            };

            if expr_mentions_any(rhs, &mutated) {
                return None;
            }

            Some(Expr {
                span: cond.span,
                kind: ExprKind::Binary {
                    left: left.clone(),
                    op: new_op,
                    right: right.clone(),
                },
            })
        }
        _ => None,
    }
}

#[cfg(feature = "z3")]
fn expr_mentions_any(expr: &Expr, names: &BTreeSet<String>) -> bool {
    match &expr.kind {
        ExprKind::Ident(id) => names.contains(&id.node),
        ExprKind::IntLit(_) | ExprKind::StringLit(_) => false,
        ExprKind::StyleLit { fields } => fields
            .iter()
            .any(|(_k, v)| expr_mentions_any(v, names)),
        ExprKind::RecordLit { fields, .. } => fields
            .iter()
            .any(|(_k, v)| expr_mentions_any(v, names)),
        ExprKind::Unary { expr: inner, .. } => expr_mentions_any(inner, names),
        ExprKind::Binary { left, right, .. } => {
            expr_mentions_any(left, names) || expr_mentions_any(right, names)
        }
        ExprKind::Member { base, member } => {
            expr_mentions_any(base, names) || names.contains(&member.node)
        }
        ExprKind::Call { callee, args, .. } => {
            if expr_mentions_any(callee, names) {
                return true;
            }
            args.iter()
                .any(|a| expr_mentions_any(call_arg_value(a), names))
        }
        ExprKind::Lambda { body, .. } => {
            // Conservative: only scan expressions directly contained in the lambda.
            for s in &body.stmts {
                let hit = match s {
                    Stmt::ExprStmt(e) => expr_mentions_any(e, names),
                    Stmt::Assign(a) => expr_mentions_any(&a.expr, names),
                    Stmt::Prop(p) => expr_mentions_any(&p.expr, names),
                    Stmt::Requires(r) => expr_mentions_any(&r.expr, names),
                    Stmt::Ensures(e) => expr_mentions_any(&e.expr, names),
                    Stmt::Assert(a) => expr_mentions_any(&a.expr, names),
                    Stmt::Assume(a) => expr_mentions_any(&a.expr, names),
                    Stmt::If(i) => expr_mentions_any(&i.cond, names),
                    Stmt::While(w) => {
                        expr_mentions_any(&w.cond, names)
                            || w
                                .invariant
                                .as_ref()
                                .is_some_and(|e| expr_mentions_any(e, names))
                            || w
                                .decreases
                                .as_ref()
                                .is_some_and(|e| expr_mentions_any(e, names))
                    }
                    _ => false,
                };
                if hit {
                    return true;
                }
            }

            body.yield_expr
                .as_ref()
                .is_some_and(|y| expr_mentions_any(y, names))
        }
        ExprKind::Flow { left, right, .. } => expr_mentions_any(left, names) || expr_mentions_any(right, names),
        ExprKind::ForAll { binders, body } | ExprKind::Exists { binders, body } => {
            let mut filtered = names.clone();
            for b in binders {
                filtered.remove(&b.name.node);
            }
            expr_mentions_any(body, &filtered)
        }
    }
}

#[cfg(feature = "z3")]
fn collect_mutated_vars(block: &aura_ast::Block, out: &mut BTreeSet<String>) {
    for s in &block.stmts {
        match s {
            Stmt::Assign(a) => {
                out.insert(a.target.node.clone());
            }
            Stmt::Layout(lb) => collect_mutated_vars(&lb.body, out),
            Stmt::Render(rb) => collect_mutated_vars(&rb.body, out),
            Stmt::While(w) => collect_mutated_vars(&w.body, out),
            Stmt::If(i) => {
                collect_mutated_vars(&i.then_block, out);
                if let Some(e) = &i.else_block {
                    collect_mutated_vars(e, out);
                }
            }
            _ => {}
        }
    }
}

fn visit_stmt<P: Prover>(stmt: &Stmt, aliases: &HashMap<String, RangeTy>, prover: &mut P) -> Result<(), VerifyError> {
    match stmt {
        Stmt::Import(_) => Ok(()),
        Stmt::StrandDef(sd) => verify_strand(sd, aliases, prover),
        Stmt::Layout(lb) => {
            for s in &lb.body.stmts {
                visit_stmt(s, aliases, prover)?;
            }
            Ok(())
        }
        Stmt::Render(rb) => {
            for s in &rb.body.stmts {
                visit_stmt(s, aliases, prover)?;
            }
            Ok(())
        }
        Stmt::CellDef(cell) => {
            for s in &cell.body.stmts {
                visit_stmt(s, aliases, prover)?;
            }
            if let Some(y) = &cell.body.yield_expr {
                let _ = y;
            }
            Ok(())
        }
        Stmt::FlowBlock(fb) => {
            for s in &fb.body.stmts {
                visit_stmt(s, aliases, prover)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn verify_strand<P: Prover>(sd: &StrandDef, aliases: &HashMap<String, RangeTy>, prover: &mut P) -> Result<(), VerifyError> {
    let Some(ty) = &sd.ty else {
        return Ok(());
    };

    if let Some(rt) = type_ref_to_range(ty, aliases) {
        prover.prove_u32_in_range(sd.span, &sd.expr, rt.lo, rt.hi)?;
    }

    if let Some(w) = &sd.where_clause {
        if let Some((lo, hi)) = refinement_u32_range_from_where(&sd.name.node, w) {
            prover.prove_u32_in_range(sd.span, &sd.expr, lo, hi)?;
        }
    }

    Ok(())
}

fn type_ref_to_range(tr: &TypeRef, aliases: &HashMap<String, RangeTy>) -> Option<RangeTy> {
    // Direct: u32[lo..hi]
    if tr.name.node == "u32" || tr.name.node == "Int" {
        if let Some(r) = &tr.range {
            let lo = const_u64(&r.lo)?;
            let hi = const_u64(&r.hi)?;
            return Some(RangeTy { lo, hi });
        }
        return None;
    }

    // Alias: Percentage => u32[0..100]
    aliases.get(&tr.name.node).cloned()
}

fn refinement_u32_range_from_where(var: &str, expr: &Expr) -> Option<(u64, u64)> {
    fn one(var: &str, expr: &Expr) -> Option<(u64, u64)> {
        let ExprKind::Binary { left, op, right } = &expr.kind else {
            return None;
        };

        let (id, lit, flipped) = match (&left.kind, &right.kind) {
            (ExprKind::Ident(id), ExprKind::IntLit(n)) => (id, *n, false),
            (ExprKind::IntLit(n), ExprKind::Ident(id)) => (id, *n, true),
            _ => return None,
        };

        if id.node != var {
            return None;
        }

        let op = if flipped {
            match op {
                aura_ast::BinOp::Lt => aura_ast::BinOp::Gt,
                aura_ast::BinOp::Le => aura_ast::BinOp::Ge,
                aura_ast::BinOp::Gt => aura_ast::BinOp::Lt,
                aura_ast::BinOp::Ge => aura_ast::BinOp::Le,
                other => *other,
            }
        } else {
            *op
        };

        match op {
            aura_ast::BinOp::Ge => Some((lit, u32::MAX as u64)),
            aura_ast::BinOp::Gt => Some((lit.saturating_add(1), u32::MAX as u64)),
            aura_ast::BinOp::Le => Some((0, lit)),
            aura_ast::BinOp::Lt => Some((0, lit.saturating_sub(1))),
            aura_ast::BinOp::Eq => Some((lit, lit)),
            _ => None,
        }
    }

    match &expr.kind {
        ExprKind::Binary {
            op: aura_ast::BinOp::And,
            left,
            right,
        } => {
            let (l_lo, l_hi) = one(var, left)?;
            let (r_lo, r_hi) = one(var, right)?;
            Some((l_lo.max(r_lo), l_hi.min(r_hi)))
        }
        _ => one(var, expr),
    }
}

fn alias_to_range(ta: &TypeAlias) -> Option<RangeTy> {
    if !ta.params.is_empty() {
        return None;
    }

    // type X = u32[lo..hi]
    if ta.target.name.node != "u32" {
        return None;
    }
    let r = ta.target.range.as_ref()?;
    let lo = const_u64(&r.lo)?;
    let hi = const_u64(&r.hi)?;
    Some(RangeTy { lo, hi })
}

fn const_u64(expr: &Expr) -> Option<u64> {
    match &expr.kind {
        ExprKind::IntLit(n) => Some(*n),
        _ => None,
    }
}
