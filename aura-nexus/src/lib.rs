#![forbid(unsafe_code)]

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::PathBuf;

use aura_ast::{Expr, Span};
use miette::Diagnostic;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum PluginCapability {
    AstExtension,
    Z3Theories,
    LlvmIntrinsics,
    UiRuntime,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct UiNode {
    pub kind: String,
    #[serde(default)]
    pub props: Vec<(String, String)>,
    #[serde(default)]
    pub children: Vec<UiNode>,
}

impl UiNode {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            props: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn set_prop(&mut self, k: impl Into<String>, v: impl Into<String>) {
        let k = k.into();
        let v = v.into();
        if let Some(existing) = self.props.iter_mut().find(|(ek, _)| *ek == k) {
            existing.1 = v;
        } else {
            self.props.push((k, v));
        }
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct PluginManifest {
    pub name: String,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,

    // Basic "sandbox" gate: only run non-core plugins when explicitly trusted.
    #[serde(default)]
    pub trusted: bool,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct NexusManifest {
    #[serde(default)]
    pub plugins: Vec<PluginManifest>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("nexus error: {message}")]
#[diagnostic(code(aura::nexus))]
pub struct NexusError {
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct NexusRelated {
    pub span: Span,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct NexusDiagnostic {
    pub plugin: &'static str,
    pub message: String,
    pub span: Span,
    pub related: Vec<NexusRelated>,
    pub model: Option<String>,
}

impl NexusDiagnostic {
    pub fn new(plugin: &'static str, span: Span, message: impl Into<String>) -> Self {
        Self {
            plugin,
            message: message.into(),
            span,
            related: Vec::new(),
            model: None,
        }
    }
}

#[derive(Default)]
pub struct NexusContext {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

#[derive(Clone, Debug, Default)]
pub struct NexusFileContext {
    /// Path of the source file currently being verified/interpreted (if known).
    pub source_path: Option<PathBuf>,

    /// Resolved path to the nearest aura.toml (if known).
    pub manifest_path: Option<PathBuf>,
}

impl NexusContext {
    pub fn insert<T: Any + Send + Sync>(&mut self, v: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(v));
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
    }

    pub fn get_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut::<T>())
    }
}

#[cfg(feature = "z3")]
#[derive(Clone, Debug)]
pub struct ProofNote {
    pub plugin: String,
    pub span: Span,
    pub message: String,
    pub smt: Option<String>,
    pub related: Vec<NexusRelated>,

    // Optional machine-parsable fields for editor UX.
    pub kind: &'static str,
    pub mask: Option<u64>,
    pub range: Option<(u64, u64)>,

    /// If available, an UNSAT core (as SMT snippets) that explains a successful proof.
    ///
    /// This is best-effort: it depends on the Z3 build and the kinds of assertions used.
    pub unsat_core: Vec<String>,

    /// A best-effort derived lemma that, when conjoined with the negated goal, stays UNSAT.
    ///
    /// This is not a full Craig interpolant implementation; it is intended for lightweight
    /// explanations in editor/report UX.
    pub interpolant: Option<String>,
}

#[cfg(feature = "z3")]
#[derive(Default)]
pub struct ProofNotes(pub Vec<ProofNote>);

#[cfg(feature = "z3")]
pub fn record_proof(nexus: &mut NexusContext, note: ProofNote) {
    if nexus.get::<ProofNotes>().is_none() {
        nexus.insert(ProofNotes::default());
    }
    let notes = nexus.get_mut::<ProofNotes>().expect("inserted");
    notes.0.push(note);
}

#[cfg(feature = "z3")]
pub fn drain_proofs(nexus: &mut NexusContext) -> Vec<ProofNote> {
    if nexus.get::<ProofNotes>().is_none() {
        return Vec::new();
    }
    let notes = nexus.get_mut::<ProofNotes>().expect("exists");
    std::mem::take(&mut notes.0)
}

pub struct PreParseResult {
    pub new_source: String,
}

pub trait AuraPlugin {
    fn name(&self) -> &'static str;

    fn capabilities(&self) -> &'static [PluginCapability] {
        &[]
    }

    fn on_pre_parse(&self, _source: &str, _nexus: &mut NexusContext) -> Result<Option<PreParseResult>, NexusDiagnostic> {
        Ok(None)
    }

    fn on_ui_render(
        &self,
        _tree: &UiNode,
        _nexus: &mut NexusContext,
    ) -> Option<Result<(), NexusDiagnostic>> {
        None
    }

    #[cfg(feature = "z3")]
    fn on_z3_int_call<'ctx>(
        &self,
        _call: &Z3Call<'_, '_>,
        _env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Option<Result<z3::ast::Int<'ctx>, NexusDiagnostic>> {
        None
    }

    #[cfg(feature = "z3")]
    fn on_z3_bool_call<'ctx>(
        &self,
        _call: &Z3Call<'_, '_>,
        _env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Option<Result<z3::ast::Bool<'ctx>, NexusDiagnostic>> {
        None
    }
}

pub trait UiPluginDispatch {
    fn try_ui_render(
        &self,
        tree: &UiNode,
        nexus: &mut NexusContext,
    ) -> Option<Result<(), NexusDiagnostic>>;
}

#[derive(Clone, Debug, Default)]
pub struct UiRuntimeFeedback {
    pub close_requested: bool,
    pub clicked_callback_id: Option<u64>,
}

pub fn take_ui_feedback(nexus: &mut NexusContext) -> UiRuntimeFeedback {
    if let Some(fb) = nexus.get_mut::<UiRuntimeFeedback>() {
        std::mem::take(fb)
    } else {
        UiRuntimeFeedback::default()
    }
}

impl UiPluginDispatch for () {
    fn try_ui_render(
        &self,
        _tree: &UiNode,
        _nexus: &mut NexusContext,
    ) -> Option<Result<(), NexusDiagnostic>> {
        None
    }
}

macro_rules! impl_ui_dispatch_tuple {
    ($($name:ident),+ $(,)?) => {
        impl<$($name: AuraPlugin),+> UiPluginDispatch for ($($name,)+) {
            fn try_ui_render(
                &self,
                tree: &UiNode,
                nexus: &mut NexusContext,
            ) -> Option<Result<(), NexusDiagnostic>> {
                let mut out: Option<Result<(), NexusDiagnostic>> = None;
                PluginSet::for_each_plugin(self, |p| {
                    if out.is_some() {
                        return;
                    }
                    out = p.on_ui_render(tree, nexus);
                });
                out
            }
        }
    };
}

impl_ui_dispatch_tuple!(A);
impl_ui_dispatch_tuple!(A, B);
impl_ui_dispatch_tuple!(A, B, C);
impl_ui_dispatch_tuple!(A, B, C, D);

pub trait PluginSet {
    fn for_each_plugin<F: FnMut(&dyn AuraPlugin)>(&self, f: F);
}

macro_rules! impl_plugin_set_tuple {
    ($($name:ident),+ $(,)?) => {
        impl<$($name: AuraPlugin),+> PluginSet for ($($name,)+) {
            fn for_each_plugin<F: FnMut(&dyn AuraPlugin)>(&self, mut f: F) {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                $(f($name);)+
            }
        }
    };
}

// Implement for tuples up to 4 plugins (easy to extend later).
impl_plugin_set_tuple!(A);
impl_plugin_set_tuple!(A, B);
impl_plugin_set_tuple!(A, B, C);
impl_plugin_set_tuple!(A, B, C, D);

#[cfg(feature = "z3")]
pub trait Z3PluginDispatch {
    fn try_z3_int_call<'ctx>(
        &self,
        call: &Z3Call<'_, '_>,
        env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Option<Result<z3::ast::Int<'ctx>, NexusDiagnostic>>;

    fn try_z3_bool_call<'ctx>(
        &self,
        call: &Z3Call<'_, '_>,
        env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Option<Result<z3::ast::Bool<'ctx>, NexusDiagnostic>>;
}

#[cfg(feature = "z3")]
impl Z3PluginDispatch for () {
    fn try_z3_int_call<'ctx>(
        &self,
        _call: &Z3Call<'_, '_>,
        _env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Option<Result<z3::ast::Int<'ctx>, NexusDiagnostic>> {
        None
    }

    fn try_z3_bool_call<'ctx>(
        &self,
        _call: &Z3Call<'_, '_>,
        _env: &mut dyn Z3CallEnv<'ctx>,
    ) -> Option<Result<z3::ast::Bool<'ctx>, NexusDiagnostic>> {
        None
    }
}

