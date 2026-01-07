#![forbid(unsafe_code)]

use std::collections::HashMap;

use aura_ir::{
    BinOp, BlockId, DebugSource, FunctionIR, InstKind, ModuleIR, RValue, Terminator, Type, UnaryOp,
    ValueId,
};
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("C backend error: {message}")]
#[diagnostic(code(aura::backend_c))]
#[allow(unused_assignments)]
pub struct CBackendError {
    pub message: String,
}

pub struct CArtifacts {
    pub runtime_h: String,
    pub module_c: String,
}

pub fn emit_module(module: &ModuleIR, debug: Option<&DebugSource>) -> Result<CArtifacts, CBackendError> {
    let mut module = module.clone();
    aura_ir::optimize_module(&mut module);

    if let Err(e) = aura_ir::validate_module(&module) {
        return Err(CBackendError {
            message: format!("IR validation failed before C emission: {e}"),
        });
    }

    let runtime_h = emit_runtime_h();
    let module_c = emit_module_c(&module, debug);
    Ok(CArtifacts { runtime_h, module_c })
}

fn emit_runtime_h() -> String {
    // A tiny runtime that is:
    // - portable (falls back when C11 threads are unavailable)
    // - runnable (stub IO + compute)
    let mut out = String::new();
    out.push_str("#pragma once\n");
    out.push_str("#include <stdint.h>\n");
    out.push_str("#include <stdbool.h>\n");
    out.push_str("#include <stdio.h>\n\n");
    out.push_str("#include <stdlib.h>\n\n");

    // From aura-stdlib (linked by the CLI when running the C backend).
    out.push_str("void aura_io_println(const char* s);\n\n");

    out.push_str("// ---- Aura runtime (prototype) ----\n\n");
    out.push_str("typedef struct Tensor { uint32_t id; } Tensor;\n\n");

    out.push_str("static inline void AURA_RANGE_CHECK_U32(uint32_t v, uint32_t lo, uint32_t hi, const char* what) {\n");
    out.push_str("  if (v < lo || v > hi) {\n");
    out.push_str("    fprintf(stderr, \"Aura range check failed for %s: %u not in [%u..%u]\\n\", what, v, lo, hi);\n");
    out.push_str("    // Fail-fast for prototype runtime\n");
    out.push_str("    fflush(stderr);\n");
    out.push_str("    abort();\n");
    out.push_str("  }\n");
    out.push_str("}\n\n");

    out.push_str("// Builtins used by main.aura\n");
    out.push_str("static inline Tensor io_load_tensor(const char* path) {\n");
    out.push_str("  (void)path;\n");
    out.push_str("  Tensor t; t.id = 42;\n");
    out.push_str("  return t;\n");
    out.push_str("}\n\n");

    out.push_str("static inline void io_display(Tensor t) {\n");
    out.push_str("  printf(\"Tensor{id=%u}\\n\", t.id);\n");
    out.push_str("}\n\n");

    out.push_str("static inline Tensor compute_gradient(Tensor data, uint32_t weight) {\n");
    out.push_str("  Tensor out;\n");
    out.push_str("  out.id = data.id + weight;\n");
    out.push_str("  return out;\n");
    out.push_str("}\n\n");

    // Async execution wrappers.
    out.push_str("// ~> async execution (prototype)\n");
    out.push_str("#if defined(__STDC_NO_THREADS__)\n");
    out.push_str("  #define aura_async_tensor2(fn, a0, a1) (fn((a0), (a1)))\n");
    out.push_str("#else\n");
    out.push_str("  #include <threads.h>\n");
    out.push_str("  typedef struct aura_task_tensor2 {\n");
    out.push_str("    Tensor (*fn)(Tensor, uint32_t);\n");
    out.push_str("    Tensor a0;\n");
    out.push_str("    uint32_t a1;\n");
    out.push_str("    Tensor out;\n");
    out.push_str("  } aura_task_tensor2;\n");
    out.push_str("  static int aura_task_tensor2_entry(void* p) {\n");
    out.push_str("    aura_task_tensor2* t = (aura_task_tensor2*)p;\n");
    out.push_str("    t->out = t->fn(t->a0, t->a1);\n");
    out.push_str("    return 0;\n");
    out.push_str("  }\n");
    out.push_str("  static inline Tensor aura_async_tensor2(Tensor (*fn)(Tensor, uint32_t), Tensor a0, uint32_t a1) {\n");
    out.push_str("    aura_task_tensor2 task;\n");
    out.push_str("    task.fn = fn; task.a0 = a0; task.a1 = a1;\n");
    out.push_str("    thrd_t thr;\n");
    out.push_str("    thrd_create(&thr, aura_task_tensor2_entry, &task);\n");
    out.push_str("    thrd_join(thr, 0);\n");
    out.push_str("    return task.out;\n");
    out.push_str("  }\n");
    out.push_str("#endif\n\n");

    out
}

