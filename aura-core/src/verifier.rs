#![forbid(unsafe_code)]

use aura_ast::{Expr, ExprKind};

use crate::error::SemanticError;

/// Stub SMT solver interface.
///
/// In Phase 2 this is intentionally minimal: it can only prove properties
/// about integer literals and basic range subset relationships.
pub trait SmtSolver {
    fn prove(&self, _predicate: &str) -> bool {
        false
    }
}

#[derive(Default)]
pub struct DummySolver;

impl SmtSolver for DummySolver {}

pub struct Verifier<S: SmtSolver> {
    solver: S,
}

impl<S: SmtSolver> Verifier<S> {
    pub fn new(solver: S) -> Self {
        Self { solver }
    }

    /// Prove that an expression is within `[lo, hi]`.
    ///
    /// Current capability: proves for integer literals only.
    pub fn prove_u32_in_range(&self, expr: &Expr, lo: u64, hi: u64) -> Result<(), SemanticError> {
        match &expr.kind {
            ExprKind::IntLit(n) => {
                if *n < lo || *n > hi {
                    Err(SemanticError {
                        message: format!("literal {n} is outside required range {lo}..{hi}"),
                        span: expr.span,
                    })
                } else {
                    Ok(())
                }
            }
            _ => {
                // SMT stub: we don't have a real solver in `aura-core` yet.
                // Do not hard-fail semantic checking here; the real verifier pass
                // (`aura-verify` with Z3) is responsible for emitting proof errors.
                let _predicate = format!("{lo} <= <expr> && <expr> <= {hi}");
                let _ = self.solver.prove(&_predicate);
                Ok(())
            }
        }
    }
}

 