#[cfg(feature = "z3")]
macro_rules! impl_z3_dispatch_tuple {
    ($($name:ident),+ $(,)?) => {
        impl<$($name: AuraPlugin),+> Z3PluginDispatch for ($($name,)+) {
            fn try_z3_int_call<'ctx>(
                &self,
                call: &Z3Call<'_, '_>,
                env: &mut dyn Z3CallEnv<'ctx>,
            ) -> Option<Result<z3::ast::Int<'ctx>, NexusDiagnostic>> {
                let mut out: Option<Result<z3::ast::Int<'ctx>, NexusDiagnostic>> = None;
                PluginSet::for_each_plugin(self, |p| {
                    if out.is_some() {
                        return;
                    }
                    out = p.on_z3_int_call(call, env);
                });
                out
            }

            fn try_z3_bool_call<'ctx>(
                &self,
                call: &Z3Call<'_, '_>,
                env: &mut dyn Z3CallEnv<'ctx>,
            ) -> Option<Result<z3::ast::Bool<'ctx>, NexusDiagnostic>> {
                let mut out: Option<Result<z3::ast::Bool<'ctx>, NexusDiagnostic>> = None;
                PluginSet::for_each_plugin(self, |p| {
                    if out.is_some() {
                        return;
                    }
                    out = p.on_z3_bool_call(call, env);
                });
                out
            }
        }
    };
}

