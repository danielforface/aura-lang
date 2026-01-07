#![forbid(unsafe_code)]

use std::collections::{BTreeSet, HashMap};

use aura_ast::{
    AssignStmt, BinOp as AstBinOp, Block, CellDef, Expr, ExprKind, FlowBlock, FlowOp, IfStmt,
    Ident, LayoutBlock, MatchStmt, Pattern, Program, RenderBlock, Stmt, StrandDef, TypeRef,
    UnaryOp as AstUnaryOp, UnsafeBlock, WhileStmt,
};
use aura_ir::{
    BasicBlock, BinOp, BlockId, CallConv, ExecutionHint, ExternFnSig, FlowKind, FunctionIR, IdGen,
    Inst, InstKind, ModuleIR, Param, RValue, Terminator, Type, UnaryOp, ValueId,
};

use crate::{Checker, SemanticError};

fn lower_sema_type_to_ir(ty: &crate::types::Type) -> Type {
    match ty {
        crate::types::Type::Unit => Type::Unit,
        crate::types::Type::Bool => Type::Bool,
        crate::types::Type::U32 => Type::U32,
        crate::types::Type::String => Type::String,
        crate::types::Type::Style => Type::Opaque("Style".to_string()),
        crate::types::Type::Model => Type::Opaque("Model".to_string()),
        crate::types::Type::Tensor { .. } => Type::Tensor,
        crate::types::Type::ConstrainedRange { base, .. } => lower_sema_type_to_ir(base),
        crate::types::Type::Named(n) => Type::Opaque(n.clone()),
        crate::types::Type::Applied { name, .. } => Type::Opaque(name.clone()),
        crate::types::Type::Unknown => Type::U32,
    }
}

pub fn lower_program(program: &Program) -> Result<ModuleIR, SemanticError> {
    let mut checker = Checker::new();
    // Prototype: range safety proofs are handled by `aura-verify` (Z3).
    checker.set_defer_range_proofs(true);
    checker.check_program(program)?;

    let mut module = ModuleIR::new();
    let mut lower = Lowerer {
        id: IdGen::default(),
        checker: &checker,
        locals: HashMap::new(),
        blocks: Vec::new(),
        current: None,
    };

    for stmt in &program.stmts {
        match stmt {
            Stmt::CellDef(cell) => {
                let f = lower.lower_cell(cell)?;
                module.functions.insert(f.name.clone(), f);
            }
            Stmt::FlowBlock(fb) => {
                let f = lower.lower_flow_block(fb)?;
                module.functions.insert(f.name.clone(), f);
            }
            Stmt::ExternCell(ext) => {
                let call_conv = if ext.name.node.starts_with("__stdcall_") {
                    CallConv::Stdcall
                } else {
                    CallConv::C
                };
                let sig = ExternFnSig {
                    params: ext.params.iter().map(|p| lower_type(&p.ty)).collect(),
                    ret: lower_type(&ext.ret),
                    call_conv,
                };
                module.externs.insert(ext.name.node.clone(), sig);
            }
            _ => {}
        }
    }

    Ok(module)
}

struct Lowerer<'c> {
    id: IdGen,
    checker: &'c Checker,
    locals: HashMap<String, ValueId>,

    blocks: Vec<BasicBlock>,
    current: Option<usize>,
}

impl<'c> Lowerer<'c> {
    fn lower_cell(&mut self, cell: &CellDef) -> Result<FunctionIR, SemanticError> {
        self.locals.clear();
        self.blocks.clear();
        let entry = self.id.fresh_block();
        let hint = match cell.flow {
            Some(FlowOp::Async) => ExecutionHint::Parallel,
            Some(FlowOp::Sync) | None => ExecutionHint::Sequential,
        };
        self.push_block(entry, cell.span, hint);

        let mut params = Vec::new();
        for p in &cell.params {
            let v = self.id.fresh_value();
            params.push(Param {
                name: p.name.node.clone(),
                ty: lower_type(&p.ty),
                span: p.span,
                value: v,
            });
            self.locals.insert(p.name.node.clone(), v);
        }

        self.lower_block(&cell.body)?;
        let ret = self
            .checker
            .function_ret_type(&cell.name.node)
            .map(lower_sema_type_to_ir)
            .unwrap_or(Type::Unit);

        let blocks = std::mem::take(&mut self.blocks);

        Ok(FunctionIR {
            name: cell.name.node.clone(),
            span: cell.span,
            params,
            ret,
            blocks,
            entry,
        })
    }

    fn lower_flow_block(&mut self, fb: &FlowBlock) -> Result<FunctionIR, SemanticError> {
        self.locals.clear();
        self.blocks.clear();
        let entry = self.id.fresh_block();
        let hint = match fb.flow {
            FlowOp::Async => ExecutionHint::Parallel,
            FlowOp::Sync => ExecutionHint::Sequential,
        };
        self.push_block(entry, fb.span, hint);

        self.lower_block(&fb.body)?;
        let blocks = std::mem::take(&mut self.blocks);
        Ok(FunctionIR {
            name: fb.name.node.clone(),
            span: fb.span,
            params: vec![],
            ret: self
                .checker
                .function_ret_type(&fb.name.node)
                .map(lower_sema_type_to_ir)
                .unwrap_or(Type::Unit),
            blocks,
            entry,
        })
    }

    fn lower_block(&mut self, block_ast: &Block) -> Result<(), SemanticError> {
        for stmt in &block_ast.stmts {
            self.lower_stmt_in_place(stmt, block_ast.span)?;
        }

        let ret = if let Some(y) = &block_ast.yield_expr {
            Some(self.lower_expr(y)?)
        } else {
            None
        };
        self.set_terminator(Terminator::Return(ret));
        Ok(())
    }

