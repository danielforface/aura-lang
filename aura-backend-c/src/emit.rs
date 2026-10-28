#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    let module_c = emit_module_c(&module, debug)?;
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

// Encode user-facing names into collision-free, C-safe identifiers without
// changing Record field ordering or accidentally using C reserved keywords.
fn encoded_record_name(s: &str) -> String {
    s.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>()
}
fn record_type_symbol(s: &str) -> String {
    format!("aura_record_{}", encoded_record_name(s))
}
fn record_field_symbol(s: &str) -> String {
    format!("aura_f_{}", encoded_record_name(s))
}
fn record_field_ctype(ty: &Type) -> Option<CType> {
    match ty {
        Type::Bool => Some(CType::Bool),
        Type::Char => Some(CType::Char),
        Type::U8 => Some(CType::U8),
        Type::U16 => Some(CType::U16),
        Type::U32 => Some(CType::U32),
        Type::U64 => Some(CType::U64),
        Type::I8 => Some(CType::I8),
        Type::I16 => Some(CType::I16),
        Type::I32 => Some(CType::I32),
        Type::I64 => Some(CType::I64),
        Type::F32 => Some(CType::F32),
        Type::F64 => Some(CType::F64),
        _ => None,
    }
}
// Each qualified nominal field retains its own type identity; a nested
// Record is NEVER reduced to a scalar width or the generic Tensor ABI.
#[derive(Clone, Debug, PartialEq, Eq)]
enum RecordFieldCType {
    Scalar(CType),
    Nominal(String),
}
impl RecordFieldCType {
    fn declaration(&self) -> String {
        match self {
            Self::Scalar(ct) => map_ctype_decl(*ct).to_string(),
            Self::Nominal(name) => record_type_symbol(name),
        }
    }
}
fn qualified_record_fields(
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
    record: &str,
) -> Result<Vec<(String, RecordFieldCType)>, CBackendError> {
    fn qualify(
        layouts: &BTreeMap<String, Vec<(String, Type)>>,
        record: &str,
        stack: &mut Vec<String>,
    ) -> Result<Vec<(String, RecordFieldCType)>, CBackendError> {
        if stack.iter().any(|entry| entry == record) {
            return Err(CBackendError {
                message: format!("unqualified C Record by-value layout cycle: {} -> {record}",
                    stack.join(" -> ")),
            });
        }
        let fields = layouts.get(record).ok_or_else(|| CBackendError {
            message: format!("unqualified C Record '{}' missing typed field layout", record),
        })?;
        if fields.is_empty() {
            return Err(CBackendError {
                message: format!("unqualified C Record '{}' empty layout not supported", record),
            });
        }
        stack.push(record.to_string());
        let mut result: Vec<(String, RecordFieldCType)> = Vec::with_capacity(fields.len());
        for (name, ty) in fields {
            if result.iter().any(|(existing, _)| existing == name) {
                return Err(CBackendError { message:
                    format!("unqualified C Record '{}' duplicate layout field '{}'", record, name) });
            }
            let field_type = if let Some(ct) = record_field_ctype(ty) {
                RecordFieldCType::Scalar(ct)
            } else if let Type::Opaque(nested) = ty {
                if !layouts.contains_key(nested) {
                    return Err(CBackendError { message: format!(
                        "unqualified C Record '{}' field '{}' type {:?} unsupported",
                        record, name, ty,
                    ) });
                }
                qualify(layouts, nested, stack)?;
                RecordFieldCType::Nominal(nested.clone())
            } else if let Type::Record { name: nested, fields: actual_fields } = ty {
                if layouts.get(nested) != Some(actual_fields) {
                    return Err(CBackendError { message: format!(
                        "unqualified C Record '{}' field '{}' nested layout mismatch '{}'",
                        record, name, nested,
                    ) });
                }
                qualify(layouts, nested, stack)?;
                RecordFieldCType::Nominal(nested.clone())
            } else {
                return Err(CBackendError { message: format!(
                    "unqualified C Record '{}' field '{}' type {:?} unsupported",
                    record, name, ty,
                ) });
            };
            result.push((name.clone(), field_type));
        }
        stack.pop();
        Ok(result)
    }
    qualify(layouts, record, &mut Vec::new())
}

