#![forbid(unsafe_code)]

#[cfg(feature = "llvm")]
use std::collections::{BTreeMap, HashMap};

use aura_ir::{DebugSource, ModuleIR};
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("LLVM backend error: {message}")]
#[diagnostic(code(aura::backend_llvm))]
#[allow(unused_assignments)]
pub struct LlvmBackendError {
    pub message: String,
}

pub struct LlvmArtifacts {
    /// LLVM IR text (for debugging and as a bridge before object emission is implemented).
    pub llvm_ir: String,
}

/// Emit LLVM IR from Aura-IR.
///
/// This is intentionally a *skeleton* in Phase 3:
/// - With `--features aura-backend-llvm/llvm`, it uses inkwell to build a module and prints IR.
/// - Without the feature, it errors with a clear message.
pub fn emit_llvm_ir(
    module: &ModuleIR,
    debug: Option<&DebugSource>,
) -> Result<LlvmArtifacts, LlvmBackendError> {
    #[cfg(feature = "llvm")]
    {
        let mut m = module.clone();
        aura_ir::optimize_module(&mut m);

        if let Err(e) = aura_ir::validate_module(&m) {
            return Err(LlvmBackendError {
                message: format!("IR validation failed before LLVM emission: {e}"),
            });
        }

        let llvm_ir = emit_module_llvm(&m, debug)?;
        Ok(LlvmArtifacts { llvm_ir })
    }

    #[cfg(not(feature = "llvm"))]
    {
        let _ = module;
        let _ = debug;
        Err(LlvmBackendError {
            message: "LLVM backend not enabled. Rebuild with `--features aura-backend-llvm/llvm`.".to_string(),
        })
    }
}

#[cfg(feature = "llvm")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExternLlvmCallConv {
    C,
    Stdcall,
}

#[cfg(feature = "llvm")]
fn resolve_extern_symbol_and_cc(
    aura_name: &str,
    declared: aura_ir::CallConv,
) -> (&str, ExternLlvmCallConv) {
    // Name-based override for ergonomics without syntax changes.
    // Example: `extern cell __stdcall_MessageBoxA(...): u32`.
    let mut sym = aura_name.strip_prefix("__ffi_").unwrap_or(aura_name);

    let mut cc = match declared {
        aura_ir::CallConv::C => ExternLlvmCallConv::C,
        aura_ir::CallConv::Stdcall => ExternLlvmCallConv::Stdcall,
    };

    if let Some(rest) = sym.strip_prefix("__stdcall_") {
        sym = rest;
        cc = ExternLlvmCallConv::Stdcall;
    } else if let Some(rest) = sym.strip_prefix("__cdecl_") {
        sym = rest;
        cc = ExternLlvmCallConv::C;
    }

    (sym, cc)
}

