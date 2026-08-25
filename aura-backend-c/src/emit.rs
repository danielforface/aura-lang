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

    // The current IR keeps string interpolation as a ConstString template, so
    // `{ident}` references are not explicit SSA dependency edges yet. Running
    // DCE on such a function can remove bindings that the C emitter still needs
    // to reproduce AVM interpolation semantics. Keep those functions intact
    // until interpolation dependencies become first-class IR; optimize all
    // other functions normally.
    for function in module.functions.values_mut() {
        if !function_has_string_interpolation(function) {
            aura_ir::optimize_function(function);
        }
    }

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
    out.push_str("#include <stddef.h>\n");
    out.push_str("#include <inttypes.h>\n");
    out.push_str("#include <stdio.h>\n");
    out.push_str("#include <stdlib.h>\n\n");

    out.push_str("// ---- Standard 128-bit Types ----\n");
    out.push_str("#if defined(__SIZEOF_INT128__)\n");
    out.push_str("typedef unsigned __int128 aura_u128_t;\n");
    out.push_str("typedef __int128 aura_i128_t;\n");
    out.push_str("#else\n");
    out.push_str("typedef struct { uint64_t lo; uint64_t hi; } aura_u128_t;\n");
    out.push_str("typedef struct { uint64_t lo; int64_t hi; } aura_i128_t;\n");
    out.push_str("#endif\n\n");

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

fn function_has_string_interpolation(function: &FunctionIR) -> bool {
    function.blocks.iter().any(|block| {
        block.insts.iter().any(|inst| {
            matches!(
                &inst.kind,
                InstKind::BindStrand {
                    expr: RValue::ConstString(template),
                    ..
                } if template.contains('{') && template.contains('}')
            )
        })
    })
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
    Char,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    F32,
    F64,
    CString,
    Tensor,
}

fn function_return_ctype(f: &FunctionIR) -> CType {
    for b in &f.blocks {
        if let Terminator::Return(Some(_)) = &b.term {
            return map_type_to_ctype(&f.ret);
        }
    }
    CType::Void
}