// Semantic record returns can recursively carry expanded Type::Record
// fields while declaration layouts contain Type::Opaque nominal references.
fn matching_record_layout_type(
    declared: &Type, actual: &Type,
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
) -> bool {
    if declared == actual { return true; }
    match (declared, actual) {
        (Type::Opaque(expected), Type::Record { name, fields }) if expected == name => {
            layouts.get(name).is_some_and(|layout|
                layout.len() == fields.len() &&
                layout.iter().zip(fields).all(|((a, at), (b, bt))|
                    a == b && matching_record_layout_type(at, bt, layouts)))
        }
        _ => false,
    }
}
fn emit_qualified_record_layout(
    out: &mut String,
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
    name: &str,
    emitted: &mut BTreeSet<String>,
) -> Result<(), CBackendError> {
    if emitted.contains(name) { return Ok(()); }
    let fields = qualified_record_fields(layouts, name)?;
    // A by-value nested C struct must be complete before the parent typedef.
    for (_, ty) in &fields {
        if let RecordFieldCType::Nominal(child) = ty {
            emit_qualified_record_layout(out, layouts, child, emitted)?;
        }
    }
    out.push_str("typedef struct {\n");
    for (field, ty) in &fields {
        out.push_str(&format!("  {} {};\n",
            ty.declaration(), record_field_symbol(field)));
    }
    out.push_str(&format!("}} {};\n\n", record_type_symbol(name)));
    emitted.insert(name.to_string());
    Ok(())
}

fn qualified_record_abi_type(
    ty: &Type,
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
) -> Result<Option<String>, CBackendError> {
    match ty {
        Type::Record { name, fields } => {
            let layout = layouts.get(name).ok_or_else(|| CBackendError {
                message: format!("unqualified Record function ABI missing layout for '{name}'"),
            })?;
            qualified_record_fields(layouts, name)?;
            if layout.len() != fields.len() ||
               !layout.iter().zip(fields).all(|((a, at), (b, bt))|
                   a == b && matching_record_layout_type(at, bt, layouts)) {
                return Err(CBackendError {
                    message: format!("unqualified Record function ABI layout mismatch for '{name}'"),
                });
            }
            Ok(Some(name.clone()))
        }
        Type::Opaque(name) if layouts.contains_key(name) => {
            qualified_record_fields(layouts, name)?;
            Ok(Some(name.clone()))
        }
        _ => Ok(None),
    }
}

fn emit_function_signature(
    out: &mut String,
    f: &FunctionIR,
    layouts: &BTreeMap<String, Vec<(String, Type)>>,
    name: &str,
) -> Result<(), CBackendError> {
    let returns_value = f.blocks.iter().any(|b| matches!(b.term, Terminator::Return(Some(_))));
    if returns_value {
        if let Some(record) = qualified_record_abi_type(&f.ret, layouts)? {
            out.push_str(&record_type_symbol(&record));
        } else {
            out.push_str(map_ctype_decl(function_return_ctype(f)));
        }
    } else {
        out.push_str("void");
    }
    out.push(' ');
    out.push_str(&c_ident(name));
    out.push('(');
    if f.params.is_empty() {
        out.push_str("void");
    } else {
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            if let Some(record) = qualified_record_abi_type(&p.ty, layouts)? {
                out.push_str(&record_type_symbol(&record));
            } else {
                out.push_str(map_type(&p.ty));
            }
            out.push(' ');
            out.push_str(&c_ident(&p.name));
        }
    }
    out.push(')');
    Ok(())
}

fn emit_module_c(module: &ModuleIR, debug: Option<&DebugSource>) -> Result<String, CBackendError> {
    let mut out = String::new();
    out.push_str("// Proof-carrying: IR validated by aura_ir::validate_module\n");
    out.push_str("#include \"aura_runtime.h\"\n\n");

    // Emit only qualified nominal layouts, in dependency order. Unused
    // unsupported declarations remain inert until they are actually used.
    let mut emitted_records = BTreeSet::new();
    for name in module.record_layouts.keys() {
        if qualified_record_fields(&module.record_layouts, name).is_ok() {
            emit_qualified_record_layout(&mut out, &module.record_layouts,
                name, &mut emitted_records)?;
        }
    }

    // Declare all functions before emitting bodies: calls may target a later
    // source cell, including a function with a nominal struct return.
    for (name, f) in &module.functions {
        let symbol = if name == "main" { "aura_main" } else { name.as_str() };
        emit_function_signature(&mut out, f, &module.record_layouts, symbol)?;
        out.push_str(";\n");
    }
    out.push('\n');

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
        emit_function(&mut out, debug, f, &ret_map, &module.record_layouts, &module.functions)?;
        out.push('\n');
    }

    // Emit Aura entry block as `void aura_main(void)` then C main.
    if let Some(main_f) = module.functions.get("main") {
        let mut f = main_f.clone();
        f.name = "aura_main".to_string();
        emit_function(&mut out, debug, &f, &ret_map, &module.record_layouts, &module.functions)?;
        out.push_str("\nint main(void) {\n  aura_main();\n  return 0;\n}\n");
    }

    Ok(out)
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

