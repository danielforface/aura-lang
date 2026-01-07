#![forbid(unsafe_code)]

use aura_ast::{ExprKind};
use std::collections::HashMap;

use aura_nexus::{AuraPlugin, NexusDiagnostic, PluginCapability, Z3Call, Z3CallEnv};

#[cfg(feature = "z3")]
use aura_nexus::{record_proof, ProofNote};

#[cfg(feature = "z3")]
use z3::ast::Ast;

pub struct AuraAiPlugin;

impl AuraAiPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl AuraPlugin for AuraAiPlugin {
    fn name(&self) -> &'static str {
        "aura-ai"
    }

    fn capabilities(&self) -> &'static [PluginCapability] {
        &[PluginCapability::Z3Theories, PluginCapability::LlvmIntrinsics]
    }

    #[cfg(feature = "z3")]
    fn on_z3_int_call<'ctx>(
        &self,
        call: &Z3Call<'_, '_>,
        env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Option<Result<z3::ast::Int<'ctx>, NexusDiagnostic>> {
        match call.name {
            "ai.load_model" => Some(ai_load_model(call, env, self.name())),
            "ai.infer" => Some(ai_infer(call, env, self.name())),
            _ => None,
        }
    }
}

#[cfg(feature = "z3")]
fn ai_load_model<'ctx>(
    call: &Z3Call<'_, '_>,
    env: &mut dyn Z3CallEnv<'ctx>,
    plugin: &'static str,
) -> Result<z3::ast::Int<'ctx>, NexusDiagnostic> {
    if call.args.len() != 1 {
        return Err(NexusDiagnostic::new(
            plugin,
            call.span,
            "ai.load_model expects 1 arg (string literal path)",
        ));
    }

    let path_s = match &call.args[0].kind {
        ExprKind::StringLit(s) => s,
        _ => {
            return Err(NexusDiagnostic::new(
                plugin,
                call.args[0].span,
                "ai.load_model path must be a string literal",
            ))
        }
    };

    let mut path = std::path::PathBuf::from(path_s);
    if path.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            path = cwd.join(path);
        }
    }

    let shapes = aura_bridge::onnx::read_onnx_io_shapes(&path).map_err(|e| {
        NexusDiagnostic::new(
            plugin,
            call.args[0].span,
            format!("failed to read ONNX model contract from '{path_s}': {e}"),
        )
    })?;

    let to_static = |dims: Vec<aura_bridge::onnx::OnnxDim>| -> Result<Vec<u64>, NexusDiagnostic> {
        let mut out: Vec<u64> = Vec::with_capacity(dims.len());
        for d in dims {
            match d {
                aura_bridge::onnx::OnnxDim::Known(v) if v > 0 => out.push(v as u64),
                aura_bridge::onnx::OnnxDim::Known(_) => {
                    return Err(NexusDiagnostic::new(
                        plugin,
                        call.args[0].span,
                        "dynamic/unknown ONNX dims are not supported yet (please export a fixed-shape model)",
                    ));
                }
                aura_bridge::onnx::OnnxDim::Symbolic(_) => {
                    return Err(NexusDiagnostic::new(
                        plugin,
                        call.args[0].span,
                        "symbolic ONNX dims are not supported yet (please export a fixed-shape model)",
                    ));
                }
            }
        }
        Ok(out)
    };

    let in_dims = to_static(shapes.input)?;
    let out_dims = to_static(shapes.output)?;

    let h = env.fresh_int("model");

    // u32-ish bounds.
    let lo = z3::ast::Int::from_u64(env.ctx(), 0);
    let hi = z3::ast::Int::from_u64(env.ctx(), 0xFFFF_FFFF);
    env.push_constraint(h.ge(&lo));
    env.push_constraint(h.le(&hi));

    // Encode shapes for later verification steps using a shared uninterpreted function:
    // model_in_dim(handle, idx) == dim
    // model_out_dim(handle, idx) == dim
    let f_in = z3::FuncDecl::new(
        env.ctx(),
        "model_in_dim",
        &[&z3::Sort::int(env.ctx()), &z3::Sort::int(env.ctx())],
        &z3::Sort::int(env.ctx()),
    );
    for (i, d) in in_dims.iter().copied().enumerate() {
        let idx = z3::ast::Int::from_u64(env.ctx(), i as u64);
        let dim = f_in.apply(&[&h, &idx]).as_int().expect("int");
        let dv = z3::ast::Int::from_u64(env.ctx(), d);
        env.push_constraint(dim._eq(&dv));
    }

    let f_out = z3::FuncDecl::new(
        env.ctx(),
        "model_out_dim",
        &[&z3::Sort::int(env.ctx()), &z3::Sort::int(env.ctx())],
        &z3::Sort::int(env.ctx()),
    );
    for (i, d) in out_dims.iter().copied().enumerate() {
        let idx = z3::ast::Int::from_u64(env.ctx(), i as u64);
        let dim = f_out.apply(&[&h, &idx]).as_int().expect("int");
        let dv = z3::ast::Int::from_u64(env.ctx(), d);
        env.push_constraint(dim._eq(&dv));
    }

    // Store shapes keyed by the model handle so `ai.infer` can check them.
    #[derive(Default)]
    struct ModelShapesByHandle(HashMap<String, (Vec<u64>, Vec<u64>)>);

    if env.nexus().get::<ModelShapesByHandle>().is_none() {
        env.nexus().insert(ModelShapesByHandle::default());
    }
    env.nexus()
        .get_mut::<ModelShapesByHandle>()
        .expect("inserted")
        .0
        .insert(h.to_string(), (in_dims, out_dims));

    Ok(h)
}

