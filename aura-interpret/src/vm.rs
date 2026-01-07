use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use aura_ast::{BinOp, CallArg, Expr, ExprKind, MatchStmt, Pattern, Program, Span, Stmt, UnaryOp};
use aura_nexus::{take_ui_feedback, NexusContext, UiNode, UiPluginDispatch};

use crate::debug::{
    DebugEvent, DebugSession, DebugWatchValue, PerfReport, PerfTimelineEvent, ParsedExpr,
};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("avm: terminated")]
#[diagnostic(code(aura::avm::terminated))]
pub struct AvmTerminated;
#[derive(Clone, Debug, PartialEq)]
pub enum AvmValue {
    Int(i64),
    Bool(bool),
    Str(String),
    Style(BTreeMap<String, AvmValue>),
    Ui(UiNode),
    Unit,
}

fn stmt_kind_name(stmt: &Stmt) -> &'static str {
    match stmt {
        Stmt::Import(_) => "Import",
        Stmt::MacroDef(_) => "MacroDef",
        Stmt::TypeAlias(_) => "TypeAlias",
        Stmt::TraitDef(_) => "TraitDef",
        Stmt::RecordDef(_) => "RecordDef",
        Stmt::EnumDef(_) => "EnumDef",
        Stmt::StrandDef(_) => "StrandDef",
        Stmt::CellDef(_) => "CellDef",
        Stmt::ExternCell(_) => "ExternCell",
        Stmt::UnsafeBlock(_) => "UnsafeBlock",
        Stmt::Layout(_) => "Layout",
        Stmt::Render(_) => "Render",
        Stmt::Prop(_) => "Prop",
        Stmt::Assign(_) => "Assign",
        Stmt::If(_) => "If",
        Stmt::Match(_) => "Match",
        Stmt::While(_) => "While",
        Stmt::Requires(_) => "Requires",
        Stmt::Ensures(_) => "Ensures",
        Stmt::Assert(_) => "Assert",
        Stmt::Assume(_) => "Assume",
        Stmt::MacroCall(_) => "MacroCall",
        Stmt::FlowBlock(_) => "FlowBlock",
        Stmt::ExprStmt(_) => "ExprStmt",
    }
}

fn estimate_value_bytes(v: &AvmValue) -> u64 {
    match v {
        AvmValue::Int(_) => 8,
        AvmValue::Bool(_) => 1,
        AvmValue::Str(s) => s.len() as u64,
        AvmValue::Style(m) => m
            .iter()
            .map(|(k, vv)| k.len() as u64 + estimate_value_bytes(vv))
            .sum(),
        AvmValue::Ui(_) => 0,
        AvmValue::Unit => 0,
    }
}

#[derive(Default, Clone, Debug)]
struct LiveMemStats {
    values_total: u64,
    ints: u64,
    bools: u64,
    strs: u64,
    styles: u64,
    style_entries: u64,
    env_keys_bytes: u64,
    string_bytes: u64,
}

fn collect_live_mem(stats: &mut LiveMemStats, v: &AvmValue) {
    stats.values_total += 1;
    match v {
        AvmValue::Int(_) => stats.ints += 1,
        AvmValue::Bool(_) => stats.bools += 1,
        AvmValue::Str(s) => {
            stats.strs += 1;
            stats.string_bytes += s.len() as u64;
        }
        AvmValue::Style(m) => {
            stats.styles += 1;
            stats.style_entries += m.len() as u64;
            for (k, vv) in m {
                stats.string_bytes += k.len() as u64;
                collect_live_mem(stats, vv);
            }
        }
        AvmValue::Ui(_) => {}
        AvmValue::Unit => {}
    }
}

#[derive(Clone, Debug)]
pub struct ExecOutcome {
    pub verified: bool,
    pub gate_error: Option<String>,
    pub gate_span: Option<Span>,
    pub value: AvmValue,
    pub stdout: String,
}

#[derive(Clone, Debug)]
pub struct AvmConfig {
    pub enable_z3_gate: bool,
    pub hot_threshold: u64,
    pub smt_profile: aura_verify::SmtProfile,
    pub debug: Option<DebugSession>,
}