#[cfg(feature = "llvm")]
fn emit_module_llvm(module: &ModuleIR, debug: Option<&DebugSource>) -> Result<String, LlvmBackendError> {
    let host = target_lexicon::HOST;
    let triple = host.to_string();

    let mut out = String::new();
    out.push_str("; ModuleID = 'aura'\n");
    out.push_str("; Proof-carrying: IR validated by aura_ir::validate_module\n");
    out.push_str("source_filename = \"aura\"\n");
    out.push_str(&format!("target triple = \"{}\"\n\n", triple));

    let mut emitter = Emitter::new(module, debug);

    // Stdlib/runtime externs (prototype ABI).
    out.push_str("declare void @aura_io_println(ptr)\n");
    out.push_str("declare void @aura_range_check_u32(i32, i32, i32)\n");
    out.push_str("declare i32 @aura_tensor_new(i32)\n");
    out.push_str("declare i32 @aura_tensor_len(i32)\n");
    out.push_str("declare i32 @aura_tensor_get(i32, i32)\n");
    out.push_str("declare void @aura_tensor_set(i32, i32, i32)\n");
    out.push_str("declare i32 @aura_ai_load_model(ptr)\n");
    out.push_str("declare i32 @aura_ai_infer(i32, i32)\n");
    out.push_str("declare i32 @io_load_tensor(ptr)\n");
    out.push_str("declare void @io_display(i32)\n");
    out.push_str("declare i32 @compute_gradient(i32, i32)\n\n");

    // User extern cells (including aura-bridge generated `__ffi_*`).
    for (name, sig) in &module.externs {
        let (sym, cc) = resolve_extern_symbol_and_cc(name, sig.call_conv);

        let (ret_ty, _ret_is_void) = map_type_to_llvm(&sig.ret).ok_or_else(|| LlvmBackendError {
            message: format!("unsupported return type for extern '{name}': {:?}", sig.ret),
        })?;

        let mut args_s = String::new();
        for (i, pty) in sig.params.iter().enumerate() {
            let (aty, _is_void) = map_type_to_llvm(pty).ok_or_else(|| LlvmBackendError {
                message: format!("unsupported param type for extern '{name}': {:?}", pty),
            })?;
            if i > 0 {
                args_s.push_str(", ");
            }
            args_s.push_str(&aty);
        }

        let cc_s = match cc {
            ExternLlvmCallConv::C => "",
            ExternLlvmCallConv::Stdcall => "x86_stdcallcc ",
        };
        out.push_str(&format!("declare {cc_s}{ret_ty} @{sym}({args_s})\n"));
    }

    if !module.externs.is_empty() {
        out.push('\n');
    }

    // Emit functions.
    for (name, f) in &module.functions {
        if name == "main" {
            continue;
        }
        emitter.emit_function(&mut out, name, f)?;
        out.push('\n');
    }

    if let Some(main_f) = module.functions.get("main") {
        emitter.emit_function(&mut out, "aura_main", main_f)?;
        out.push('\n');
    }

    // Emit any string literal globals referenced by functions.
    // (LLVM allows globals anywhere at top-level, so this can come after functions.)
    emitter.emit_globals(&mut out);

    // Always provide a runtime entrypoint.
    out.push_str("define i32 @aura_entry() {\n");
    out.push_str("entry:\n");
    if module.functions.contains_key("main") {
        out.push_str("  call void @aura_main()\n");
    }
    out.push_str("  ret i32 0\n");
    out.push_str("}\n");

    Ok(out)
}

#[cfg(feature = "llvm")]
struct Emitter<'m> {
    module: &'m ModuleIR,
    debug: Option<&'m DebugSource>,
    /// Map Aura function name -> LLVM symbol name.
    fn_syms: BTreeMap<String, String>,
    next_global: u32,
    globals: Vec<(String, String, usize)>,
}

#[cfg(feature = "llvm")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LlvmTy {
    I1,
    I32,
    Ptr,
}

#[cfg(feature = "llvm")]
impl LlvmTy {
    fn as_str(self) -> &'static str {
        match self {
            LlvmTy::I1 => "i1",
            LlvmTy::I32 => "i32",
            LlvmTy::Ptr => "ptr",
        }
    }
}

#[cfg(feature = "llvm")]
impl<'m> Emitter<'m> {
    fn new(module: &'m ModuleIR, debug: Option<&'m DebugSource>) -> Self {
        let mut fn_syms = BTreeMap::new();
        for name in module.functions.keys() {
            if name == "main" {
                fn_syms.insert(name.clone(), "aura_main".to_string());
            } else {
                fn_syms.insert(name.clone(), llvm_ident(name));
            }
        }
        Self {
            module,
            debug,
            fn_syms,
            next_global: 0,
            globals: Vec::new(),
        }
    }

    fn emit_dbg_comment(&self, out: &mut String, span: aura_ast::Span) {
        let Some(dbg) = self.debug else { return };
        let lc = dbg.line_col(span);
        out.push_str(&format!("  ; dbg {}:{}:{}\n", dbg.file_name, lc.line, lc.col));
    }

    fn emit_globals(&mut self, out: &mut String) {
        for (name, data, nbytes) in self.globals.drain(..) {
            out.push_str(&format!(
                "@{name} = private unnamed_addr constant [{nbytes} x i8] c\"{data}\", align 1\n"
            ));
        }
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push('\n');
    }