fn emit_module_c(module: &ModuleIR, debug: Option<&DebugSource>) -> String {
    let mut out = String::new();
    out.push_str("// Proof-carrying: IR validated by aura_ir::validate_module\n");
    out.push_str("#include \"aura_runtime.h\"\n\n");

    // Determine function return types by terminator shape (prototype).
    let mut ret_map: HashMap<String, CType> = HashMap::new();
    for (name, f) in &module.functions {
        ret_map.insert(name.clone(), function_return_ctype(f));
    }

    // Emit non-entry functions first.
    for (name, f) in &module.functions {
        if name == "main" {
            continue;
        }
        emit_function(&mut out, debug, f, &ret_map);
        out.push('\n');
    }

    // Emit Aura entry block as `void aura_main(void)` then C main.
    if let Some(main_f) = module.functions.get("main") {
        let mut f = main_f.clone();
        f.name = "aura_main".to_string();
        emit_function(&mut out, debug, &f, &ret_map);
        out.push_str("\nint main(void) {\n  aura_main();\n  return 0;\n}\n");
    }

    out
}

fn c_escape_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}

fn emit_line_directive(out: &mut String, debug: Option<&DebugSource>, span: aura_ast::Span) {
    let Some(dbg) = debug else { return };
    let lc = dbg.line_col(span);
    let file = c_escape_string_literal(&dbg.file_name);
    out.push_str(&format!("#line {} \"{}\"\n", lc.line, file));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CType {
    Void,
    Bool,
    U32,
    CString,
    Tensor,
}

fn function_return_ctype(f: &FunctionIR) -> CType {
    for b in &f.blocks {
        if let Terminator::Return(Some(_)) = &b.term {
            return CType::Tensor;
        }
    }
    CType::Void
}

fn emit_function(out: &mut String, debug: Option<&DebugSource>, f: &FunctionIR, ret_map: &HashMap<String, CType>) {
    let ret = function_return_ctype(f);
    out.push_str(match ret {
        CType::Void => "void ",
        CType::Tensor => "Tensor ",
        CType::U32 => "uint32_t ",
        CType::CString => "const char* ",
        CType::Bool => "bool ",
    });

    out.push_str(&c_ident(&f.name));
    out.push('(');
    for (i, p) in f.params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let cty = map_type(&p.ty);
        out.push_str(cty);
        out.push(' ');
        out.push_str(&c_ident(&p.name));
    }
    out.push_str(") {\n");

    // Value table: ValueId -> (ctype, name)
    let mut values: HashMap<ValueId, (CType, String)> = HashMap::new();
    for p in &f.params {
        values.insert(p.value, (map_type_to_ctype(&p.ty), c_ident(&p.name)));
    }

    // Precompute Phi injections: (pred, target) -> [(dest, incoming)]
    let mut phi_inj: HashMap<(BlockId, BlockId), Vec<(ValueId, ValueId)>> = HashMap::new();
    for b in &f.blocks {
        for inst in &b.insts {
            if let InstKind::Phi { incomings } = &inst.kind {
                let Some(dest) = inst.dest else { continue };
                for (pred, incoming) in incomings {
                    phi_inj
                        .entry((*pred, b.id))
                        .or_default()
                        .push((dest, *incoming));
                }
                // Create a name for dest so we can assign to it.
                values.entry(dest).or_insert((CType::U32, format!("v{}", dest.0)));
            }
        }
    }

    // Emit an entry jump (we use gotos for CFG).
    out.push_str("  goto ");
    out.push_str(&bb_label(f.entry));
    out.push_str(";\n");

    // Emit blocks
    for b in &f.blocks {
        emit_line_directive(out, debug, b.span);
        out.push_str("\n");
        out.push_str(&bb_label(b.id));
        out.push_str(":\n");

        for inst in &b.insts {
            emit_line_directive(out, debug, inst.span);
            match &inst.kind {
                InstKind::AllocCapability { .. } => {}

                InstKind::Phi { .. } => {
                    // Phi nodes are implemented by assignments in predecessors (see terminators).
                }

                InstKind::RangeCheckU32 { value, lo, hi } => {
                    if let Some((_ct, name)) = values.get(value) {
                        out.push_str("  AURA_RANGE_CHECK_U32(");
                        out.push_str(name);
                        out.push_str(", ");
                        out.push_str(&format!("{lo}u"));
                        out.push_str(", ");
                        out.push_str(&format!("{hi}u"));
                        out.push_str(", \"");
                        out.push_str(name);
                        out.push_str("\");\n");
                    }
                }

                InstKind::BindStrand { name, expr } => {
                    if let Some(dest) = inst.dest {
                        let (ct, decl) = emit_rvalue_decl(dest, name, expr);
                        out.push_str("  ");
                        out.push_str(&decl);
                        out.push('\n');
                        values.insert(dest, (ct, format!("v{}", dest.0)));
                    }
                }

                InstKind::Unary { op, operand } => {
                    let Some(dest) = inst.dest else { continue };
                    let operand_name = values
                        .get(operand)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| format!("v{}", operand.0));
                    let (ct, expr_s) = match op {
                        UnaryOp::Neg => (CType::U32, format!("-(int32_t)({operand_name})")),
                        UnaryOp::Not => (CType::Bool, format!("!({operand_name})")),
                    };
                    out.push_str("  ");
                    out.push_str(map_ctype_decl(ct));
                    out.push(' ');
                    out.push_str(&format!("v{} = {};
", dest.0, expr_s));
                    values.insert(dest, (ct, format!("v{}", dest.0)));
                }

                InstKind::Binary { op, left, right } => {
                    let Some(dest) = inst.dest else { continue };
                    let ln = values
                        .get(left)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| format!("v{}", left.0));
                    let rn = values
                        .get(right)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| format!("v{}", right.0));
                    let (ct, expr_s) = emit_binop(*op, &ln, &rn);
                    out.push_str("  ");
                    out.push_str(map_ctype_decl(ct));
                    out.push(' ');
                    out.push_str(&format!("v{} = {};
", dest.0, expr_s));
                    values.insert(dest, (ct, format!("v{}", dest.0)));
                }

                InstKind::Call { callee, args } => {
                    emit_call(out, inst.dest, callee, args, &mut values, ret_map);
                }
                InstKind::ComputeKernel { callee, args } => {
                    emit_kernel(out, inst.dest, callee, args, &mut values);
                }
            }
        }

        // Terminator
        match &b.term {
            Terminator::Return(Some(v)) => {
                if let Some((_ct, name)) = values.get(v) {
                    out.push_str("  return ");
                    out.push_str(name);
                    out.push_str(";\n");
                } else {
                    out.push_str("  return (Tensor){0};\n");
                }
            }
            Terminator::Return(None) => {
                out.push_str("  return;\n");
            }
            Terminator::Br(t) => {
                emit_phi_assigns(out, &phi_inj, &values, b.id, *t);
                out.push_str("  goto ");
                out.push_str(&bb_label(*t));
                out.push_str(";\n");
            }
            Terminator::CondBr {
                cond,
                then_bb,
                else_bb,
            } => {
                let cn = values
                    .get(cond)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| format!("v{}", cond.0));
                out.push_str("  if (");
                out.push_str(&cn);
                out.push_str(") {\n");
                emit_phi_assigns(out, &phi_inj, &values, b.id, *then_bb);
                out.push_str("    goto ");
                out.push_str(&bb_label(*then_bb));
                out.push_str(";\n");
                out.push_str("  } else {\n");
                emit_phi_assigns(out, &phi_inj, &values, b.id, *else_bb);
                out.push_str("    goto ");
                out.push_str(&bb_label(*else_bb));
                out.push_str(";\n");
                out.push_str("  }\n");
            }
            Terminator::Switch {
                scrut,
                default_bb,
                cases,
            } => {
                let sn = values
                    .get(scrut)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| format!("v{}", scrut.0));

                out.push_str("  switch (");
                out.push_str(&sn);
                out.push_str(") {\n");

                for (k, bb2) in cases {
                    out.push_str(&format!("    case {}u: ", k));
                    emit_phi_assigns(out, &phi_inj, &values, b.id, *bb2);
                    out.push_str("goto ");
                    out.push_str(&bb_label(*bb2));
                    out.push_str(";\n");
                }

                out.push_str("    default: ");
                emit_phi_assigns(out, &phi_inj, &values, b.id, *default_bb);
                out.push_str("goto ");
                out.push_str(&bb_label(*default_bb));
                out.push_str(";\n");

                out.push_str("  }\n");
            }
        }
    }

    out.push_str("}\n");
}

