use std::collections::{BTreeMap, HashMap};
use std::{fs, io};
use std::sync::mpsc;
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

    // Minimal built-in app state for interactive AVM demos/tools.
    shop: ShopState,

    // Minimal UI state for controlled inputs (prototype).
    ui_event_text: String,
    ui_text_state: HashMap<String, String>,

    // Background stdin reader so UI callbacks don't block the render loop.
    stdin_rx: Option<mpsc::Receiver<String>>,

    debug: Option<DebugSession>,
}

#[derive(Clone, Debug, Default)]
struct ShopItem {
    name: String,
    qty: String,
    notes: String,
    purchased: bool,
}

#[derive(Clone, Debug, Default)]
struct ShopState {
    path: Option<String>,
    items: Vec<ShopItem>,

    selected: Option<usize>,

    pending: Option<ShopPending>,
    status: String,
}

#[derive(Clone, Debug)]
enum ShopPending {
    Add {
        stage: u8,
        name: String,
        qty: String,
        notes: String,
    },
    Edit {
        index: usize,
        stage: u8,
        name: String,
        qty: String,
        notes: String,
    },
}

impl ShopState {
    fn default_path() -> String {
        if let Ok(appdata) = std::env::var("APPDATA") {
            format!("{}\\AuraShopList\\shopping_list.tsv", appdata)
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{}/.aura_shop_list.tsv", home)
        }
    }

    fn ensure_path(&mut self) -> String {
        if let Some(p) = self.path.clone() {
            return p;
        }
        let p = Self::default_path();
        self.path = Some(p.clone());
        p
    }

    fn select(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected = Some(index);
        } else {
            self.selected = None;
        }
    }

    fn clear_selection(&mut self) {
        self.selected = None;
    }

    fn has_selection(&self) -> bool {
        self.selected.is_some()
    }

    fn selected_index_u32(&self) -> u32 {
        self.selected.unwrap_or(0) as u32
    }

    fn load_from_path(&mut self, path: &str) -> miette::Result<()> {
        self.path = Some(path.to_string());
        let raw = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.items.clear();
                return Ok(());
            }
            Err(e) => return Err(miette::miette!("failed to read {path}: {e}")),
        };

        let mut items = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            // TSV: name\tqty\tnotes\tpurchased
            let mut parts = line.splitn(4, '\t');
            let name = parts.next().unwrap_or("").to_string();
            if name.trim().is_empty() {
                continue;
            }
            let qty = parts.next().unwrap_or("1").to_string();
            let notes = parts.next().unwrap_or("").to_string();
            let purchased = match parts.next().unwrap_or("0") {
                "1" | "true" | "TRUE" | "yes" | "YES" => true,
                _ => false,
            };
            items.push(ShopItem {
                name,
                qty,
                notes,
                purchased,
            });
        }
        self.items = items;
        Ok(())
    }

    fn save_to_path(&mut self, path: &str) -> miette::Result<()> {
        self.path = Some(path.to_string());
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }
        let mut out = String::new();
        for it in &self.items {
            let purchased = if it.purchased { "1" } else { "0" };
            out.push_str(&it.name.replace('\n', " "));
            out.push('\t');
            out.push_str(&it.qty.replace('\n', " "));
            out.push('\t');
            out.push_str(&it.notes.replace('\n', " "));
            out.push('\t');
            out.push_str(purchased);
            out.push('\n');
        }
        fs::write(path, out).map_err(|e| miette::miette!("failed to write {path}: {e}"))
    }

    fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn status(&self) -> String {
        if self.status.trim().is_empty() {
            "Ready".to_string()
        } else {
            self.status.clone()
        }
    }

    fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    fn cancel_pending(&mut self) {
        self.pending = None;
        self.set_status("Canceled");
    }

    fn begin_add(&mut self) {
        self.pending = Some(ShopPending::Add {
            stage: 0,
            name: String::new(),
            qty: String::new(),
            notes: String::new(),
        });
        self.set_status("Add: type item name in the terminal, press Enter");
    }

    fn begin_edit(&mut self, index: usize) {
        let Some(it) = self.items.get(index).cloned() else {
            self.set_status("Edit: invalid index");
            return;
        };
        self.pending = Some(ShopPending::Edit {
            index,
            stage: 0,
            name: it.name,
            qty: it.qty,
            notes: it.notes,
        });
        self.set_status("Edit: type new name (or blank to keep), press Enter");
    }

    fn consume_input_line(&mut self, line: &str) {
        let line = line.trim_end_matches(['\n', '\r']);
        let Some(p) = &mut self.pending else {
            return;
        };

        match p {
            ShopPending::Add {
                stage,
                name,
                qty,
                notes,
            } => match *stage {
                0 => {
                    if line.trim().is_empty() {
                        self.set_status("Add: name cannot be empty. Type a name, press Enter");
                    } else {
                        *name = line.trim().to_string();
                        *stage = 1;
                        self.set_status("Add: type quantity (Enter for 1), press Enter");
                    }
                }
                1 => {
                    *qty = if line.trim().is_empty() {
                        "1".to_string()
                    } else {
                        line.trim().to_string()
                    };
                    *stage = 2;
                    self.set_status("Add: type notes (Enter to skip), press Enter");
                }
                _ => {
                    *notes = line.trim().to_string();
                    let name_s = name.trim().to_string();
                    let qty_s = if qty.trim().is_empty() {
                        "1".to_string()
                    } else {
                        qty.trim().to_string()
                    };
                    let notes_s = notes.trim().to_string();

                    if !name_s.is_empty() {
                        self.items.push(ShopItem {
                            name: name_s.clone(),
                            qty: qty_s,
                            notes: notes_s,
                            purchased: false,
                        });
                        let path = self.ensure_path();
                        let _ = self.save_to_path(&path);
                        self.set_status(format!("Added: {name_s}"));
                    }

                    self.pending = None;
                }
            },
            ShopPending::Edit {
                index,
                stage,
                name,
                qty,
                notes,
            } => match *stage {
                0 => {
                    if !line.trim().is_empty() {
                        *name = line.trim().to_string();
                    }
                    *stage = 1;
                    self.set_status("Edit: type new quantity (or blank to keep), press Enter");
                }
                1 => {
                    if !line.trim().is_empty() {
                        *qty = line.trim().to_string();
                    }
                    *stage = 2;
                    self.set_status("Edit: type new notes (or blank to keep), press Enter");
                }
                _ => {
                    if !line.trim().is_empty() {
                        *notes = line.trim().to_string();
                    }
                    if let Some(it) = self.items.get_mut(*index) {
                        it.name = name.trim().to_string();
                        it.qty = qty.trim().to_string();
                        it.notes = notes.trim().to_string();
                        let path = self.ensure_path();
                        let _ = self.save_to_path(&path);
                        self.set_status("Saved edit".to_string());
                    } else {
                        self.set_status("Edit: item no longer exists".to_string());
                    }
                    self.pending = None;
                }
            },
        }
    }
}