    fn emit_function(
        &mut self,
        out: &mut String,
        public_name: &str,
        f: &aura_ir::FunctionIR,
    ) -> Result<(), LlvmBackendError> {
        let llvm_name = llvm_ident(public_name);
        let (ret_ty, ret_is_void) = map_type_to_llvm(&f.ret)
            .ok_or_else(|| LlvmBackendError { message: format!("unsupported return type in function '{}': {:?}", f.name, f.ret) })?;

        // Parameters
        let mut args_s = String::new();
        let mut value_names: HashMap<aura_ir::ValueId, String> = HashMap::new();
        let mut value_types: HashMap<aura_ir::ValueId, LlvmTy> = HashMap::new();
        for (i, p) in f.params.iter().enumerate() {
            let (pty, _is_void) = map_type_to_llvm(&p.ty)
                .ok_or_else(|| LlvmBackendError { message: format!("unsupported param type in function '{}': {:?}", f.name, p.ty) })?;
            if i > 0 {
                args_s.push_str(", ");
            }
            let vname = value_reg(p.value);
            args_s.push_str(&format!("{pty} {vname}"));
            value_names.insert(p.value, vname);
            let pty_enum = map_type_to_llvm_ty(&p.ty).ok_or_else(|| LlvmBackendError {
                message: format!("unsupported param type in function '{}': {:?}", f.name, p.ty),
            })?;
            value_types.insert(p.value, pty_enum);
        }

        out.push_str(&format!("define {ret_ty} @{llvm_name}({args_s}) {{\n"));

        // Emit blocks.
        // Ensure entry is first for readability.
        let mut blocks: Vec<&aura_ir::BasicBlock> = f.blocks.iter().collect();
        blocks.sort_by_key(|b| if b.id == f.entry { 0 } else { 1 });

        // Track which blocks exist (for validation).
        let mut block_labels: HashMap<aura_ir::BlockId, String> = HashMap::new();
        for b in &f.blocks {
            block_labels.insert(b.id, block_label(b.id));
        }

        // Infer value types (needed for correct `phi` emission, esp. for bool short-circuit lowering).
        infer_value_types(self.module, f, &mut value_types)?;

        for b in blocks {
            let label = block_labels
                .get(&b.id)
                .cloned()
                .unwrap_or_else(|| block_label(b.id));
            out.push_str(&format!("{label}:\n"));

            // Best-effort source mapping for reading/stepping through emitted IR.
            self.emit_dbg_comment(out, b.span);

            // Emit phi nodes first (LLVM requirement).
            for inst in &b.insts {
                if let aura_ir::InstKind::Phi { incomings } = &inst.kind {
                    self.emit_dbg_comment(out, inst.span);
                    let Some(dest) = inst.dest else {
                        return Err(LlvmBackendError { message: "phi instruction missing dest".to_string() });
                    };

                    let phi_ty = value_types
                        .get(&dest)
                        .copied()
                        .or_else(|| infer_phi_type_from_incomings(incomings, &value_types))
                        .ok_or_else(|| {
                            LlvmBackendError {
                                message: format!(
                                    "cannot infer phi type for v{} in function '{}'",
                                    dest.0, f.name
                                ),
                            }
                        })?
                        .as_str();
                    let dest_name = value_reg(dest);
                    value_names.insert(dest, dest_name.clone());

                    let mut incoming_s = String::new();
                    for (i, (pred, v)) in incomings.iter().enumerate() {
                        if i > 0 {
                            incoming_s.push_str(", ");
                        }
                        let pred_label = block_labels
                            .get(pred)
                            .cloned()
                            .unwrap_or_else(|| block_label(*pred));
                        let vref = value_ref(*v, &value_names);
                        incoming_s.push_str(&format!("[ {vref}, %{pred_label} ]"));
                    }
                    out.push_str(&format!("  {dest_name} = phi {phi_ty} {incoming_s}\n"));
                }
            }

            for inst in &b.insts {
                self.emit_dbg_comment(out, inst.span);
                match &inst.kind {
                    aura_ir::InstKind::Phi { .. } => {}

                    aura_ir::InstKind::AllocCapability { .. } => {}

                    aura_ir::InstKind::RangeCheckU32 { value, lo, hi } => {
                        let vref = value_ref(*value, &value_names);
                        out.push_str(&format!(
                            "  call void @aura_range_check_u32(i32 {vref}, i32 {}, i32 {})\n",
                            *lo,
                            *hi
                        ));
                    }

                    aura_ir::InstKind::BindStrand { expr, .. } => {
                        let Some(dest) = inst.dest else { continue };
                        match expr {
                            aura_ir::RValue::ConstU32(n) => {
                                value_names.insert(dest, n.to_string());
                                value_types.insert(dest, LlvmTy::I32);
                            }
                            aura_ir::RValue::ConstBool(b) => {
                                value_names.insert(dest, if *b { "true".to_string() } else { "false".to_string() });
                                value_types.insert(dest, LlvmTy::I1);
                            }
                            aura_ir::RValue::ConstString(_) => {
                                // Lower string literal to a private global and materialize a `ptr` via gep.
                                let s = match expr {
                                    aura_ir::RValue::ConstString(s) => s,
                                    _ => unreachable!(),
                                };
                                let (gname, nbytes, _data) = self.intern_c_string(s);
                                let dest_name = value_reg(dest);
                                out.push_str(&format!(
                                    "  {dest_name} = getelementptr inbounds [{nbytes} x i8], ptr @{gname}, i64 0, i64 0\n"
                                ));
                                value_names.insert(dest, dest_name);
                                value_types.insert(dest, LlvmTy::Ptr);
                            }
                            aura_ir::RValue::Local(src) => {
                                let src_ref = value_ref(*src, &value_names);
                                value_names.insert(dest, src_ref);
                                if let Some(t) = value_types.get(src).copied() {
                                    value_types.insert(dest, t);
                                }
                            }
                        }
                    }

                    aura_ir::InstKind::Unary { op, operand } => {
                        let Some(dest) = inst.dest else { continue };
                        let dest_name = value_reg(dest);
                        let operand_ref = value_ref(*operand, &value_names);
                        match op {
                            aura_ir::UnaryOp::Neg => {
                                out.push_str(&format!("  {dest_name} = sub i32 0, {operand_ref}\n"));
                                value_types.insert(dest, LlvmTy::I32);
                            }
                            aura_ir::UnaryOp::Not => {
                                // i1 logical not: xor with true
                                out.push_str(&format!("  {dest_name} = xor i1 true, {operand_ref}\n"));
                                value_types.insert(dest, LlvmTy::I1);
                            }
                        }
                        value_names.insert(dest, dest_name);
                    }

                    aura_ir::InstKind::Binary { op, left, right } => {
                        let Some(dest) = inst.dest else { continue };
                        let dest_name = value_reg(dest);
                        let lref = value_ref(*left, &value_names);
                        let rref = value_ref(*right, &value_names);

                        // In LLVM IR, `udiv` by 0 is UB. Enforce a runtime trap.
                        if *op == aura_ir::BinOp::Div {
                            out.push_str(&format!(
                                "  call void @aura_range_check_u32(i32 {rref}, i32 1, i32 -1)\n"
                            ));
                        }

                        let (ty, instr) = emit_binop_llvm(*op);
                        out.push_str(&format!("  {dest_name} = {instr} {ty} {lref}, {rref}\n"));
                        value_names.insert(dest, dest_name);
                        value_types.insert(dest, if ty == "i1" { LlvmTy::I1 } else { LlvmTy::I32 });
                    }

                    aura_ir::InstKind::Call { callee, args } => {
                        self.emit_call_like(
                            out,
                            f,
                            inst.dest,
                            callee,
                            args,
                            &value_names,
                            &mut value_types,
                        )?;
                    }

                    aura_ir::InstKind::ComputeKernel { .. } => {
                        // For now, treat compute kernels as plain calls (sync). The C backend may map these to async.
                        let aura_ir::InstKind::ComputeKernel { callee, args } = &inst.kind else {
                            unreachable!();
                        };
                        self.emit_call_like(
                            out,
                            f,
                            inst.dest,
                            callee,
                            args,
                            &value_names,
                            &mut value_types,
                        )?;
                    }
                }
            }

            // Terminator
            match &b.term {
                aura_ir::Terminator::Return(None) => {
                    if !ret_is_void {
                        return Err(LlvmBackendError {
                            message: format!("function '{}' returns {ret_ty} but block returns void", f.name),
                        });
                    }
                    out.push_str("  ret void\n");
                }
                aura_ir::Terminator::Return(Some(v)) => {
                    if ret_is_void {
                        return Err(LlvmBackendError {
                            message: format!("function '{}' is void but block returns a value", f.name),
                        });
                    }
                    let vref = value_ref(*v, &value_names);
                    out.push_str(&format!("  ret {ret_ty} {vref}\n"));
                }
                aura_ir::Terminator::Br(tgt) => {
                    let tgt_label = block_labels
                        .get(tgt)
                        .cloned()
                        .unwrap_or_else(|| block_label(*tgt));
                    out.push_str(&format!("  br label %{tgt_label}\n"));
                }
                aura_ir::Terminator::CondBr {
                    cond,
                    then_bb,
                    else_bb,
                } => {
                    let cref = value_ref(*cond, &value_names);
                    let then_label = block_labels
                        .get(then_bb)
                        .cloned()
                        .unwrap_or_else(|| block_label(*then_bb));
                    let else_label = block_labels
                        .get(else_bb)
                        .cloned()
                        .unwrap_or_else(|| block_label(*else_bb));
                    out.push_str(&format!(
                        "  br i1 {cref}, label %{then_label}, label %{else_label}\n"
                    ));
                }
                aura_ir::Terminator::Switch {
                    scrut,
                    default_bb,
                    cases,
                } => {
                    let sref = value_ref(*scrut, &value_names);
                    let default_label = block_labels
                        .get(default_bb)
                        .cloned()
                        .unwrap_or_else(|| block_label(*default_bb));

                    out.push_str(&format!("  switch i32 {sref}, label %{default_label} [\n"));
                    for (k, bb2) in cases {
                        let case_label = block_labels
                            .get(bb2)
                            .cloned()
                            .unwrap_or_else(|| block_label(*bb2));
                        out.push_str(&format!("    i32 {}, label %{case_label}\n", *k as u32));
                    }
                    out.push_str("  ]\n");
                }
            }

            out.push('\n');
        }

        out.push_str("}\n");
        Ok(())
    }