    fn lower_stmt_in_place(&mut self, stmt: &Stmt, span: aura_ast::Span) -> Result<(), SemanticError> {
        match stmt {
            Stmt::Import(_) | Stmt::TypeAlias(_) | Stmt::MacroDef(_) | Stmt::MacroCall(_) => Ok(()),
            Stmt::StrandDef(sd) => self.lower_strand(sd),
            Stmt::Prop(p) => {
                let _ = self.lower_expr(&p.expr)?;
                Ok(())
            }
            Stmt::Assign(a) => self.lower_assign(a),
            Stmt::If(i) => self.lower_if(i),
            Stmt::Match(m) => self.lower_match(m),
            Stmt::While(w) => self.lower_while(w),
            Stmt::Requires(_) | Stmt::Ensures(_) | Stmt::Assert(_) | Stmt::Assume(_) => Ok(()),
            Stmt::ExprStmt(expr) => {
                let _ = self.lower_expr(expr)?;
                Ok(())
            }
            Stmt::Layout(lb) => self.lower_layout_block(lb, span),
            Stmt::Render(rb) => self.lower_render_block(rb, span),
            other => Err(SemanticError {
                message: format!("lowering: unsupported statement: {other:?}"),
                span,
            }),
        }
    }

    fn lower_layout_block(&mut self, lb: &LayoutBlock, span: aura_ast::Span) -> Result<(), SemanticError> {
        for s in &lb.body.stmts {
            self.lower_stmt_in_place(s, span)?;
        }
        if let Some(y) = &lb.body.yield_expr {
            let _ = self.lower_expr(y)?;
        }
        Ok(())
    }

    fn lower_render_block(&mut self, rb: &RenderBlock, span: aura_ast::Span) -> Result<(), SemanticError> {
        for s in &rb.body.stmts {
            self.lower_stmt_in_place(s, span)?;
        }
        if let Some(y) = &rb.body.yield_expr {
            let _ = self.lower_expr(y)?;
        }
        Ok(())
    }

    fn lower_strand(&mut self, sd: &StrandDef) -> Result<(), SemanticError> {
        let v = self.id.fresh_value();
        let expr = self.lower_rvalue(&sd.expr)?;
        self.push_inst(Inst {
            span: sd.span,
            dest: Some(v),
            kind: InstKind::BindStrand {
                name: sd.name.node.clone(),
                expr,
            },
        });
        self.locals.insert(sd.name.node.clone(), v);
        Ok(())
    }

    fn lower_assign(&mut self, a: &AssignStmt) -> Result<(), SemanticError> {
        if !self.locals.contains_key(&a.target.node) {
            return Err(SemanticError {
                message: format!("lowering: assignment to unknown local '{}'", a.target.node),
                span: a.target.span,
            });
        }
        let v = self.lower_expr(&a.expr)?;
        self.locals.insert(a.target.node.clone(), v);
        Ok(())
    }

    fn lower_if(&mut self, i: &IfStmt) -> Result<(), SemanticError> {
        let cond_v = self.lower_expr(&i.cond)?;
        let then_bb = self.id.fresh_block();
        let else_bb = self.id.fresh_block();
        let join_bb = self.id.fresh_block();

        self.set_terminator(Terminator::CondBr {
            cond: cond_v,
            then_bb,
            else_bb,
        });

        let saved_locals = self.locals.clone();

        // then
        self.push_block(then_bb, i.then_block.span, ExecutionHint::Sequential);
        self.lower_block(&i.then_block)?;
        if !self.has_terminator() {
            self.set_terminator(Terminator::Br(join_bb));
        }
        let then_locals = self.locals.clone();

        // else
        self.locals = saved_locals.clone();
        self.push_block(else_bb, i.else_block.as_ref().map(|b| b.span).unwrap_or(i.span), ExecutionHint::Sequential);
        if let Some(else_block) = &i.else_block {
            self.lower_block(else_block)?;
        }
        if !self.has_terminator() {
            self.set_terminator(Terminator::Br(join_bb));
        }
        let else_locals = self.locals.clone();

        // join
        self.locals = saved_locals;
        self.push_block(join_bb, i.span, ExecutionHint::Sequential);
        self.merge_locals_with_phi(&then_locals, then_bb, &else_locals, else_bb, i.span);

        Ok(())
    }