impl Avm {
    fn ui_set_event_text(&mut self, s: impl Into<String>) {
        self.ui_event_text = s.into();
    }

    fn builtin_ui_dispatch(&mut self, name: &str, args: &[CallArg]) -> miette::Result<AvmValue> {
        match name {
            "ui.event_text" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: ui.event_text expects 0 arguments"));
                }
                Ok(AvmValue::Str(self.ui_event_text.clone()))
            }
            "ui.get_text" => {
                if args.len() != 1 {
                    return Err(miette::miette!("AVM: ui.get_text expects 1 argument"));
                }
                let k = self.eval_expr(call_arg_value(&args[0]))?;
                let AvmValue::Str(key) = k else {
                    return Err(miette::miette!("AVM: ui.get_text expects string key"));
                };
                Ok(AvmValue::Str(
                    self.ui_text_state.get(&key).cloned().unwrap_or_default(),
                ))
            }
            "ui.set_text" => {
                if args.len() != 2 {
                    return Err(miette::miette!("AVM: ui.set_text expects 2 arguments"));
                }
                let k = self.eval_expr(call_arg_value(&args[0]))?;
                let v = self.eval_expr(call_arg_value(&args[1]))?;
                let AvmValue::Str(key) = k else {
                    return Err(miette::miette!("AVM: ui.set_text expects string key"));
                };
                let AvmValue::Str(val) = v else {
                    return Err(miette::miette!("AVM: ui.set_text expects string value"));
                };
                self.ui_text_state.insert(key, val);
                Ok(AvmValue::Unit)
            }
            _ => Err(miette::miette!("AVM: unknown ui builtin '{name}'")),
        }
    }
}