#[cfg(feature = "z3")]
fn ai_infer<'ctx>(
    call: &Z3Call<'_, '_>,
    env: &mut dyn Z3CallEnv<'ctx>,
    plugin: &'static str,
) -> Result<z3::ast::Int<'ctx>, NexusDiagnostic> {
    if call.args.len() != 2 {
        return Err(NexusDiagnostic::new(
            plugin,
            call.span,
            "ai.infer expects 2 args (model, input)",
        ));
    }

    let model = env.eval_int(call.args[0])?;
    let input = env.eval_int(call.args[1])?;

    #[derive(Default)]
    struct ModelShapesByHandle(HashMap<String, (Vec<u64>, Vec<u64>)>);

    let shapes = env
        .nexus()
        .get::<ModelShapesByHandle>()
        .and_then(|m| m.0.get(&model.to_string()).cloned())
        .ok_or_else(|| {
            NexusDiagnostic::new(
                plugin,
                call.args[0].span,
                "model contract is unknown; bind ai.load_model(...) to a val before calling infer",
            )
        })?;

    let in_shape = shapes.0;
    let out_shape = shapes.1;

    let f_tdim = z3::FuncDecl::new(
        env.ctx(),
        "tensor_dim",
        &[&z3::Sort::int(env.ctx()), &z3::Sort::int(env.ctx())],
        &z3::Sort::int(env.ctx()),
    );
    let f_mdim = z3::FuncDecl::new(
        env.ctx(),
        "model_in_dim",
        &[&z3::Sort::int(env.ctx()), &z3::Sort::int(env.ctx())],
        &z3::Sort::int(env.ctx()),
    );

    let mut eqs: Vec<String> = Vec::new();
    for (i, _d) in in_shape.iter().enumerate() {
        let idx = z3::ast::Int::from_u64(env.ctx(), i as u64);
        let td = f_tdim.apply(&[&input, &idx]).as_int().expect("int");
        let md = f_mdim.apply(&[&model, &idx]).as_int().expect("int");
        let ok = td._eq(&md);

        eqs.push(ok.to_string());

        env.prove_implied(
            ok.not(),
            call.args[1].span,
            "Inference Error: input tensor shape does not match model contract",
        )?;
        env.push_constraint(ok);
    }

    if !in_shape.is_empty() {
        record_proof(
            env.nexus(),
            ProofNote {
                plugin: plugin.to_string(),
                span: call.span,
                message: format!(
                    "Verified by Aura-AI Plugin: input tensor shape matches model contract ({} dims).",
                    in_shape.len()
                ),
                smt: Some(eqs.join(" AND ")),
                related: Vec::new(),
                kind: "verified",
                mask: None,
                range: None,
                unsat_core: Vec::new(),
                interpolant: None,
            },
        );
    }

    let out = env.fresh_int("tensor");
    let lo = z3::ast::Int::from_u64(env.ctx(), 0);
    let hi = z3::ast::Int::from_u64(env.ctx(), 0xFFFF_FFFF);
    env.push_constraint(out.ge(&lo));
    env.push_constraint(out.le(&hi));

    if !out_shape.is_empty() {
        let f_mout = z3::FuncDecl::new(
            env.ctx(),
            "model_out_dim",
            &[&z3::Sort::int(env.ctx()), &z3::Sort::int(env.ctx())],
            &z3::Sort::int(env.ctx()),
        );
        for (i, _d) in out_shape.iter().enumerate() {
            let idx = z3::ast::Int::from_u64(env.ctx(), i as u64);
            let td = f_tdim.apply(&[&out, &idx]).as_int().expect("int");
            let md = f_mout.apply(&[&model, &idx]).as_int().expect("int");
            env.push_constraint(td._eq(&md));
        }
    }

    Ok(out)
}