    fn intern_c_string(&mut self, s: &str) -> (String, usize, String) {
        let name = format!(".str{}", self.next_global);
        self.next_global += 1;

        // Emit as a C string (escape + add NUL).
        let mut data = String::new();
        for b in s.as_bytes() {
            match *b {
                b'\\' => data.push_str("\\5C"),
                b'\n' => data.push_str("\\0A"),
                b'\r' => data.push_str("\\0D"),
                b'\t' => data.push_str("\\09"),
                b'\"' => data.push_str("\\22"),
                0x20..=0x7E => data.push(*b as char),
                other => data.push_str(&format!("\\{:02X}", other)),
            }
        }
        data.push_str("\\00");
        let nbytes = s.as_bytes().len() + 1;

        self.globals.push((name.clone(), data.clone(), nbytes));
        (name, nbytes, data)
    }

    fn emit_call_like(
        &mut self,
        out: &mut String,
        cur_fn: &aura_ir::FunctionIR,
        dest: Option<aura_ir::ValueId>,
        callee: &str,
        args: &[aura_ir::ValueId],
        value_names: &HashMap<aura_ir::ValueId, String>,
        value_types: &mut HashMap<aura_ir::ValueId, LlvmTy>,
    ) -> Result<(), LlvmBackendError> {
        // Resolve callee: module fn, user extern, or builtin extern.
        if let Some(target) = self.module.functions.get(callee) {
            let callee_sym = self
                .fn_syms
                .get(callee)
                .cloned()
                .unwrap_or_else(|| llvm_ident(callee));

            let (callee_ret_ty, callee_ret_is_void) = map_type_to_llvm(&target.ret)
                .ok_or_else(|| LlvmBackendError { message: format!("unsupported return type for callee '{callee}': {:?}", target.ret) })?;
            if args.len() != target.params.len() {
                return Err(LlvmBackendError {
                    message: format!(
                        "arity mismatch calling '{callee}': expected {}, got {}",
                        target.params.len(),
                        args.len()
                    ),
                });
            }

            let mut args_s = String::new();
            for (i, (arg_id, param)) in args.iter().zip(target.params.iter()).enumerate() {
                let (pty, _) = map_type_to_llvm(&param.ty)
                    .ok_or_else(|| LlvmBackendError { message: format!("unsupported param type for callee '{callee}': {:?}", param.ty) })?;
                if i > 0 {
                    args_s.push_str(", ");
                }
                args_s.push_str(&format!("{pty} {}", value_ref(*arg_id, value_names)));
            }

            return emit_call_site(
                out,
                dest,
                callee_ret_ty,
                callee_ret_is_void,
                &callee_sym,
                &args_s,
                value_types,
                map_type_to_llvm_ty(&target.ret),
                None,
            );
        }

        if let Some(ext) = self.module.externs.get(callee) {
            let (sym, cc) = resolve_extern_symbol_and_cc(callee, ext.call_conv);
            let (callee_ret_ty, callee_ret_is_void) = map_type_to_llvm(&ext.ret).ok_or_else(|| {
                LlvmBackendError {
                    message: format!(
                        "unsupported return type for extern '{callee}': {:?}",
                        ext.ret
                    ),
                }
            })?;

            if args.len() != ext.params.len() {
                return Err(LlvmBackendError {
                    message: format!(
                        "arity mismatch calling extern '{callee}': expected {}, got {}",
                        ext.params.len(),
                        args.len()
                    ),
                });
            }

            let mut args_s = String::new();
            for (i, (arg_id, param_ty)) in args.iter().zip(ext.params.iter()).enumerate() {
                let (pty, _) = map_type_to_llvm(param_ty).ok_or_else(|| LlvmBackendError {
                    message: format!(
                        "unsupported param type for extern '{callee}': {:?}",
                        param_ty
                    ),
                })?;
                if i > 0 {
                    args_s.push_str(", ");
                }
                args_s.push_str(&format!("{pty} {}", value_ref(*arg_id, value_names)));
            }

            return emit_call_site(out, dest, callee_ret_ty, callee_ret_is_void, sym, &args_s, value_types, map_type_to_llvm_ty(&ext.ret), match cc {
                ExternLlvmCallConv::C => None,
                ExternLlvmCallConv::Stdcall => Some("x86_stdcallcc"),
            });
        }

        // Builtin externs.
        let (sym, ret_ty, ret_kind, arg_tys) = builtin_abi(callee).ok_or_else(|| LlvmBackendError {
            message: format!("unknown callee '{callee}' in function '{}' (missing stdlib binding)", cur_fn.name),
        })?;

        if args.len() != arg_tys.len() {
            return Err(LlvmBackendError {
                message: format!(
                    "arity mismatch calling '{callee}': expected {}, got {}",
                    arg_tys.len(),
                    args.len()
                ),
            });
        }

        let mut args_s = String::new();
        for (i, (arg_id, aty)) in args.iter().zip(arg_tys.iter()).enumerate() {
            if i > 0 {
                args_s.push_str(", ");
            }
            args_s.push_str(&format!("{} {}", aty, value_ref(*arg_id, value_names)));
        }

        emit_call_site(
            out,
            dest,
            ret_ty.to_string(),
            ret_kind == LlvmRetKind::Void,
            sym,
            &args_s,
            value_types,
            match ret_kind {
                LlvmRetKind::Void => None,
                LlvmRetKind::I32 => Some(LlvmTy::I32),
            },
            None,
        )
    }
}

