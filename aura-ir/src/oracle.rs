#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use crate::{
    BinOp, BlockId, FunctionIR, InstKind, ModuleIR, RValue, Terminator, Type, UnaryOp, ValueId,
};

#[derive(Debug, Clone, PartialEq)]
pub struct OracleOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub return_value: Option<OracleValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OracleValue {
    Unit,
    Bool(bool),
    Char(char),
    U32(u32),
    I64(i64),
    F64(f64),
    String(String),
    Tensor(u32),
    Record {
        name: String,
        fields: HashMap<String, OracleValue>,
    },
    EnumVariant {
        enum_name: String,
        variant_name: String,
        tag: u32,
        args: Vec<OracleValue>,
    },
    Opaque(String),
}

#[derive(Debug, Clone)]
pub struct OracleError {
    pub message: String,
}

impl std::fmt::Display for OracleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for OracleError {}

#[derive(Clone, Debug)]
pub struct OracleConfig {
    pub max_steps: usize,
}

impl Default for OracleConfig {
    fn default() -> Self {
        Self { max_steps: 100_000 }
    }
}

/// Minimal IR semantics oracle.
///
/// Intended for translation validation experiments and differential testing. This intentionally
/// supports only a conservative subset of Aura-IR.
pub fn run_oracle_entry(
    module: &ModuleIR,
    entry: &str,
    args: &[OracleValue],
    cfg: OracleConfig,
) -> Result<OracleOutput, OracleError> {
    validate_module(module)?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    let Some(func) = module.functions.get(entry) else {
        return Err(OracleError {
            message: format!("oracle: entry function '{entry}' not found"),
        });
    };

    let (rv, ok) = run_function(module, func, args, &mut stdout, &mut stderr, cfg)?;

    Ok(OracleOutput {
        ok,
        stdout,
        stderr,
        return_value: rv,
    })
}

/// Basic IR validation used as a per-pass invariant checker.
///
/// This is a lightweight, structural sanity check (not a full verifier).
pub fn validate_module(module: &ModuleIR) -> Result<(), OracleError> {
    for (name, f) in &module.functions {
        validate_function(module, name, f)?;
    }
    Ok(())
}

// EV-193: all non-Phi instruction uses are ordinary SSA uses at their
// instruction position. Phi inputs are different: they are read on incoming
// predecessor edges and are validated separately below.
fn non_phi_operands(inst: &InstKind) -> Vec<ValueId> {
    match inst {
        InstKind::AllocCapability { .. } => Vec::new(),
        InstKind::BindStrand { expr: RValue::Local(value), .. } => vec![*value],
        InstKind::BindStrand { .. } => Vec::new(),
        InstKind::Call { args, .. } | InstKind::ComputeKernel { args, .. } |
        InstKind::ConstructEnumVariant { args, .. } => args.clone(),
        InstKind::RangeCheckU32 { value, .. } | InstKind::RangeCheck { value, .. } => vec![*value],
        InstKind::SetField { base, value, .. } => vec![*base, *value],
        InstKind::ConstructRecord { fields, .. } =>
            fields.iter().map(|(_, value)| *value).collect(),
        InstKind::GetField { base, .. } | InstKind::GetEnumTag { base } |
        InstKind::GetEnumPayload { base, .. } => vec![*base],
        InstKind::Unary { operand, .. } => vec![*operand],
        InstKind::Binary { left, right, .. } => vec![*left, *right],
        InstKind::Phi { .. } => Vec::new(),
    }
}

// EV-194: deliberately limited primitive-category facts available from
// trusted parameter declarations and unambiguous literal definitions.
// This is NOT a complete SSA type inference pass or full IR type checker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KnownPrimitiveKind { Bool, Integer }

fn known_primitive_type(ty: &Type) -> Option<KnownPrimitiveKind> {
    match ty {
        Type::Bool => Some(KnownPrimitiveKind::Bool),
        Type::U8 | Type::U16 | Type::U32 | Type::U64 | Type::U128 | Type::USize |
        Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::I128 | Type::ISize =>
            Some(KnownPrimitiveKind::Integer),
        _ => None,
    }
}