impl Default for AvmConfig {
    fn default() -> Self {
        Self {
            enable_z3_gate: true,
            hot_threshold: 50,
            smt_profile: aura_verify::SmtProfile::Ci,
            debug: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AvmEvent {
    Promoted { cell_id: u64 },
}

#[derive(Debug)]
pub struct Avm {
    cfg: AvmConfig,
    // State
    env: HashMap<String, AvmValue>,

    // Frame-local callback registry used by UI runtimes.
    next_callback_id: u64,
    callbacks: HashMap<u64, aura_ast::Block>,

    // Incremental verification cache keyed by a stable hash of the accumulated source.
    verify_cache: HashMap<u64, bool>,

    // Hotness tracking per "cell" (top-level statement index) for hybrid hand-off.
    hot: HashMap<u64, u64>,

    // Captured stdout from externs like io.println
    stdout: String,

    debug: Option<DebugSession>,
}

impl Avm {
    pub fn new(cfg: AvmConfig) -> Self {
        let debug = cfg.debug.clone();
        Self {
            cfg,
            env: HashMap::new(),
            next_callback_id: 1,
            callbacks: HashMap::new(),
            verify_cache: HashMap::new(),
            hot: HashMap::new(),
            stdout: String::new(),
            debug,
        }
    }

    fn stmt_span(stmt: &Stmt) -> Span {
        match stmt {
            Stmt::Import(x) => x.span,
            Stmt::MacroDef(x) => x.span,
            Stmt::TypeAlias(x) => x.span,
            Stmt::TraitDef(x) => x.span,
            Stmt::RecordDef(x) => x.span,
            Stmt::EnumDef(x) => x.span,
            Stmt::StrandDef(x) => x.span,
            Stmt::CellDef(x) => x.span,
            Stmt::ExternCell(x) => x.span,
            Stmt::UnsafeBlock(x) => x.span,
            Stmt::Layout(x) => x.span,
            Stmt::Render(x) => x.span,
            Stmt::Prop(x) => x.span,
            Stmt::Assign(x) => x.span,
            Stmt::If(x) => x.span,
            Stmt::Match(x) => x.span,
            Stmt::While(x) => x.span,
            Stmt::Requires(x) => x.span,
            Stmt::Ensures(x) => x.span,
            Stmt::Assert(x) => x.span,
            Stmt::Assume(x) => x.span,
            Stmt::MacroCall(x) => x.span,
            Stmt::FlowBlock(x) => x.span,
            Stmt::ExprStmt(e) => e.span,
        }
    }

    fn debug_stop_if_needed(&mut self, span: Span) -> miette::Result<()> {
        let Some(sess) = self.debug.clone() else {
            return Ok(());
        };

        // Terminate is handled as a cooperative cancel: stop at statement boundaries.
        if sess.take_terminate_requested() {
            sess.emit(DebugEvent::Terminated {
                target: "devvm".to_string(),
            });
            return Err(miette::Report::new(AvmTerminated));
        }

        let stop_reason = sess.should_stop_before_stmt(span, |expr| {
            let v = self.eval_expr_pure(expr).map_err(|e| e.to_string())?;
            match v {
                AvmValue::Bool(b) => Ok(b),
                AvmValue::Int(i) => Ok(i != 0),
                AvmValue::Unit => Ok(false),
                other => Err(format!("expected bool, got {other:?}")),
            }
        });

        let Some(reason) = stop_reason else {
            return Ok(());
        };

        let (line, col, file) = sess.line_col(span);
        let mut env: HashMap<String, String> = HashMap::new();
        for (k, v) in &self.env {
            env.insert(k.clone(), format!("{v:?}"));
        }

        let watches = sess
            .watches()
            .into_iter()
            .map(|w| match &w.expr {
                ParsedExpr::Ok(e) => match self.eval_expr_pure(e) {
                    Ok(v) => DebugWatchValue {
                        expr: w.src,
                        value: Some(format!("{v:?}")),
                        error: None,
                    },
                    Err(err) => DebugWatchValue {
                        expr: w.src,
                        value: None,
                        error: Some(err.to_string()),
                    },
                },
                ParsedExpr::Err(e) => DebugWatchValue {
                    expr: w.src,
                    value: None,
                    error: Some(e.clone()),
                },
            })
            .collect::<Vec<_>>();

        sess.emit(DebugEvent::Stopped {
            reason,
            file,
            line,
            col,
            env,
            watches,
        });

        sess.wait_while_paused();

        // If a terminate came in while paused, unwind now.
        if sess.take_terminate_requested() {
            sess.emit(DebugEvent::Terminated {
                target: "devvm".to_string(),
            });
            return Err(miette::Report::new(AvmTerminated));
        }
        Ok(())
    }

    fn eval_expr_pure(&mut self, expr: &Expr) -> miette::Result<AvmValue> {
        // "Pure" evaluation for debugger watches/conditions: forbid calls, lambdas, and UI.
        match &expr.kind {
            ExprKind::Call { .. } => Err(miette::miette!(
                "AVM: function calls are not allowed in watch/condition expressions"
            )),
            ExprKind::Lambda { .. } => Err(miette::miette!(
                "AVM: lambdas are not allowed in watch/condition expressions"
            )),
            ExprKind::Flow { .. } => Err(miette::miette!(
                "AVM: flow expressions are not allowed in watch/condition expressions"
            )),
            _ => self.eval_expr(expr),
        }
    }

    fn reset_frame_callbacks(&mut self) {
        self.next_callback_id = 1;
        self.callbacks.clear();
    }

    fn interpolate_string(&self, s: &str) -> String {
        // Minimal interpolation: replaces `{ident}` with the current value from `env`.
        // Unknown identifiers are left unchanged.
        use std::fmt::Write;

        let mut out = String::with_capacity(s.len());
        let mut rest = s;

        while let Some(open) = rest.find('{') {
            // Copy the chunk before `{`.
            out.push_str(&rest[..open]);

            // Try to parse `{ident}`.
            let after_open = &rest[open + 1..];
            if let Some(close_rel) = after_open.find('}') {
                let key = &after_open[..close_rel];
                if let Some(v) = self.env.get(key) {
                    match v {
                        AvmValue::Int(n) => {
                            let _ = write!(&mut out, "{n}");
                        }
                        AvmValue::Bool(b) => {
                            let _ = write!(&mut out, "{b}");
                        }
                        AvmValue::Str(st) => out.push_str(st),
                        AvmValue::Style(_) => out.push_str("<style>"),
                        AvmValue::Ui(_) => out.push_str("<ui>"),
                        AvmValue::Unit => out.push_str("()"),
                    }

                    // Advance past the closing `}`.
                    rest = &after_open[close_rel + 1..];
                    continue;
                }

                // Unknown key: preserve the original placeholder.
                out.push('{');
                out.push_str(key);
                out.push('}');
                rest = &after_open[close_rel + 1..];
                continue;
            }

            // No closing `}`: preserve the '{' and continue.
            out.push('{');
            rest = after_open;
        }

        out.push_str(rest);
        out
    }

    fn register_callback(&mut self, body: &aura_ast::Block) -> u64 {
        let id = self.next_callback_id;
        self.next_callback_id = self.next_callback_id.saturating_add(1);
        self.callbacks.insert(id, body.clone());
        id
    }

    pub fn env_snapshot(&self) -> HashMap<String, AvmValue> {
        self.env.clone()
    }

    pub fn exec_source(&mut self, source: &str) -> miette::Result<ExecOutcome> {
        let mut nexus = NexusContext::default();
        self.exec_source_with_ui_plugins(source, &(), &mut nexus)
    }

    pub fn exec_source_with_ui_plugins(
        &mut self,
        source: &str,
        ui_plugins: &dyn UiPluginDispatch,
        nexus: &mut NexusContext,
    ) -> miette::Result<ExecOutcome> {
        self.stdout.clear();

        // Allow REPL-friendly plugin imports like `import aura-ai` even though `-` isn't a valid
        // identifier in the lexer. We normalize these imports to `aura_ai/aura_iot`.
        let normalized = normalize_source_for_plugin_imports(source);

        if let Some(sess) = &self.debug {
            sess.set_source("<repl>".to_string(), &normalized);
        }

        let key = stable_hash(&normalized);
        let (verified, gate_error, gate_span) = if !self.cfg.enable_z3_gate {
            (true, None, None)
        } else if let Some(v) = self.verify_cache.get(&key).copied() {
            (v, None, None)
        } else {
            let (ok, err, span) = verify_source_z3(&normalized, self.cfg.smt_profile)?;
            self.verify_cache.insert(key, ok);
            (ok, err, span)
        };

        if !verified {
            return Ok(ExecOutcome {
                verified,
                gate_error,
                gate_span,
                value: AvmValue::Unit,
                stdout: self.stdout.clone(),
            });
        }

        let program = aura_parse::parse_source(&normalized)?;

        // Execute program statements in order.
        let mut last = AvmValue::Unit;
        for (idx, stmt) in program.stmts.iter().enumerate() {
            last = self.exec_stmt(stmt, ui_plugins, nexus)?;

            let cell_id = idx as u64;
            let c = self.hot.entry(cell_id).or_insert(0);
            *c += 1;
        }

        self.debug_finish_run();

        Ok(ExecOutcome {
            verified,
            gate_error: None,
            gate_span: None,
            value: last,
            stdout: self.stdout.clone(),
        })
    }

    pub fn exec_entry_cell(&mut self, source: &str, entry: &str) -> miette::Result<ExecOutcome> {
        let mut nexus = NexusContext::default();
        self.exec_entry_cell_with_ui_plugins(source, entry, &(), &mut nexus)
    }

    pub fn exec_entry_cell_with_ui_plugins(
        &mut self,
        source: &str,
        entry: &str,
        ui_plugins: &dyn UiPluginDispatch,
        nexus: &mut NexusContext,
    ) -> miette::Result<ExecOutcome> {
        self.stdout.clear();

        // Keep behavior consistent with `exec_source`.
        let normalized = normalize_source_for_plugin_imports(source);

        if let Some(sess) = &self.debug {
            sess.set_source("<entry>".to_string(), &normalized);
        }

        let key = stable_hash(&normalized);
        let (verified, gate_error, gate_span) = if !self.cfg.enable_z3_gate {
            (true, None, None)
        } else if let Some(v) = self.verify_cache.get(&key).copied() {
            (v, None, None)
        } else {
            let (ok, err, span) = verify_source_z3(&normalized, self.cfg.smt_profile)?;
            self.verify_cache.insert(key, ok);
            (ok, err, span)
        };

        if !verified {
            return Ok(ExecOutcome {
                verified,
                gate_error,
                gate_span,
                value: AvmValue::Unit,
                stdout: self.stdout.clone(),
            });
        }

        let program = aura_parse::parse_source(&normalized)?;

        // Allow top-level initialization statements.
        let mut last = AvmValue::Unit;
        for stmt in &program.stmts {
            match stmt {
                Stmt::CellDef(_)
                | Stmt::ExternCell(_)
                | Stmt::FlowBlock(_)
                | Stmt::Import(_)
                | Stmt::TypeAlias(_) => {}
                other => {
                    last = self.exec_stmt(other, ui_plugins, nexus)?;
                }
            }
        }

        // Execute the entry cell body.
        let entry_cell = program.stmts.iter().find_map(|s| match s {
            Stmt::CellDef(c) if c.name.node == entry => Some(c),
            _ => None,
        });

        if let Some(cell) = entry_cell {
            if !cell.params.is_empty() {
                return Err(miette::miette!(
                    "AVM: entry cell '{}' must have no parameters",
                    entry
                ));
            }
            last = self.exec_block(&cell.body, ui_plugins, nexus)?;
        }

        self.debug_finish_run();

        Ok(ExecOutcome {
            verified,
            gate_error: None,
            gate_span: None,
            value: last,
            stdout: self.stdout.clone(),
        })
    }

    fn debug_finish_run(&mut self) {
        let Some(sess) = &self.debug else {
            return;
        };
        if !sess.perf_enabled() {
            return;
        }

        let timeline = sess.perf_take();
        if timeline.is_empty() {
            return;
        }

        let mut totals_by_site: HashMap<String, u64> = HashMap::new();
        let mut total_ns: u64 = 0;
        for ev in &timeline {
            let key = format!("{}@{}:{}", ev.kind, ev.line, ev.col);
            *totals_by_site.entry(key).or_insert(0) += ev.dur_ns;
            total_ns = total_ns.saturating_add(ev.dur_ns);
        }

        let mut flame_lines: Vec<String> = totals_by_site
            .into_iter()
            .map(|(k, v)| format!("main;{k} {v}"))
            .collect();
        flame_lines.sort();
        let flame_folded = flame_lines.join("\n");

        let mut mem = LiveMemStats::default();
        for (k, v) in &self.env {
            mem.env_keys_bytes = mem.env_keys_bytes.saturating_add(k.len() as u64);
            collect_live_mem(&mut mem, v);
        }

        let mut memory: HashMap<String, u64> = HashMap::new();
        memory.insert("env_bindings".to_string(), self.env.len() as u64);
        memory.insert("env_keys_bytes".to_string(), mem.env_keys_bytes);
        memory.insert("values_total".to_string(), mem.values_total);
        memory.insert("values_int".to_string(), mem.ints);
        memory.insert("values_bool".to_string(), mem.bools);
        memory.insert("values_str".to_string(), mem.strs);
        memory.insert("values_style".to_string(), mem.styles);
        memory.insert("style_entries".to_string(), mem.style_entries);
        memory.insert("string_bytes".to_string(), mem.string_bytes);
        memory.insert("stdout_bytes".to_string(), self.stdout.len() as u64);
        memory.insert("callbacks".to_string(), self.callbacks.len() as u64);
        memory.insert("verify_cache_entries".to_string(), self.verify_cache.len() as u64);
        memory.insert("perf_total_ns".to_string(), total_ns);

        sess.emit(DebugEvent::PerfReport {
            report: PerfReport {
                timeline,
                flame_folded,
                memory,
            },
        });
    }

    fn exec_stmt(
        &mut self,
        stmt: &Stmt,
        ui_plugins: &dyn UiPluginDispatch,
        nexus: &mut NexusContext,
    ) -> miette::Result<AvmValue> {
        let span = Self::stmt_span(stmt);
        self.debug_stop_if_needed(span)?;

        let t0 = Instant::now();
        let out = match stmt {
            // Type-level statements are ignored by the VM.
            Stmt::TraitDef(_) | Stmt::RecordDef(_) | Stmt::EnumDef(_) => Ok(AvmValue::Unit),

            // Macro nodes should not survive parsing (they are expanded there), but ignore defensively.
            Stmt::MacroDef(_) | Stmt::MacroCall(_) => Ok(AvmValue::Unit),

            Stmt::UnsafeBlock(s) => self.exec_block(&s.body, ui_plugins, nexus),

            Stmt::StrandDef(sd) => {
                let v = self.eval_expr(&sd.expr)?;
                self.env.insert(sd.name.node.clone(), v.clone());
                Ok(AvmValue::Unit)
            }
            Stmt::Layout(lb) => {
                // Layout blocks can yield a UI tree (e.g. `VStack { ... }`).
                // If a UI plugin is active, tick a live loop so the window stays open.
                self.reset_frame_callbacks();
                let v = self.exec_block(&lb.body, ui_plugins, nexus)?;
                let AvmValue::Ui(_) = v else {
                    return Ok(AvmValue::Unit);
                };

                // If a plugin handles UI, keep rendering until it requests close.
                let debug_ui = std::env::var("AURA_UI_DEBUG").is_ok();
                let mut frames: u32 = 0;
                loop {
                    self.reset_frame_callbacks();
                    let v = self.exec_block(&lb.body, ui_plugins, nexus)?;
                    let AvmValue::Ui(node) = v else {
                        if debug_ui {
                            eprintln!("AURA_UI_DEBUG: layout loop ended (non-UI value)");
                        }
                        break;
                    };

                    let Some(res) = ui_plugins.try_ui_render(&node, nexus) else {
                        if debug_ui {
                            eprintln!("AURA_UI_DEBUG: layout loop ended (no UI plugin)");
                        }
                        break;
                    };
                    res.map_err(|d| {
                        miette::miette!("{}", d.message).with_source_code(source_note(d.span))
                    })?;

                    let fb = take_ui_feedback(nexus);
                    if debug_ui && frames < 5 {
                        eprintln!(
                            "AURA_UI_DEBUG: frame={} close={} clicked={:?}",
                            frames, fb.close_requested, fb.clicked_callback_id
                        );
                    }
                    frames = frames.saturating_add(1);
                    if fb.close_requested {
                        if debug_ui {
                            eprintln!("AURA_UI_DEBUG: layout loop ended (close requested)");
                        }
                        break;
                    }

                    if let Some(cb) = fb.clicked_callback_id {
                        if let Some(body) = self.callbacks.get(&cb).cloned() {
                            let _ = self.exec_block(&body, ui_plugins, nexus)?;
                        }
                    }
                }
                Ok(AvmValue::Unit)
            }
            Stmt::Render(rb) => {
                // Live UI loop: rebuild UI, render a frame, handle click callbacks, repeat.
                // If no UI plugin is active, execute once and return.
                self.reset_frame_callbacks();
                let first = self.exec_block(&rb.body, ui_plugins, nexus)?;
                let AvmValue::Ui(_) = first else {
                    return Ok(AvmValue::Unit);
                };

                let debug_ui = std::env::var("AURA_UI_DEBUG").is_ok();
                let mut frames: u32 = 0;
                loop {
                    self.reset_frame_callbacks();
                    let v = self.exec_block(&rb.body, ui_plugins, nexus)?;
                    let AvmValue::Ui(node) = v else {
                        if debug_ui {
                            eprintln!("AURA_UI_DEBUG: render loop ended (non-UI value)");
                        }
                        break;
                    };

                    let Some(res) = ui_plugins.try_ui_render(&node, nexus) else {
                        if debug_ui {
                            eprintln!("AURA_UI_DEBUG: render loop ended (no UI plugin)");
                        }
                        break;
                    };
                    res.map_err(|d| {
                        miette::miette!("{}", d.message).with_source_code(source_note(d.span))
                    })?;

                    let fb = take_ui_feedback(nexus);
                    if debug_ui && frames < 5 {
                        eprintln!(
                            "AURA_UI_DEBUG: frame={} close={} clicked={:?}",
                            frames, fb.close_requested, fb.clicked_callback_id
                        );
                    }
                    frames = frames.saturating_add(1);
                    if fb.close_requested {
                        if debug_ui {
                            eprintln!("AURA_UI_DEBUG: render loop ended (close requested)");
                        }
                        break;
                    }

                    if let Some(cb) = fb.clicked_callback_id {
                        if let Some(body) = self.callbacks.get(&cb).cloned() {
                            let _ = self.exec_block(&body, ui_plugins, nexus)?;
                        }
                    }
                }
                Ok(AvmValue::Unit)
            }
            Stmt::Assign(a) => {
                let v = self.eval_expr(&a.expr)?;
                self.env.insert(a.target.node.clone(), v);
                Ok(AvmValue::Unit)
            }
            Stmt::Prop(p) => {
                let _ = self.eval_expr(&p.expr)?;
                Ok(AvmValue::Unit)
            }
            Stmt::Requires(r) => {
                let _ = self.eval_expr(&r.expr)?;
                Ok(AvmValue::Unit)
            }
            Stmt::Ensures(e) => {
                let _ = self.eval_expr(&e.expr)?;
                Ok(AvmValue::Unit)
            }
            Stmt::Assume(a) => {
                let _ = self.eval_expr(&a.expr)?;
                Ok(AvmValue::Unit)
            }
            Stmt::Assert(a) => {
                let v = self.eval_expr(&a.expr)?;
                if matches!(v, AvmValue::Bool(true)) {
                    Ok(AvmValue::Unit)
                } else {
                    Err(miette::miette!("assertion failed")
                        .with_source_code(source_note(stmt_span(stmt))))
                }
            }
            Stmt::ExprStmt(e) => self.eval_expr(e),
            // DX-focused REPL: ignore declarations not used in scripts.
            Stmt::Import(_)
            | Stmt::TypeAlias(_)
            | Stmt::CellDef(_)
            | Stmt::ExternCell(_)
            | Stmt::FlowBlock(_) => {
                Ok(AvmValue::Unit)
            }
            Stmt::If(i) => {
                let c = self.eval_expr(&i.cond)?;
                let take_then = matches!(c, AvmValue::Bool(true));
                if take_then {
                    self.exec_block(&i.then_block, ui_plugins, nexus)
                } else if let Some(b) = &i.else_block {
                    self.exec_block(b, ui_plugins, nexus)
                } else {
                    Ok(AvmValue::Unit)
                }
            }
            Stmt::Match(m) => self.exec_match(m, ui_plugins, nexus),
            Stmt::While(w) => {
                // No invariant enforcement here; Z3 gate is expected to prove invariants where present.
                let mut guard = 0u64;
                loop {
                    guard += 1;
                    if guard > 5_000_000 {
                        return Err(
                            miette::miette!("AVM: loop guard tripped (possible infinite loop)")
                                .with_source_code(source_note(stmt_span(stmt))),
                        );
                    }

                    let c = self.eval_expr(&w.cond)?;
                    if !matches!(c, AvmValue::Bool(true)) {
                        break;
                    }
                    let _ = self.exec_block(&w.body, ui_plugins, nexus)?;
                }
                Ok(AvmValue::Unit)
            }
            // All statement variants are handled above.
        };

        let dur = t0.elapsed();
        if let Some(sess) = &self.debug {
            if sess.perf_enabled() {
                let (line, col, _file) = sess.line_col(span);
                sess.perf_push(PerfTimelineEvent {
                    line,
                    col,
                    kind: stmt_kind_name(stmt).to_string(),
                    dur_ns: dur.as_nanos() as u64,
                });
            }
            sess.note_stmt_executed();
        }

        out
    }

    fn exec_match(
        &mut self,
        m: &MatchStmt,
        ui_plugins: &dyn UiPluginDispatch,
        nexus: &mut NexusContext,
    ) -> miette::Result<AvmValue> {
        let v = self.eval_expr(&m.scrutinee)?;
        for arm in &m.arms {
            if pat_matches_value(&arm.pat, &v) {
                let _ = self.exec_block(&arm.body, ui_plugins, nexus)?;
                break;
            }
        }
        Ok(AvmValue::Unit)
    }

    fn exec_block(
        &mut self,
        b: &aura_ast::Block,
        ui_plugins: &dyn UiPluginDispatch,
        nexus: &mut NexusContext,
    ) -> miette::Result<AvmValue> {
        let mut last = AvmValue::Unit;
        for s in &b.stmts {
            last = self.exec_stmt(s, ui_plugins, nexus)?;
        }
        if let Some(e) = &b.yield_expr {
            last = self.eval_expr(e)?;
        }
        Ok(last)
    }

    fn eval_expr(&mut self, expr: &Expr) -> miette::Result<AvmValue> {
        match &expr.kind {
            ExprKind::IntLit(n) => Ok(AvmValue::Int(*n as i64)),
            ExprKind::StringLit(s) => Ok(AvmValue::Str(self.interpolate_string(s))),
            ExprKind::StyleLit { fields } => {
                let mut map: BTreeMap<String, AvmValue> = BTreeMap::new();
                for (k, v) in fields {
                    let vv = self.eval_expr(v)?;
                    map.insert(k.node.clone(), vv);
                }
                Ok(AvmValue::Style(map))
            }
            ExprKind::RecordLit { fields, .. } => {
                let mut map: BTreeMap<String, AvmValue> = BTreeMap::new();
                for (k, v) in fields {
                    let vv = self.eval_expr(v)?;
                    map.insert(k.node.clone(), vv);
                }
                // MVP runtime representation: treat records like maps.
                Ok(AvmValue::Style(map))
            }
            ExprKind::Ident(id) => self
                .env
                .get(&id.node)
                .cloned()
                .ok_or_else(|| miette::miette!("AVM: unknown identifier '{}'", id.node)),
            ExprKind::Unary { op, expr } => {
                let v = self.eval_expr(expr)?;
                match (op, v) {
                    (UnaryOp::Neg, AvmValue::Int(i)) => Ok(AvmValue::Int(-i)),
                    (UnaryOp::Not, AvmValue::Bool(b)) => Ok(AvmValue::Bool(!b)),
                    _ => Err(miette::miette!("AVM: unsupported unary op")),
                }
            }
            ExprKind::Binary { left, op, right } => {
                let l = self.eval_expr(left)?;
                let r = self.eval_expr(right)?;
                self.eval_binop(*op, l, r)
            }
            ExprKind::ForAll { .. } | ExprKind::Exists { .. } => Err(miette::miette!(
                "AVM: quantifiers are verification-only and cannot be evaluated"
            )),
            ExprKind::Member { base, member } => {
                // Minimal: allow `io.println` by treating `io` as a namespace.
                let b = self.eval_expr(base)?;
                match b {
                    AvmValue::Str(ns) => Ok(AvmValue::Str(format!("{ns}.{}", member.node))),
                    AvmValue::Unit => Ok(AvmValue::Str(member.node.clone())),
                    AvmValue::Int(_) | AvmValue::Bool(_) => Err(miette::miette!("AVM: member access unsupported")),
                    AvmValue::Style(map) => map
                        .get(&member.node)
                        .cloned()
                        .ok_or_else(|| miette::miette!("AVM: unknown field '{}'", member.node)),
                    AvmValue::Ui(_) => Err(miette::miette!("AVM: member access unsupported")),
                }
            }
            ExprKind::Call { callee, args, trailing } => {
                // Support extern calls by name.
                // Special-case `Ident` callee so UI constructors like `Button(...)` don't require
                // pre-binding a namespace value.
                let name = match &callee.kind {
                    ExprKind::Ident(id) => id.node.clone(),
                    _ => {
                        let callee_v = self.eval_expr(callee)?;
                        match callee_v {
                            AvmValue::Str(s) => s,
                            AvmValue::Unit => {
                                if let ExprKind::Ident(id) = &callee.kind {
                                    id.node.clone()
                                } else {
                                    return Err(miette::miette!("AVM: invalid callee"));
                                }
                            }
                            _ => return Err(miette::miette!("AVM: invalid callee")),
                        }
                    }
                };

                let call_result = if name == "io.println" || name == "log" {
                    if args.len() != 1 {
                        return Err(miette::miette!("AVM: {} expects 1 argument", name));
                    }
                    let v = self.eval_expr(call_arg_value(&args[0]))?;
                    match v {
                        AvmValue::Str(s) => {
                            self.stdout.push_str(&s);
                            self.stdout.push('\n');
                            Ok(AvmValue::Unit)
                        }
                        _ => Err(miette::miette!(
                            "AVM: io.println only supports string literals in AVM"
                        )),
                    }
                } else if is_ui_call(&name, trailing.is_some()) {
                    let mut node = UiNode::new(name);

                    // Call-site args become props.
                    for (idx, a) in args.iter().enumerate() {
                        match a {
                            CallArg::Positional(e) => {
                                let v = self.eval_expr(e)?;
                                node.set_prop(format!("_${idx}"), avm_value_to_prop_string(&v));
                            }
                            CallArg::Named { name, value } => {
                                if matches!(value.kind, ExprKind::Lambda { .. }) {
                                    let ExprKind::Lambda { body, .. } = &value.kind else {
                                        unreachable!("matched lambda")
                                    };
                                    let id = self.register_callback(body);
                                    node.set_prop(name.node.clone(), format!("cb:{id}"));
                                    continue;
                                }

                                let v = self.eval_expr(value)?;
                                if name.node == "style" {
                                    if let AvmValue::Style(map) = v {
                                        for (k, vv) in map {
                                            node.set_prop(k, avm_value_to_prop_string(&vv));
                                        }
                                        // Do not set a synthetic `style` prop; we expand.
                                        continue;
                                    }
                                }

                                node.set_prop(name.node.clone(), avm_value_to_prop_string(&v));
                            }
                        }
                    }

                    if let Some(tb) = trailing {
                        self.eval_ui_trailing_block(&mut node, tb)?;
                    }

                    Ok(AvmValue::Ui(node))
                } else {
                    Err(miette::miette!("AVM: unknown call '{name}'"))
                }?;

                Ok(call_result)
            }
            ExprKind::Lambda { .. } => Ok(AvmValue::Unit),
            ExprKind::Flow { left, op: _op, right } => {
                // Minimal: execute `left`, then `right`.
                let _ = self.eval_expr(left)?;
                self.eval_expr(right)
            }
        }
    }

    fn eval_ui_trailing_block(&mut self, node: &mut UiNode, b: &aura_ast::Block) -> miette::Result<()> {
        for s in &b.stmts {
            match s {
                Stmt::Prop(p) => {
                    if matches!(p.expr.kind, ExprKind::Lambda { .. }) {
                        let ExprKind::Lambda { body, .. } = &p.expr.kind else {
                            unreachable!("matched lambda")
                        };
                        let id = self.register_callback(body);
                        node.set_prop(p.name.node.clone(), format!("cb:{id}"));
                        continue;
                    }

                    let v = self.eval_expr(&p.expr)?;
                    if p.name.node == "style" {
                        if let AvmValue::Style(map) = v {
                            for (k, vv) in map {
                                node.set_prop(k, avm_value_to_prop_string(&vv));
                            }
                            continue;
                        }
                    }

                    node.set_prop(p.name.node.clone(), avm_value_to_prop_string(&v));
                }
                Stmt::ExprStmt(e) => {
                    let v = self.eval_expr(e)?;
                    if let AvmValue::Ui(child) = v {
                        node.children.push(child);
                    }
                }
                Stmt::Render(r) => {
                    let v = self.exec_block(&r.body, &(), &mut NexusContext::default())?;
                    if let AvmValue::Ui(child) = v {
                        node.children.push(child);
                    }
                }
                // Keep other statements legal, but ignore them for now.
                _ => {
                    // Non-UI statements inside a UI block are currently ignored for rendering.
                    // (Callbacks are stored as <callback> placeholders.)
                }
            }
        }

        if let Some(y) = &b.yield_expr {
            let v = self.eval_expr(y)?;
            if let AvmValue::Ui(child) = v {
                node.children.push(child);
            }
        }

        Ok(())
    }

    fn eval_binop(&self, op: BinOp, l: AvmValue, r: AvmValue) -> miette::Result<AvmValue> {
        match (op, l, r) {
            (BinOp::Add, AvmValue::Int(a), AvmValue::Int(b)) => Ok(AvmValue::Int(a + b)),
            (BinOp::Sub, AvmValue::Int(a), AvmValue::Int(b)) => Ok(AvmValue::Int(a - b)),
            (BinOp::Mul, AvmValue::Int(a), AvmValue::Int(b)) => Ok(AvmValue::Int(a * b)),
            (BinOp::Div, AvmValue::Int(a), AvmValue::Int(b)) => Ok(AvmValue::Int(a / b)),

            (BinOp::Eq, a, b) => Ok(AvmValue::Bool(a == b)),
            (BinOp::Ne, a, b) => Ok(AvmValue::Bool(a != b)),

            (BinOp::Lt, AvmValue::Int(a), AvmValue::Int(b)) => Ok(AvmValue::Bool(a < b)),
            (BinOp::Gt, AvmValue::Int(a), AvmValue::Int(b)) => Ok(AvmValue::Bool(a > b)),
            (BinOp::Le, AvmValue::Int(a), AvmValue::Int(b)) => Ok(AvmValue::Bool(a <= b)),
            (BinOp::Ge, AvmValue::Int(a), AvmValue::Int(b)) => Ok(AvmValue::Bool(a >= b)),

            (BinOp::And, AvmValue::Bool(a), AvmValue::Bool(b)) => Ok(AvmValue::Bool(a && b)),
            (BinOp::Or, AvmValue::Bool(a), AvmValue::Bool(b)) => Ok(AvmValue::Bool(a || b)),

            _ => Err(miette::miette!("AVM: unsupported binary op")),
        }
    }
}

fn verify_source_z3(
    source: &str,
    profile: aura_verify::SmtProfile,
) -> miette::Result<(bool, Option<String>, Option<Span>)> {
    let program: Program = aura_parse::parse_source(source)?;

    let mut checker = aura_core::Checker::new();
    checker.check_program(&program).map_err(|e| miette::Report::new(e))?;

    // Z3 verification: if it returns an error, treat as not verified.
    let mut prover = aura_verify::Z3Prover::new();
    let mut nexus = aura_nexus::NexusContext::default();

    // Dynamic plugin loading for REPL/scripts.
    // Note: UI/runtime plugins (e.g. lumina) should not affect the Z3 gate.
    // Unknown plugins still fail the gate.
    let requested = detect_requested_plugins(&program);
    let known = ["aura-ai", "aura-iot", "aura-lumina"];
    if requested.iter().any(|p| !known.contains(&p.as_str())) {
        return Ok((false, Some("unknown plugin import(s)".to_string()), None));
    }

    // Only Z3-theory plugins participate in the gate.
    let requested: Vec<String> = requested
        .into_iter()
        .filter(|p| p == "aura-ai" || p == "aura-iot")
        .collect();

    let (ok, err, span) = match requested.as_slice() {
        [] => {
            let plugins = (
                aura_plugin_iot::AuraIotPlugin::new(),
                aura_plugin_ai::AuraAiPlugin::new(),
            );
            aura_verify::verify_program_z3_profile(&program, &mut prover, &plugins, &mut nexus, profile)
                .map(|_| (true, None, None))
                .unwrap_or_else(|e| (false, Some(e.message), Some(e.span)))
        }
        [a] if a == "aura-ai" => {
            let plugins = (aura_plugin_ai::AuraAiPlugin::new(),);
            aura_verify::verify_program_z3_profile(&program, &mut prover, &plugins, &mut nexus, profile)
                .map(|_| (true, None, None))
                .unwrap_or_else(|e| (false, Some(e.message), Some(e.span)))
        }
        [a] if a == "aura-iot" => {
            let plugins = (aura_plugin_iot::AuraIotPlugin::new(),);
            aura_verify::verify_program_z3_profile(&program, &mut prover, &plugins, &mut nexus, profile)
                .map(|_| (true, None, None))
                .unwrap_or_else(|e| (false, Some(e.message), Some(e.span)))
        }
        [a, b]
            if (a == "aura-ai" && b == "aura-iot") || (a == "aura-iot" && b == "aura-ai") =>
        {
            let plugins = (
                aura_plugin_iot::AuraIotPlugin::new(),
                aura_plugin_ai::AuraAiPlugin::new(),
            );
            aura_verify::verify_program_z3_profile(&program, &mut prover, &plugins, &mut nexus, profile)
                .map(|_| (true, None, None))
                .unwrap_or_else(|e| (false, Some(e.message), Some(e.span)))
        }
        _ => {
            // Unknown plugin import(s): treat as not verified to preserve the safety gate.
            (false, Some("unknown plugin import(s)".to_string()), None)
        }
    };

    Ok((ok, err, span))
}

fn normalize_source_for_plugin_imports(source: &str) -> String {
    // Minimal text-level normalization just for the REPL/interpreter.
    // We rewrite `import aura-ai` to `import aura_ai` (same for iot), so the lexer can parse it.
    // Keep it conservative: only rewrite the `import ...` token sequence.
    source
        .lines()
        .map(|line| {
            let t = line.trim_start();
            if t.starts_with("import ") {
                let rewritten = t
                    .replace("import aura-ai", "import aura_ai")
                    .replace("import aura-iot", "import aura_iot")
                    .replace("import aura-lumina", "import aura_lumina");
                // Preserve indentation.
                let indent_len = line.len().saturating_sub(t.len());
                format!("{}{}", " ".repeat(indent_len), rewritten)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn detect_requested_plugins(program: &Program) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for stmt in &program.stmts {
        let Stmt::Import(i) = stmt else { continue };
        let last = i.path.last().map(|s| s.node.as_str()).unwrap_or("");

        // Allow:
        // - import aura_ai / aura_iot / aura_lumina
        // - import aura::ai / aura::iot / aura::lumina
        let plugin = match (i.path.as_slice(), last) {
            (_, "aura_ai") => Some("aura-ai"),
            (_, "aura_iot") => Some("aura-iot"),
            (_, "aura_lumina") => Some("aura-lumina"),
            ([a, b], "ai") if a.node == "aura" && b.node == "ai" => Some("aura-ai"),
            ([a, b], "iot") if a.node == "aura" && b.node == "iot" => Some("aura-iot"),
            ([a, b], "lumina") if a.node == "aura" && b.node == "lumina" => Some("aura-lumina"),
            _ => None,
        };

        if let Some(p) = plugin {
            if !out.iter().any(|x| x == p) {
                out.push(p.to_string());
            }
        }
    }
    out.sort();
    out
}

fn stable_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Import(s) => s.span,
        Stmt::MacroDef(s) => s.span,
        Stmt::MacroCall(s) => s.span,
        Stmt::TypeAlias(s) => s.span,
        Stmt::TraitDef(s) => s.span,
        Stmt::RecordDef(s) => s.span,
        Stmt::EnumDef(s) => s.span,
        Stmt::StrandDef(s) => s.span,
        Stmt::CellDef(s) => s.span,
        Stmt::ExternCell(s) => s.span,
        Stmt::UnsafeBlock(s) => s.span,
        Stmt::Assign(s) => s.span,
        Stmt::If(s) => s.span,
        Stmt::Match(s) => s.span,
        Stmt::While(s) => s.span,
        Stmt::Requires(s) => s.span,
        Stmt::Ensures(s) => s.span,
        Stmt::Assert(s) => s.span,
        Stmt::Assume(s) => s.span,
        Stmt::FlowBlock(s) => s.span,
        Stmt::Layout(s) => s.span,
        Stmt::Render(s) => s.span,
        Stmt::Prop(s) => s.span,
        Stmt::ExprStmt(e) => e.span,
    }
}

fn pat_matches_value(p: &Pattern, v: &AvmValue) -> bool {
    match (p, v) {
        (Pattern::Wildcard { .. }, _) => true,
        (Pattern::IntLit { value, .. }, AvmValue::Int(i)) => *i == *value as i64,
        (Pattern::StringLit { value, .. }, AvmValue::Str(s)) => s == value,
        // MVP: the VM does not have a structured enum representation yet.
        (Pattern::Ctor { .. }, _) => false,
        _ => false,
    }
}

fn call_arg_value(arg: &CallArg) -> &Expr {
    match arg {
        CallArg::Positional(e) => e,
        CallArg::Named { value, .. } => value,
    }
}

fn source_note(_span: Span) -> miette::NamedSource<String> {
    // Placeholder: we don't have stable source attachment here yet.
    // Keep errors readable.
    miette::NamedSource::new("avm".to_string(), "".to_string())
}

fn is_ui_call(name: &str, has_trailing: bool) -> bool {
    if has_trailing {
        return true;
    }
    matches!(
        name,
        "App"
            | "Window"
            | "VStack"
            | "HStack"
            | "Text"
            | "Button"
            | "Spacer"
            | "Rect"
    )
}

fn avm_value_to_prop_string(v: &AvmValue) -> String {
    match v {
        AvmValue::Int(i) => i.to_string(),
        AvmValue::Bool(b) => b.to_string(),
        // UI runtimes typically expect raw string payloads (e.g. Color names, labels).
        AvmValue::Str(s) => s.clone(),
        AvmValue::Style(_) => "<style>".to_string(),
        AvmValue::Ui(n) => format!("<{}>", n.kind),
        AvmValue::Unit => "Unit".to_string(),
    }
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