    fn lower_match(&mut self, m: &MatchStmt) -> Result<(), SemanticError> {
        let scrut_v = self.lower_expr(&m.scrutinee)?;

        let join_bb = self.id.fresh_block();
        let mut body_bbs: Vec<BlockId> = Vec::with_capacity(m.arms.len());
        for _ in 0..m.arms.len() {
            body_bbs.push(self.id.fresh_block());
        }

        // Dispatch into the appropriate arm.
        // Sema enforces that the final arm is a wildcard, so dispatch always terminates.
        let mut wildcard_arm: Option<(usize, BlockId)> = None;
        let mut int_arms: Vec<(u64, BlockId, aura_ast::Span)> = Vec::new();
        let mut string_arms: Vec<(String, BlockId, aura_ast::Span)> = Vec::new();
        let mut ctor_arms: Vec<(u64, BlockId, aura_ast::Span)> = Vec::new();

        for (idx, arm) in m.arms.iter().enumerate() {
            let body_bb = body_bbs[idx];
            match &arm.pat {
                Pattern::Wildcard { .. } => wildcard_arm = Some((idx, body_bb)),
                Pattern::IntLit { value, .. } => int_arms.push((*value, body_bb, arm.span)),
                Pattern::StringLit { value, .. } => string_arms.push((value.clone(), body_bb, arm.span)),
                Pattern::Ctor { ty, variant, .. } => {
                    let Some((tag, _arity)) =
                        self.checker.enum_variant_info(&ty.node, &variant.node)
                    else {
                        return Err(SemanticError {
                            message: format!(
                                "lowering: unknown enum variant '{}::{}' in match pattern",
                                ty.node, variant.node
                            ),
                            span: arm.span,
                        });
                    };
                    ctor_arms.push((tag as u64, body_bb, arm.span));
                }
            }
        }

        let default_bb = wildcard_arm
            .map(|(_, bb)| bb)
            .ok_or_else(|| SemanticError {
                message: "lowering: match without trailing wildcard".to_string(),
                span: m.span,
            })?;

        // If this is an integer-literal match, emit a balanced decision tree (or switch) instead of a linear chain.
        // This is a small but meaningful step toward "advanced pattern matching compilation".
        if !int_arms.is_empty() && string_arms.is_empty() && ctor_arms.is_empty() {
            int_arms.sort_by_key(|(v, _, _)| *v);
            let items: Vec<(u64, BlockId)> = int_arms.iter().map(|(v, bb, _)| (*v, *bb)).collect();
            self.lower_match_int_switch_or_tree(scrut_v, &items, default_bb, m.span);
        } else if int_arms.is_empty() && !string_arms.is_empty() && ctor_arms.is_empty() {
            // Keep the simple linear chain for string matches.
            for (idx, arm) in m.arms.iter().enumerate() {
                let body_bb = body_bbs[idx];
                match &arm.pat {
                    Pattern::Wildcard { .. } => {
                        self.set_terminator(Terminator::Br(body_bb));
                        break;
                    }
                    Pattern::StringLit { value, .. } => {
                        let lit_v = self.lower_const_string(value.clone(), arm.span);
                        let cond_v = self.id.fresh_value();
                        self.push_inst(Inst {
                            span: arm.span,
                            dest: Some(cond_v),
                            kind: InstKind::Binary {
                                op: BinOp::Eq,
                                left: scrut_v,
                                right: lit_v,
                            },
                        });

                        let else_bb = self.id.fresh_block();
                        self.set_terminator(Terminator::CondBr {
                            cond: cond_v,
                            then_bb: body_bb,
                            else_bb,
                        });
                        self.push_block(else_bb, arm.span, ExecutionHint::Sequential);
                    }
                    Pattern::IntLit { .. } | Pattern::Ctor { .. } => {
                        return Err(SemanticError {
                            message: "lowering: mixed literal patterns are not supported in IR".to_string(),
                            span: arm.span,
                        });
                    }
                }
            }
        } else if int_arms.is_empty() && string_arms.is_empty() && !ctor_arms.is_empty() {
            // Enum/ctor match: dispatch on the enum tag stored in slot 0.
            ctor_arms.sort_by_key(|(v, _, _)| *v);
            let tag_v = self.lower_tensor_get(scrut_v, 0, m.span);
            let items: Vec<(u64, BlockId)> = ctor_arms.iter().map(|(v, bb, _)| (*v, *bb)).collect();
            self.lower_match_int_switch_or_tree(tag_v, &items, default_bb, m.span);
        } else {
            // Either only wildcard exists, or the match contains a mixed set of literal kinds.
            if int_arms.is_empty() && string_arms.is_empty() {
                self.set_terminator(Terminator::Br(default_bb));
            } else {
                return Err(SemanticError {
                    message: "lowering: mixed literal patterns are not supported in IR".to_string(),
                    span: m.span,
                });
            }
        }

        let saved_locals = self.locals.clone();
        let mut arm_locals: Vec<(BlockId, HashMap<String, ValueId>)> = Vec::with_capacity(m.arms.len());

        for (idx, arm) in m.arms.iter().enumerate() {
            let bb = body_bbs[idx];
            self.locals = saved_locals.clone();
            self.push_block(bb, arm.body.span, ExecutionHint::Sequential);

            // Pattern binders for enum ctor arms are scoped to the arm; do not escape the match.
            if let Pattern::Ctor {
                ty,
                variant,
                binders,
                ..
            } = &arm.pat
            {
                let Some((_tag, arity)) =
                    self.checker.enum_variant_info(&ty.node, &variant.node)
                else {
                    return Err(SemanticError {
                        message: format!(
                            "lowering: unknown enum variant '{}::{}' in match arm",
                            ty.node, variant.node
                        ),
                        span: arm.span,
                    });
                };

                if binders.len() != arity {
                    return Err(SemanticError {
                        message: format!(
                            "lowering: wrong number of binders for pattern '{}::{}': expected {}, got {}",
                            ty.node,
                            variant.node,
                            arity,
                            binders.len()
                        ),
                        span: arm.span,
                    });
                }

                for (i, b) in binders.iter().enumerate() {
                    let field_v = self.lower_tensor_get(scrut_v, (1 + i) as u64, arm.span);
                    self.locals.insert(b.node.clone(), field_v);
                }
            }

            self.lower_block(&arm.body)?;
            if !self.has_terminator() {
                self.set_terminator(Terminator::Br(join_bb));
            }
            arm_locals.push((bb, self.locals.clone()));
        }

        // join
        self.locals = saved_locals;
        self.push_block(join_bb, m.span, ExecutionHint::Sequential);
        self.merge_locals_with_phi_multi(&arm_locals, m.span);
        Ok(())
    }