fn emit_rvalue_decl(dest: ValueId, _name: &str, rv: &RValue) -> (CType, String) {
    let var = format!("v{}", dest.0);
    match rv {
        RValue::ConstU32(n) => (
            CType::U32,
            format!("const uint32_t {var} = {n}u;"),
        ),
        RValue::ConstBool(b) => (
            CType::Bool,
            format!("const bool {var} = {};", if *b { "true" } else { "false" }),
        ),
        RValue::ConstString(s) => (
            CType::CString,
            format!("const char* {var} = \"{}\";", escape_c_string(s)),
        ),
        RValue::Local(v) => (
            CType::Tensor,
            format!("/* alias */ Tensor {var} = v{};", v.0),
        ),
    }
}

fn emit_call(
    out: &mut String,
    dest: Option<ValueId>,
    callee: &str,
    args: &[ValueId],
    values: &mut HashMap<ValueId, (CType, String)>,
    ret_map: &HashMap<String, CType>,
) {
    let c_fn = map_callee(callee);
    let ret = ret_map.get(callee).copied().unwrap_or_else(|| builtin_return_ctype(&c_fn));

    if let Some(d) = dest {
        if ret == CType::Void {
            out.push_str("  ");
        } else {
            let decl = match ret {
                CType::Tensor => "Tensor",
                CType::U32 => "uint32_t",
                CType::CString => "const char*",
                CType::Bool => "bool",
                CType::Void => "void",
            };
            out.push_str("  ");
            out.push_str(decl);
            out.push(' ');
            out.push_str(&format!("v{} = ", d.0));
        }

        out.push_str(&c_fn);
        out.push('(');
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&values.get(a).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("v{}", a.0)));
        }
        out.push_str(");\n");

        if ret != CType::Void {
            values.insert(d, (ret, format!("v{}", d.0)));
        }
    } else {
        out.push_str("  ");
        out.push_str(&c_fn);
        out.push('(');
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&values.get(a).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("v{}", a.0)));
        }
        out.push_str(");\n");
    }
}

