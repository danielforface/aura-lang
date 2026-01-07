#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{BinOp, BlockId, FunctionIR, InstKind, ModuleIR, RValue, Terminator, UnaryOp, ValueId};

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConstVal {
    U32(u64),
    Bool(bool),
    String(String),
}

pub fn optimize_module(module: &mut ModuleIR) {
    for f in module.functions.values_mut() {
        optimize_function(f);
    }
}

pub fn optimize_function(f: &mut FunctionIR) {
    // These are small, local passes intended to be safe and deterministic.
    // Run a couple of iterations to catch cascading simplifications.
    for _ in 0..4 {
        let mut changed = false;
        changed |= const_fold_and_simplify_cfg(f);
        changed |= dce(f);
        changed |= remove_unreachable_blocks(f);
        if !changed {
            break;
        }
    }
}

fn const_fold_and_simplify_cfg(f: &mut FunctionIR) -> bool {
    let mut changed = false;

    // Collect known constants (best-effort). This is not full global const-prop; it’s enough
    // to fold local expressions produced from literals.
    let mut consts: BTreeMap<ValueId, ConstVal> = BTreeMap::new();
    for b in &f.blocks {
        for inst in &b.insts {
            let Some(dest) = inst.dest else { continue };
            if let InstKind::BindStrand { expr, .. } = &inst.kind {
                if let Some(c) = const_from_rvalue(expr, &consts) {
                    consts.insert(dest, c);
                }
            }
        }
    }

    for b in &mut f.blocks {
        for inst in &mut b.insts {
            match &mut inst.kind {
                InstKind::Unary { op, operand } => {
                    if let Some(c) = consts.get(operand).cloned() {
                        if let Some(out) = fold_unary(*op, c) {
                            inst.kind = InstKind::BindStrand {
                                name: "$fold".to_string(),
                                expr: rvalue_from_const(out),
                            };
                            if let Some(dest) = inst.dest {
                                if let Some(c2) = const_from_rvalue(match &inst.kind {
                                    InstKind::BindStrand { expr, .. } => expr,
                                    _ => unreachable!(),
                                }, &consts) {
                                    consts.insert(dest, c2);
                                }
                            }
                            changed = true;
                        }
                    }
                }
                InstKind::Binary { op, left, right } => {
                    let (Some(cl), Some(cr)) = (consts.get(left).cloned(), consts.get(right).cloned()) else {
                        continue;
                    };
                    if let Some(out) = fold_binary(*op, cl, cr) {
                        inst.kind = InstKind::BindStrand {
                            name: "$fold".to_string(),
                            expr: rvalue_from_const(out),
                        };
                        if let Some(dest) = inst.dest {
                            if let Some(c2) = const_from_rvalue(match &inst.kind {
                                InstKind::BindStrand { expr, .. } => expr,
                                _ => unreachable!(),
                            }, &consts) {
                                consts.insert(dest, c2);
                            }
                        }
                        changed = true;
                    }
                }
                InstKind::RangeCheckU32 { value, lo, hi } => {
                    if let Some(ConstVal::U32(v)) = consts.get(value) {
                        if *v >= *lo && *v <= *hi {
                            // Proved at compile time: remove the check.
                            inst.kind = InstKind::BindStrand {
                                name: "$rc_elided".to_string(),
                                expr: RValue::ConstBool(true),
                            };
                            inst.dest = None;
                            changed = true;
                        }
                    }
                }
                _ => {}
            }
        }

        // CondBr folding.
        if let Terminator::CondBr {
            cond,
            then_bb,
            else_bb,
        } = b.term
        {
            if let Some(ConstVal::Bool(v)) = consts.get(&cond) {
                b.term = Terminator::Br(if *v { then_bb } else { else_bb });
                changed = true;
            }
        }

        // Switch folding.
        if let Terminator::Switch {
            scrut,
            default_bb,
            ref cases,
        } = b.term
        {
            if let Some(ConstVal::U32(v)) = consts.get(&scrut) {
                let mut target = default_bb;
                for (k, bb) in cases {
                    if *k == *v {
                        target = *bb;
                        break;
                    }
                }
                b.term = Terminator::Br(target);
                changed = true;
            }
        }
    }

    changed
}

fn const_from_rvalue(rv: &RValue, consts: &BTreeMap<ValueId, ConstVal>) -> Option<ConstVal> {
    match rv {
        RValue::ConstU32(n) => Some(ConstVal::U32(*n)),
        RValue::ConstBool(b) => Some(ConstVal::Bool(*b)),
        RValue::ConstString(s) => Some(ConstVal::String(s.clone())),
        RValue::Local(v) => consts.get(v).cloned(),
    }
}

fn rvalue_from_const(c: ConstVal) -> RValue {
    match c {
        ConstVal::U32(n) => RValue::ConstU32(n),
        ConstVal::Bool(b) => RValue::ConstBool(b),
        ConstVal::String(s) => RValue::ConstString(s),
    }
}