    fn lower_match_int_tree(&mut self, scrut_v: ValueId, items: &[(u64, BlockId)], default_bb: BlockId, span: aura_ast::Span) {
        // Build a balanced decision tree using <= splits, ending in == checks.
        // This reduces branch depth vs the linear chain for large matches.
        fn build(lower: &mut Lowerer<'_>, scrut_v: ValueId, items: &[(u64, BlockId)], default_bb: BlockId, span: aura_ast::Span) {
            match items.len() {
                0 => {
                    lower.set_terminator(Terminator::Br(default_bb));
                }
                1 => {
                    let (value, body_bb) = items[0];
                    let lit_v = lower.lower_const_u32(value, span);
                    let cond_v = lower.id.fresh_value();
                    lower.push_inst(Inst {
                        span,
                        dest: Some(cond_v),
                        kind: InstKind::Binary {
                            op: BinOp::Eq,
                            left: scrut_v,
                            right: lit_v,
                        },
                    });
                    lower.set_terminator(Terminator::CondBr {
                        cond: cond_v,
                        then_bb: body_bb,
                        else_bb: default_bb,
                    });
                }
                _ => {
                    // Pick a pivot that guarantees progress.
                    // For len=2, we want mid=0 so left/right are size 1.
                    let mid = (items.len() - 1) / 2;
                    let pivot = items[mid].0;

                    let pivot_v = lower.lower_const_u32(pivot, span);
                    let cond_v = lower.id.fresh_value();
                    lower.push_inst(Inst {
                        span,
                        dest: Some(cond_v),
                        kind: InstKind::Binary {
                            op: BinOp::Le,
                            left: scrut_v,
                            right: pivot_v,
                        },
                    });

                    let left_bb = lower.id.fresh_block();
                    let right_bb = lower.id.fresh_block();
                    lower.set_terminator(Terminator::CondBr {
                        cond: cond_v,
                        then_bb: left_bb,
                        else_bb: right_bb,
                    });

                    lower.push_block(left_bb, span, ExecutionHint::Sequential);
                    build(lower, scrut_v, &items[..=mid], default_bb, span);

                    lower.push_block(right_bb, span, ExecutionHint::Sequential);
                    build(lower, scrut_v, &items[mid + 1..], default_bb, span);
                }
            }
        }

        build(self, scrut_v, items, default_bb, span);
    }

    fn lower_match_int_switch_or_tree(
        &mut self,
        scrut_v: ValueId,
        items: &[(u64, BlockId)],
        default_bb: BlockId,
        span: aura_ast::Span,
    ) {
        if items.len() >= 4 {
            let min = items.iter().map(|(v, _)| *v).min().unwrap_or(0);
            let max = items.iter().map(|(v, _)| *v).max().unwrap_or(0);
            let range = max.saturating_sub(min).saturating_add(1);
            // Heuristic density: if range is not much larger than number of cases, emit a switch.
            if range <= (items.len() as u64).saturating_mul(2) {
                self.set_terminator(Terminator::Switch {
                    scrut: scrut_v,
                    default_bb,
                    cases: items.to_vec(),
                });
                return;
            }
        }

        self.lower_match_int_tree(scrut_v, items, default_bb, span);
    }

    fn lower_tensor_get(&mut self, tensor_v: ValueId, index: u64, span: aura_ast::Span) -> ValueId {
        let idx_v = self.lower_const_u32(index, span);
        let out = self.id.fresh_value();
        self.push_inst(Inst {
            span,
            dest: Some(out),
            kind: InstKind::Call {
                callee: "tensor.get".to_string(),
                args: vec![tensor_v, idx_v],
            },
        });
        out
    }

    fn lower_const_u32(&mut self, n: u64, span: aura_ast::Span) -> ValueId {
        let v = self.id.fresh_value();
        self.push_inst(Inst {
            span,
            dest: Some(v),
            kind: InstKind::BindStrand {
                name: format!("$lit{v:?}"),
                expr: RValue::ConstU32(n),
            },
        });
        v
    }

    fn lower_const_string(&mut self, s: String, span: aura_ast::Span) -> ValueId {
        let v = self.id.fresh_value();
        self.push_inst(Inst {
            span,
            dest: Some(v),
            kind: InstKind::BindStrand {
                name: format!("$str{v:?}"),
                expr: RValue::ConstString(s),
            },
        });
        v
    }

    fn merge_locals_with_phi_multi(
        &mut self,
        preds: &[(BlockId, HashMap<String, ValueId>)],
        span: aura_ast::Span,
    ) {
        // Only merge locals that existed before the match.
        // (Arm-local `val` bindings are scoped to the arm block and should not escape.)
        let names: BTreeSet<String> = self.locals.keys().cloned().collect();

        for name in names {
            let mut first: Option<ValueId> = None;
            let mut all_same = true;
            let mut incomings: Vec<(BlockId, ValueId)> = Vec::with_capacity(preds.len());

            let entry_v = self.locals.get(&name).copied();
            for (bb, locals) in preds {
                let v = locals.get(&name).copied().or(entry_v);
                let Some(v) = v else { continue };
                if let Some(f) = first {
                    if v != f {
                        all_same = false;
                    }
                } else {
                    first = Some(v);
                }
                incomings.push((*bb, v));
            }

            let Some(v0) = first else { continue };
            if all_same {
                self.locals.insert(name, v0);
                continue;
            }

            let out = self.id.fresh_value();
            self.push_inst(Inst {
                span,
                dest: Some(out),
                kind: InstKind::Phi { incomings },
            });
            self.locals.insert(name, out);
        }
    }