#[cfg(feature = "llvm")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LlvmRetKind {
    Void,
    I32,
}

#[cfg(feature = "llvm")]
fn emit_call_site(
    out: &mut String,
    dest: Option<aura_ir::ValueId>,
    ret_ty: String,
    ret_is_void: bool,
    callee_sym: &str,
    args_s: &str,
    value_types: &mut HashMap<aura_ir::ValueId, LlvmTy>,
    dest_ty: Option<LlvmTy>,
    call_cc: Option<&'static str>,
) -> Result<(), LlvmBackendError> {
    let cc_s = call_cc.map(|c| format!("{c} ")).unwrap_or_default();
    match (dest, ret_is_void) {
        (Some(d), false) => {
            let dest_name = value_reg(d);
            out.push_str(&format!("  {dest_name} = call {cc_s}{ret_ty} @{callee_sym}({args_s})\n"));
            if let Some(dt) = dest_ty {
                value_types.insert(d, dt);
            }
            Ok(())
        }
        (None, false) => Err(LlvmBackendError {
            message: "call produced a value but destination was None".to_string(),
        }),
        (Some(_), true) => Err(LlvmBackendError {
            message: format!("call to void function '{callee_sym}' assigned to a value"),
        }),
        (None, true) => {
            out.push_str(&format!("  call {cc_s}{ret_ty} @{callee_sym}({args_s})\n"));
            Ok(())
        }
    }
}

