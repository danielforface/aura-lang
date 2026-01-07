use std::path::PathBuf;

use aura_ir::{
    BinOp, BlockId, ExecutionHint, FunctionIR, IdGen, Inst, InstKind, ModuleIR, RValue, Terminator,
    UnaryOp, ValueId,
};
use miette::{IntoDiagnostic, Result};

fn span0() -> aura_ast::Span {
    aura_ast::Span::new(miette::SourceOffset::from(0usize), 0usize)
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ir")
}

fn backend_ok_or_explicit_reject(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("unsupported") || lower.contains("unimplemented") || lower.contains("not enabled")
}

#[derive(Debug)]
struct ParsedBlock {
    id: BlockId,
    insts: Vec<Inst>,
    term: Option<Terminator>,
}

fn parse_value_id(tok: &str) -> Result<ValueId> {
    let s = tok.trim();
    let s = s.strip_prefix('%').unwrap_or(s);
    let n: u32 = s.parse().into_diagnostic()?;
    Ok(ValueId(n))
}

fn parse_block_id(tok: &str) -> Result<BlockId> {
    let n: u32 = tok.trim().parse().into_diagnostic()?;
    Ok(BlockId(n))
}

fn parse_rvalue(parts: &[&str]) -> Result<RValue> {
    match parts {
        ["const_u32", n] => Ok(RValue::ConstU32(n.parse::<u64>().into_diagnostic()?)),
        ["const_bool", b] => Ok(RValue::ConstBool(*b == "true")),
        ["const_string", s] => {
            // Expect a single token like "hello" (quotes included).
            let raw = s.trim();
            let unq = raw
                .strip_prefix('"')
                .and_then(|r| r.strip_suffix('"'))
                .unwrap_or(raw);
            Ok(RValue::ConstString(unq.to_string()))
        }
        ["local", v] => Ok(RValue::Local(parse_value_id(v)?)),
        _ => Err(miette::miette!("invalid rvalue syntax: {parts:?}")),
    }
}

fn parse_unary(op: &str) -> Result<UnaryOp> {
    match op {
        "neg" => Ok(UnaryOp::Neg),
        "not" => Ok(UnaryOp::Not),
        _ => Err(miette::miette!("unknown unary op '{op}'")),
    }
}

fn parse_binary(op: &str) -> Result<BinOp> {
    match op {
        "add" => Ok(BinOp::Add),
        "sub" => Ok(BinOp::Sub),
        "mul" => Ok(BinOp::Mul),
        "div" => Ok(BinOp::Div),

        "eq" => Ok(BinOp::Eq),
        "ne" => Ok(BinOp::Ne),
        "lt" => Ok(BinOp::Lt),
        "gt" => Ok(BinOp::Gt),
        "le" => Ok(BinOp::Le),
        "ge" => Ok(BinOp::Ge),

        "and" => Ok(BinOp::And),
        "or" => Ok(BinOp::Or),

        _ => Err(miette::miette!("unknown binary op '{op}'")),
    }
}

fn parse_args_list(s: &str) -> Result<Vec<ValueId>> {
    let s = s.trim();
    let inner = s
        .strip_prefix('(')
        .and_then(|r| r.strip_suffix(')'))
        .ok_or_else(|| miette::miette!("expected arg list '(...)', got '{s}'"))?;
    let inner = inner.trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|t| parse_value_id(t.trim()))
        .collect()
}

fn parse_phi_incomings(s: &str) -> Result<Vec<(BlockId, ValueId)>> {
    // Example: (1 %1), (2 %2)
    let mut out = Vec::new();
    for item in s.split("),") {
        let item = item.trim();
        let item = item.strip_suffix(')').unwrap_or(item);
        let item = item.trim();
        let item = item
            .strip_prefix('(')
            .ok_or_else(|| miette::miette!("phi incoming must start with '(', got '{item}'"))?;
        let mut parts = item.split_whitespace();
        let bb = parts
            .next()
            .ok_or_else(|| miette::miette!("phi incoming missing block id"))?;
        let v = parts
            .next()
            .ok_or_else(|| miette::miette!("phi incoming missing value id"))?;
        out.push((parse_block_id(bb)?, parse_value_id(v)?));
    }
    Ok(out)
}