fn emit_kernel(
    out: &mut String,
    dest: Option<ValueId>,
    callee: &str,
    args: &[ValueId],
    values: &mut HashMap<ValueId, (CType, String)>,
) {
    // Prototype supports the common case: Tensor f(Tensor, u32)
    // args[0] is Tensor, args[1] is u32.
    let c_fn = map_callee(callee);

    if let Some(d) = dest {
        out.push_str("  Tensor ");
        out.push_str(&format!("v{} = ", d.0));
        if args.len() == 2 {
            out.push_str("aura_async_tensor2(");
            out.push_str(&c_fn);
            out.push_str(", ");
            out.push_str(&values.get(&args[0]).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("v{}", args[0].0)));
            out.push_str(", ");
            out.push_str(&values.get(&args[1]).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("v{}", args[1].0)));
            out.push_str(")");
        } else {
            // Fallback: run synchronously
            out.push_str(&c_fn);
            out.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&values.get(a).map(|(_, n)| n.clone()).unwrap_or_else(|| format!("v{}", a.0)));
            }
            out.push(')');
        }
        out.push_str(";\n");
        values.insert(d, (CType::Tensor, format!("v{}", d.0)));
    }
}

fn map_type(ty: &Type) -> &'static str {
    match ty {
        Type::Unit => "void",
        Type::Bool => "bool",
        Type::U32 => "uint32_t",
        Type::String => "const char*",
        Type::Tensor => "Tensor",
        Type::Opaque(_) => "Tensor",
    }
}