fn fold_unary(op: UnaryOp, v: ConstVal) -> Option<ConstVal> {
    match (op, v) {
        (UnaryOp::Neg, ConstVal::U32(n)) => Some(ConstVal::U32((0u64).wrapping_sub(n))),
        (UnaryOp::Not, ConstVal::Bool(b)) => Some(ConstVal::Bool(!b)),
        _ => None,
    }
}

fn fold_binary(op: BinOp, l: ConstVal, r: ConstVal) -> Option<ConstVal> {
    match (op, l, r) {
        (BinOp::Add, ConstVal::U32(a), ConstVal::U32(b)) => Some(ConstVal::U32(a.wrapping_add(b))),
        (BinOp::Sub, ConstVal::U32(a), ConstVal::U32(b)) => Some(ConstVal::U32(a.wrapping_sub(b))),
        (BinOp::Mul, ConstVal::U32(a), ConstVal::U32(b)) => Some(ConstVal::U32(a.wrapping_mul(b))),
        (BinOp::Div, ConstVal::U32(a), ConstVal::U32(b)) if b != 0 => Some(ConstVal::U32(a / b)),

        (BinOp::Eq, a, b) => Some(ConstVal::Bool(a == b)),
        (BinOp::Ne, a, b) => Some(ConstVal::Bool(a != b)),

        (BinOp::Lt, ConstVal::U32(a), ConstVal::U32(b)) => Some(ConstVal::Bool(a < b)),
        (BinOp::Gt, ConstVal::U32(a), ConstVal::U32(b)) => Some(ConstVal::Bool(a > b)),
        (BinOp::Le, ConstVal::U32(a), ConstVal::U32(b)) => Some(ConstVal::Bool(a <= b)),
        (BinOp::Ge, ConstVal::U32(a), ConstVal::U32(b)) => Some(ConstVal::Bool(a >= b)),

        (BinOp::And, ConstVal::Bool(a), ConstVal::Bool(b)) => Some(ConstVal::Bool(a && b)),
        (BinOp::Or, ConstVal::Bool(a), ConstVal::Bool(b)) => Some(ConstVal::Bool(a || b)),

        _ => None,
    }
}

fn dce(f: &mut FunctionIR) -> bool {
    let mut changed = false;

    // Build a def map: ValueId -> (block_index, inst_index).
    let mut defs: BTreeMap<ValueId, (usize, usize)> = BTreeMap::new();
    for (bi, b) in f.blocks.iter().enumerate() {
        for (ii, inst) in b.insts.iter().enumerate() {
            if let Some(v) = inst.dest {
                defs.insert(v, (bi, ii));
            }
        }
    }

    // Seed: any values referenced by terminators, and any operands of side-effect instructions.
    let mut needed_values: BTreeSet<ValueId> = BTreeSet::new();
    let mut work: VecDeque<ValueId> = VecDeque::new();

    for b in &f.blocks {
        seed_value_uses_from_term(&b.term, &mut needed_values, &mut work);
        for inst in &b.insts {
            if is_side_effecting(&inst.kind) {
                seed_value_uses_from_inst(&inst.kind, &mut needed_values, &mut work);
            }
        }
    }

    while let Some(v) = work.pop_front() {
        let Some((bi, ii)) = defs.get(&v).copied() else { continue };
        let inst = &f.blocks[bi].insts[ii];
        seed_value_uses_from_inst(&inst.kind, &mut needed_values, &mut work);
    }

    for b in &mut f.blocks {
        let before = b.insts.len();
        b.insts.retain(|inst| {
            let Some(dest) = inst.dest else { return true };
            if needed_values.contains(&dest) {
                return true;
            }
            // If it produces a value we don't need, only keep it if it has side effects.
            is_side_effecting(&inst.kind)
        });
        if b.insts.len() != before {
            changed = true;
        }
    }

    changed
}

fn is_side_effecting(k: &InstKind) -> bool {
    matches!(k, InstKind::Call { .. } | InstKind::ComputeKernel { .. } | InstKind::RangeCheckU32 { .. })
}

fn seed_value_uses_from_term(term: &Terminator, needed: &mut BTreeSet<ValueId>, work: &mut VecDeque<ValueId>) {
    match term {
        Terminator::Return(Some(v)) => {
            if needed.insert(*v) {
                work.push_back(*v);
            }
        }
        Terminator::Return(None) => {}
        Terminator::Br(_) => {}
        Terminator::CondBr { cond, .. } => {
            if needed.insert(*cond) {
                work.push_back(*cond);
            }
        }
        Terminator::Switch { scrut, .. } => {
            if needed.insert(*scrut) {
                work.push_back(*scrut);
            }
        }
    }
}