fn validate_function(module: &ModuleIR, name: &str, f: &FunctionIR) -> Result<(), OracleError> {
    if f.name != name {
        return Err(OracleError {
            message: format!("oracle: function key '{name}' mismatches FunctionIR.name='{}'", f.name),
        });
    }

    if f.blocks.is_empty() {
        return Err(OracleError {
            message: format!("oracle: function '{}' has no blocks", f.name),
        });
    }

    let mut blocks_by_id: BTreeMap<BlockId, usize> = BTreeMap::new();
    for (i, bb) in f.blocks.iter().enumerate() {
        if blocks_by_id.insert(bb.id, i).is_some() {
            return Err(OracleError {
                message: format!("oracle: function '{}' has duplicate block id {:?}", f.name, bb.id),
            });
        }
    }

    if !blocks_by_id.contains_key(&f.entry) {
        return Err(OracleError {
            message: format!("oracle: function '{}' entry block {:?} missing", f.name, f.entry),
        });
    }

    for bb in &f.blocks {
        match &bb.term {
            Terminator::Return(_) => {}
            Terminator::Br(b) => {
                if !blocks_by_id.contains_key(b) {
                    return Err(OracleError {
                        message: format!(
                            "oracle: function '{}' terminator branches to missing block {:?}",
                            f.name, b
                        ),
                    });
                }
            }
            Terminator::CondBr { then_bb, else_bb, .. } => {
                if !blocks_by_id.contains_key(then_bb) || !blocks_by_id.contains_key(else_bb) {
                    return Err(OracleError {
                        message: format!(
                            "oracle: function '{}' terminator branches to missing block(s) {:?}/{:?}",
                            f.name, then_bb, else_bb
                        ),
                    });
                }
            }
            Terminator::Switch {
                default_bb,
                cases,
                ..
            } => {
                if !blocks_by_id.contains_key(default_bb) {
                    return Err(OracleError {
                        message: format!(
                            "oracle: function '{}' terminator branches to missing default block {:?}",
                            f.name, default_bb
                        ),
                    });
                }
                for (_k, bb) in cases {
                    if !blocks_by_id.contains_key(bb) {
                        return Err(OracleError {
                            message: format!(
                                "oracle: function '{}' terminator branches to missing case block {:?}",
                                f.name, bb
                            ),
                        });
                    }
                }
            }
        }
    }

    // EV-191: a Phi is an edge-indexed selection, not an arbitrary set of
    // block/value pairs. Establish the reachable CFG from *terminators*,
    // independently of the incoming labels supplied by the IR.
    let mut reachable = BTreeSet::from([f.entry]);
    let mut pending = VecDeque::from([f.entry]);
    let mut predecessors: BTreeMap<BlockId, BTreeSet<BlockId>> = BTreeMap::new();
    while let Some(id) = pending.pop_front() {
        let bb = &f.blocks[blocks_by_id[&id]];
        let edges: Vec<BlockId> = match &bb.term {
            Terminator::Return(_) => Vec::new(),
            Terminator::Br(target) => vec![*target],
            Terminator::CondBr { then_bb, else_bb, .. } => vec![*then_bb, *else_bb],
            Terminator::Switch { default_bb, cases, .. } =>
                std::iter::once(*default_bb)
                    .chain(cases.iter().map(|(_, target)| *target)).collect(),
        };
        for successor in edges {
            predecessors.entry(successor).or_default().insert(id);
            if reachable.insert(successor) {
                pending.push_back(successor);
            }
        }
    }

    // Fixed-point dominators over reachable blocks. Incoming SSA values must
    // be available at the *end* of the predecessor edge (not only somewhere
    // in the function or along a different branch).
    let mut dominators: BTreeMap<BlockId, BTreeSet<BlockId>> = reachable.iter()
        .map(|&bb| (bb, if bb == f.entry {
            BTreeSet::from([bb])
        } else {
            reachable.clone()
        })).collect();
    loop {
        let mut changed = false;
        for &bb in &reachable {
            if bb == f.entry { continue; }
            let mut parent_sets = predecessors.get(&bb).into_iter()
                .flat_map(|ps| ps.iter())
                .map(|pred| dominators[pred].clone());
            let Some(mut intersection) = parent_sets.next() else { continue; };
            for parent_dom in parent_sets {
                intersection = intersection.intersection(&parent_dom).copied().collect();
            }
            intersection.insert(bb);
            if dominators[&bb] != intersection {
                dominators.insert(bb, intersection);
                changed = true;
            }
        }
        if !changed { break; }
    }
    // Record the defining instruction index (params are available from entry).
    // A globally duplicated SSA destination is never a valid redefinition,
    // even if both definitions live in different branches.
    let mut definitions: HashMap<ValueId, (BlockId, Option<usize>)> = HashMap::new();
    for param in &f.params {
        if definitions.insert(param.value, (f.entry, None)).is_some() {
            return Err(OracleError { message: format!(
                "oracle: function '{}' has duplicate parameter SSA definition {:?}",
                f.name, param.value
            ) });
        }
    }
    for bb in &f.blocks {
        for (index, inst) in bb.insts.iter().enumerate() {
            if let Some(value) = inst.dest {
                if definitions.insert(value, (bb.id, Some(index))).is_some() {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' has duplicate SSA definition {:?} in {:?}",
                        f.name, value, bb.id
                    ) });
                }
            }
        }
    }
    // A Phi reads values at block entry, before any ordinary instruction.
    // Even if the source back end currently happens to accept it, a Phi
    // appearing after an ordinary instruction violates our SSA IR contract.
    // Check placement, result and non-empty incoming list in *all* blocks,
    // including unreachable IR that later optimization may expose.
    for bb in &f.blocks {
        let mut has_non_phi = false;
        for inst in &bb.insts {
            if let InstKind::Phi { incomings } = &inst.kind {
                if has_non_phi {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' Phi in {:?} occurs after a non-Phi instruction",
                        f.name, bb.id
                    ) });
                }
                if inst.dest.is_none() {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' Phi in {:?} has no SSA destination",
                        f.name, bb.id
                    ) });
                }
                if incomings.is_empty() {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' Phi in {:?} has no incoming edges",
                        f.name, bb.id
                    ) });
                }
            } else {
                has_non_phi = true;
            }
        }
    }

    for bb in &f.blocks {
        if !reachable.contains(&bb.id) { continue; }
        let actual = predecessors.get(&bb.id).cloned().unwrap_or_default();
        for inst in &bb.insts {
            let InstKind::Phi { incomings } = &inst.kind else { continue; };
            let mut present = BTreeSet::new();
            for (from, value) in incomings {
                if !present.insert(*from) {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' Phi in {:?} has duplicate incoming predecessor {:?}",
                        f.name, bb.id, from
                    ) });
                }
                if !actual.contains(from) {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' Phi in {:?} has incoming {:?} that is not a reachable CFG predecessor",
                        f.name, bb.id, from
                    ) });
                }
                let (def_bb, _) = definitions.get(value).ok_or_else(|| OracleError {
                    message: format!(
                        "oracle: function '{}' Phi in {:?} references undefined incoming SSA value {:?}",
                        f.name, bb.id, value
                    )
                })?;
                if !dominators[from].contains(def_bb) {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' Phi in {:?} incoming {:?} definition in {:?} does not dominate predecessor {:?}",
                        f.name, bb.id, value, def_bb, from
                    ) });
                }
            }
            if present != actual {
                return Err(OracleError { message: format!(
                    "oracle: function '{}' Phi in {:?} missing incoming predecessor(s): expected {:?}, got {:?}",
                    f.name, bb.id, actual, present
                ) });
            }
        }
    }

    // For non-Phi uses, the definition must dominate the *use block*,
    // not merely one incoming predecessor. Inside one block, it must also
    // precede the use. A terminator reads values after the last instruction.
    // All reachable ordinary instruction/terminator operand variants are
    // checked, not just Binary or Return.
    let check_use = |value: ValueId, use_bb: BlockId, before: usize, context: &str|
        -> Result<(), OracleError>
    {
        let &(def_bb, def_pos) = definitions.get(&value).ok_or_else(|| OracleError {
            message: format!(
                "oracle: function '{}' undefined SSA value {:?} in {} of {:?}",
                f.name, value, context, use_bb
            )
        })?;
        if def_bb == use_bb {
            if let Some(pos) = def_pos {
                if pos >= before {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' SSA {:?} use-before-definition in {} of {:?} (definition index {}, use index {})",
                        f.name, value, context, use_bb, pos, before
                    ) });
                }
            }
        } else if !dominators[&use_bb].contains(&def_bb) {
            return Err(OracleError { message: format!(
                "oracle: function '{}' SSA {:?} definition in {:?} does not dominate {} in {:?}",
                f.name, value, def_bb, context, use_bb
            ) });
        }
        Ok(())
    };
    for bb in &f.blocks {
        if !reachable.contains(&bb.id) { continue; }
        for (index, inst) in bb.insts.iter().enumerate() {
            if matches!(&inst.kind, InstKind::Phi { .. }) { continue; }
            for value in non_phi_operands(&inst.kind) {
                check_use(value, bb.id, index, "non-Phi instruction")?;
            }
        }
        let terminal_uses: Vec<ValueId> = match &bb.term {
            Terminator::Return(Some(value)) => vec![*value],
            Terminator::CondBr { cond, .. } => vec![*cond],
            Terminator::Switch { scrut, .. } => vec![*scrut],
            Terminator::Return(None) | Terminator::Br(_) => Vec::new(),
        };
        for value in terminal_uses {
            check_use(value, bb.id, bb.insts.len(), "terminator")?;
        }
    }

    // Fail closed when an operand's primitive category is *already known*.
    // Integer widths may differ legally at source-refined ABI boundaries,
    // so validate Bool-vs-integer only; do not falsely promise width proof.
    // EV-195: derive primitive-category facts only where they follow from
    // a trusted parameter, a literal, a known operation result, or the SSA
    // equality constraint of a Phi. Iterate because loop-header Phi values
    // and backedge Binary expressions can form a cyclic definition graph.
    // Unknown values remain unknown: never default an unresolved producer to
    // U32/Bool, and never reinterpret an existing known category.
    let mut known: HashMap<ValueId, KnownPrimitiveKind> = HashMap::new();
    for p in &f.params {
        if let Some(kind) = known_primitive_type(&p.ty) { known.insert(p.value, kind); }
    }
    loop {
        let mut progressed = false;
        for bb in &f.blocks {
            if !reachable.contains(&bb.id) { continue; }
            for inst in &bb.insts {
                let Some(dest) = inst.dest else { continue; };
                let kind = match &inst.kind {
                    InstKind::BindStrand { expr, .. } => match expr {
                        RValue::ConstBool(_) => Some(KnownPrimitiveKind::Bool),
                        RValue::ConstU32(_) | RValue::ConstI64(_) =>
                            Some(KnownPrimitiveKind::Integer),
                        RValue::Local(value) => known.get(value).copied(),
                        _ => None,
                    },
                    // EV-196: the declared result of a *defined* user Cell
                    // is a stable primitive-category fact. Unknown builtins,
                    // external calls, nominal returns and all widths remain
                    // outside this limited type-provenance guarantee.
                    InstKind::Call { callee, .. } =>
                        module.functions.get(callee)
                            .and_then(|target| known_primitive_type(&target.ret)),
                    InstKind::Unary { op: UnaryOp::Not, operand } =>
                        (known.get(operand) == Some(&KnownPrimitiveKind::Bool))
                            .then_some(KnownPrimitiveKind::Bool),
                    InstKind::Unary { op: UnaryOp::Neg, operand } =>
                        (known.get(operand) == Some(&KnownPrimitiveKind::Integer))
                            .then_some(KnownPrimitiveKind::Integer),
                    InstKind::Binary { op, left, right } => match op {
                        // The IR operators below yield a Bool category
                        // regardless of the primitive *width* of their inputs.
                        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt |
                        BinOp::Le | BinOp::Ge | BinOp::And | BinOp::Or =>
                            Some(KnownPrimitiveKind::Bool),
                        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div =>
                            (known.get(left) == Some(&KnownPrimitiveKind::Integer)
                                && known.get(right) == Some(&KnownPrimitiveKind::Integer))
                                .then_some(KnownPrimitiveKind::Integer),
                    },
                    InstKind::Phi { incomings } => {
                        let mut candidate = None;
                        for (_, incoming) in incomings {
                            if let Some(actual) = known.get(incoming).copied() {
                                if let Some(previous) = candidate {
                                    if previous != actual {
                                        return Err(OracleError { message: format!(
                                            "oracle: function '{}' Phi type mismatch in {:?}: SSA {:?} has {:?} and {:?} incoming categories",
                                            f.name, bb.id, dest, previous, actual
                                        ) });
                                    }
                                } else {
                                    candidate = Some(actual);
                                }
                            }
                        }
                        candidate
                    }
                    _ => None,
                };
                if let Some(actual) = kind {
                    if let Some(previous) = known.get(&dest) {
                        if *previous != actual {
                            return Err(OracleError { message: format!(
                                "oracle: function '{}' SSA type conflict for {:?} in {:?}: {:?} vs {:?}",
                                f.name, dest, bb.id, previous, actual
                            ) });
                        }
                    } else {
                        known.insert(dest, actual);
                        progressed = true;
                    }
                }
            }
        }
        if !progressed { break; }
    }
    // EV-196: an actual SSA argument with a *known* incompatible category
    // cannot satisfy a known internal Cell's parameter category, regardless
    // of whether the argument and parameter have independently valid SSA
    // dominance. Width-specific, Record and extern ABI checks remain at
    // their already-qualified backend/source boundaries.
    for bb in &f.blocks {
        if !reachable.contains(&bb.id) { continue; }
        for inst in &bb.insts {
            let InstKind::Call { callee, args } = &inst.kind else { continue; };
            let Some(target) = module.functions.get(callee) else { continue; };
            for (index, (arg, param)) in args.iter().zip(&target.params).enumerate() {
                if let (Some(required), Some(actual)) =
                    (known_primitive_type(&param.ty), known.get(arg).copied())
                {
                    if actual != required {
                        return Err(OracleError { message: format!(
                            "oracle: function '{}' Call '{}' arg {} type mismatch in {:?}: SSA {:?} has {:?}, declared {:?}",
                            f.name, callee, index, bb.id, arg, actual, required
                        ) });
                    }
                }
            }
        }
    }

    for bb in &f.blocks {
        if !reachable.contains(&bb.id) { continue; }
        match &bb.term {
            Terminator::CondBr { cond, .. } => {
                if matches!(known.get(cond), Some(KnownPrimitiveKind::Integer)) {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' CondBr type mismatch in {:?}: SSA {:?} is known integer, expected Bool",
                        f.name, bb.id, cond
                    ) });
                }
            }
            Terminator::Switch { scrut, .. } => {
                if matches!(known.get(scrut), Some(KnownPrimitiveKind::Bool)) {
                    return Err(OracleError { message: format!(
                        "oracle: function '{}' Switch type mismatch in {:?}: SSA {:?} is Bool, expected integer",
                        f.name, bb.id, scrut
                    ) });
                }
            }
            Terminator::Return(Some(value)) => {
                if let (Some(expected), Some(actual)) =
                    (known_primitive_type(&f.ret), known.get(value).copied())
                {
                    if actual != expected {
                        return Err(OracleError { message: format!(
                            "oracle: function '{}' Return type mismatch in {:?}: SSA {:?} is {:?}, signature expects {:?}",
                            f.name, bb.id, value, actual, expected
                        ) });
                    }
                }
            }
            Terminator::Return(None) | Terminator::Br(_) => {}
        }
    }

    Ok(())
}