    fn lower_while(&mut self, w: &WhileStmt) -> Result<(), SemanticError> {
        let cond_bb = self.id.fresh_block();
        let body_bb = self.id.fresh_block();
        let exit_bb = self.id.fresh_block();

        let preheader_bb = self.current_block_id().ok_or_else(|| SemanticError {
            message: "lowering: while without an active block".to_string(),
            span: w.span,
        })?;

        // jump to condition
        self.set_terminator(Terminator::Br(cond_bb));

        let saved_locals = self.locals.clone();

        // Only variables that are actually assigned within the loop body need
        // to be loop-carried via phi nodes. Carrying loop-invariant values
        // creates invalid self-referential phi incoming edges in nested loops.
        let mut mutated: BTreeSet<String> = BTreeSet::new();
        collect_assigned_names(&w.body, &mut mutated);

        // cond
        self.push_block(cond_bb, w.cond.span, ExecutionHint::Sequential);

        // Loop-carried SSA: introduce phis for locals that may change in the loop body.
        // Incoming edges: preheader -> cond, body -> cond.
        let cond_block_idx = self.current.expect("current block");
        let mut phi_fixups: Vec<(usize, String, ValueId)> = Vec::new();
        for (name, &pre_v) in &saved_locals {
            if !mutated.contains(name) {
                continue;
            }
            let out = self.id.fresh_value();
            let inst_idx = self.current_block_mut().insts.len();
            self.push_inst(Inst {
                span: w.span,
                dest: Some(out),
                kind: InstKind::Phi {
                    incomings: vec![(preheader_bb, pre_v), (body_bb, pre_v)],
                },
            });
            self.locals.insert(name.clone(), out);
            phi_fixups.push((inst_idx, name.clone(), pre_v));
        }

        let cond_v = self.lower_expr(&w.cond)?;
        let cond_locals = self.locals.clone();
        self.set_terminator(Terminator::CondBr {
            cond: cond_v,
            then_bb: body_bb,
            else_bb: exit_bb,
        });

        // body
        self.push_block(body_bb, w.body.span, ExecutionHint::Sequential);
        self.locals = cond_locals.clone();
        self.lower_block(&w.body)?;
        if !self.has_terminator() {
            self.set_terminator(Terminator::Br(cond_bb));
        }
        let backedge_bb = self.current_block_id().unwrap_or(body_bb);
        let body_locals = self.locals.clone();

        // Patch phi incoming values from the body.
        // Note: only names present at loop entry get phis; new names in the body don't escape.
        for (inst_idx, name, pre_v) in phi_fixups {
            let body_v = body_locals.get(&name).copied().unwrap_or(pre_v);
            let inst = &mut self.blocks[cond_block_idx].insts[inst_idx];
            if let InstKind::Phi { incomings } = &mut inst.kind {
                if incomings.len() >= 2 {
                    incomings[1].0 = backedge_bb;
                    incomings[1].1 = body_v;
                }
            }
        }

        // exit
        // Exit is reached only from the condition block, so values are the loop-header SSA values.
        self.locals = cond_locals;
        self.push_block(exit_bb, w.span, ExecutionHint::Sequential);

        Ok(())
    }