fn emit_function(
    out: &mut String,
    debug: Option<&DebugSource>,
    f: &FunctionIR,
    ret_map: &HashMap<String, CType>,
    record_layouts: &BTreeMap<String, Vec<(String, Type)>>,
    functions: &BTreeMap<String, FunctionIR>,
) -> Result<(), CBackendError> {
    emit_function_signature(out, f, record_layouts, &f.name)?;
    out.push_str(" {\n");

    // Value table: ValueId -> (ctype, generated C name)
    let mut values: HashMap<ValueId, (CType, String)> = HashMap::new();
    // Nominal Record SSA lives in a separate typed namespace. Never pass
    // a Record as a Tensor/Scalar to unqualified operations or ABI paths.
    let mut records: HashMap<ValueId, (String, String)> = HashMap::new();

    // Preserve source-level names for AVM-compatible `{ident}` interpolation.
    let mut named_values: HashMap<String, (CType, String)> = HashMap::new();

    // Keep the raw template associated with a string SSA value so io.println
    // can lower interpolation instead of forwarding literal braces.
    let mut string_literals: HashMap<ValueId, String> = HashMap::new();

    for p in &f.params {
        if let Some(record) = qualified_record_abi_type(&p.ty, record_layouts)? {
            records.insert(p.value, (record, c_ident(&p.name)));
        } else {
            let binding = (map_type_to_ctype(&p.ty), c_ident(&p.name));
            named_values.insert(p.name.clone(), binding.clone());
            values.insert(p.value, binding);
        }
    }

    // Static SSA type prepass for supported value-producing instructions.
    // Phi incoming definitions can be in any predecessor block; the normal
    // code emitter has not yet visited those blocks when it allocates Phi
    // storage. Infer types from the same typed operands used by codegen,
    // iterating to a fixed point for chains and other already-resolved Phi.
    //
    // This is intentionally NOT a default U32, a full IR type checker, or
    // inference for opaque/composite instructions: unresolved Phi inputs
    // remain rejected below, never treated as a successful C emission.
    let mut known_ssa_types: HashMap<ValueId, CType> = values
        .iter()
        .map(|(value, (ty, _))| (*value, *ty))
        .collect();
    let max_rounds = f.blocks.iter().map(|b| b.insts.len()).sum::<usize>() + 1;
    for _ in 0..max_rounds {
        let mut progressed = false;
        for b in &f.blocks {
            for inst in &b.insts {
                let Some(dest) = inst.dest else { continue };
                let inferred = match &inst.kind {
                    InstKind::BindStrand { expr, .. } => match expr {
                        RValue::ConstU32(_) => Some(CType::U32),
                        RValue::ConstI64(_) => Some(CType::I64),
                        RValue::ConstF64(_) => Some(CType::F64),
                        RValue::ConstChar(_) => Some(CType::Char),
                        RValue::ConstBool(_) => Some(CType::Bool),
                        RValue::ConstString(_) => Some(CType::CString),
                        RValue::Local(source) => known_ssa_types.get(source).copied(),
                    },
                    InstKind::Unary { op, operand } => known_ssa_types
                        .get(operand)
                        .copied()
                        .and_then(|ty| match op {
                            UnaryOp::Neg if matches!(
                                ty, CType::I32 | CType::I64 | CType::U32 | CType::U64
                                    | CType::F32 | CType::F64
                            ) => Some(ty),
                            UnaryOp::Not if ty == CType::Bool => Some(CType::Bool),
                            _ => None,
                        }),
                    InstKind::Binary { op, left, right } => {
                        match (known_ssa_types.get(left), known_ssa_types.get(right)) {
                            (Some(&lt), Some(&rt)) => emit_binop(*op, lt, rt, "lhs", "rhs")
                                .ok()
                                .map(|(ty, _)| ty),
                            _ => None,
                        }
                    }
                    InstKind::Phi { incomings } if !incomings.is_empty() => incomings
                        .iter()
                        .map(|(_, value)| known_ssa_types.get(value).copied())
                        .collect::<Option<Vec<_>>>()
                        .and_then(|ts| {
                            ts.first().copied().filter(|first| ts.iter().all(|ty| ty == first))
                        }),
                    _ => None,
                };
                if let Some(ty) = inferred {
                    match known_ssa_types.get(&dest) {
                        Some(previous) if *previous != ty => return Err(CBackendError {
                            message: format!(
                                "conflicting inferred SSA types for v{}: {previous:?} vs {ty:?}",
                                dest.0
                            ),
                        }),
                        Some(_) => {}
                        None => {
                            known_ssa_types.insert(dest, ty);
                            progressed = true;
                        }
                    }
                }
            }
        }
        if !progressed { break; }
    }

    // Nominal Record SSA prepass. Unlike primitive CType inference, a Record
    // must carry its exact declared name across copies, functional SetField
    // updates, and join-block Phi nodes. Never infer it as Tensor from width
    // or initializer shape. Fixed-point inference handles predecessor blocks
    // appearing after a join in the serialized IR.
    let mut known_record_types: HashMap<ValueId, String> = records.iter().map(|(id, (name, _))| (*id, name.clone())).collect();
    for _ in 0..max_rounds {
        let mut progressed = false;
        for block in &f.blocks {
            for inst in &block.insts {
                let Some(dest) = inst.dest else { continue };
                let inferred = match &inst.kind {
                    InstKind::ConstructRecord { record_name, .. } => {
                        // Missing or unqualified layouts cannot become a
                        // nominal Record at Phi simply by supplying a name.
                        qualified_record_fields(record_layouts, record_name)?;
                        Some(record_name.clone())
                    }
                    InstKind::BindStrand { expr: RValue::Local(source), .. } =>
                        known_record_types.get(source).cloned(),
                    InstKind::SetField { base, .. } =>
                        known_record_types.get(base).cloned(),
                    // A Record returned by another function is a nominal SSA
                    // producer too. Without this edge, a later if/else Phi
                    // cannot resolve the type of values sourced from calls.
                    InstKind::Call { callee, .. } => {
                        if let Some(called) = functions.get(callee) {
                            qualified_record_abi_type(&called.ret, record_layouts)?
                        } else {
                            None
                        }
                    }
                    InstKind::Phi { incomings } if !incomings.is_empty() => {
                        let incoming_names = incomings.iter()
                            .map(|(_, incoming)| known_record_types.get(incoming).cloned())
                            .collect::<Option<Vec<_>>>();
                        incoming_names.and_then(|names| {
                            names.first().cloned().filter(|name|
                                names.iter().all(|candidate| candidate == name)
                            )
                        })
                    }
                    _ => None,
                };
                if let Some(record) = inferred {
                    if let Some(scalar) = known_ssa_types.get(&dest) {
                        return Err(CBackendError { message: format!(
                            "unqualified Record SSA v{} conflicts with scalar type {scalar:?}",
                            dest.0,
                        ) });
                    }
                    match known_record_types.get(&dest) {
                        Some(previous) if previous != &record =>
                            return Err(CBackendError { message: format!(
                                "unqualified Record SSA v{} mixes '{}' and '{}'",
                                dest.0, previous, record,
                            ) }),
                        Some(_) => {}
                        None => { known_record_types.insert(dest, record); progressed = true; }
                    }
                }
            }
        }
        if !progressed { break; }
    }

    // A Phi is SSA-level typed storage whose writes occur on its incoming
    // predecessor edges. Declare it before all block labels so every goto
    // and edge assignment references an in-scope C variable.
    //
    // A Phi is emitted only if all incoming SSA types were inferred
    // consistently; unresolved, cyclic or unmodeled producers fail closed.
    let mut phi_inj: HashMap<(BlockId, BlockId), Vec<(ValueId, ValueId)>> = HashMap::new();
    let mut phi_decls: Vec<(ValueId, CType)> = Vec::new();
    let mut record_phi_decls: Vec<(ValueId, String)> = Vec::new();
    for b in &f.blocks {
        for inst in &b.insts {
            if let InstKind::Phi { incomings } = &inst.kind {
                let dest = inst.dest.ok_or_else(|| CBackendError {
                    message: "Phi instruction is missing its SSA destination".to_string(),
                })?;
                if incomings.is_empty() {
                    return Err(CBackendError {
                        message: format!("Phi v{} has no predecessor values", dest.0),
                    });
                }
                // All incoming values must be the SAME nominal Record. A
                // scalar/Record mix or distinct Record names is a typed error,
                // never a Tensor fallback or raw struct bitcast.
                if incomings.iter().any(|(_, incoming)| known_record_types.contains_key(incoming)) {
                    let mut nominal: Option<&str> = None;
                    for (pred, incoming) in incomings {
                        let incoming_name = known_record_types.get(incoming).ok_or_else(|| {
                            CBackendError { message: format!(
                                "unqualified Record Phi v{} missing nominal type of incoming v{}",
                                dest.0, incoming.0,
                            ) }
                        })?;
                        if let Some(previous) = nominal {
                            if previous != incoming_name {
                                return Err(CBackendError { message: format!(
                                    "unqualified Record Phi v{} mixes nominal layouts '{}' and '{}'",
                                    dest.0, previous, incoming_name,
                                ) });
                            }
                        } else {
                            nominal = Some(incoming_name);
                        }
                        if known_ssa_types.contains_key(incoming) {
                            return Err(CBackendError { message: format!(
                                "unqualified Record Phi v{} incoming v{} is also scalar",
                                dest.0, incoming.0,
                            ) });
                        }
                        phi_inj.entry((*pred, b.id)).or_default().push((dest, *incoming));
                    }
                    let record_name = nominal.expect("nonempty Record Phi");
                    qualified_record_fields(record_layouts, record_name)?;
                    if values.contains_key(&dest) || records.contains_key(&dest) {
                        return Err(CBackendError { message: format!(
                            "unqualified Record Phi v{} duplicate SSA destination", dest.0,
                        ) });
                    }
                    records.insert(dest, (record_name.to_string(), format!("v{}", dest.0)));
                    record_phi_decls.push((dest, record_name.to_string()));
                    continue;
                }
                let mut unified: Option<CType> = None;
                for (pred, incoming) in incomings {
                    let incoming_type = known_ssa_types.get(incoming)
                        .copied()
                        .ok_or_else(|| CBackendError {
                            message: format!(
                                "unqualified Phi v{}: type of incoming v{} is not available",
                                dest.0, incoming.0
                            ),
                        })?;
                    if let Some(ty) = unified {
                        if ty != incoming_type {
                            return Err(CBackendError {
                                message: format!(
                                    "Phi v{} merges incompatible types {ty:?} and {incoming_type:?}",
                                    dest.0
                                ),
                            });
                        }
                    } else {
                        unified = Some(incoming_type);
                    }
                    phi_inj.entry((*pred, b.id)).or_default().push((dest, *incoming));
                }
                let ty = unified.expect("nonempty Phi");
                if ty == CType::Void || values.contains_key(&dest) {
                    return Err(CBackendError {
                        message: format!("Phi v{} has void or duplicate SSA destination", dest.0),
                    });
                }
                values.insert(dest, (ty, format!("v{}", dest.0)));
                phi_decls.push((dest, ty));
            }
        }
    }
    for (dest, ty) in &phi_decls {
        out.push_str(&format!("  {} v{};\n", map_ctype_decl(*ty), dest.0));
    }

    for (dest, record) in &record_phi_decls {
        out.push_str(&format!("  {} v{};\n", record_type_symbol(record), dest.0));
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
        // C11 requires a label to precede a statement, not a declaration.
        // Record and scalar SSA blocks may begin with variable declarations.
        out.push_str(":\n  ;\n");

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

                InstKind::ConstructRecord { record_name, fields } => {
                    let Some(dest) = inst.dest else {
                        return Err(CBackendError { message:
                            "unqualified C Record emission without SSA destination".into() });
                    };
                    let layout = qualified_record_fields(record_layouts, record_name)?;
                    if fields.len() != layout.len() {
                        return Err(CBackendError { message: format!(
                            "unqualified C Record '{}' field count mismatch: got {}, expected {}",
                            record_name, fields.len(), layout.len()
                        ) });
                    }
                    let mut initializers = Vec::new();
                    for (field, expected) in &layout {
                        let mut matching = fields.iter().filter(|(given, _)| given == field);
                        let Some((_, value)) = matching.next() else {
                            return Err(CBackendError { message: format!(
                                "unqualified C Record '{}' missing field '{}'", record_name, field
                            ) });
                        };
                        if matching.next().is_some() {
                            return Err(CBackendError { message: format!(
                                "unqualified C Record '{}' duplicate field '{}'", record_name, field
                            ) });
                        }
                        let (actual, cname) = values.get(value).ok_or_else(|| CBackendError {
                            message: format!("unqualified C Record '{}' field '{}' unemitted SSA v{}",
                                record_name, field, value.0),
                        })?;
                        if actual != expected {
                            return Err(CBackendError { message: format!(
                                "unqualified C Record '{}' field '{}' type mismatch {:?} != {:?}",
                                record_name, field, actual, expected
                            ) });
                        }
                        initializers.push(format!(".{} = {}", record_field_symbol(field), cname));
                    }
                    let cname = format!("v{}", dest.0);
                    out.push_str(&format!(
                        "  {} {} = {{ {} }};\n",
                        record_type_symbol(record_name), cname, initializers.join(", ")
                    ));
                    records.insert(dest, (record_name.clone(), cname));
                }
                InstKind::GetField { base, field_name } => {
                    let Some(dest) = inst.dest else {
                        return Err(CBackendError { message:
                            "unqualified C Record GetField without SSA destination".into() });
                    };
                    let (record, cname) = records.get(base).ok_or_else(|| CBackendError {
                        message: format!("unqualified C Record GetField from unavailable v{}", base.0),
                    })?;
                    let layout = qualified_record_fields(record_layouts, record)?;
                    let (_, ty) = layout.iter().find(|(name, _)| name == field_name)
                        .ok_or_else(|| CBackendError { message: format!(
                            "unqualified C Record '{}' unknown field '{}'", record, field_name
                        ) })?;
                    let dest_name = format!("v{}", dest.0);
                    out.push_str(&format!("  {} {} = {}.{};\n",
                        map_ctype_decl(*ty), dest_name, cname, record_field_symbol(field_name)));
                    values.insert(dest, (*ty, dest_name));
                }
                InstKind::SetField { base, field_name, value } => {
                    // This IR instruction is a functional update: the Oracle
                    // creates a NEW Record value only when dest is Some.
                    // Never overwrite the base SSA value (or aliases of it).
                    let Some(dest) = inst.dest else {
                        return Err(CBackendError {
                            message: format!(
                                "unqualified C Record SetField without result SSA at source span {:?}; refusing divergent in-place mutation",
                                inst.span,
                            ),
                        });
                    };
                    if records.contains_key(&dest) || values.contains_key(&dest) {
                        return Err(CBackendError { message: format!(
                            "unqualified C Record SetField duplicate destination v{}", dest.0,
                        ) });
                    }
                    let (record, original_name) = records.get(base).cloned().ok_or_else(|| {
                        CBackendError { message: format!(
                            "unqualified C Record SetField from unavailable base SSA v{}", base.0,
                        ) }
                    })?;
                    let layout = qualified_record_fields(record_layouts, &record)?;
                    let (_, expected) = layout.iter().find(|(field, _)| field == field_name)
                        .ok_or_else(|| CBackendError { message: format!(
                            "unqualified C Record '{}' unknown SetField '{}'", record, field_name,
                        ) })?;
                    let (actual, rhs) = values.get(value).cloned().ok_or_else(|| CBackendError {
                        message: format!(
                            "unqualified C Record '{}' SetField '{}' unavailable value SSA v{}",
                            record, field_name, value.0,
                        ),
                    })?;
                    if actual != *expected {
                        return Err(CBackendError { message: format!(
                            "unqualified C Record '{}' SetField '{}' type mismatch {:?} != {:?}",
                            record, field_name, actual, expected,
                        ) });
                    }
                    let updated_name = format!("v{}", dest.0);
                    out.push_str(&format!(
                        "  {} {} = {};\n",
                        record_type_symbol(&record), updated_name, original_name,
                    ));
                    out.push_str(&format!(
                        "  {}.{} = {};\n",
                        updated_name, record_field_symbol(field_name), rhs,
                    ));
                    records.insert(dest, (record, updated_name));
                }

                InstKind::ConstructEnumVariant { .. }
                | InstKind::GetEnumTag { .. }
                | InstKind::GetEnumPayload { .. } => {
                    // Tagged Enum layout, payload widths and discriminant
                    // projection have not been qualified in this C backend.
                    // Never silently emit a valid-looking C artifact whose
                    // live Enum value was discarded.
                    return Err(CBackendError {
                        message: format!(
                            "unqualified C Enum emission: unsupported instruction {:?} at source span {:?}; typed tag and payload representation required",
                            &inst.kind, inst.span,
                        ),
                    });
                }

                InstKind::BindStrand { name, expr } => {
                    if let Some(dest) = inst.dest {
                        // A Record local must preserve its nominal C struct
                        // identity; do not cast/copy it through Tensor ABI.
                        if let RValue::Local(source) = expr {
                            if let Some((record, from)) = records.get(source).cloned() {
                                let cname = format!("v{}", dest.0);
                                out.push_str(&format!("  {} {} = {};\n",
                                    record_type_symbol(&record), cname, from));
                                records.insert(dest, (record, cname));
                                continue;
                            }
                        }
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
                    let (operand_type, operand_name) = values
                        .get(operand)
                        .cloned()
                        .ok_or_else(|| CBackendError {
                            message: format!("untyped operand v{} in unary instruction", operand.0),
                        })?;
                    let (ct, expr_s) = match op {
                        UnaryOp::Neg if matches!(
                            operand_type,
                            CType::I32 | CType::I64 | CType::U32 | CType::U64
                                | CType::F32 | CType::F64
                        ) => (operand_type, format!("-({operand_name})")),
                        UnaryOp::Not if operand_type == CType::Bool => {
                            (CType::Bool, format!("!({operand_name})"))
                        }
                        _ => return Err(CBackendError {
                            message: format!(
                                "C emission does not qualify unary {op:?} on {operand_type:?}"
                            ),
                        }),
                    };
                    out.push_str("  ");
                    out.push_str(map_ctype_decl(ct));
                    out.push(' ');
                    out.push_str(&format!("v{} = {};\n", dest.0, expr_s));
                    values.insert(dest, (ct, format!("v{}", dest.0)));
                }

                InstKind::Binary { op, left, right } => {
                    let Some(dest) = inst.dest else { continue };
                    let (left_type, ln) = values
                        .get(left)
                        .cloned()
                        .ok_or_else(|| CBackendError {
                            message: format!("untyped left operand v{} in binary instruction", left.0),
                        })?;
                    let (right_type, rn) = values
                        .get(right)
                        .cloned()
                        .ok_or_else(|| CBackendError {
                            message: format!("untyped right operand v{} in binary instruction", right.0),
                        })?;
                    let (ct, expr_s) = emit_binop(*op, left_type, right_type, &ln, &rn)?;
                    out.push_str("  ");
                    out.push_str(map_ctype_decl(ct));
                    out.push(' ');
                    out.push_str(&format!("v{} = {};\n", dest.0, expr_s));
                    values.insert(dest, (ct, format!("v{}", dest.0)));
                }

                InstKind::Call { callee, args } => {
                    if let Some(target) = functions.get(callee) {
                        if target.params.len() != args.len() {
                            return Err(CBackendError { message: format!(
                                "Record ABI call '{}' has wrong arity: expected {}, got {}",
                                callee, target.params.len(), args.len(),
                            ) });
                        }
                        for (i, (arg, expected)) in args.iter().zip(target.params.iter()).enumerate() {
                            let expected_record = qualified_record_abi_type(&expected.ty, record_layouts)?;
                            match expected_record {
                                Some(name) => match records.get(arg) {
                                    Some((actual, _)) if actual == &name => {}
                                    _ => return Err(CBackendError { message: format!(
                                        "Record ABI call '{}' arg {} expects nominal '{}', got {:?}",
                                        callee, i, name, records.get(arg).map(|r| &r.0),
                                    ) }),
                                },
                                None => {
                                    if records.contains_key(arg) {
                                        return Err(CBackendError { message: format!(
                                            "Record ABI call '{}' arg {} passes Record to scalar parameter",
                                            callee, i,
                                        ) });
                                    }
                                    if let Some((actual, _)) = values.get(arg) {
                                        let required = map_type_to_ctype(&expected.ty);
                                        if *actual != required {
                                            return Err(CBackendError { message: format!(
                                                "Record ABI call '{}' arg {} scalar type mismatch {:?} != {:?}",
                                                callee, i, actual, required,
                                            ) });
                                        }
                                    } else {
                                        return Err(CBackendError { message: format!(
                                            "Record ABI call '{}' arg {} has unemitted scalar v{}",
                                            callee, i, arg.0,
                                        ) });
                                    }
                                }
                            }
                        }
                        if let Some(record) = qualified_record_abi_type(&target.ret, record_layouts)? {
                            let arg_names = args.iter().map(|a| {
                                records.get(a).map(|(_, n)| n.clone())
                                    .or_else(|| values.get(a).map(|(_, n)| n.clone()))
                                    .expect("all Record ABI call args checked")
                            }).collect::<Vec<_>>();
                            if let Some(dest) = inst.dest {
                                if records.contains_key(&dest) || values.contains_key(&dest) {
                                    return Err(CBackendError { message: format!(
                                        "Record ABI call '{}' duplicates destination v{}", callee, dest.0,
                                    ) });
                                }
                                let local = format!("v{}", dest.0);
                                out.push_str(&format!("  {} {} = {}({});\n",
                                    record_type_symbol(&record), local, c_ident(callee), arg_names.join(", ")));
                                records.insert(dest, (record, local));
                            } else {
                                out.push_str(&format!("  {}({});\n", c_ident(callee), arg_names.join(", ")));
                            }
                            continue;
                        }
                    } else if args.iter().any(|arg| records.contains_key(arg)) {
                        return Err(CBackendError { message: format!(
                            "unqualified C Record argument in external/builtin call '{}'", callee
                        ) });
                    }
                    emit_call(
                        out, inst.dest, callee, args, &mut values, &records, ret_map,
                        &string_literals, &named_values,
                    );
                }
                InstKind::ComputeKernel { callee, args } => {
                    if args.iter().any(|arg| records.contains_key(arg)) {
                        return Err(CBackendError { message: format!(
                            "unqualified C Record argument in kernel '{}'", callee
                        ) });
                    }
                    emit_kernel(out, inst.dest, callee, args, &mut values);
                }
            }
        }

        // Terminator
        match &b.term {
            Terminator::Return(Some(v)) => {
                let expected_record = qualified_record_abi_type(&f.ret, record_layouts)?;
                let name = if let Some(expected) = expected_record {
                    let (actual, c_name) = records.get(v).ok_or_else(|| CBackendError {
                        message: format!("Record ABI return in '{}' is not emitted Record v{}", f.name, v.0),
                    })?;
                    if actual != &expected {
                        return Err(CBackendError { message: format!(
                            "Record ABI return in '{}' expects nominal '{}', got '{}'", f.name, expected, actual,
                        ) });
                    }
                    c_name
                } else {
                    if records.contains_key(v) {
                        return Err(CBackendError { message: format!(
                            "Record ABI return in '{}' passes Record to non-Record result", f.name,
                        ) });
                    }
                    let (_ct, c_name) = values.get(v).ok_or_else(|| CBackendError {
                        message: format!(
                            "unemitted C return SSA v{} in function '{}' block bb{} at source span {:?}; refusing fabricated return",
                            v.0, f.name, b.id.0, b.span,
                        ),
                    })?;
                    c_name
                };
                out.push_str("  return ");
                out.push_str(name);
                out.push_str(";\n");
            }
            Terminator::Return(None) => {
                out.push_str("  return;\n");
            }
            Terminator::Br(t) => {
                emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *t);
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
                emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *then_bb);
                out.push_str("    goto ");
                out.push_str(&bb_label(*then_bb));
                out.push_str(";\n");
                out.push_str("  } else {\n");
                emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *else_bb);
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
                    emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *bb2);
                    out.push_str("goto ");
                    out.push_str(&bb_label(*bb2));
                    out.push_str(";\n");
                }

                out.push_str("    default: ");
                emit_phi_assigns(out, &phi_inj, &values, &records, b.id, *default_bb);
                out.push_str("goto ");
                out.push_str(&bb_label(*default_bb));
                out.push_str(";\n");

                out.push_str("  }\n");
            }
        }
    }

    out.push_str("}\n");
    Ok(())
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
    records: &HashMap<ValueId, (String, String)>,
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
            // Record formal parameters use their C identifier (e.g. p),
            // not a fabricated SSA temporary. Preserve the nominal C ABI.
            let emitted = values.get(a).map(|(_, n)| n.clone())
                .or_else(|| records.get(a).map(|(_, n)| n.clone()))
                .unwrap_or_else(|| format!("v{}", a.0));
            out.push_str(&emitted);
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
            // Record formal parameters use their C identifier (e.g. p),
            // not a fabricated SSA temporary. Preserve the nominal C ABI.
            let emitted = values.get(a).map(|(_, n)| n.clone())
                .or_else(|| records.get(a).map(|(_, n)| n.clone()))
                .unwrap_or_else(|| format!("v{}", a.0));
            out.push_str(&emitted);
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