#[cfg(feature = "llvm")]
fn builtin_abi(name: &str) -> Option<(&'static str, &'static str, LlvmRetKind, Vec<&'static str>)> {
    match name {
        "io.println" => Some(("aura_io_println", "void", LlvmRetKind::Void, vec!["ptr"])),
        "io.load_tensor" => Some(("io_load_tensor", "i32", LlvmRetKind::I32, vec!["ptr"])),
        "io.display" => Some(("io_display", "void", LlvmRetKind::Void, vec!["i32"])),
        "compute_gradient" => Some(("compute_gradient", "i32", LlvmRetKind::I32, vec!["i32", "i32"])),
        "tensor.new" => Some(("aura_tensor_new", "i32", LlvmRetKind::I32, vec!["i32"])),
        "tensor.len" => Some(("aura_tensor_len", "i32", LlvmRetKind::I32, vec!["i32"])),
        "tensor.get" => Some(("aura_tensor_get", "i32", LlvmRetKind::I32, vec!["i32", "i32"])),
        "tensor.set" => Some(("aura_tensor_set", "void", LlvmRetKind::Void, vec!["i32", "i32", "i32"])),
        "ai.load_model" => Some(("aura_ai_load_model", "i32", LlvmRetKind::I32, vec!["ptr"])),
        "ai.infer" => Some(("aura_ai_infer", "i32", LlvmRetKind::I32, vec!["i32", "i32"])),
        _ => None,
    }
}