fn run_function(
    module: &ModuleIR,
    func: &FunctionIR,
    args: &[OracleValue],
    stdout: &mut String,
    stderr: &mut String,
    cfg: OracleConfig,
) -> Result<(Option<OracleValue>, bool), OracleError> {
    if args.len() != func.params.len() {
        return Err(OracleError {
            message: format!(
                "oracle: function '{}' expected {} args, got {}",
                func.name,
                func.params.len(),
                args.len()
            ),
        });
    }

    let mut env: HashMap<ValueId, OracleValue> = HashMap::new();
    for (p, v) in func.params.iter().zip(args.iter()) {
        env.insert(p.value, v.clone());
    }

    let mut blocks_by_id: HashMap<BlockId, usize> = HashMap::new();
    for (i, bb) in func.blocks.iter().enumerate() {
        blocks_by_id.insert(bb.id, i);
    }

    let mut current = func.entry;
    let mut prev: Option<BlockId> = None;
    let mut steps: usize = 0;

    loop {
        steps += 1;
        if steps > cfg.max_steps {
            return Err(OracleError {
                message: format!("oracle: exceeded max_steps={} (possible infinite loop)", cfg.max_steps),
            });
        }

        let bb_idx = *blocks_by_id.get(&current).ok_or_else(|| OracleError {
            message: format!("oracle: missing block {:?}", current),
        })?;
        let bb = &func.blocks[bb_idx];

        for inst in &bb.insts {
            match &inst.kind {
                InstKind::AllocCapability { .. } => {
                    // Capabilities are not modeled in the oracle.
                    if let Some(dest) = inst.dest {
                        env.insert(dest, OracleValue::Unit);
                    }
                }
                InstKind::BindStrand { expr, .. } => {
                    let v = eval_rvalue(expr, &env)?;
                    if let Some(dest) = inst.dest {
                        env.insert(dest, v);
                    }
                }
                InstKind::RangeCheckU32 { value, lo, hi } | InstKind::RangeCheck { value, lo, hi } => {
                    let v = env.get(value).ok_or_else(|| OracleError {
                        message: format!("oracle: missing value {:?} for range check", value),
                    })?;
                    let val_u64 = match v {
                        OracleValue::U32(u) => *u as u64,
                        OracleValue::I64(i) => *i as u64,
                        _ => {
                            return Err(OracleError {
                                message: "oracle: RangeCheck expects numeric value".to_string(),
                            });
                        }
                    };
                    if val_u64 < *lo || val_u64 > *hi {
                        *stderr = format!(
                            "Aura range check failed: {} not in [{}..{}]\n",
                            val_u64, lo, hi
                        );
                        return Ok((None, false));
                    }
                }
                InstKind::ConstructRecord { record_name, fields } => {
                    let mut rec_fields = HashMap::new();
                    for (name, val_id) in fields {
                        let fv = env.get(val_id).cloned().ok_or_else(|| OracleError {
                            message: format!("oracle: missing field value {:?}", val_id),
                        })?;
                        rec_fields.insert(name.clone(), fv);
                    }
                    if let Some(dest) = inst.dest {
                        env.insert(
                            dest,
                            OracleValue::Record {
                                name: record_name.clone(),
                                fields: rec_fields,
                            },
                        );
                    }
                }
                InstKind::GetField { base, field_name } => {
                    let bv = env.get(base).ok_or_else(|| OracleError {
                        message: format!("oracle: missing record base {:?}", base),
                    })?;
                    let OracleValue::Record { fields, .. } = bv else {
                        return Err(OracleError {
                            message: "oracle: GetField expects Record".to_string(),
                        });
                    };
                    let fv = fields.get(field_name).cloned().ok_or_else(|| OracleError {
                        message: format!("oracle: missing field '{}' in record", field_name),
                    })?;
                    if let Some(dest) = inst.dest {
                        env.insert(dest, fv);
                    }
                }
                InstKind::SetField { base, field_name, value } => {
                    let bv = env.get(base).cloned().ok_or_else(|| OracleError {
                        message: format!("oracle: missing record base {:?}", base),
                    })?;
                    let OracleValue::Record { name, mut fields } = bv else {
                        return Err(OracleError {
                            message: "oracle: SetField expects Record".to_string(),
                        });
                    };
                    let fv = env.get(value).cloned().ok_or_else(|| OracleError {
                        message: format!("oracle: missing set field value {:?}", value),
                    })?;
                    fields.insert(field_name.clone(), fv);
                    if let Some(dest) = inst.dest {
                        env.insert(dest, OracleValue::Record { name, fields });
                    }
                }
                InstKind::ConstructEnumVariant { enum_name, variant_name, tag, args } => {
                    let mut arg_vals = Vec::with_capacity(args.len());
                    for a in args {
                        let av = env.get(a).cloned().ok_or_else(|| OracleError {
                            message: format!("oracle: missing enum arg {:?}", a),
                        })?;
                        arg_vals.push(av);
                    }
                    if let Some(dest) = inst.dest {
                        env.insert(
                            dest,
                            OracleValue::EnumVariant {
                                enum_name: enum_name.clone(),
                                variant_name: variant_name.clone(),
                                tag: *tag,
                                args: arg_vals,
                            },
                        );
                    }
                }
                InstKind::GetEnumTag { base } => {
                    let bv = env.get(base).ok_or_else(|| OracleError {
                        message: format!("oracle: missing enum base {:?}", base),
                    })?;
                    let tag = match bv {
                        OracleValue::EnumVariant { tag, .. } => *tag,
                        _ => return Err(OracleError {
                            message: "oracle: GetEnumTag expects EnumVariant".to_string(),
                        }),
                    };
                    if let Some(dest) = inst.dest {
                        env.insert(dest, OracleValue::U32(tag));
                    }
                }
                InstKind::GetEnumPayload { base, payload_index, .. } => {
                    let bv = env.get(base).ok_or_else(|| OracleError {
                        message: format!("oracle: missing enum base {:?}", base),
                    })?;
                    let val = match bv {
                        OracleValue::EnumVariant { args, .. } => {
                            args.get(*payload_index).cloned().ok_or_else(|| OracleError {
                                message: format!("oracle: invalid payload index {}", payload_index),
                            })?
                        }
                        _ => return Err(OracleError {
                            message: "oracle: GetEnumPayload expects EnumVariant".to_string(),
                        }),
                    };
                    if let Some(dest) = inst.dest {
                        env.insert(dest, val);
                    }
                }
                InstKind::Unary { op, operand } => {
                    let v = env.get(operand).ok_or_else(|| OracleError {
                        message: format!("oracle: missing operand {:?}", operand),
                    })?;
                    let out = eval_unary(*op, v)?;
                    if let Some(dest) = inst.dest {
                        env.insert(dest, out);
                    }
                }
                InstKind::Binary { op, left, right } => {
                    let l = env.get(left).ok_or_else(|| OracleError {
                        message: format!("oracle: missing left operand {:?}", left),
                    })?;
                    let r = env.get(right).ok_or_else(|| OracleError {
                        message: format!("oracle: missing right operand {:?}", right),
                    })?;
                    let out = eval_binary(*op, l, r)?;
                    if let Some(dest) = inst.dest {
                        env.insert(dest, out);
                    }
                }
                InstKind::Phi { incomings } => {
                    let pred = prev.ok_or_else(|| OracleError {
                        message: "oracle: phi executed with no predecessor".to_string(),
                    })?;
                    let mut chosen: Option<ValueId> = None;
                    for (from, v) in incomings {
                        if *from == pred {
                            chosen = Some(*v);
                            break;
                        }
                    }
                    let Some(chosen) = chosen else {
                        return Err(OracleError {
                            message: format!("oracle: phi has no incoming for predecessor {:?}", pred),
                        });
                    };
                    let v = env.get(&chosen).ok_or_else(|| OracleError {
                        message: format!("oracle: phi incoming value {:?} missing", chosen),
                    })?;
                    if let Some(dest) = inst.dest {
                        env.insert(dest, v.clone());
                    }
                }
                InstKind::Call { callee, args } | InstKind::ComputeKernel { callee, args } => {
                    let call_args = args
                        .iter()
                        .map(|id| {
                            env.get(id)
                                .cloned()
                                .ok_or_else(|| OracleError {
                                    message: format!("oracle: missing call arg {:?}", id),
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    let ret = if let Some(f) = module.functions.get(callee) {
                        let (rv, ok) = run_function(module, f, &call_args, stdout, stderr, cfg.clone())?;
                        if !ok {
                            return Ok((None, false));
                        }
                        rv
                    } else if is_modeled_runtime_callee(callee) {
                        run_extern(callee, &call_args, stdout)?
                    } else {
                        return Err(OracleError {
                            message: format!("oracle: extern '{}' not modeled", callee),
                        });
                    };

                    if let Some(dest) = inst.dest {
                        if let Some(rv) = ret {
                            env.insert(dest, rv);
                        } else {
                            env.insert(dest, OracleValue::Unit);
                        }
                    }
                }
            }
        }

        match &bb.term {
            Terminator::Return(v) => {
                let rv = match v {
                    None => None,
                    Some(id) => Some(
                        env.get(id)
                            .cloned()
                            .ok_or_else(|| OracleError {
                                message: format!("oracle: missing return value {:?}", id),
                            })?,
                    ),
                };
                return Ok((rv, true));
            }
            Terminator::Br(next) => {
                prev = Some(current);
                current = *next;
            }
            Terminator::CondBr { cond, then_bb, else_bb } => {
                let v = env.get(cond).ok_or_else(|| OracleError {
                    message: format!("oracle: missing cond {:?}", cond),
                })?;
                let OracleValue::Bool(b) = v else {
                    return Err(OracleError {
                        message: "oracle: CondBr expects Bool".to_string(),
                    });
                };
                prev = Some(current);
                current = if *b { *then_bb } else { *else_bb };
            }
            Terminator::Switch {
                scrut,
                default_bb,
                cases,
            } => {
                let v = env.get(scrut).ok_or_else(|| OracleError {
                    message: format!("oracle: missing switch scrut {:?}", scrut),
                })?;
                let key: u64 = match v {
                    OracleValue::U32(n) => (*n) as u64,
                    OracleValue::I64(n) => (*n) as u64,
                    OracleValue::Bool(b) => if *b { 1 } else { 0 },
                    OracleValue::Tensor(n) => (*n) as u64,
                    _ => {
                        return Err(OracleError {
                            message: "oracle: Switch expects U32/I64/Bool/Tensor".to_string(),
                        });
                    }
                };

                let mut target = *default_bb;
                for (k, bb) in cases {
                    if *k == key {
                        target = *bb;
                        break;
                    }
                }

                prev = Some(current);
                current = target;
            }
        }
    }
}

fn eval_rvalue(rv: &RValue, env: &HashMap<ValueId, OracleValue>) -> Result<OracleValue, OracleError> {
    Ok(match rv {
        RValue::ConstU32(u) => OracleValue::U32(*u as u32),
        RValue::ConstI64(i) => OracleValue::I64(*i),
        RValue::ConstF64(f) => OracleValue::F64(*f),
        RValue::ConstBool(b) => OracleValue::Bool(*b),
        RValue::ConstChar(c) => OracleValue::Char(*c),
        RValue::ConstString(s) => OracleValue::String(s.clone()),
        RValue::Local(id) => env.get(id).cloned().ok_or_else(|| OracleError {
            message: format!("oracle: missing local {:?}", id),
        })?,
    })
}

fn eval_unary(op: UnaryOp, v: &OracleValue) -> Result<OracleValue, OracleError> {
    match (op, v) {
        (UnaryOp::Neg, OracleValue::U32(x)) => Ok(OracleValue::U32(x.wrapping_neg())),
        (UnaryOp::Neg, OracleValue::I64(x)) => Ok(OracleValue::I64(x.wrapping_neg())),
        (UnaryOp::Neg, OracleValue::F64(x)) => Ok(OracleValue::F64(-x)),
        (UnaryOp::Not, OracleValue::Bool(b)) => Ok(OracleValue::Bool(!b)),
        _ => Err(OracleError {
            message: format!("oracle: unsupported unary op {:?} for value {:?}", op, v),
        }),
    }
}

fn eval_binary(op: BinOp, l: &OracleValue, r: &OracleValue) -> Result<OracleValue, OracleError> {
    use BinOp::*;

    match (op, l, r) {
        (Add, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::U32(a.wrapping_add(*b))),
        (Sub, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::U32(a.wrapping_sub(*b))),
        (Mul, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::U32(a.wrapping_mul(*b))),
        (Div, OracleValue::U32(a), OracleValue::U32(b)) => {
            if *b == 0 {
                return Err(OracleError {
                    message: "oracle: division by zero".to_string(),
                });
            }
            Ok(OracleValue::U32(a / b))
        }

        (Add, OracleValue::I64(a), OracleValue::I64(b)) => Ok(OracleValue::I64(a.wrapping_add(*b))),
        (Sub, OracleValue::I64(a), OracleValue::I64(b)) => Ok(OracleValue::I64(a.wrapping_sub(*b))),
        (Mul, OracleValue::I64(a), OracleValue::I64(b)) => Ok(OracleValue::I64(a.wrapping_mul(*b))),
        (Div, OracleValue::I64(a), OracleValue::I64(b)) => {
            if *b == 0 {
                return Err(OracleError {
                    message: "oracle: division by zero".to_string(),
                });
            }
            Ok(OracleValue::I64(a / b))
        }

        (Add, OracleValue::F64(a), OracleValue::F64(b)) => Ok(OracleValue::F64(a + b)),
        (Sub, OracleValue::F64(a), OracleValue::F64(b)) => Ok(OracleValue::F64(a - b)),
        (Mul, OracleValue::F64(a), OracleValue::F64(b)) => Ok(OracleValue::F64(a * b)),
        (Div, OracleValue::F64(a), OracleValue::F64(b)) => Ok(OracleValue::F64(a / b)),

        (Eq, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a == b)),
        (Ne, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a != b)),
        (Lt, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a < b)),
        (Gt, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a > b)),
        (Le, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a <= b)),
        (Ge, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a >= b)),

        (Eq, OracleValue::I64(a), OracleValue::I64(b)) => Ok(OracleValue::Bool(a == b)),
        (Ne, OracleValue::I64(a), OracleValue::I64(b)) => Ok(OracleValue::Bool(a != b)),
        (Lt, OracleValue::I64(a), OracleValue::I64(b)) => Ok(OracleValue::Bool(a < b)),
        (Gt, OracleValue::I64(a), OracleValue::I64(b)) => Ok(OracleValue::Bool(a > b)),
        (Le, OracleValue::I64(a), OracleValue::I64(b)) => Ok(OracleValue::Bool(a <= b)),
        (Ge, OracleValue::I64(a), OracleValue::I64(b)) => Ok(OracleValue::Bool(a >= b)),

        (Eq, OracleValue::Bool(a), OracleValue::Bool(b)) => Ok(OracleValue::Bool(a == b)),
        (Ne, OracleValue::Bool(a), OracleValue::Bool(b)) => Ok(OracleValue::Bool(a != b)),

        (And, OracleValue::Bool(a), OracleValue::Bool(b)) => Ok(OracleValue::Bool(*a && *b)),
        (Or, OracleValue::Bool(a), OracleValue::Bool(b)) => Ok(OracleValue::Bool(*a || *b)),

        _ => Err(OracleError {
            message: format!("oracle: unsupported binary op {:?} for values {:?} and {:?}", op, l, r),
        }),
    }
}

