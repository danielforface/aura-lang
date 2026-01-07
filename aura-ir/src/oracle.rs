#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashMap};

use crate::{
    BinOp, BlockId, FunctionIR, InstKind, ModuleIR, RValue, Terminator, Type, UnaryOp, ValueId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub return_value: Option<OracleValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleValue {
    Unit,
    Bool(bool),
    U32(u32),
    String(String),
    Tensor(u32),
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
        validate_function(name, f)?;
    }
    Ok(())
}

fn validate_function(name: &str, f: &FunctionIR) -> Result<(), OracleError> {
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
                InstKind::RangeCheckU32 { value, lo, hi } => {
                    let v = env.get(value).ok_or_else(|| OracleError {
                        message: format!("oracle: missing value {:?} for range check", value),
                    })?;
                    let OracleValue::U32(u) = v else {
                        return Err(OracleError {
                            message: "oracle: RangeCheckU32 expects U32".to_string(),
                        });
                    };
                    if (*u as u64) < *lo || (*u as u64) > *hi {
                        *stderr = format!(
                            "Aura range check failed: {} not in [{}..{}]\n",
                            u, lo, hi
                        );
                        return Ok((None, false));
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
                    } else if module.externs.contains_key(callee) {
                        run_extern(callee, &call_args, stdout)?
                    } else {
                        return Err(OracleError {
                            message: format!("oracle: unknown callee '{callee}'"),
                        });
                    };

                    if let Some(dest) = inst.dest {
                        if let Some(v) = ret {
                            env.insert(dest, v);
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
                    OracleValue::Bool(b) => if *b { 1 } else { 0 },
                    OracleValue::Tensor(n) => (*n) as u64,
                    _ => {
                        return Err(OracleError {
                            message: "oracle: Switch expects U32/Bool/Tensor".to_string(),
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
        RValue::ConstBool(b) => OracleValue::Bool(*b),
        RValue::ConstString(s) => OracleValue::String(s.clone()),
        RValue::Local(id) => env.get(id).cloned().ok_or_else(|| OracleError {
            message: format!("oracle: missing local {:?}", id),
        })?,
    })
}

fn eval_unary(op: UnaryOp, v: &OracleValue) -> Result<OracleValue, OracleError> {
    match (op, v) {
        (UnaryOp::Neg, OracleValue::U32(x)) => Ok(OracleValue::U32(x.wrapping_neg())),
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

        (Eq, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a == b)),
        (Ne, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a != b)),
        (Lt, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a < b)),
        (Gt, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a > b)),
        (Le, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a <= b)),
        (Ge, OracleValue::U32(a), OracleValue::U32(b)) => Ok(OracleValue::Bool(a >= b)),

        (Eq, OracleValue::Bool(a), OracleValue::Bool(b)) => Ok(OracleValue::Bool(a == b)),
        (Ne, OracleValue::Bool(a), OracleValue::Bool(b)) => Ok(OracleValue::Bool(a != b)),

        (And, OracleValue::Bool(a), OracleValue::Bool(b)) => Ok(OracleValue::Bool(*a && *b)),
        (Or, OracleValue::Bool(a), OracleValue::Bool(b)) => Ok(OracleValue::Bool(*a || *b)),

        _ => Err(OracleError {
            message: format!("oracle: unsupported binary op {:?} for values {:?} and {:?}", op, l, r),
        }),
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
        OracleValue::U32(_) => Type::U32,
        OracleValue::String(_) => Type::String,
        OracleValue::Tensor(_) => Type::Tensor,
        OracleValue::Opaque(s) => Type::Opaque(s.clone()),
    }
}