#[cfg(feature = "llvm")]
fn llvm_ident(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        let ok = ch.is_ascii_alphanumeric() || ch == '_';
        let ch = if ok { ch } else { '_' };
        if i == 0 && ch.is_ascii_digit() {
            out.push('_');
        }
        out.push(ch);
    }
    if out.is_empty() {
        out.push_str("_anon");
    }
    out
}

#[cfg(feature = "llvm")]
fn block_label(id: aura_ir::BlockId) -> String {
    format!("bb{}", id.0)
}

#[cfg(feature = "llvm")]
fn value_reg(id: aura_ir::ValueId) -> String {
    format!("%v{}", id.0)
}

#[cfg(feature = "llvm")]
fn value_ref(id: aura_ir::ValueId, value_names: &HashMap<aura_ir::ValueId, String>) -> String {
    value_names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| value_reg(id))
}

#[cfg(feature = "llvm")]
fn map_type_to_llvm(ty: &aura_ir::Type) -> Option<(String, bool)> {
    match ty {
        aura_ir::Type::Unit => Some(("void".to_string(), true)),
        aura_ir::Type::Bool => Some(("i1".to_string(), false)),
        aura_ir::Type::U32 => Some(("i32".to_string(), false)),
        aura_ir::Type::String => Some(("ptr".to_string(), false)),
        aura_ir::Type::Tensor => Some(("i32".to_string(), false)),
        aura_ir::Type::Opaque(_) => Some(("i32".to_string(), false)),
    }
}

#[cfg(feature = "llvm")]
fn map_type_to_llvm_ty(ty: &aura_ir::Type) -> Option<LlvmTy> {
    match ty {
        aura_ir::Type::Bool => Some(LlvmTy::I1),
        aura_ir::Type::U32 => Some(LlvmTy::I32),
        aura_ir::Type::String => Some(LlvmTy::Ptr),
        aura_ir::Type::Tensor => Some(LlvmTy::I32),
        aura_ir::Type::Unit => None,
        aura_ir::Type::Opaque(_) => Some(LlvmTy::I32),
    }
}