#[cfg(feature = "z3")]
impl_z3_dispatch_tuple!(A);
#[cfg(feature = "z3")]
impl_z3_dispatch_tuple!(A, B);
#[cfg(feature = "z3")]
impl_z3_dispatch_tuple!(A, B, C);
#[cfg(feature = "z3")]
impl_z3_dispatch_tuple!(A, B, C, D);

#[cfg(feature = "z3")]
#[derive(Clone, Debug)]
pub struct Z3Call<'name, 'expr> {
    pub name: &'name str,
    pub args: &'expr [&'expr Expr],
    pub span: Span,
}

#[cfg(feature = "z3")]
pub trait Z3CallEnv<'ctx> {
    fn ctx(&self) -> &'ctx z3::Context;

    fn nexus(&mut self) -> &mut NexusContext;

    fn fresh_int(&mut self, prefix: &str) -> z3::ast::Int<'ctx>;

    fn push_constraint(&mut self, c: z3::ast::Bool<'ctx>);

    fn prove_implied(
        &mut self,
        not_condition: z3::ast::Bool<'ctx>,
        span: Span,
        message: &str,
    ) -> Result<(), NexusDiagnostic>;

    fn eval_int(&mut self, expr: &Expr) -> Result<z3::ast::Int<'ctx>, NexusDiagnostic>;

    fn eval_bool(&mut self, expr: &Expr) -> Result<z3::ast::Bool<'ctx>, NexusDiagnostic>;
}

pub fn format_ui_tree(root: &UiNode) -> String {
    let mut out = String::new();
    format_ui_tree_into(&mut out, root, 0);
    out
}

fn format_ui_tree_into(out: &mut String, node: &UiNode, indent: usize) {
    out.push_str(&"  ".repeat(indent));
    out.push_str(&node.kind);

    if !node.props.is_empty() {
        out.push('(');
        for (i, (k, v)) in node.props.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(k);
            out.push_str(": ");
            out.push_str(v);
        }
        out.push(')');
    }
    out.push('\n');

    for c in &node.children {
        format_ui_tree_into(out, c, indent + 1);
    }
}