    fn lower_rvalue(&mut self, expr: &Expr) -> Result<RValue, SemanticError> {
        match &expr.kind {
            ExprKind::IntLit(n) => Ok(RValue::ConstU32(*n)),
            ExprKind::StringLit(s) => Ok(RValue::ConstString(s.clone())),
            ExprKind::StyleLit { fields } => Ok(RValue::ConstString(format_style_lit(fields))),
            _ => Ok(RValue::Local(self.lower_expr(expr)?)),
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> Result<ValueId, SemanticError> {
        match &expr.kind {
            ExprKind::Ident(id) => self
                .locals
                .get(&id.node)
                .cloned()
                .ok_or_else(|| SemanticError {
                    message: format!("lowering: unknown identifier '{}'", id.node),
                    span: id.span,
                }),

            ExprKind::IntLit(n) => {
                let v = self.id.fresh_value();
                self.push_inst(Inst {
                    span: expr.span,
                    dest: Some(v),
                    kind: InstKind::BindStrand {
                        name: format!("$lit{v:?}"),
                        expr: RValue::ConstU32(*n),
                    },
                });
                Ok(v)
            }

            ExprKind::StringLit(s) => {
                let v = self.id.fresh_value();
                self.push_inst(Inst {
                    span: expr.span,
                    dest: Some(v),
                    kind: InstKind::BindStrand {
                        name: format!("$str{v:?}"),
                        expr: RValue::ConstString(s.clone()),
                    },
                });
                Ok(v)
            }

            ExprKind::ForAll { .. } | ExprKind::Exists { .. } => Err(SemanticError {
                message: "quantifiers are verification-only and cannot be lowered".to_string(),
                span: expr.span,
            }),

            ExprKind::StyleLit { fields } => {
                let v = self.id.fresh_value();
                self.push_inst(Inst {
                    span: expr.span,
                    dest: Some(v),
                    kind: InstKind::BindStrand {
                        name: format!("$style{v:?}"),
                        expr: RValue::ConstString(format_style_lit(fields)),
                    },
                });
                Ok(v)
            }

            ExprKind::Lambda { .. } => {
                // Closures/callbacks are not lowered into IR yet.
                // Keep it as an opaque placeholder value.
                let v = self.id.fresh_value();
                self.push_inst(Inst {
                    span: expr.span,
                    dest: Some(v),
                    kind: InstKind::BindStrand {
                        name: format!("$lambda{v:?}"),
                        expr: RValue::ConstU32(0),
                    },
                });
                Ok(v)
            }

            ExprKind::Unary { op, expr: inner } => {
                let operand = self.lower_expr(inner)?;
                let v = self.id.fresh_value();
                let op = match op {
                    AstUnaryOp::Neg => UnaryOp::Neg,
                    AstUnaryOp::Not => UnaryOp::Not,
                };
                self.push_inst(Inst {
                    span: expr.span,
                    dest: Some(v),
                    kind: InstKind::Unary { op, operand },
                });
                Ok(v)
            }

            ExprKind::Binary { left, op, right } => {
                // Short-circuiting for boolean ops via CFG.
                if matches!(op, AstBinOp::And | AstBinOp::Or) {
                    return self.lower_short_circuit_bool(expr.span, left, *op, right);
                }

                let lv = self.lower_expr(left)?;
                let rv = self.lower_expr(right)?;
                let v = self.id.fresh_value();
                let op = map_binop(*op);
                self.push_inst(Inst {
                    span: expr.span,
                    dest: Some(v),
                    kind: InstKind::Binary {
                        op,
                        left: lv,
                        right: rv,
                    },
                });
                Ok(v)
            }

            ExprKind::Member { base, member } => {
                // We lower member access by turning it into a callee string when used as callee.
                // If evaluated as a value, it's an opaque handle.
                let _ = self.lower_expr(base)?;
                let v = self.id.fresh_value();
                self.push_inst(Inst {
                    span: expr.span,
                    dest: Some(v),
                    kind: InstKind::BindStrand {
                        name: format!("{}.{}", "<member>", member.node),
                        expr: RValue::ConstString(member.node.clone()),
                    },
                });
                Ok(v)
            }

            ExprKind::Call { callee, args, trailing } => {
                // Enum constructor lowering: `Type::Variant(a, b, ...)`.
                // We encode enum values as `Tensor` handles where:
                // - slot 0 = tag (variant index)
                // - slots 1.. = field payloads (u32-ish handles)
                if let ExprKind::Member { base, member } = &callee.kind {
                    if let ExprKind::Ident(ty_id) = &base.kind {
                        if let Some((tag, arity)) =
                            self.checker.enum_variant_info(&ty_id.node, &member.node)
                        {
                            if args.len() != arity {
                                return Err(SemanticError {
                                    message: format!(
                                        "lowering: wrong number of constructor args for '{}::{}': expected {}, got {}",
                                        ty_id.node,
                                        member.node,
                                        arity,
                                        args.len()
                                    ),
                                    span: expr.span,
                                });
                            }

                            let len_v = self.lower_const_u32((1 + arity) as u64, expr.span);
                            let enum_v = self.id.fresh_value();
                            self.push_inst(Inst {
                                span: expr.span,
                                dest: Some(enum_v),
                                kind: InstKind::Call {
                                    callee: "tensor.new".to_string(),
                                    args: vec![len_v],
                                },
                            });

                            let idx0 = self.lower_const_u32(0, expr.span);
                            let tag_v = self.lower_const_u32(tag as u64, expr.span);
                            self.push_inst(Inst {
                                span: expr.span,
                                dest: None,
                                kind: InstKind::Call {
                                    callee: "tensor.set".to_string(),
                                    args: vec![enum_v, idx0, tag_v],
                                },
                            });

                            for (i, a) in args.iter().enumerate() {
                                let field_v = self.lower_expr(Self::call_arg_value(a))?;
                                let idx_v = self.lower_const_u32((1 + i) as u64, expr.span);
                                self.push_inst(Inst {
                                    span: expr.span,
                                    dest: None,
                                    kind: InstKind::Call {
                                        callee: "tensor.set".to_string(),
                                        args: vec![enum_v, idx_v, field_v],
                                    },
                                });
                            }

                            // Trailing blocks are not supported on enum constructors (enforced in sema),
                            // but ignore defensively.
                            if let Some(tb) = trailing {
                                for s in &tb.stmts {
                                    self.lower_stmt_in_place(s, tb.span)?;
                                }
                                if let Some(y) = &tb.yield_expr {
                                    let _ = self.lower_expr(y)?;
                                }
                            }

                            return Ok(enum_v);
                        }
                    }
                }

                // Prototype method lowering: treat tensor instance methods `.len/.get/.set`
                // as `tensor.*(receiver, ...)`.
                let (callee_name, lowered_args) = match &callee.kind {
                    ExprKind::Member { base, member }
                        if matches!(member.node.as_str(), "len" | "get" | "set")
                            && !matches!(&base.kind, aura_ast::ExprKind::Ident(id) if id.node == "tensor") =>
                    {
                        let recv = self.lower_expr(base)?;
                        let mut v = Vec::with_capacity(args.len() + 1);
                        v.push(recv);
                        for a in args {
                            v.push(self.lower_expr(Self::call_arg_value(a))?);
                        }
                        (format!("tensor.{}", member.node), v)
                    }
                    ExprKind::Member { base, member }
                        if member.node == "infer"
                            && !matches!(&base.kind, aura_ast::ExprKind::Ident(id) if id.node == "ai") =>
                    {
                        let recv = self.lower_expr(base)?;
                        let mut v = Vec::with_capacity(args.len() + 1);
                        v.push(recv);
                        for a in args {
                            v.push(self.lower_expr(Self::call_arg_value(a))?);
                        }
                        ("ai.infer".to_string(), v)
                    }
                    _ => {
                        let name = expr_to_callee_name(callee);
                        let mut v = Vec::with_capacity(args.len());
                        for a in args {
                            v.push(self.lower_expr(Self::call_arg_value(a))?);
                        }
                        (name, v)
                    }
                };

                // Stdlib prototype: lower collections.vector_* to tensor.* for now.
                let callee_name = match callee_name.as_str() {
                    "collections.vector_new" => "tensor.new".to_string(),
                    "collections.vector_len" => "tensor.len".to_string(),
                    "collections.vector_get" => "tensor.get".to_string(),
                    "collections.vector_set" => "tensor.set".to_string(),
                    _ => callee_name,
                };
                let v = self.id.fresh_value();
                let is_void = self.checker.is_void_function(&callee_name);
                self.push_inst(Inst {
                    span: expr.span,
                    dest: if is_void { None } else { Some(v) },
                    kind: InstKind::Call {
                        callee: callee_name,
                        args: lowered_args,
                    },
                });

                // Phase A.5: trailing blocks execute in caller context.
                if let Some(tb) = trailing {
                    for s in &tb.stmts {
                        self.lower_stmt_in_place(s, tb.span)?;
                    }
                    if let Some(y) = &tb.yield_expr {
                        let _ = self.lower_expr(y)?;
                    }
                }
                Ok(v)
            }

            ExprKind::Flow { left, op, right } => {
                let kind = match op {
                    FlowOp::Sync => FlowKind::Sync,
                    FlowOp::Async => FlowKind::Async,
                };

                // Heuristic lowering: treat flow into a call as either Call (sync) or ComputeKernel (async).
                let left_v = self.lower_expr(left)?;

                match &right.kind {
                    ExprKind::Call { callee, args, trailing } => {
                        // If flowing into a method call, preserve the same lowering rules as above.
                        let (callee_name, lowered_args) = match &callee.kind {
                            ExprKind::Member { base, member }
                                if matches!(member.node.as_str(), "len" | "get" | "set")
                                    && !matches!(&base.kind, aura_ast::ExprKind::Ident(id) if id.node == "tensor") =>
                            {
                                let recv = self.lower_expr(base)?;
                                let mut v = Vec::with_capacity(args.len() + 2);
                                v.push(left_v);
                                v.push(recv);
                                for a in args {
                                    v.push(self.lower_expr(Self::call_arg_value(a))?);
                                }
                                (format!("tensor.{}", member.node), v)
                            }
                            ExprKind::Member { base, member }
                                if member.node == "infer"
                                    && !matches!(&base.kind, aura_ast::ExprKind::Ident(id) if id.node == "ai") =>
                            {
                                let recv = self.lower_expr(base)?;
                                let mut v = Vec::with_capacity(args.len() + 2);
                                v.push(left_v);
                                v.push(recv);
                                for a in args {
                                    v.push(self.lower_expr(Self::call_arg_value(a))?);
                                }
                                ("ai.infer".to_string(), v)
                            }
                            _ => {
                                let name = expr_to_callee_name(callee);
                                let mut v = Vec::with_capacity(args.len() + 1);
                                v.push(left_v);
                                for a in args {
                                    v.push(self.lower_expr(Self::call_arg_value(a))?);
                                }
                                (name, v)
                            }
                        };

                        // Stdlib prototype: lower collections.vector_* to tensor.* for now.
                        let callee_name = match callee_name.as_str() {
                            "collections.vector_new" => "tensor.new".to_string(),
                            "collections.vector_len" => "tensor.len".to_string(),
                            "collections.vector_get" => "tensor.get".to_string(),
                            "collections.vector_set" => "tensor.set".to_string(),
                            _ => callee_name,
                        };
                        let v = self.id.fresh_value();
                        let is_void = self.checker.is_void_function(&callee_name);
                        let inst_kind = match kind {
                            FlowKind::Sync => InstKind::Call {
                                callee: callee_name,
                                args: lowered_args,
                            },
                            FlowKind::Async => InstKind::ComputeKernel {
                                callee: callee_name,
                                args: lowered_args,
                            },
                        };
                        self.push_inst(Inst {
                            span: expr.span,
                            dest: if is_void { None } else { Some(v) },
                            kind: inst_kind,
                        });

                        // Execute trailing block after call (Phase A.5).
                        if let Some(tb) = trailing {
                            for s in &tb.stmts {
                                self.lower_stmt_in_place(s, tb.span)?;
                            }
                            if let Some(y) = &tb.yield_expr {
                                let _ = self.lower_expr(y)?;
                            }
                        }
                        Ok(v)
                    }
                    _ => {
                        // Fallback: evaluate right and return it.
                        let _ = left_v;
                        self.lower_expr(right)
                    }
                }
            }

            ExprKind::RecordLit { .. } => Err(SemanticError {
                message: "lowering: record literals are not supported in IR yet".to_string(),
                span: expr.span,
            }),
        }
    }

fn call_arg_value(arg: &aura_ast::CallArg) -> &Expr {
    match arg {
        aura_ast::CallArg::Positional(e) => e,
        aura_ast::CallArg::Named { value, .. } => value,
    }
}

    fn lower_short_circuit_bool(
        &mut self,
        span: aura_ast::Span,
        left: &Expr,
        op: AstBinOp,
        right: &Expr,
    ) -> Result<ValueId, SemanticError> {
        let left_v = self.lower_expr(left)?;
        let rhs_bb = self.id.fresh_block();
        let join_bb = self.id.fresh_block();

        // If (A && B): if A then eval B else false
        // If (A || B): if A then true else eval B
        let (then_bb, else_bb) = match op {
            AstBinOp::And => (rhs_bb, join_bb),
            AstBinOp::Or => (join_bb, rhs_bb),
            _ => unreachable!(),
        };

        self.set_terminator(Terminator::CondBr {
            cond: left_v,
            then_bb,
            else_bb,
        });

        let left_block = self.current_block_id().ok_or_else(|| SemanticError {
            message: "lowering: missing current block".to_string(),
            span,
        })?;

        // RHS block
        self.push_block(rhs_bb, right.span, ExecutionHint::Sequential);
        let right_v = self.lower_expr(right)?;
        self.set_terminator(Terminator::Br(join_bb));

        let rhs_end = rhs_bb;

        // Join block
        self.push_block(join_bb, span, ExecutionHint::Sequential);

        // Build constant for the short-circuit path
        let const_val = match op {
            AstBinOp::And => false,
            AstBinOp::Or => true,
            _ => unreachable!(),
        };
        let const_v = self.id.fresh_value();
        self.push_inst(Inst {
            span,
            dest: Some(const_v),
            kind: InstKind::BindStrand {
                name: format!("$bool{const_v:?}"),
                expr: RValue::ConstBool(const_val),
            },
        });

        let out_v = self.id.fresh_value();
        self.push_inst(Inst {
            span,
            dest: Some(out_v),
            kind: InstKind::Phi {
                incomings: vec![(left_block, const_v), (rhs_end, right_v)],
            },
        });
        Ok(out_v)
    }

    fn merge_locals_with_phi(
        &mut self,
        a: &HashMap<String, ValueId>,
        a_bb: BlockId,
        b: &HashMap<String, ValueId>,
        b_bb: BlockId,
        span: aura_ast::Span,
    ) {
        for (name, &a_v) in a {
            if let Some(&b_v) = b.get(name) {
                if a_v == b_v {
                    self.locals.insert(name.clone(), a_v);
                    continue;
                }
                let out = self.id.fresh_value();
                self.push_inst(Inst {
                    span,
                    dest: Some(out),
                    kind: InstKind::Phi {
                        incomings: vec![(a_bb, a_v), (b_bb, b_v)],
                    },
                });
                self.locals.insert(name.clone(), out);
            } else {
                // If missing from b, keep a.
                self.locals.insert(name.clone(), a_v);
            }
        }
        // Include names present only in b.
        for (name, &b_v) in b {
            self.locals.entry(name.clone()).or_insert(b_v);
        }
    }

    fn push_block(&mut self, id: BlockId, span: aura_ast::Span, hint: ExecutionHint) {
        self.blocks.push(BasicBlock {
            id,
            span,
            hint,
            insts: Vec::new(),
            term: Terminator::Return(None),
        });
        self.current = Some(self.blocks.len() - 1);
    }

    fn current_block_mut(&mut self) -> &mut BasicBlock {
        let idx = self.current.expect("current block");
        &mut self.blocks[idx]
    }

    fn current_block_id(&self) -> Option<BlockId> {
        self.current.map(|idx| self.blocks[idx].id)
    }

    fn push_inst(&mut self, inst: Inst) {
        self.current_block_mut().insts.push(inst);
    }

    fn set_terminator(&mut self, term: Terminator) {
        self.current_block_mut().term = term;
    }

    fn has_terminator(&self) -> bool {
        let idx = self.current.expect("current block");
        !matches!(self.blocks[idx].term, Terminator::Return(None))
    }
}

fn collect_assigned_names(block: &Block, out: &mut BTreeSet<String>) {
    for s in &block.stmts {
        match s {
            Stmt::Assign(AssignStmt { target, .. }) => {
                out.insert(target.node.clone());
            }
            Stmt::Layout(LayoutBlock { body, .. }) => {
                collect_assigned_names(body, out);
            }
            Stmt::Render(RenderBlock { body, .. }) => {
                collect_assigned_names(body, out);
            }
            Stmt::If(IfStmt { then_block, else_block, .. }) => {
                collect_assigned_names(then_block, out);
                if let Some(b) = else_block {
                    collect_assigned_names(b, out);
                }
            }
            Stmt::Match(MatchStmt { arms, .. }) => {
                for arm in arms {
                    collect_assigned_names(&arm.body, out);
                }
            }
            Stmt::While(WhileStmt { body, .. }) => {
                collect_assigned_names(body, out);
            }
            Stmt::FlowBlock(FlowBlock { body, .. }) => {
                collect_assigned_names(body, out);
            }
            Stmt::UnsafeBlock(UnsafeBlock { body, .. }) => {
                collect_assigned_names(body, out);
            }
            Stmt::Prop(_) => {}
            Stmt::Requires(_) | Stmt::Ensures(_) | Stmt::Assert(_) | Stmt::Assume(_) => {}
            Stmt::CellDef(_)
            | Stmt::ExternCell(_)
            | Stmt::Import(_)
            | Stmt::MacroDef(_)
            | Stmt::MacroCall(_)
            | Stmt::TypeAlias(_)
            | Stmt::TraitDef(_)
            | Stmt::RecordDef(_)
            | Stmt::EnumDef(_)
            | Stmt::StrandDef(_)
            | Stmt::ExprStmt(_) => {}
        }
    }
}

fn map_binop(op: AstBinOp) -> BinOp {
    match op {
        AstBinOp::Add => BinOp::Add,
        AstBinOp::Sub => BinOp::Sub,
        AstBinOp::Mul => BinOp::Mul,
        AstBinOp::Div => BinOp::Div,
        AstBinOp::Eq => BinOp::Eq,
        AstBinOp::Ne => BinOp::Ne,
        AstBinOp::Lt => BinOp::Lt,
        AstBinOp::Gt => BinOp::Gt,
        AstBinOp::Le => BinOp::Le,
        AstBinOp::Ge => BinOp::Ge,
        AstBinOp::And => BinOp::And,
        AstBinOp::Or => BinOp::Or,
    }
}

fn lower_type(tr: &TypeRef) -> Type {
    match tr.name.node.as_str() {
        "bool" => Type::Bool,
        "u32" => Type::U32,
        "Tensor" => Type::Tensor,
        "String" => Type::String,
        "Unit" => Type::Unit,
        "Style" => Type::Opaque("Style".to_string()),
        other => Type::Opaque(other.to_string()),
    }
}

fn expr_to_callee_name(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Ident(id) => id.node.clone(),
        ExprKind::Member { base, member } => format!("{}.{}", expr_to_callee_name(base), member.node),
        _ => "<unknown>".to_string(),
    }
}

fn format_style_lit(fields: &[(Ident, Expr)]) -> String {
    fn value_to_string(e: &Expr) -> String {
        match &e.kind {
            ExprKind::IntLit(n) => n.to_string(),
            ExprKind::StringLit(s) => s.clone(),
            ExprKind::Ident(id) => id.node.clone(),
            ExprKind::StyleLit { fields } => format_style_lit(fields),
            _ => "<expr>".to_string(),
        }
    }

    let mut out = String::from("Style{");
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&k.node);
        out.push(':');
        out.push_str(&value_to_string(v));
    }
    out.push('}');
    out
}
