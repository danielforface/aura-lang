#![forbid(unsafe_code)]
#![allow(unused_assignments)]

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct RelatedInfo {
    pub span: aura_ast::Span,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct DiagnosticMetadata {
    pub model: Option<String>,
    pub bindings: Vec<(String, String)>,
    pub related: Vec<RelatedInfo>,
    pub unsat_core: Vec<String>,
    pub hints: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("verification error: {message}")]
#[diagnostic(code(aura::verify))]
#[allow(unused_assignments)]
pub struct VerifyError {
    pub message: String,
    #[label]
    pub span: aura_ast::Span,
    pub model: Option<String>,
    pub meta: Option<DiagnosticMetadata>,
}

pub trait Prover {
    fn prove_u32_in_range(
        &mut self,
        span: aura_ast::Span,
        value_expr: &aura_ast::Expr,
        lo: u64,
        hi: u64,
    ) -> Result<(), VerifyError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmtProfile {
    /// Very low timeouts; quantifiers rejected.
    Fast,
    /// CI-friendly medium timeouts; quantifiers rejected.
    Ci,
    /// Higher timeouts; quantifiers allowed.
    Thorough,
}

/// Fallback prover when compiled without `--features aura-verify/z3`.
///
/// This keeps the workspace buildable on machines without Z3.
pub struct NoZ3Prover;

impl Prover for NoZ3Prover {
    fn prove_u32_in_range(
        &mut self,
        span: aura_ast::Span,
        _value_expr: &aura_ast::Expr,
        _lo: u64,
        _hi: u64,
    ) -> Result<(), VerifyError> {
        Err(VerifyError {
            message: "Z3 prover is not enabled. Rebuild with `--features aura-verify/z3`.".to_string(),
            span,
            model: None,
            meta: None,
        })
    }
}

#[cfg(feature = "z3")]
pub mod z3_prover {
    use super::{Prover, VerifyError};
    use aura_ast::{Expr, ExprKind, Span};

    use z3::{
        ast::{Ast, Bool, Int},
        Config, Context, SatResult, Solver,
    };

    #[derive(Debug)]
    struct Z3Session {
        solver: Solver<'static>,
        nonce: u64,
    }

    pub struct Z3Prover {
        ctx: &'static Context,
        session: Option<Z3Session>,
    }

    impl Z3Prover {
        pub fn new() -> Self {
            let mut cfg = Config::new();
            cfg.set_model_generation(true);
            // Leak the Z3 context so we can safely keep a warm solver session without
            // creating self-referential structs or using unsafe code.
            //
            // This is acceptable for long-lived processes (LSP/daemon) where the
            // context should live for the duration of the process anyway.
            let ctx: &'static Context = Box::leak(Box::new(Context::new(&cfg)));
            Self {
                ctx,
                session: None,
            }
        }

        pub fn ctx(&self) -> &Context {
            self.ctx
        }

        pub fn ctx_static(&self) -> &'static Context {
            self.ctx
        }

        pub fn warm_solver(&mut self) -> &mut Solver<'static> {
            if self.session.is_none() {
                let solver = Solver::new(self.ctx);
                self.session = Some(Z3Session { solver, nonce: 0 });
            }
            &mut self.session.as_mut().expect("just set").solver
        }

        pub fn next_nonce(&mut self) -> u64 {
            if self.session.is_none() {
                let _ = self.warm_solver();
            }
            let sess = self.session.as_mut().expect("exists");
            sess.nonce = sess.nonce.wrapping_add(1);
            sess.nonce
        }
    }

    impl Prover for Z3Prover {
        fn prove_u32_in_range(
            &mut self,
            span: Span,
            value_expr: &Expr,
            lo: u64,
            hi: u64,
        ) -> Result<(), VerifyError> {
            // We do a refutation-style check:
            // show UNSAT for: (v == expr) AND (v < lo OR v > hi)
            // If SAT => counterexample exists => unsafe.

            let solver = Solver::new(self.ctx);
            let v = Int::new_const(self.ctx, "v");

            // Only handle literals right now (enough to prove Percentage=80).
            let expr_ast = match &value_expr.kind {
                ExprKind::IntLit(n) => Int::from_u64(self.ctx, *n),
                _ => {
                    return Err(VerifyError {
                        message: "Z3 prover currently only supports integer literals (symbolic execution TBD)".to_string(),
                        span,
                        model: None,
                        meta: None,
                    });
                }
            };

            solver.assert(&v._eq(&expr_ast));
            let lo_i = Int::from_u64(self.ctx, lo);
            let hi_i = Int::from_u64(self.ctx, hi);

            let below = v.lt(&lo_i);
            let above = v.gt(&hi_i);
            solver.assert(&Bool::or(self.ctx, &[&below, &above]));

            match solver.check() {
                SatResult::Unsat => Ok(()),
                SatResult::Sat => {
                    let model = solver.get_model().map(|m| m.to_string());
                    Err(VerifyError {
                        message: format!("range proof failed: value may be outside [{lo}..{hi}]"),
                        span,
                        model,
                        meta: None,
                    })
                }
                SatResult::Unknown => Err(VerifyError {
                    message: "Z3 returned unknown for range proof".to_string(),
                    span,
                    model: None,
                    meta: None,
                }),
            }
        }
    }
}