fn is_modeled_runtime_callee(callee: &str) -> bool {
    // Deliberate allowlist: adding a runtime builtin here also requires
    // implementing its exact semantics in run_extern().
    matches!(callee, "io.println")
}

#[cfg(test)]
mod oracle_runtime_dispatch_tests {
    use super::{is_modeled_runtime_callee, run_extern, OracleValue};

    #[test]
    fn modeled_runtime_callee_allowlist_is_exact() {
        assert!(is_modeled_runtime_callee("io.println"));
        assert!(!is_modeled_runtime_callee("io.print"));
        assert!(!is_modeled_runtime_callee("unknown.fn"));
    }

    #[test]
    fn modeled_io_println_semantics_are_stable() {
        let mut stdout = String::new();
        let result = run_extern(
            "io.println",
            &[OracleValue::String("hello".to_string())],
            &mut stdout,
        )
        .expect("io.println must be modeled");

        assert_eq!(result, None);
        assert_eq!(stdout, "hello\n");
    }

    #[test]
    fn unmodeled_runtime_callee_remains_rejected() {
        let mut stdout = String::new();
        let err = run_extern("unknown.fn", &[], &mut stdout)
            .expect_err("unmodeled runtime callee must remain rejected");

        assert_eq!(err.message, "oracle: extern 'unknown.fn' not modeled");
        assert!(stdout.is_empty());
    }
}