fn emit_function(out: &mut String, debug: Option<&DebugSource>, f: &FunctionIR, ret_map: &HashMap<String, CType>) {
    let ret = function_return_ctype(f);
    out.push_str(map_ctype_decl(ret));
    out.push(' ');

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

    // Value table: ValueId -> (ctype, generated C name)
    let mut values: HashMap<ValueId, (CType, String)> = HashMap::new();

    // Preserve source-level names for AVM-compatible `{ident}` interpolation.
    let mut named_values: HashMap<String, (CType, String)> = HashMap::new();

    // Keep the raw template associated with a string SSA value so io.println
    // can lower interpolation instead of forwarding literal braces.
    let mut string_literals: HashMap<ValueId, String> = HashMap::new();

    for p in &f.params {
        let binding = (map_type_to_ctype(&p.ty), c_ident(&p.name));
        named_values.insert(p.name.clone(), binding.clone());
        values.insert(p.value, binding);
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

                InstKind::RangeCheckU32 { value, lo, hi } | InstKind::RangeCheck { value, lo, hi } => {
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

                InstKind::ConstructRecord { .. }
                | InstKind::GetField { .. }
                | InstKind::SetField { .. }
                | InstKind::ConstructEnumVariant { .. }
                | InstKind::GetEnumTag { .. }
                | InstKind::GetEnumPayload { .. } => {
                    // Lowered as prototype values in C emission
                }

                InstKind::BindStrand { name, expr } => {
                    if let Some(dest) = inst.dest {
                        // RValue::Local is an alias, so its C type must be the
                        // source value's type. The old prototype hard-coded
                        // Tensor here, which is incorrect for u32/bool/string.
                        let (ct, decl) = match expr {
                            RValue::Local(source) => {
                                let (source_ct, source_name) = values
                                    .get(source)
                                    .cloned()
                                    .unwrap_or((CType::Tensor, format!("v{}", source.0)));
                                (
                                    source_ct,
                                    format!(
                                        "const {} v{} = {};",
                                        map_ctype_decl(source_ct),
                                        dest.0,
                                        source_name
                                    ),
                                )
                            }
                            _ => emit_rvalue_decl(dest, name, expr),
                        };

                        out.push_str("  ");
                        out.push_str(&decl);
                        out.push('\n');

                        let binding = (ct, format!("v{}", dest.0));
                        values.insert(dest, binding.clone());
                        named_values.insert(name.clone(), binding);

                        match expr {
                            RValue::ConstString(template) => {
                                string_literals.insert(dest, template.clone());
                            }
                            RValue::Local(source) => {
                                if let Some(template) = string_literals.get(source).cloned() {
                                    string_literals.insert(dest, template);
                                } else {
                                    string_literals.remove(&dest);
                                }
                            }
                            _ => {
                                string_literals.remove(&dest);
                            }
                        }
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
                    emit_call(
                        out,
                        inst.dest,
                        callee,
                        args,
                        &mut values,
                        ret_map,
                        &string_literals,
                        &named_values,
                    );
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
        RValue::ConstI64(n) => (
            CType::I64,
            format!("const int64_t {var} = {n}LL;"),
        ),
        RValue::ConstF64(f) => (
            CType::F64,
            format!("const double {var} = {f};"),
        ),
        RValue::ConstChar(c) => (
            CType::Char,
            format!("const uint32_t {var} = {}u;", *c as u32),
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

fn push_c_printf_literal(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '%' => out.push_str("%%"),
            _ => out.push(ch),
        }
    }
}

fn emit_interpolated_println(
    out: &mut String,
    template: &str,
    named_values: &HashMap<String, (CType, String)>,
) -> bool {
    let mut format_string = String::with_capacity(template.len() + 8);
    let mut arguments: Vec<String> = Vec::new();
    let mut cursor = 0usize;
    let mut replaced_any = false;

    while cursor < template.len() {
        let Some(open_rel) = template[cursor..].find('{') else {
            push_c_printf_literal(&mut format_string, &template[cursor..]);
            break;
        };

        let open = cursor + open_rel;
        push_c_printf_literal(&mut format_string, &template[cursor..open]);

        let after_open = open + 1;
        let Some(close_rel) = template[after_open..].find('}') else {
            push_c_printf_literal(&mut format_string, &template[open..]);
            break;
        };

        let close = after_open + close_rel;
        let key = &template[after_open..close];

        let replacement = named_values.get(key).and_then(|(ct, c_name)| match *ct {
            CType::U32 => Some(("%u", format!("(unsigned int)({c_name})"))),
            CType::I64 => Some(("%lld", format!("(long long)({c_name})"))),
            CType::F64 => Some(("%f", format!("(double)({c_name})"))),
            CType::Bool => Some(("%s", format!("(({c_name}) ? \"true\" : \"false\")"))),
            CType::CString => Some(("%s", format!("({c_name})"))),
            _ => None,
        });

        if let Some((specifier, argument)) = replacement {
            format_string.push_str(specifier);
            arguments.push(argument);
            replaced_any = true;
        } else {
            // Match AVM behavior: unknown/unsupported identifiers remain
            // visible as their original `{ident}` text.
            push_c_printf_literal(&mut format_string, &template[open..close + 1]);
        }

        cursor = close + 1;
    }

    if !replaced_any {
        return false;
    }

    out.push_str("  printf(\"");
    out.push_str(&format_string);
    out.push_str("\\n\"");
    for argument in arguments {
        out.push_str(", ");
        out.push_str(&argument);
    }
    out.push_str(");\n");
    true
}

fn emit_call(
    out: &mut String,
    dest: Option<ValueId>,
    callee: &str,
    args: &[ValueId],
    values: &mut HashMap<ValueId, (CType, String)>,
    ret_map: &HashMap<String, CType>,
    string_literals: &HashMap<ValueId, String>,
    named_values: &HashMap<String, (CType, String)>,
) {
    let c_fn = map_callee(callee);

    if c_fn == "aura_io_println" && args.len() == 1 {
        if let Some(template) = string_literals.get(&args[0]) {
            if emit_interpolated_println(out, template, named_values) {
                return;
            }
        }
    }

    let ret = ret_map.get(callee).copied().unwrap_or_else(|| builtin_return_ctype(&c_fn));

    if let Some(d) = dest {
        if ret == CType::Void {
            out.push_str("  ");
        } else {
            let decl = map_ctype_decl(ret);
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
        Type::Char => "uint32_t",
        Type::U8 => "uint8_t",
        Type::U16 => "uint16_t",
        Type::U32 => "uint32_t",
        Type::U64 => "uint64_t",
        Type::U128 => "aura_u128_t",
        Type::USize => "size_t",
        Type::I8 => "int8_t",
        Type::I16 => "int16_t",
        Type::I32 => "int32_t",
        Type::I64 => "int64_t",
        Type::I128 => "aura_i128_t",
        Type::ISize => "ptrdiff_t",
        Type::F32 => "float",
        Type::F64 => "double",
        Type::Str | Type::String => "const char*",
        Type::Tensor => "Tensor",
        Type::Ptr(_) => "void*",
        Type::Record { .. } | Type::Enum { .. } | Type::Opaque(_) => "Tensor",
    }
}

fn map_type_to_ctype(ty: &Type) -> CType {
    match ty {
        Type::Unit => CType::Void,
        Type::Bool => CType::Bool,
        Type::Char => CType::Char,
        Type::U8 => CType::U8,
        Type::U16 => CType::U16,
        Type::U32 => CType::U32,
        Type::U64 => CType::U64,
        Type::U128 => CType::U128,
        Type::USize => CType::USize,
        Type::I8 => CType::I8,
        Type::I16 => CType::I16,
        Type::I32 => CType::I32,
        Type::I64 => CType::I64,
        Type::I128 => CType::I128,
        Type::ISize => CType::ISize,
        Type::F32 => CType::F32,
        Type::F64 => CType::F64,
        Type::Str | Type::String => CType::CString,
        Type::Tensor | Type::Ptr(_) | Type::Record { .. } | Type::Enum { .. } | Type::Opaque(_) => CType::Tensor,
    }
}

fn bb_label(id: BlockId) -> String {
    format!("bb{}", id.0)
}

fn map_ctype_decl(ct: CType) -> &'static str {
    match ct {
        CType::Void => "void",
        CType::Bool => "bool",
        CType::Char => "uint32_t",
        CType::U8 => "uint8_t",
        CType::U16 => "uint16_t",
        CType::U32 => "uint32_t",
        CType::U64 => "uint64_t",
        CType::U128 => "aura_u128_t",
        CType::USize => "size_t",
        CType::I8 => "int8_t",
        CType::I16 => "int16_t",
        CType::I32 => "int32_t",
        CType::I64 => "int64_t",
        CType::I128 => "aura_i128_t",
        CType::ISize => "ptrdiff_t",
        CType::F32 => "float",
        CType::F64 => "double",
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

#[cfg(test)]
mod emit_tests {
    use super::*;
    use aura_ir::{BasicBlock, ExecutionHint, FunctionIR, Inst, InstKind, ModuleIR, Param, RValue, Terminator, Type, ValueId};
    use aura_ast::Span;
    use std::collections::BTreeMap;

    #[test]
    fn test_c_emission_primitives_and_typedefs() {
        let mut module = ModuleIR {
            functions: BTreeMap::new(),
            externs: BTreeMap::new(),
        };

        let dummy_span: Span = (0, 0).into();
        let func = FunctionIR {
            name: "test_fn".to_string(),
            span: dummy_span,
            params: vec![
                Param {
                    name: "x".to_string(),
                    ty: Type::U32,
                    span: dummy_span,
                    value: ValueId(0),
                },
                Param {
                    name: "y".to_string(),
                    ty: Type::I64,
                    span: dummy_span,
                    value: ValueId(1),
                },
            ],
            ret: Type::U32,
            entry: BlockId(0),
            blocks: vec![BasicBlock {
                id: BlockId(0),
                span: dummy_span,
                hint: ExecutionHint::Sequential,
                insts: vec![
                    Inst {
                        span: dummy_span,
                        dest: Some(ValueId(2)),
                        kind: InstKind::BindStrand {
                            name: "v2".to_string(),
                            expr: RValue::ConstU32(42),
                        },
                    },
                ],
                term: Terminator::Return(Some(ValueId(2))),
            }],
        };

        module.functions.insert("test_fn".to_string(), func);

        let artifacts = emit_module(&module, None).expect("emit module");
        assert!(artifacts.runtime_h.contains("aura_u128_t"));
        assert!(artifacts.runtime_h.contains("aura_i128_t"));
        assert!(artifacts.module_c.contains("uint32_t test_fn(uint32_t x, int64_t y)"));
    }
}