#[cfg(feature = "llvm")]
fn emit_binop_llvm(op: aura_ir::BinOp) -> (&'static str, &'static str) {
    use aura_ir::BinOp;

    match op {
        BinOp::Add => ("i32", "add"),
        BinOp::Sub => ("i32", "sub"),
        BinOp::Mul => ("i32", "mul"),
        BinOp::Div => ("i32", "udiv"),

        BinOp::Eq => ("i32", "icmp eq"),
        BinOp::Ne => ("i32", "icmp ne"),
        BinOp::Lt => ("i32", "icmp ult"),
        BinOp::Gt => ("i32", "icmp ugt"),
        BinOp::Le => ("i32", "icmp ule"),
        BinOp::Ge => ("i32", "icmp uge"),

        BinOp::And => ("i1", "and"),
        BinOp::Or => ("i1", "or"),
    }
}

#[cfg(feature = "llvm")]
fn infer_phi_type_from_incomings(
    incomings: &[(aura_ir::BlockId, aura_ir::ValueId)],
    value_types: &HashMap<aura_ir::ValueId, LlvmTy>,
) -> Option<LlvmTy> {
    let mut ty: Option<LlvmTy> = None;
    for (_, v) in incomings {
        let Some(vt) = value_types.get(v).copied() else { continue };
        match ty {
            None => ty = Some(vt),
            Some(t) if t == vt => {}
            Some(_) => return None,
        }
    }
    ty
}

#[cfg(feature = "llvm")]
fn infer_value_types(
    module: &ModuleIR,
    f: &aura_ir::FunctionIR,
    value_types: &mut HashMap<aura_ir::ValueId, LlvmTy>,
) -> Result<(), LlvmBackendError> {
    // Iterate to a fixed point since types can flow through `BindStrand Local` and `Phi`.
    let mut changed = true;
    let mut iters = 0;
    while changed {
        iters += 1;
        if iters > 16 {
            break;
        }
        changed = false;

        for b in &f.blocks {
            for inst in &b.insts {
                let Some(dest) = inst.dest else { continue };
                if value_types.contains_key(&dest) {
                    continue;
                }

                let inferred: Option<LlvmTy> = match &inst.kind {
                    aura_ir::InstKind::AllocCapability { .. } => None,
                    aura_ir::InstKind::RangeCheckU32 { .. } => None,
                    aura_ir::InstKind::ComputeKernel { .. } => None,

                    aura_ir::InstKind::BindStrand { expr, .. } => match expr {
                        aura_ir::RValue::ConstU32(_) => Some(LlvmTy::I32),
                        aura_ir::RValue::ConstBool(_) => Some(LlvmTy::I1),
                        aura_ir::RValue::ConstString(_) => Some(LlvmTy::Ptr),
                        aura_ir::RValue::Local(src) => value_types.get(src).copied(),
                    },

                    aura_ir::InstKind::Unary { op, .. } => match op {
                        aura_ir::UnaryOp::Neg => Some(LlvmTy::I32),
                        aura_ir::UnaryOp::Not => Some(LlvmTy::I1),
                    },

                    aura_ir::InstKind::Binary { op, .. } => match op {
                        aura_ir::BinOp::Add
                        | aura_ir::BinOp::Sub
                        | aura_ir::BinOp::Mul
                        | aura_ir::BinOp::Div => Some(LlvmTy::I32),

                        aura_ir::BinOp::Eq
                        | aura_ir::BinOp::Ne
                        | aura_ir::BinOp::Lt
                        | aura_ir::BinOp::Gt
                        | aura_ir::BinOp::Le
                        | aura_ir::BinOp::Ge
                        | aura_ir::BinOp::And
                        | aura_ir::BinOp::Or => Some(LlvmTy::I1),
                    },

                    aura_ir::InstKind::Phi { incomings } => infer_phi_type_from_incomings(incomings, value_types),

                    aura_ir::InstKind::Call { callee, .. } => {
                        if let Some(t) = module.functions.get(callee) {
                            map_type_to_llvm_ty(&t.ret)
                        } else if let Some(ext) = module.externs.get(callee) {
                            map_type_to_llvm_ty(&ext.ret)
                        } else if let Some((_sym, _ret_ty, ret_kind, _arg_tys)) = builtin_abi(callee) {
                            match ret_kind {
                                LlvmRetKind::Void => None,
                                LlvmRetKind::I32 => Some(LlvmTy::I32),
                            }
                        } else {
                            None
                        }
                    }
                };

                if let Some(ty) = inferred {
                    value_types.insert(dest, ty);
                    changed = true;
                }
            }
        }
    }

    Ok(())
}