fn run_extern(
    callee: &str,
    args: &[OracleValue],
    stdout: &mut String,
) -> Result<Option<OracleValue>, OracleError> {
    match callee {
        "io.println" => {
            let [OracleValue::String(s)] = args else {
                return Err(OracleError {
                    message: "oracle: io.println expects a single String arg".to_string(),
                });
            };
            stdout.push_str(s);
            if !stdout.ends_with('\n') {
                stdout.push('\n');
            }
            Ok(None)
        }
        _ => Err(OracleError {
            message: format!("oracle: extern '{callee}' not modeled"),
        }),
    }
}

pub fn oracle_type_of(v: &OracleValue) -> Type {
    match v {
        OracleValue::Unit => Type::Unit,
        OracleValue::Bool(_) => Type::Bool,
        OracleValue::Char(_) => Type::Char,
        OracleValue::U32(_) => Type::U32,
        OracleValue::I64(_) => Type::I64,
        OracleValue::F64(_) => Type::F64,
        OracleValue::String(_) => Type::String,
        OracleValue::Tensor(_) => Type::Tensor,
        OracleValue::Record { name, .. } => Type::Opaque(name.clone()),
        OracleValue::EnumVariant { enum_name, .. } => Type::Opaque(enum_name.clone()),
        OracleValue::Opaque(s) => Type::Opaque(s.clone()),
    }
}
