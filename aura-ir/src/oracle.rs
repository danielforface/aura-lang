#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashMap};

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