fn map_type_to_ctype(ty: &Type) -> CType {
    match ty {
        Type::Unit => CType::Void,
        Type::Bool => CType::Bool,
        Type::U32 => CType::U32,
        Type::String => CType::CString,
        Type::Tensor | Type::Opaque(_) => CType::Tensor,
    }
}

fn bb_label(id: BlockId) -> String {
    format!("bb{}", id.0)
}

fn map_ctype_decl(ct: CType) -> &'static str {
    match ct {
        CType::Void => "void",
        CType::Bool => "bool",
        CType::U32 => "uint32_t",
        CType::CString => "const char*",
        CType::Tensor => "Tensor",
    }
}

fn emit_binop(op: BinOp, l: &str, r: &str) -> (CType, String) {
    match op {
        BinOp::Add => (CType::U32, format!("({l}) + ({r})")),
        BinOp::Sub => (CType::U32, format!("({l}) - ({r})")),
        BinOp::Mul => (CType::U32, format!("({l}) * ({r})")),
        // In C, unsigned division by 0 is UB. Enforce a runtime trap.
        BinOp::Div => (
            CType::U32,
            format!(
                "(AURA_RANGE_CHECK_U32((uint32_t)({r}), 1u, 0xFFFFFFFFu, \"divisor\"), ({l}) / ({r}))"
            ),
        ),

        BinOp::Eq => (CType::Bool, format!("({l}) == ({r})")),
        BinOp::Ne => (CType::Bool, format!("({l}) != ({r})")),
        BinOp::Lt => (CType::Bool, format!("({l}) < ({r})")),
        BinOp::Gt => (CType::Bool, format!("({l}) > ({r})")),
        BinOp::Le => (CType::Bool, format!("({l}) <= ({r})")),
        BinOp::Ge => (CType::Bool, format!("({l}) >= ({r})")),

        BinOp::And => (CType::Bool, format!("({l}) && ({r})")),
        BinOp::Or => (CType::Bool, format!("({l}) || ({r})")),
    }
}

fn emit_phi_assigns(
    out: &mut String,
    phi_inj: &HashMap<(BlockId, BlockId), Vec<(ValueId, ValueId)>>,
    values: &HashMap<ValueId, (CType, String)>,
    pred: BlockId,
    target: BlockId,
) {
    if let Some(assigns) = phi_inj.get(&(pred, target)) {
        for (dest, incoming) in assigns {
            let dn = format!("v{}", dest.0);
            let sn = values
                .get(incoming)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| format!("v{}", incoming.0));
            out.push_str("    ");
            out.push_str(&dn);
            out.push_str(" = ");
            out.push_str(&sn);
            out.push_str(";\n");
        }
    }
}

fn builtin_return_ctype(c_fn: &str) -> CType {
    match c_fn {
        "aura_io_println" => CType::Void,
        "io_load_tensor" => CType::Tensor,
        "io_display" => CType::Void,
        "compute_gradient" => CType::Tensor,
        _ => CType::Tensor,
    }
}

fn map_callee(name: &str) -> String {
    match name {
        // Match LLVM backend's builtin ABI mapping.
        "io.println" => "aura_io_println".to_string(),
        _ => {
            // Convert Aura module-style calls `io.load_tensor` -> `io_load_tensor`
            name.replace('.', "_")
        }
    }
}

fn c_ident(name: &str) -> String {
    name.replace('.', "_")
}

fn escape_c_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