fn seed_value_uses_from_inst(k: &InstKind, needed: &mut BTreeSet<ValueId>, work: &mut VecDeque<ValueId>) {
    let mut use_v = |v: ValueId| {
        if needed.insert(v) {
            work.push_back(v);
        }
    };

    match k {
        InstKind::AllocCapability { .. } => {}
        InstKind::BindStrand { expr, .. } => {
            if let RValue::Local(v) = expr {
                use_v(*v);
            }
        }
        InstKind::Call { args, .. } | InstKind::ComputeKernel { args, .. } => {
            for v in args {
                use_v(*v);
            }
        }
        InstKind::RangeCheckU32 { value, .. } => use_v(*value),
        InstKind::Unary { operand, .. } => use_v(*operand),
        InstKind::Binary { left, right, .. } => {
            use_v(*left);
            use_v(*right);
        }
        InstKind::Phi { incomings } => {
            for (_bb, v) in incomings {
                use_v(*v);
            }
        }
    }
}

fn remove_unreachable_blocks(f: &mut FunctionIR) -> bool {
    let mut reachable: BTreeSet<BlockId> = BTreeSet::new();
    let mut work: VecDeque<BlockId> = VecDeque::new();
    reachable.insert(f.entry);
    work.push_back(f.entry);

    let by_id: BTreeMap<BlockId, usize> = f
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();

    while let Some(bb) = work.pop_front() {
        let Some(&idx) = by_id.get(&bb) else { continue };
        match f.blocks[idx].term {
            Terminator::Return(_) => {}
            Terminator::Br(t) => {
                if reachable.insert(t) {
                    work.push_back(t);
                }
            }
            Terminator::CondBr { then_bb, else_bb, .. } => {
                if reachable.insert(then_bb) {
                    work.push_back(then_bb);
                }
                if reachable.insert(else_bb) {
                    work.push_back(else_bb);
                }
            }
            Terminator::Switch {
                default_bb,
                ref cases,
                ..
            } => {
                if reachable.insert(default_bb) {
                    work.push_back(default_bb);
                }
                for (_k, bb) in cases {
                    if reachable.insert(*bb) {
                        work.push_back(*bb);
                    }
                }
            }
        }
    }

    let before = f.blocks.len();
    if reachable.len() == before {
        return false;
    }

    let dead: BTreeSet<BlockId> = f.blocks.iter().map(|b| b.id).filter(|id| !reachable.contains(id)).collect();

    // Drop dead blocks.
    f.blocks.retain(|b| reachable.contains(&b.id));

    // Clean up phi incomings from dead predecessors; if it collapses, leave it to DCE.
    for b in &mut f.blocks {
        for inst in &mut b.insts {
            if let InstKind::Phi { incomings } = &mut inst.kind {
                incomings.retain(|(pred, _)| !dead.contains(pred));
                if incomings.len() == 1 {
                    let v = incomings[0].1;
                    inst.kind = InstKind::BindStrand {
                        name: "$phi".to_string(),
                        expr: RValue::Local(v),
                    };
                }
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use aura_ast::span;

    use super::*;
    use crate::{BasicBlock, ExecutionHint, Inst, Type};

    fn v(n: u32) -> ValueId {
        ValueId(n)
    }

    fn b(n: u32) -> BlockId {
        BlockId(n)
    }

    #[test]
    fn folds_constants_and_dce_removes_unused() {
        // x = 1
        // y = 2
        // z = x + y  (used)
        // w = x * y  (unused)
        let mut f = FunctionIR {
            name: "f".to_string(),
            span: span(0, 0),
            params: vec![],
            ret: Type::U32,
            blocks: vec![BasicBlock {
                id: b(0),
                span: span(0, 0),
                hint: ExecutionHint::Sequential,
                insts: vec![
                    Inst {
                        span: span(0, 1),
                        dest: Some(v(0)),
                        kind: InstKind::BindStrand {
                            name: "x".to_string(),
                            expr: RValue::ConstU32(1),
                        },
                    },
                    Inst {
                        span: span(2, 1),
                        dest: Some(v(1)),
                        kind: InstKind::BindStrand {
                            name: "y".to_string(),
                            expr: RValue::ConstU32(2),
                        },
                    },
                    Inst {
                        span: span(4, 1),
                        dest: Some(v(2)),
                        kind: InstKind::Binary {
                            op: BinOp::Add,
                            left: v(0),
                            right: v(1),
                        },
                    },
                    Inst {
                        span: span(6, 1),
                        dest: Some(v(3)),
                        kind: InstKind::Binary {
                            op: BinOp::Mul,
                            left: v(0),
                            right: v(1),
                        },
                    },
                ],
                term: Terminator::Return(Some(v(2))),
            }],
            entry: b(0),
        };

        optimize_function(&mut f);

        let insts = &f.blocks[0].insts;
        assert!(insts.iter().any(|i| matches!(&i.kind, InstKind::BindStrand{ expr: RValue::ConstU32(3), .. })), "expected folded z = 3");
        assert!(!insts.iter().any(|i| i.dest == Some(v(3))), "expected unused w to be DCE'd");
    }
}