fn parse_fixture_to_module(src: &str) -> Result<ModuleIR> {
    let mut blocks: Vec<ParsedBlock> = Vec::new();
    let mut current: Option<ParsedBlock> = None;

    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("block ") {
            let rest = rest
                .strip_suffix(':')
                .ok_or_else(|| miette::miette!("block header must end with ':'"))?;
            let id = parse_block_id(rest.trim())?;
            if let Some(b) = current.take() {
                blocks.push(b);
            }
            current = Some(ParsedBlock {
                id,
                insts: Vec::new(),
                term: None,
            });
            continue;
        }

        let Some(b) = current.as_mut() else {
            return Err(miette::miette!("instruction outside of block: '{line}'"));
        };

        // Terminators.
        if line == "ret" {
            b.term = Some(Terminator::Return(None));
            continue;
        }
        if let Some(v) = line.strip_prefix("ret ") {
            b.term = Some(Terminator::Return(Some(parse_value_id(v.trim())?)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("br ") {
            b.term = Some(Terminator::Br(parse_block_id(rest.trim())?));
            continue;
        }
        if let Some(rest) = line.strip_prefix("condbr ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 3 {
                return Err(miette::miette!(
                    "condbr expects: condbr %cond <then> <else>"
                ));
            }
            b.term = Some(Terminator::CondBr {
                cond: parse_value_id(parts[0])?,
                then_bb: parse_block_id(parts[1])?,
                else_bb: parse_block_id(parts[2])?,
            });
            continue;
        }

        // Non-dest instructions.
        if let Some(rest) = line.strip_prefix("call ") {
            let (callee, args_s) = rest
                .split_once(' ')
                .ok_or_else(|| miette::miette!("call expects: call <callee> (<args>)"))?;
            let args = parse_args_list(args_s.trim())?;
            b.insts.push(Inst {
                span: span0(),
                dest: None,
                kind: InstKind::Call {
                    callee: callee.to_string(),
                    args,
                },
            });
            continue;
        }
        if let Some(rest) = line.strip_prefix("compute_kernel ") {
            let (callee, args_s) = rest.split_once(' ').ok_or_else(|| {
                miette::miette!("compute_kernel expects: compute_kernel <callee> (<args>)")
            })?;
            let args = parse_args_list(args_s.trim())?;
            b.insts.push(Inst {
                span: span0(),
                dest: None,
                kind: InstKind::ComputeKernel {
                    callee: callee.to_string(),
                    args,
                },
            });
            continue;
        }
        if let Some(rest) = line.strip_prefix("range_check_u32 ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 3 {
                return Err(miette::miette!(
                    "range_check_u32 expects: range_check_u32 %v <lo> <hi>"
                ));
            }
            b.insts.push(Inst {
                span: span0(),
                dest: None,
                kind: InstKind::RangeCheckU32 {
                    value: parse_value_id(parts[0])?,
                    lo: parts[1].parse::<u64>().into_diagnostic()?,
                    hi: parts[2].parse::<u64>().into_diagnostic()?,
                },
            });
            continue;
        }

        // Dest-form instructions: "%N = ..."
        let (dest_s, rest) = line
            .split_once('=')
            .ok_or_else(|| miette::miette!("expected instruction or terminator, got '{line}'"))?;
        let dest = parse_value_id(dest_s.trim())?;
        let rest = rest.trim();

        if let Some(name) = rest.strip_prefix("alloc_capability ") {
            b.insts.push(Inst {
                span: span0(),
                dest: Some(dest),
                kind: InstKind::AllocCapability {
                    name: name.trim().to_string(),
                },
            });
            continue;
        }

        if let Some(rest) = rest.strip_prefix("bind_strand ") {
            // bind_strand <name> <rvalue...>
            let mut parts = rest.split_whitespace();
            let name = parts
                .next()
                .ok_or_else(|| miette::miette!("bind_strand missing name"))?;
            let rv_parts: Vec<&str> = parts.collect();
            let rv = parse_rvalue(&rv_parts)?;
            b.insts.push(Inst {
                span: span0(),
                dest: Some(dest),
                kind: InstKind::BindStrand {
                    name: name.to_string(),
                    expr: rv,
                },
            });
            continue;
        }

        if let Some(rest) = rest.strip_prefix("unary ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(miette::miette!("unary expects: unary <op> %v"));
            }
            b.insts.push(Inst {
                span: span0(),
                dest: Some(dest),
                kind: InstKind::Unary {
                    op: parse_unary(parts[0])?,
                    operand: parse_value_id(parts[1])?,
                },
            });
            continue;
        }

        if let Some(rest) = rest.strip_prefix("binary ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() != 3 {
                return Err(miette::miette!("binary expects: binary <op> %l %r"));
            }
            b.insts.push(Inst {
                span: span0(),
                dest: Some(dest),
                kind: InstKind::Binary {
                    op: parse_binary(parts[0])?,
                    left: parse_value_id(parts[1])?,
                    right: parse_value_id(parts[2])?,
                },
            });
            continue;
        }

        if let Some(rest) = rest.strip_prefix("phi ") {
            b.insts.push(Inst {
                span: span0(),
                dest: Some(dest),
                kind: InstKind::Phi {
                    incomings: parse_phi_incomings(rest.trim())?,
                },
            });
            continue;
        }

        return Err(miette::miette!("unknown instruction: '{line}'"));
    }

    if let Some(b) = current.take() {
        blocks.push(b);
    }

    if blocks.is_empty() {
        return Err(miette::miette!("fixture has no blocks"));
    }

    let entry = blocks[0].id;

    let mut f_blocks = Vec::new();
    for b in blocks {
        let term = b
            .term
            .ok_or_else(|| miette::miette!("block {:?} missing terminator", b.id))?;
        f_blocks.push(aura_ir::BasicBlock {
            id: b.id,
            span: span0(),
            hint: ExecutionHint::Sequential,
            insts: b.insts,
            term,
        });
    }

    let mut m = ModuleIR::new();
    m.functions.insert(
        "main".to_string(),
        FunctionIR {
            name: "main".to_string(),
            span: span0(),
            params: Vec::new(),
            ret: aura_ir::Type::Unit,
            blocks: f_blocks,
            entry,
        },
    );

    // Simple sanity: ensure IDs are stable (no-op today but keeps invariants obvious).
    let _ = IdGen::default();

    Ok(m)
}

#[test]
fn ir_golden_fixtures_emit_or_explicit_reject() -> Result<()> {
    let dir = fixtures_dir();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_diagnostic()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("air"))
        .collect();
    entries.sort();

    if entries.is_empty() {
        return Err(miette::miette!("no IR fixtures found under {}", dir.display()));
    }

    for p in entries {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("fixture");
        let src = std::fs::read_to_string(&p).into_diagnostic()?;
        let module = parse_fixture_to_module(&src)?;

        // C backend must emit or explicitly reject.
        match aura_backend_c::emit_module(&module, None) {
            Ok(_a) => {}
            Err(e) => {
                assert!(
                    backend_ok_or_explicit_reject(&e.message),
                    "C backend must either implement or explicitly reject fixture '{name}'; got: {e:?}"
                );
            }
        }

        // LLVM backend must emit or explicitly reject.
        match aura_backend_llvm::emit_llvm_ir(&module, None) {
            Ok(_a) => {}
            Err(e) => {
                assert!(
                    backend_ok_or_explicit_reject(&e.message),
                    "LLVM backend must either implement or explicitly reject fixture '{name}'; got: {e:?}"
                );
            }
        }
    }

    Ok(())
}