impl Avm {
    pub fn new(cfg: AvmConfig) -> Self {
        let debug = cfg.debug.clone();

        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let stdin = io::stdin();
            loop {
                let mut line = String::new();
                if stdin.read_line(&mut line).is_err() {
                    break;
                }
                while line.ends_with(['\n', '\r']) {
                    line.pop();
                }
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        Self {
            cfg,
            env: HashMap::new(),
            next_callback_id: 1,
            callbacks: HashMap::new(),
            verify_cache: HashMap::new(),
            hot: HashMap::new(),
            stdout: String::new(),
            shop: ShopState::default(),
            ui_event_text: String::new(),
            ui_text_state: HashMap::new(),
            stdin_rx: Some(rx),
            debug,
        }
    }

    fn poll_shop_stdin(&mut self) {
        let Some(rx) = &self.stdin_rx else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(line) => {
                    // Only consume lines when we're in an add/edit flow.
                    if self.shop.is_pending() {
                        self.shop.consume_input_line(&line);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.stdin_rx = None;
                    break;
                }
            }
        }
    }

    fn builtin_io_read_line(&mut self, prompt: &str) -> miette::Result<AvmValue> {
        if !prompt.is_empty() {
            self.stdout.push_str(prompt);
            if !prompt.ends_with(' ') {
                self.stdout.push(' ');
            }
        }

        // Note: blocks the render loop while waiting.
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|e| miette::miette!("failed to read stdin: {e}"))?;
        while line.ends_with(['\n', '\r']) {
            line.pop();
        }
        Ok(AvmValue::Str(line))
    }

    fn builtin_io_read_text(&self, path: &str) -> miette::Result<AvmValue> {
        let s = fs::read_to_string(path)
            .map_err(|e| miette::miette!("failed to read {path}: {e}"))?;
        Ok(AvmValue::Str(s))
    }

    fn builtin_io_write_text(&self, path: &str, text: &str) -> miette::Result<AvmValue> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }
        fs::write(path, text).map_err(|e| miette::miette!("failed to write {path}: {e}"))?;
        Ok(AvmValue::Unit)
    }

    fn builtin_shop_dispatch(&mut self, name: &str, args: &[CallArg]) -> miette::Result<AvmValue> {
        let eval1 = |vm: &mut Avm, idx: usize| -> miette::Result<AvmValue> {
            let a = args.get(idx).ok_or_else(|| {
                miette::miette!("AVM: {} expects at least {} argument(s)", name, idx + 1)
            })?;
            vm.eval_expr(call_arg_value(a))
        };

        match name {
            "shop.selection_status" => {
                if !args.is_empty() {
                    return Err(miette::miette!(
                        "AVM: shop.selection_status expects 0 arguments"
                    ));
                }
                let s = if let Some(sel) = self.shop.selected {
                    if let Some(it) = self.shop.items.get(sel) {
                        if it.name.trim().is_empty() {
                            format!("Editing item #{sel}")
                        } else {
                            format!("Editing: {}", it.name.trim())
                        }
                    } else {
                        "Editing item".to_string()
                    }
                } else {
                    "Add item".to_string()
                };
                Ok(AvmValue::Str(s))
            }
            "shop.select" => {
                if args.len() != 1 {
                    return Err(miette::miette!("AVM: shop.select expects 1 argument"));
                }
                let idx = match eval1(self, 0)? {
                    AvmValue::Int(i) => i,
                    _ => return Err(miette::miette!("AVM: shop.select expects integer index")),
                };
                if idx < 0 {
                    self.shop.clear_selection();
                    return Ok(AvmValue::Unit);
                }
                self.shop.select(idx as usize);
                Ok(AvmValue::Unit)
            }
            "shop.clear_selection" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.clear_selection expects 0 arguments"));
                }
                self.shop.clear_selection();
                Ok(AvmValue::Unit)
            }
            "shop.has_selection" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.has_selection expects 0 arguments"));
                }
                Ok(AvmValue::Bool(self.shop.has_selection()))
            }
            "shop.selected_index" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.selected_index expects 0 arguments"));
                }
                Ok(AvmValue::Int(self.shop.selected_index_u32() as i64))
            }
            "shop.status" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.status expects 0 arguments"));
                }
                Ok(AvmValue::Str(self.shop.status()))
            }
            "shop.is_pending" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.is_pending expects 0 arguments"));
                }
                Ok(AvmValue::Bool(self.shop.is_pending()))
            }
            "shop.cancel" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.cancel expects 0 arguments"));
                }
                self.shop.cancel_pending();
                Ok(AvmValue::Unit)
            }
            "shop.begin_add" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.begin_add expects 0 arguments"));
                }
                self.shop.begin_add();
                Ok(AvmValue::Unit)
            }
            "shop.begin_edit" => {
                if args.len() != 1 {
                    return Err(miette::miette!("AVM: shop.begin_edit expects 1 argument"));
                }
                let idx = match eval1(self, 0)? {
                    AvmValue::Int(i) => i,
                    _ => return Err(miette::miette!("AVM: shop.begin_edit expects integer index")),
                };
                if idx < 0 {
                    return Ok(AvmValue::Unit);
                }
                self.shop.begin_edit(idx as usize);
                Ok(AvmValue::Unit)
            }
            "shop.path" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.path expects 0 arguments"));
                }
                Ok(AvmValue::Str(self.shop.ensure_path()))
            }
            "shop.load" => {
                if args.len() > 1 {
                    return Err(miette::miette!("AVM: shop.load expects 0 or 1 arguments"));
                }
                let path = if args.is_empty() {
                    self.shop.ensure_path()
                } else {
                    match eval1(self, 0)? {
                        AvmValue::Str(s) => s,
                        _ => return Err(miette::miette!("AVM: shop.load expects string path")),
                    }
                };
                self.shop.load_from_path(&path)?;
                // First-run UX: ensure the file exists even if empty.
                self.shop.save_to_path(&path)?;
                Ok(AvmValue::Unit)
            }
            "shop.save" => {
                if args.len() > 1 {
                    return Err(miette::miette!("AVM: shop.save expects 0 or 1 arguments"));
                }
                let path = if args.is_empty() {
                    self.shop.ensure_path()
                } else {
                    match eval1(self, 0)? {
                        AvmValue::Str(s) => s,
                        _ => return Err(miette::miette!("AVM: shop.save expects string path")),
                    }
                };
                self.shop.save_to_path(&path)?;
                Ok(AvmValue::Unit)
            }
            "shop.count" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.count expects 0 arguments"));
                }
                Ok(AvmValue::Int(self.shop.items.len() as i64))
            }
            "shop.get_name" | "shop.get_qty" | "shop.get_notes" | "shop.get_done" => {
                if args.len() != 1 {
                    return Err(miette::miette!("AVM: {} expects 1 argument", name));
                }
                let idx = match eval1(self, 0)? {
                    AvmValue::Int(i) => i,
                    _ => return Err(miette::miette!("AVM: {} expects integer index", name)),
                };
                if idx < 0 {
                    return Ok(AvmValue::Str("".to_string()));
                }
                let i = idx as usize;
                let Some(it) = self.shop.items.get(i) else {
                    return Ok(AvmValue::Str("".to_string()));
                };
                match name {
                    "shop.get_name" => Ok(AvmValue::Str(it.name.clone())),
                    "shop.get_qty" => Ok(AvmValue::Str(it.qty.clone())),
                    "shop.get_notes" => Ok(AvmValue::Str(it.notes.clone())),
                    "shop.get_done" => Ok(AvmValue::Bool(it.purchased)),
                    _ => unreachable!(),
                }
            }
            "shop.add" => {
                if args.len() < 1 || args.len() > 3 {
                    return Err(miette::miette!(
                        "AVM: shop.add expects 1..3 arguments (name, qty?, notes?)"
                    ));
                }
                let name_s = match eval1(self, 0)? {
                    AvmValue::Str(s) => s,
                    _ => return Err(miette::miette!("AVM: shop.add expects string name")),
                };
                let qty_s = if args.len() >= 2 {
                    match eval1(self, 1)? {
                        AvmValue::Str(s) => s,
                        _ => return Err(miette::miette!("AVM: shop.add expects string qty")),
                    }
                } else {
                    "1".to_string()
                };
                let notes_s = if args.len() >= 3 {
                    match eval1(self, 2)? {
                        AvmValue::Str(s) => s,
                        _ => return Err(miette::miette!("AVM: shop.add expects string notes")),
                    }
                } else {
                    "".to_string()
                };

                if name_s.trim().is_empty() {
                    return Ok(AvmValue::Unit);
                }
                self.shop.items.push(ShopItem {
                    name: name_s.trim().to_string(),
                    qty: if qty_s.trim().is_empty() {
                        "1".to_string()
                    } else {
                        qty_s.trim().to_string()
                    },
                    notes: notes_s.trim().to_string(),
                    purchased: false,
                });
                let path = self.shop.ensure_path();
                let _ = self.shop.save_to_path(&path);
                Ok(AvmValue::Unit)
            }
            "shop.upsert" => {
                if args.len() < 1 || args.len() > 3 {
                    return Err(miette::miette!(
                        "AVM: shop.upsert expects 1..3 arguments (name, qty?, notes?)"
                    ));
                }

                let name_s = match eval1(self, 0)? {
                    AvmValue::Str(s) => s,
                    _ => return Err(miette::miette!("AVM: shop.upsert expects string name")),
                };
                let qty_s = if args.len() >= 2 {
                    match eval1(self, 1)? {
                        AvmValue::Str(s) => s,
                        _ => return Err(miette::miette!("AVM: shop.upsert expects string qty")),
                    }
                } else {
                    "".to_string()
                };
                let notes_s = if args.len() >= 3 {
                    match eval1(self, 2)? {
                        AvmValue::Str(s) => s,
                        _ => return Err(miette::miette!(
                            "AVM: shop.upsert expects string notes"
                        )),
                    }
                } else {
                    "".to_string()
                };

                if name_s.trim().is_empty() {
                    return Ok(AvmValue::Unit);
                }

                let qty_trim = qty_s.trim();
                let notes_trim = notes_s.trim();

                if let Some(sel) = self.shop.selected {
                    if sel < self.shop.items.len() {
                        let new_name = name_s.trim().to_string();
                        let new_qty = if qty_trim.is_empty() {
                            self.shop.items[sel].qty.clone()
                        } else {
                            qty_trim.to_string()
                        };
                        let new_notes = notes_trim.to_string();

                        let it = &mut self.shop.items[sel];
                        it.name = new_name;
                        it.qty = new_qty;
                        it.notes = new_notes;
                    }
                    self.shop.selected = None;
                } else {
                    self.shop.items.push(ShopItem {
                        name: name_s.trim().to_string(),
                        qty: if qty_trim.is_empty() {
                            "1".to_string()
                        } else {
                            qty_trim.to_string()
                        },
                        notes: notes_trim.to_string(),
                        purchased: false,
                    });
                }

                let path = self.shop.ensure_path();
                let _ = self.shop.save_to_path(&path);
                Ok(AvmValue::Unit)
            }
            "shop.remove" | "shop.toggle" => {
                if args.len() != 1 {
                    return Err(miette::miette!("AVM: {} expects 1 argument", name));
                }
                let idx = match eval1(self, 0)? {
                    AvmValue::Int(i) => i,
                    _ => return Err(miette::miette!("AVM: {} expects integer index", name)),
                };
                if idx < 0 {
                    return Ok(AvmValue::Unit);
                }
                let i = idx as usize;
                if i >= self.shop.items.len() {
                    return Ok(AvmValue::Unit);
                }

                if name == "shop.remove" {
                    self.shop.items.remove(i);

                    if let Some(sel) = self.shop.selected {
                        if sel == i {
                            self.shop.selected = None;
                        } else if sel > i {
                            self.shop.selected = Some(sel - 1);
                        }
                    }
                } else {
                    let it = &mut self.shop.items[i];
                    it.purchased = !it.purchased;
                }
                let path = self.shop.ensure_path();
                let _ = self.shop.save_to_path(&path);
                Ok(AvmValue::Unit)
            }
            "shop.edit" => {
                if args.len() < 2 || args.len() > 4 {
                    return Err(miette::miette!(
                        "AVM: shop.edit expects 2..4 arguments (idx, name, qty?, notes?)"
                    ));
                }
                let idx = match eval1(self, 0)? {
                    AvmValue::Int(i) => i,
                    _ => return Err(miette::miette!("AVM: shop.edit expects integer index")),
                };
                let name_s = match eval1(self, 1)? {
                    AvmValue::Str(s) => s,
                    _ => return Err(miette::miette!("AVM: shop.edit expects string name")),
                };
                let qty_s = if args.len() >= 3 {
                    match eval1(self, 2)? {
                        AvmValue::Str(s) => s,
                        _ => return Err(miette::miette!("AVM: shop.edit expects string qty")),
                    }
                } else {
                    "".to_string()
                };
                let notes_s = if args.len() >= 4 {
                    match eval1(self, 3)? {
                        AvmValue::Str(s) => s,
                        _ => return Err(miette::miette!("AVM: shop.edit expects string notes")),
                    }
                } else {
                    "".to_string()
                };

                if idx < 0 {
                    return Ok(AvmValue::Unit);
                }
                let i = idx as usize;
                if i >= self.shop.items.len() {
                    return Ok(AvmValue::Unit);
                }
                let it = &mut self.shop.items[i];
                if !name_s.trim().is_empty() {
                    it.name = name_s.trim().to_string();
                }
                if !qty_s.trim().is_empty() {
                    it.qty = qty_s.trim().to_string();
                }
                if !notes_s.trim().is_empty() {
                    it.notes = notes_s.trim().to_string();
                }
                let path = self.shop.ensure_path();
                let _ = self.shop.save_to_path(&path);
                Ok(AvmValue::Unit)
            }
            "shop.clear" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.clear expects 0 arguments"));
                }
                self.shop.items.clear();
                self.shop.selected = None;
                let path = self.shop.ensure_path();
                let _ = self.shop.save_to_path(&path);
                Ok(AvmValue::Unit)
            }
            "shop.clear_completed" => {
                if !args.is_empty() {
                    return Err(miette::miette!("AVM: shop.clear_completed expects 0 arguments"));
                }
                self.shop.items.retain(|x| !x.purchased);

                if let Some(sel) = self.shop.selected {
                    if sel >= self.shop.items.len() {
                        self.shop.selected = None;
                    }
                }

                let path = self.shop.ensure_path();
                let _ = self.shop.save_to_path(&path);
                Ok(AvmValue::Unit)
            }
            _ => Err(miette::miette!("AVM: unknown builtin '{name}'")),
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
                let max_frames = std::env::var("AURA_UI_MAX_FRAMES")
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok());
                let mut frames: u32 = 0;
                loop {
                    self.poll_shop_stdin();
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
                    if let Some(max) = max_frames {
                        if frames >= max {
                            if debug_ui {
                                eprintln!(
                                    "AURA_UI_DEBUG: layout loop ended (max frames reached: {})",
                                    max
                                );
                            }
                            break;
                        }
                    }
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

                    for ev in fb.text_input_events {
                        self.ui_set_event_text(ev.text);
                        if let Some(body) = self.callbacks.get(&ev.callback_id).cloned() {
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
                let max_frames = std::env::var("AURA_UI_MAX_FRAMES")
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok());
                let mut frames: u32 = 0;
                loop {
                    self.poll_shop_stdin();
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
                    if let Some(max) = max_frames {
                        if frames >= max {
                            if debug_ui {
                                eprintln!(
                                    "AURA_UI_DEBUG: render loop ended (max frames reached: {})",
                                    max
                                );
                            }
                            break;
                        }
                    }
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

                    for ev in fb.text_input_events {
                        self.ui_set_event_text(ev.text);
                        if let Some(body) = self.callbacks.get(&ev.callback_id).cloned() {
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
                .or_else(|| match id.node.as_str() {
                    // Minimal: allow module-style calls like `io.println(...)` and `shop.count()`
                    // without requiring `val io = "io"` pre-bindings.
                    "io" => Some(AvmValue::Str("io".to_string())),
                    "shop" => Some(AvmValue::Str("shop".to_string())),
                    "ui" => Some(AvmValue::Str("ui".to_string())),
                    _ => None,
                })
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
                } else if name == "io.read_line" {
                    if args.len() != 1 {
                        return Err(miette::miette!("AVM: io.read_line expects 1 argument"));
                    }
                    let v = self.eval_expr(call_arg_value(&args[0]))?;
                    let AvmValue::Str(prompt) = v else {
                        return Err(miette::miette!("AVM: io.read_line expects a string prompt"));
                    };
                    self.builtin_io_read_line(&prompt)
                } else if name == "io.read_text" {
                    if args.len() != 1 {
                        return Err(miette::miette!("AVM: io.read_text expects 1 argument"));
                    }
                    let v = self.eval_expr(call_arg_value(&args[0]))?;
                    let AvmValue::Str(path) = v else {
                        return Err(miette::miette!("AVM: io.read_text expects a string path"));
                    };
                    self.builtin_io_read_text(&path)
                } else if name == "io.write_text" {
                    if args.len() != 2 {
                        return Err(miette::miette!("AVM: io.write_text expects 2 arguments"));
                    }
                    let p = self.eval_expr(call_arg_value(&args[0]))?;
                    let t = self.eval_expr(call_arg_value(&args[1]))?;
                    let AvmValue::Str(path) = p else {
                        return Err(miette::miette!("AVM: io.write_text expects a string path"));
                    };
                    let AvmValue::Str(text) = t else {
                        return Err(miette::miette!("AVM: io.write_text expects a string payload"));
                    };
                    self.builtin_io_write_text(&path, &text)
                } else if name.starts_with("shop.") {
                    self.builtin_shop_dispatch(&name, args)
                } else if name.starts_with("ui.") {
                    self.builtin_ui_dispatch(&name, args)
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
            | "TextInput"
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