fn emit_binop(
    op: BinOp, lhs_type: CType, rhs_type: CType, l: &str, r: &str,
) -> Result<(CType, String), CBackendError> {
    use BinOp::*;
    // The IR presently carries no explicit per-instruction result type.
    // Do not use C's implicit mixed-type conversions or fabricate U32:
    // the supported arithmetic subset requires matching numeric operands.
    let arithmetic = matches!(op, Add | Sub | Mul);
    if arithmetic {
        if lhs_type != rhs_type || !matches!(
            lhs_type,
            CType::U32 | CType::U64 | CType::I32 | CType::I64
                | CType::F32 | CType::F64
        ) {
            return Err(CBackendError {
                message: format!(
                    "unqualified C numeric arithmetic: {op:?} {lhs_type:?} vs {rhs_type:?}"
                ),
            });
        }
        let symbol = match op { Add => "+", Sub => "-", Mul => "*", _ => unreachable!() };
        return Ok((lhs_type, format!("({l}) {symbol} ({r})")));
    }

    let pair = match op {
        // Preserve the original bounded U32 division rule ONLY for U32.
        // 64-bit, signed and floating division require their own zero and
        // overflow policy before being accepted; the old u32 divisor cast
        // silently corrupted these types.
        Div if lhs_type == CType::U32 && rhs_type == CType::U32 => (
            CType::U32,
            format!(
                "(AURA_RANGE_CHECK_U32((uint32_t)({r}), 1u, 0xFFFFFFFFu, \"divisor\"), ({l}) / ({r}))"
            ),
        ),
        Div => return Err(CBackendError {
            message: format!(
                "unqualified C numeric division: {lhs_type:?} vs {rhs_type:?}"
            ),
        }),
        Eq => (CType::Bool, format!("({l}) == ({r})")),
        Ne => (CType::Bool, format!("({l}) != ({r})")),
        Lt => (CType::Bool, format!("({l}) < ({r})")),
        Gt => (CType::Bool, format!("({l}) > ({r})")),
        Le => (CType::Bool, format!("({l}) <= ({r})")),
        Ge => (CType::Bool, format!("({l}) >= ({r})")),
        And => (CType::Bool, format!("({l}) && ({r})")),
        Or => (CType::Bool, format!("({l}) || ({r})")),
        _ => unreachable!("arithmetic handled above"),
    };
    Ok(pair)
}

fn emit_phi_assigns(
    out: &mut String,
    phi_inj: &HashMap<(BlockId, BlockId), Vec<(ValueId, ValueId)>>,
    values: &HashMap<ValueId, (CType, String)>,
    records: &HashMap<ValueId, (String, String)>,
    pred: BlockId,
    target: BlockId,
) {
    if let Some(assigns) = phi_inj.get(&(pred, target)) {
        for (dest, incoming) in assigns {
            let dn = format!("v{}", dest.0);
            // Phi predecessor edges may carry named Record formals as
            // well as local vN values. A fallback vN for a formal parameter
            // would reference an undeclared variable, breaking native ABI.
            let sn = values.get(incoming).map(|(_, n)| n.clone())
                .or_else(|| records.get(incoming).map(|(_, n)| n.clone()))
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
            record_layouts: BTreeMap::new(),
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
