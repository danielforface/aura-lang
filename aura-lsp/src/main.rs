#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

const AURA_LSP_PROTOCOL_VERSION: u32 = 1;

use aura_verify::VerifyError;
use miette::{SourceOffset, SourceSpan};
use serde_json::json;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug, Clone, serde::Serialize)]
struct CounterexampleBindingV1 {
    name: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value_kind: Option<String>,
    relevant: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    best_range: Option<Range>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CounterexampleInjectionV1 {
    range: Range,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CounterexampleV1 {
    schema: &'static str,
    bindings: Vec<CounterexampleBindingV1>,
    injections: Vec<CounterexampleInjectionV1>,
}

#[derive(Debug, Clone, Default)]
struct AuraClientTelemetryCaps {
    proof_timings: bool,
    proof_cache: bool,
}

#[derive(Debug, Clone, Default)]
struct AuraClientCaps {
    protocol_version: Option<u64>,
    phases: Option<Vec<String>>,
    telemetry: AuraClientTelemetryCaps,
}

fn parse_aura_client_caps(params: &InitializeParams) -> AuraClientCaps {
    fn parse_from_value(v: &serde_json::Value) -> AuraClientCaps {
        // Expected shape (best-effort; everything optional):
        // {
        //   "aura": {
        //     "protocolVersion": 1,
        //     "phases": ["parse","sema","normalize","z3"],
        //     "telemetry": {"proofTimings": true, "proofCache": true}
        //   }
        // }
        let aura = v.get("aura").unwrap_or(v);

        let protocol_version = aura
            .get("protocolVersion")
            .and_then(|x| x.as_u64());

        let phases = aura.get("phases").and_then(|x| {
            x.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
        });

        let telemetry_v = aura.get("telemetry");
        let telemetry = AuraClientTelemetryCaps {
            proof_timings: telemetry_v
                .and_then(|t| t.get("proofTimings"))
                .and_then(|b| b.as_bool())
                .unwrap_or(false),
            proof_cache: telemetry_v
                .and_then(|t| t.get("proofCache"))
                .and_then(|b| b.as_bool())
                .unwrap_or(false),
        };

        AuraClientCaps {
            protocol_version,
            phases,
            telemetry,
        }
    }

    // Prefer capabilities.experimental (true LSP capability negotiation).
    if let Some(exp) = params.capabilities.experimental.as_ref() {
        if exp.get("aura").is_some() {
            return parse_from_value(exp);
        }
    }

    // Fallback for clients that can't customize capabilities (e.g. vscode-languageclient).
    if let Some(init) = params.initialization_options.as_ref() {
        if init.get("aura").is_some() {
            return parse_from_value(init);
        }
    }

    AuraClientCaps::default()
}

use sha2::{Digest, Sha256};

const PROOF_CACHE_STORE_VERSION: u32 = 1;
const PROOF_CACHE_FILE_NAME: &str = "proof-cache-v1.json";

fn env_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            matches!(t.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

fn env_usize(name: &str, default_value: usize) -> usize {
    match std::env::var(name) {
        Ok(v) => v.trim().parse::<usize>().unwrap_or(default_value),
        Err(_) => default_value,
    }
}

fn proof_cache_enabled() -> bool {
    !env_truthy("AURA_PROOF_CACHE_DISABLE")
}

fn proof_cache_persist_enabled() -> bool {
    if !proof_cache_enabled() {
        return false;
    }
    !env_truthy("AURA_PROOF_CACHE_PERSIST_DISABLE")
}

fn proof_cache_clear_on_start() -> bool {
    env_truthy("AURA_PROOF_CACHE_CLEAR_ON_START")
}

fn proof_cache_store_max_entries() -> usize {
    env_usize("AURA_PROOF_CACHE_MAX_ENTRIES", 256)
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct ProofCacheStoreV1 {
    version: u32,
    // base_key -> entry
    entries: HashMap<String, ProofCacheEntry>,
}

fn proof_cache_path_for_root(root: &Path) -> PathBuf {
    root.join(".aura").join("cache").join(PROOF_CACHE_FILE_NAME)
}

fn file_id_for_cache(root: Option<&Path>, uri: &Url) -> String {
    let path = uri.to_file_path().ok();
    if let (Some(root), Some(path)) = (root, path) {
        if let Ok(rel) = path.strip_prefix(root) {
            return rel.to_string_lossy().replace('\\', "/");
        }
    }
    uri.to_string()
}

fn extract_import_paths_fast(text: &str) -> Vec<Vec<String>> {
    // Best-effort, line-based scan:
    //   import foo::bar
    // We ignore everything after the path token.
    let mut out: Vec<Vec<String>> = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        if !t.starts_with("import ") {
            continue;
        }
        let rest = t.trim_start_matches("import ").trim();
        let token = rest
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches(';');
        if token.is_empty() {
            continue;
        }
        let segs: Vec<String> = token
            .split("::")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if !segs.is_empty() {
            out.push(segs);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn resolve_import_paths(root: &Path, imports: &[Vec<String>]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for segs in imports {
        if segs.is_empty() {
            continue;
        }
        // Skip stdlib imports; the verifier already augments source with default std.
        if segs.first().map(|s| s.as_str()) == Some("aura") {
            continue;
        }

        let joined = segs.join(std::path::MAIN_SEPARATOR_STR);
        let cand1 = root.join(format!("{joined}.aura"));
        if cand1.exists() {
            out.push(cand1);
            continue;
        }
        if let Some(last) = segs.last() {
            let cand2 = root.join(format!("{last}.aura"));
            if cand2.exists() {
                out.push(cand2);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn dep_hash_from_imports(root: Option<&Path>, text: &str) -> String {
    let Some(root) = root else {
        return sha256_hex("");
    };
    let imports = extract_import_paths_fast(text);
    let deps = resolve_import_paths(root, &imports);
    let mut input = String::new();
    for p in deps {
        input.push_str(&p.to_string_lossy().replace('\\', "/"));
        input.push('\n');
        let dep_text = fs::read_to_string(&p).unwrap_or_default();
        input.push_str(&sha256_hex(&dep_text));
        input.push('\n');
    }
    sha256_hex(&input)
}

#[derive(Clone, Debug)]
struct StmtHashInfo {
    hash: String,
    is_ui: bool,
}

fn compute_checkable_stmt_merkle_hashes(
    program: &aura_ast::Program,
    text: &str,
    dep_hash: &str,
) -> Vec<Option<StmtHashInfo>> {
    use aura_ast::{Expr, ExprKind, Stmt};

    fn walk_expr_call_names(out: &mut std::collections::BTreeSet<String>, expr: &Expr) {
        match &expr.kind {
            ExprKind::Call { callee, args, trailing } => {
                if let Some(name) = expr_callee_name(callee) {
                    out.insert(name);
                }
                walk_expr_call_names(out, callee);
                for a in args {
                    match a {
                        aura_ast::CallArg::Positional(e) => walk_expr_call_names(out, e),
                        aura_ast::CallArg::Named { value, .. } => walk_expr_call_names(out, value),
                    }
                }
                if let Some(b) = trailing {
                    walk_block_call_names(out, b);
                }
            }
            ExprKind::Unary { expr: inner, .. } => walk_expr_call_names(out, inner),
            ExprKind::Binary { left, right, .. } => {
                walk_expr_call_names(out, left);
                walk_expr_call_names(out, right);
            }
            ExprKind::Member { base, .. } => walk_expr_call_names(out, base),
            ExprKind::Lambda { body, .. } => walk_block_call_names(out, body),
            ExprKind::Flow { left, right, .. } => {
                walk_expr_call_names(out, left);
                walk_expr_call_names(out, right);
            }
            ExprKind::StyleLit { fields } => {
                for (_, v) in fields {
                    walk_expr_call_names(out, v);
                }
            }
            ExprKind::RecordLit { fields, .. } => {
                for (_, v) in fields {
                    walk_expr_call_names(out, v);
                }
            }
            ExprKind::ForAll { binders: _, body } | ExprKind::Exists { binders: _, body } => {
                walk_expr_call_names(out, body);
            }
            ExprKind::Ident(_) | ExprKind::IntLit(_) | ExprKind::StringLit(_) => {}
        }
    }

    fn walk_stmt_call_names(out: &mut std::collections::BTreeSet<String>, stmt: &Stmt) {
        match stmt {
            Stmt::StrandDef(sd) => walk_expr_call_names(out, &sd.expr),
            Stmt::Assign(a) => walk_expr_call_names(out, &a.expr),
            Stmt::Prop(p) => walk_expr_call_names(out, &p.expr),
            Stmt::ExprStmt(e) => walk_expr_call_names(out, e),
            Stmt::Requires(r) => walk_expr_call_names(out, &r.expr),
            Stmt::Ensures(e) => walk_expr_call_names(out, &e.expr),
            Stmt::Assert(a) => walk_expr_call_names(out, &a.expr),
            Stmt::Assume(a) => walk_expr_call_names(out, &a.expr),
            Stmt::If(i) => {
                walk_expr_call_names(out, &i.cond);
                walk_block_call_names(out, &i.then_block);
                if let Some(b) = &i.else_block {
                    walk_block_call_names(out, b);
                }
            }
            Stmt::Match(m) => {
                walk_expr_call_names(out, &m.scrutinee);
                for arm in &m.arms {
                    walk_block_call_names(out, &arm.body);
                }
            }
            Stmt::While(w) => {
                walk_expr_call_names(out, &w.cond);
                if let Some(inv) = &w.invariant {
                    walk_expr_call_names(out, inv);
                }
                if let Some(dec) = &w.decreases {
                    walk_expr_call_names(out, dec);
                }
                walk_block_call_names(out, &w.body);
            }
            Stmt::CellDef(c) => walk_block_call_names(out, &c.body),
            Stmt::FlowBlock(f) => walk_block_call_names(out, &f.body),
            Stmt::Layout(l) => walk_block_call_names(out, &l.body),
            Stmt::Render(r) => walk_block_call_names(out, &r.body),
            Stmt::MacroCall(m) => {
                for a in &m.args {
                    walk_expr_call_names(out, a);
                }
            }
            Stmt::MacroDef(m) => walk_block_call_names(out, &m.body),
            Stmt::UnsafeBlock(ub) => walk_block_call_names(out, &ub.body),
            Stmt::Import(_) | Stmt::ExternCell(_) | Stmt::TypeAlias(_) | Stmt::TraitDef(_) | Stmt::RecordDef(_) | Stmt::EnumDef(_) => {}
        }
    }

    fn walk_block_call_names(out: &mut std::collections::BTreeSet<String>, block: &aura_ast::Block) {
        for s in &block.stmts {
            walk_stmt_call_names(out, s);
        }
        if let Some(y) = &block.yield_expr {
            walk_expr_call_names(out, y);
        }
    }

    let n = program.stmts.len();
    let mut out: Vec<Option<StmtHashInfo>> = vec![None; n];

    // Map top-level callable defs (cells/flows) to statement indices.
    let mut top_level_callables: HashMap<String, usize> = HashMap::new();
    for (i, stmt) in program.stmts.iter().enumerate() {
        match stmt {
            Stmt::CellDef(c) => {
                top_level_callables.insert(c.name.node.clone(), i);
            }
            Stmt::FlowBlock(f) => {
                top_level_callables.insert(f.name.node.clone(), i);
            }
            _ => {}
        }
    }

    // Content hashes for checkable top-level units.
    let mut content_hash_by_idx: HashMap<usize, String> = HashMap::new();
    let mut kind_by_idx: HashMap<usize, &'static str> = HashMap::new();
    let mut ui_by_idx: HashMap<usize, bool> = HashMap::new();

    for (i, stmt) in program.stmts.iter().enumerate() {
        let (is_checkable, is_ui) = match stmt {
            Stmt::CellDef(_) => (true, true),
            Stmt::FlowBlock(_) => (true, false),
            Stmt::Layout(_) => (true, true),
            Stmt::Render(_) => (true, true),
            _ => (false, false),
        };
        if !is_checkable {
            continue;
        }

        let (kind, sp) = match stmt {
            Stmt::CellDef(s) => ("cell", s.span),
            Stmt::FlowBlock(s) => ("flow", s.span),
            Stmt::Layout(s) => ("layout", s.span),
            Stmt::Render(s) => ("render", s.span),
            _ => continue,
        };
        let start = sp.offset();
        let end = start.saturating_add(sp.len());
        let slice = text.get(start..end).unwrap_or("");
        let content_hash = sha256_hex(&format!("stmt-content-v1\nkind={kind}\n{slice}"));

        content_hash_by_idx.insert(i, content_hash);
        kind_by_idx.insert(i, kind);
        ui_by_idx.insert(i, is_ui);
    }

    // Direct deps via call edges.
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, stmt) in program.stmts.iter().enumerate() {
        if !content_hash_by_idx.contains_key(&i) {
            continue;
        }
        let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        walk_stmt_call_names(&mut names, stmt);

        let mut edges: Vec<usize> = Vec::new();
        for name in names {
            if let Some(&j) = top_level_callables.get(&name) {
                if j != i {
                    edges.push(j);
                }
            }
        }
        edges.sort();
        edges.dedup();
        deps[i] = edges;
    }

    // Transitive closure of deps (by index), restricted to checkable nodes.
    for i in 0..n {
        if !content_hash_by_idx.contains_key(&i) {
            continue;
        }
        let mut stack: Vec<usize> = deps[i].clone();
        let mut seen: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        while let Some(j) = stack.pop() {
            if j == i {
                continue;
            }
            if !content_hash_by_idx.contains_key(&j) {
                continue;
            }
            if !seen.insert(j) {
                continue;
            }
            for &k in &deps[j] {
                stack.push(k);
            }
        }

        let kind = kind_by_idx.get(&i).copied().unwrap_or("stmt");
        let content_hash = content_hash_by_idx.get(&i).cloned().unwrap_or_else(|| sha256_hex(""));

        let mut input = String::new();
        input.push_str("stmt-merkle-v1\n");
        input.push_str(&format!("kind={kind}\n"));
        input.push_str(&format!("content={content_hash}\n"));
        input.push_str(&format!("imports={dep_hash}\n"));
        for j in seen {
            // Encode deps deterministically by name if we have one, else by index.
            let dep_name = top_level_callables
                .iter()
                .find_map(|(name, idx)| if *idx == j { Some(name.clone()) } else { None })
                .unwrap_or_else(|| format!("#{j}"));
            let dep_content = content_hash_by_idx
                .get(&j)
                .cloned()
                .unwrap_or_else(|| sha256_hex(""));
            input.push_str(&format!("dep={dep_name}:{dep_content}\n"));
        }

        let hash = sha256_hex(&input);
        out[i] = Some(StmtHashInfo {
            hash,
            is_ui: ui_by_idx.get(&i).copied().unwrap_or(false),
        });
    }

    out
}

use aura_nexus::{NexusFileContext, PluginManifest};

// Stable, documented diagnostic codes for Aura (versioned).
// See docs/diagnostic-codes.md.
const DIAG_PARSE_ERROR: &str = "AUR-0.2-0001";
const DIAG_SEMA_ERROR: &str = "AUR-0.2-0002";
const DIAG_VERIFY_ERROR: &str = "AUR-0.2-0003";
const DIAG_INTERNAL_ERROR: &str = "AUR-0.2-9000";

fn diagnostic_code_href(_code: &str) -> Option<Url> {
    // We intentionally do not embed an external URL here.
    // The stable code itself is the contract; the mapping is documented in docs/diagnostic-codes.md.
    None
}

fn offset_from_position(text: &str, pos: Position) -> usize {
    let mut line: u32 = 0;
    let mut col: u32 = 0;
    let mut i: usize = 0;

    for ch in text.chars() {
        if line > pos.line || (line == pos.line && col >= pos.character) {
            break;
        }

        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        i += ch.len_utf8();
    }

    i
}

fn position_leq(a: Position, b: Position) -> bool {
    a.line < b.line || (a.line == b.line && a.character <= b.character)
}

fn position_lt(a: Position, b: Position) -> bool {
    a.line < b.line || (a.line == b.line && a.character < b.character)
}

fn range_contains_position(range: Range, pos: Position) -> bool {
    // LSP ranges are typically [start, end) (end is exclusive).
    position_leq(range.start, pos) && position_lt(pos, range.end)
}

fn is_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn infer_value_kind(value: &str) -> Option<String> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("false") {
        return Some("bool".to_string());
    }
    if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
        return Some("string".to_string());
    }
    // Common Z3-ish forms.
    if v.starts_with("(_ bv") && v.ends_with(')') {
        return Some("bitvec".to_string());
    }
    if v.starts_with("#x") || v.starts_with("#b") {
        return Some("bitvec".to_string());
    }

    // Integer-ish.
    let mut chars = v.chars();
    if let Some('-') = chars.next() {
        if chars.clone().all(|c| c.is_ascii_digit()) {
            return Some("int".to_string());
        }
    } else if v.chars().all(|c| c.is_ascii_digit()) {
        return Some("int".to_string());
    }

    None
}

fn is_ident_boundary_byte(b: u8) -> bool {
    !(b == b'_' || b.is_ascii_alphanumeric())
}

fn find_ident_occurrences(text: &str, name: &str) -> Vec<(usize, Range)> {
    if name.is_empty() {
        return Vec::new();
    }
    // Only attempt byte-based matching for simple identifier names.
    if !is_valid_ident_name(name) {
        return Vec::new();
    }
    let hay = text.as_bytes();
    let needle = name.as_bytes();
    if needle.is_empty() || needle.len() > hay.len() {
        return Vec::new();
    }

    let mut out: Vec<(usize, Range)> = Vec::new();
    let mut i: usize = 0;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            let before_ok = if i == 0 {
                true
            } else {
                is_ident_boundary_byte(hay[i - 1])
            };
            let after_i = i + needle.len();
            let after_ok = if after_i >= hay.len() {
                true
            } else {
                is_ident_boundary_byte(hay[after_i])
            };
            if before_ok && after_ok {
                let start = position_from_offset(text, i);
                let end = position_from_offset(text, i + needle.len());
                out.push((i, Range { start, end }));
                i = after_i;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn pick_best_occurrence(
    occurrences: &[(usize, Range)],
    prefer_start: usize,
    prefer_end: usize,
) -> Option<Range> {
    let mut best: Option<(bool, usize, Range)> = None;
    for (off, r) in occurrences {
        let inside = *off >= prefer_start && *off <= prefer_end;
        let dist = if *off >= prefer_start {
            *off - prefer_start
        } else {
            prefer_start - *off
        };
        match &best {
            None => best = Some((inside, dist, r.clone())),
            Some((best_inside, best_dist, _)) => {
                if inside && !*best_inside {
                    best = Some((inside, dist, r.clone()));
                } else if inside == *best_inside && dist < *best_dist {
                    best = Some((inside, dist, r.clone()));
                }
            }
        }
    }
    best.map(|(_, _, r)| r)
}

fn ident_prefix_at(text: &str, offset: usize) -> String {
    let before = &text[..std::cmp::min(offset, text.len())];
    let mut prefix = String::new();
    for ch in before.chars().rev() {
        if is_ident_char(ch) {
            prefix.push(ch);
        } else {
            break;
        }
    }
    prefix.chars().rev().collect()
}

fn collect_identifiers(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if is_ident_char(ch) {
            current.push(ch);
        } else {
            if current.len() >= 2 {
                out.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if current.len() >= 2 {
        out.push(current);
    }

    out.sort();
    out.dedup();
    out
}

fn sha256_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ProofsParams {
    uri: Url,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ProofsResponse {
    uri: Url,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ProofsStreamStartParams {
    uri: Url,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    ranges: Option<Vec<Range>>, 
}

#[derive(Debug, Clone, serde::Serialize)]
struct ProofsStreamStartResponse {
    id: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ProofsStreamCancelParams {
    id: u64,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct ProofCacheClearParams {
    // Reserved for future: file-scoped clearing.
    #[serde(default)]
    uri: Option<Url>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ProofCacheClearResponse {
    cleared: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ProofsStreamEvent {
    id: u64,
    uri: Url,
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostics: Option<Vec<Diagnostic>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    telemetry: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

enum AuraProofsStreamNotification {}

impl tower_lsp::lsp_types::notification::Notification for AuraProofsStreamNotification {
    type Params = ProofsStreamEvent;
    const METHOD: &'static str = "aura/proofsStream";
}

fn diagnostic_from_span(text: &str, span: SourceSpan, code: &str, message: String) -> Diagnostic {
    let range = range_from_source_span(text, span);
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.to_string())),
        code_description: diagnostic_code_href(code).map(|href| CodeDescription { href }),
        source: Some("aura".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

fn compute_diagnostics(uri: &Url, text: &str, prover: &mut aura_verify::Z3Prover) -> Vec<Diagnostic> {
    let mut diags: Vec<Diagnostic> = Vec::new();

    let text = match aura_sdk::augment_source_with_default_std(text) {
        Ok(t) => t,
        Err(_) => text.to_string(),
    };

    let source_path = uri.to_file_path().ok();

    let program = match aura_parse::parse_source(&text) {
        Ok(p) => p,
        Err(e) => {
            diags.push(diagnostic_from_miette(uri, &text, e));
            return diags;
        }
    };

    // Semantic checks (best effort). If sema fails, surface as a diagnostic.
    let mut checker = aura_core::Checker::new();
    if let Err(e) = checker.check_program(&program) {
        diags.push(diagnostic_from_miette(uri, &text, e.into()));
        return diags;
    }

    // Z3 verification diagnostics.
    {
        let manifest = find_aura_toml_for_uri(uri);
        let manifest_plugins = manifest
            .as_deref()
            .and_then(load_manifest_plugins)
            .unwrap_or_default();

        let mut nexus = aura_nexus::NexusContext::default();
        nexus.insert(NexusFileContext {
            source_path,
            manifest_path: manifest.clone(),
        });
        match verify_with_manifest_plugins(&program, prover, &mut nexus, &manifest_plugins) {
            Ok(report) => {
                for p in report.proofs {
                    diags.push(diagnostic_from_proof_note(
                        uri,
                        &text,
                        &p,
                        manifest.as_deref(),
                        &manifest_plugins,
                    ));
                }
            }
            Err(err) => diags.push(diagnostic_from_verify_error(uri, &text, err)),
        }
    }

    diags
}

fn verify_with_manifest_plugins(
    program: &aura_ast::Program,
    prover: &mut aura_verify::Z3Prover,
    nexus: &mut aura_nexus::NexusContext,
    manifest_plugins: &[PluginManifest],
) -> std::result::Result<aura_verify::VerificationReport, aura_verify::VerifyError> {
    let requested = requested_plugins_from_manifest(manifest_plugins)?;

    match requested.as_slice() {
        [a] if a == "aura-ai" => {
            let plugins = (aura_plugin_ai::AuraAiPlugin::new(),);
            aura_verify::verify_program_z3_report(program, prover, &plugins, nexus)
        }
        [a] if a == "aura-iot" => {
            let plugins = (aura_plugin_iot::AuraIotPlugin::new(),);
            aura_verify::verify_program_z3_report(program, prover, &plugins, nexus)
        }
        [a, b]
            if (a == "aura-ai" && b == "aura-iot") || (a == "aura-iot" && b == "aura-ai") =>
        {
            let plugins = (
                aura_plugin_iot::AuraIotPlugin::new(),
                aura_plugin_ai::AuraAiPlugin::new(),
            );
            aura_verify::verify_program_z3_report(program, prover, &plugins, nexus)
        }
        other => Err(aura_verify::VerifyError {
            message: format!(
                "unsupported Nexus plugin set: {:?}. Supported built-ins: ['aura-iot', 'aura-ai']",
                other
            ),
            span: SourceSpan::new(SourceOffset::from(0usize), 0usize),
            model: None,
            meta: None,
        }),
    }
}

fn requested_plugins_from_manifest(
    manifest_plugins: &[PluginManifest],
) -> std::result::Result<Vec<String>, aura_verify::VerifyError> {
    // If aura.toml lists plugins, require them to be trusted.
    // If it lists none (or no manifest exists), default to built-ins.
    let mut requested: Vec<String> = Vec::new();
    for p in manifest_plugins {
        if !p.trusted {
            return Err(aura_verify::VerifyError {
                message: format!(
                    "Nexus plugin '{}' is not trusted. Set `trusted = true` in aura.toml to enable it.",
                    p.name
                ),
                span: SourceSpan::new(SourceOffset::from(0usize), 0usize),
                model: None,
                meta: None,
            });
        }
        requested.push(p.name.to_ascii_lowercase());
    }
    requested.sort();
    requested.dedup();
    if requested.is_empty() {
        requested.push("aura-ai".to_string());
        requested.push("aura-iot".to_string());
        requested.sort();
    }
    Ok(requested)
}

fn diagnostic_from_proof_note(
    uri: &Url,
    text: &str,
    p: &aura_nexus::ProofNote,
    manifest_path: Option<&Path>,
    manifest_plugins: &[PluginManifest],
) -> Diagnostic {
    let range = range_from_source_span(text, p.span);

    let mut related: Vec<DiagnosticRelatedInformation> = Vec::new();

    if let Some(smt) = p.smt.as_deref() {
        related.push(DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range,
            },
            message: format!("SMT: {smt}"),
        });
    }

    if !p.unsat_core.is_empty() {
        // Keep the reasoning trace scannable: add one related entry per core element.
        // (Some clients truncate very long diagnostic lines.)
        for (i, smt) in p.unsat_core.iter().take(50).enumerate() {
            related.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range,
                },
                message: format!(
                    "UNSAT core {}/{}: {}",
                    i + 1,
                    p.unsat_core.len(),
                    smt
                ),
            });
        }
    }

    if let Some(itp) = p.interpolant.as_deref() {
        related.push(DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range,
            },
            message: format!("Derived lemma: {itp}"),
        });
    }

    if let Some(mp) = manifest_path {
        if let Some(entry) = find_plugin_entry(mp, &p.plugin, manifest_plugins) {
            related.push(entry);
        }
    }

    for ri in &p.related {
        related.push(DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: range_from_source_span(text, ri.span),
            },
            message: ri.message.clone(),
        });
    }

    let code = {
        let mut parts: Vec<String> = Vec::new();
        parts.push(format!("nexus:{}", p.plugin));
        parts.push(format!("kind={}", p.kind));
        if let Some(mask) = p.mask {
            parts.push(format!("mask=0x{mask:X}"));
        }
        if let Some((lo, hi)) = p.range {
            parts.push(format!("range=0x{lo:X}..0x{hi:X}"));
        }
        parts.join(";")
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::INFORMATION),
        code: Some(NumberOrString::String(code)),
        code_description: None,
        source: Some("aura-nexus".to_string()),
        message: p.message.clone(),
        related_information: if related.is_empty() { None } else { Some(related) },
        tags: None,
        data: Some(json!({
            "plugin": p.plugin,
            "kind": p.kind,
            "mask": p.mask,
            "range": p.range.map(|(lo, hi)| { json!({"lo": lo, "hi": hi}) }),
            "smt": p.smt,
            "unsat_core": p.unsat_core.clone(),
            "interpolant": p.interpolant.clone(),
            "related": p.related.iter().map(|ri| json!({
                "message": ri.message,
                "span": {"offset": ri.span.offset(), "len": ri.span.len()},
            })).collect::<Vec<_>>(),
        })),
    }
}

fn find_aura_toml_for_uri(uri: &Url) -> Option<PathBuf> {
    let p = uri.to_file_path().ok()?;
    find_manifest(&p)
}

fn find_manifest(start: &Path) -> Option<PathBuf> {
    let mut cur = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let candidate = cur.join("aura.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        cur = cur.parent()?.to_path_buf();
    }
}

#[derive(Default, serde::Deserialize)]
struct AuraToml {
    #[serde(default)]
    plugins: Vec<PluginManifest>,
}

fn load_manifest_plugins(path: &Path) -> Option<Vec<PluginManifest>> {
    let raw = fs::read_to_string(path).ok()?;
    let parsed: AuraToml = toml::from_str(&raw).ok()?;
    Some(parsed.plugins)
}

fn find_plugin_entry(
    manifest_path: &Path,
    plugin_name: &str,
    manifest_plugins: &[PluginManifest],
) -> Option<DiagnosticRelatedInformation> {
    let raw = fs::read_to_string(manifest_path).ok()?;
    let plugin_lower = plugin_name.to_ascii_lowercase();

    // Best-effort: try to find the plugin name where it's assigned to `name = "..."`.
    // Supports both `[[plugins]]` table-array form and inline arrays (`plugins = [{...}]`).
    let mut found_idx: Option<usize> = None;

    // 1) Prefer `[[plugins]]` blocks.
    {
        let mut offset: usize = 0;
        let mut in_plugins_block = false;
        for line in raw.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("[[") {
                in_plugins_block = trimmed.starts_with("[[plugins]]");
            }

            if in_plugins_block {
                let lower = line.to_ascii_lowercase();
                if lower.contains("name") {
                    // look for "<plugin>" on this line
                    let quoted = format!("\"{}\"", plugin_lower);
                    if let Some(rel) = lower.find(&quoted) {
                        found_idx = Some(offset + rel + 1); // +1 => start of name inside quotes
                        break;
                    }
                }
            }

            // +1 for the newline we lost in `.lines()`.
            offset = offset.saturating_add(line.len()).saturating_add(1);
        }
    }

    // 2) Fallback: scan for a `name = "<plugin>"` assignment anywhere.
    if found_idx.is_none() {
        let lower = raw.to_ascii_lowercase();
        let bytes = lower.as_bytes();
        let name_bytes = b"name";
        let mut i: usize = 0;
        while i + name_bytes.len() <= bytes.len() {
            if &bytes[i..i + name_bytes.len()] == name_bytes {
                let mut j = i + name_bytes.len();
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j >= bytes.len() || bytes[j] != b'=' {
                    i += 1;
                    continue;
                }
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j >= bytes.len() || bytes[j] != b'\"' {
                    i += 1;
                    continue;
                }
                j += 1;
                if j + plugin_lower.len() <= lower.len()
                    && &lower[j..j + plugin_lower.len()] == plugin_lower
                {
                    found_idx = Some(j);
                    break;
                }
            }
            i += 1;
        }
    }

    let idx = found_idx?;
    let span = SourceSpan::new(SourceOffset::from(idx), plugin_name.len());
    let range = range_from_source_span(&raw, span);

    let trusted = manifest_plugins
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(plugin_name))
        .map(|p| p.trusted);

    Some(DiagnosticRelatedInformation {
        location: Location {
            uri: Url::from_file_path(manifest_path).ok()?,
            range,
        },
        message: match trusted {
            Some(true) => format!("Enabled by aura.toml plugin entry: '{plugin_name}' (trusted)"),
            Some(false) => format!("Enabled by aura.toml plugin entry: '{plugin_name}' (NOT trusted)"),
            None => format!("Enabled by aura.toml plugin entry: '{plugin_name}'"),
        },
    })
}

fn range_from_source_span(text: &str, span: SourceSpan) -> Range {
    let start = span.offset();
    let len = span.len();
    let end = start.saturating_add(len);

    let start_pos = position_from_offset(text, start);
    let end_pos = position_from_offset(text, end);
    Range {
        start: start_pos,
        end: end_pos,
    }
}

fn position_from_offset(text: &str, offset: usize) -> Position {
    let mut line: u32 = 0;
    let mut col: u32 = 0;
    let mut i: usize = 0;

    for ch in text.chars() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        i += ch.len_utf8();
    }

    Position {
        line,
        character: col,
    }
}

fn diagnostic_from_verify_error(uri: &Url, text: &str, err: VerifyError) -> Diagnostic {
    let range = range_from_source_span(text, err.span);

    let related_information = err.meta.as_ref().and_then(|meta| {
        let mut out: Vec<DiagnosticRelatedInformation> = Vec::new();

        for ri in &meta.related {
            out.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range: range_from_source_span(text, ri.span),
                },
                message: ri.message.clone(),
            });
        }

        for h in &meta.hints {
            out.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range,
                },
                message: format!("Hint: {h}"),
            });
        }

        for s in &meta.suggestions {
            out.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range,
                },
                message: format!("Suggestion: {s}"),
            });
        }

        if !meta.unsat_core.is_empty() {
            out.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range,
                },
                message: format!("UNSAT core ({}): {}", meta.unsat_core.len(), meta.unsat_core.join(" AND ")),
            });
        }

        if out.is_empty() { None } else { Some(out) }
    });

    let message_text = err.message.clone();

    let model_text = err
        .meta
        .as_ref()
        .and_then(|m| m.model.clone())
        .or(err.model);

    let model_for_data = model_text.clone();

    let bindings = err
        .meta
        .as_ref()
        .map(|m| m.bindings.clone())
        .unwrap_or_default();

    let related_msgs: Vec<String> = err
        .meta
        .as_ref()
        .map(|m| m.related.iter().map(|ri| ri.message.clone()).collect())
        .unwrap_or_default();

    let mut bindings_for_data: Vec<serde_json::Value> = Vec::new();
    let mut relevant_bindings: Vec<serde_json::Value> = Vec::new();
    for (name, value) in bindings.iter().take(50) {
        let relevant = message_text.contains(name) || related_msgs.iter().any(|m| m.contains(name));
        let entry = json!({
            "name": name,
            "value": value,
            "relevant": relevant,
        });
        if relevant {
            relevant_bindings.push(entry.clone());
        }
        bindings_for_data.push(entry);
    }

    // Counterexample mapping layer (best-effort): map binding names back to identifier occurrences
    // in the source, and compute inline injection suggestions.
    let err_off: usize = err.span.offset().into();
    let err_end: usize = err_off.saturating_add(err.span.len().into());

    let mut mapped_bindings: Vec<CounterexampleBindingV1> = Vec::new();
    // Prefer relevant bindings for mapping; fall back to all.
    let mapping_iter: Vec<(&String, &String, bool)> = if !relevant_bindings.is_empty() {
        bindings
            .iter()
            .filter(|(n, _)| message_text.contains(n) || related_msgs.iter().any(|m| m.contains(n)))
            .take(24)
            .map(|(n, v)| (n, v, true))
            .collect()
    } else {
        bindings
            .iter()
            .take(24)
            .map(|(n, v)| (n, v, false))
            .collect()
    };

    let mut injections: Vec<CounterexampleInjectionV1> = Vec::new();
    for (name, value, relevant) in mapping_iter {
        let occ = find_ident_occurrences(text, name);
        let best_range = pick_best_occurrence(&occ, err_off, err_end);
        let kind = infer_value_kind(value);

        mapped_bindings.push(CounterexampleBindingV1 {
            name: name.clone(),
            value: value.clone(),
            value_kind: kind,
            relevant,
            best_range: best_range.clone(),
        });

        if injections.len() < 8 {
            if let Some(r) = best_range {
                injections.push(CounterexampleInjectionV1 {
                    range: r,
                    text: format!(" /* {} = {} */", name, value),
                    name: Some(name.clone()),
                });
            }
        }
    }

    let counterexample_v1 = if !mapped_bindings.is_empty() {
        Some(CounterexampleV1 {
            schema: "aura.counterexample.v1",
            bindings: mapped_bindings,
            injections,
        })
    } else {
        None
    };

    // Minimal model slice: prefer relevant bindings; otherwise take the first few.
    let minimal_slice = if !relevant_bindings.is_empty() {
        relevant_bindings.clone()
    } else {
        bindings_for_data.iter().take(8).cloned().collect::<Vec<_>>()
    };

    let diag_message = match model_text.as_deref() {
        Some(model) => format!("{}\n\nModel:\n{}", err.message, model),
        None => err.message,
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(DIAG_VERIFY_ERROR.to_string())),
        code_description: diagnostic_code_href(DIAG_VERIFY_ERROR).map(|href| CodeDescription { href }),
        source: Some("aura".to_string()),
        message: diag_message,
        related_information,
        tags: None,
        data: Some(json!({
            "stable_code": DIAG_VERIFY_ERROR,
            "message": message_text,
            "model": model_for_data,
            "counterexample": {
                "schema": counterexample_v1.as_ref().map(|c| c.schema),
                "slice": minimal_slice,
                "mapped": counterexample_v1.as_ref().map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null)),
            },
            "meta": err.meta.as_ref().map(|m| json!({
                "bindings": bindings_for_data,
                "relevantBindings": relevant_bindings,
                "related": m.related.iter().map(|ri| json!({
                    "message": ri.message,
                    "span": {"offset": ri.span.offset(), "len": ri.span.len()},
                })).collect::<Vec<_>>(),
                "unsatCore": m.unsat_core.clone(),
                "hints": m.hints.clone(),
                "suggestions": m.suggestions.clone(),
            })),
        })),
    }
}

fn diagnostic_from_miette(uri: &Url, text: &str, report: miette::Report) -> Diagnostic {
    if let Some(e) = report.downcast_ref::<aura_parse::ParseError>() {
        return diagnostic_from_span(
            text,
            e.span,
            DIAG_PARSE_ERROR,
            format!("parse error: {}", e.message),
        );
    }

    if let Some(e) = report.downcast_ref::<aura_core::SemanticError>() {
        // Best-effort enhancement: for linear capability errors like
        // "use after move: 'x' (consumed at byte offset 123 len 4)", surface
        // a related location for the consumption site so UIs can jump.
        fn consumed_span_from_message(msg: &str) -> Option<(usize, usize)> {
            let needle = "consumed at byte offset ";
            let idx = msg.find(needle)?;
            let mut i = idx + needle.len();

            // Parse decimal offset.
            let mut off_end = i;
            for (j, ch) in msg[i..].char_indices() {
                if !ch.is_ascii_digit() {
                    break;
                }
                off_end = i + j + ch.len_utf8();
            }
            if off_end == i {
                return None;
            }
            let off = msg[i..off_end].parse::<usize>().ok()?;
            i = off_end;

            // Optional: " len <n>"
            let needle_len = " len ";
            if let Some(len_idx) = msg[i..].find(needle_len) {
                let start = i + len_idx + needle_len.len();
                let mut end = start;
                for (j, ch) in msg[start..].char_indices() {
                    if !ch.is_ascii_digit() {
                        break;
                    }
                    end = start + j + ch.len_utf8();
                }
                if end > start {
                    let len = msg[start..end].parse::<usize>().ok()?;
                    return Some((off, len));
                }
            }

            Some((off, 1))
        }

        let range = range_from_source_span(text, e.span);

        let related_information = consumed_span_from_message(&e.message).map(|(off, len)| {
            let off = off.min(text.len());
            let end_off = if len == 0 {
                (off + 1).min(text.len())
            } else {
                off.saturating_add(len).min(text.len())
            };

            let start = position_from_offset(text, off);
            let end = position_from_offset(text, end_off);
            vec![DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range: Range { start, end },
                },
                message: "consumed here".to_string(),
            }]
        });

        return Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(DIAG_SEMA_ERROR.to_string())),
            code_description: diagnostic_code_href(DIAG_SEMA_ERROR).map(|href| CodeDescription { href }),
            source: Some("aura".to_string()),
            message: format!("semantic error: {}", e.message),
            related_information,
            tags: None,
            data: Some(json!({
                "stable_code": DIAG_SEMA_ERROR,
                "message": e.message,
            })),
        };
    }

    // Fallback: mark entire document.
    let range = Range {
        start: Position { line: 0, character: 0 },
        end: position_from_offset(text, text.len()),
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(DIAG_INTERNAL_ERROR.to_string())),
        code_description: diagnostic_code_href(DIAG_INTERNAL_ERROR).map(|href| CodeDescription { href }),
        source: Some("aura".to_string()),
        message: report.to_string(),
        related_information: None,
        tags: None,
        data: Some(json!({
            "stable_code": DIAG_INTERNAL_ERROR,
            "report": report.to_string(),
        })),
    }
}

fn is_valid_ident_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else { return false };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn file_uri_from_path(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}

fn list_aura_files(root: &Path) -> Vec<PathBuf> {
    fn should_skip_dir(name: &str) -> bool {
        matches!(
            name,
            "target" | "target-alt" | "target-lsp-test" | "dist" | "node_modules" | ".git" | "vendor"
        )
    }

    let mut out: Vec<PathBuf> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let rd = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if should_skip_dir(name) {
                        continue;
                    }
                }
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("aura") {
                out.push(path);
            }
        }
    }

    out.sort();
    out
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DefKey {
    uri: Url,
    name: String,
    span: SourceSpan,
    kind: &'static str,
}

#[derive(Clone, Debug)]
struct DefInfo {
    key: DefKey,
    range: Range,
}

#[derive(Clone, Debug)]
struct RefInfo {
    key: DefKey,
    range: Range,
}

fn expr_callee_name(expr: &aura_ast::Expr) -> Option<String> {
    use aura_ast::ExprKind;
    match &expr.kind {
        ExprKind::Ident(id) => Some(id.node.clone()),
        ExprKind::Member { base, member } => {
            if let ExprKind::Ident(b) = &base.kind {
                Some(format!("{}.{}", b.node, member.node))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn collect_file_symbols(uri: &Url, text: &str) -> (Vec<DefInfo>, Vec<RefInfo>) {
    let program = match aura_parse::parse_source(text) {
        Ok(p) => p,
        Err(_) => return (vec![], vec![]),
    };

    let mut defs: Vec<DefInfo> = Vec::new();
    let mut refs: Vec<RefInfo> = Vec::new();

    // First, collect top-level defs so cross-file lookup works.
    for stmt in &program.stmts {
        match stmt {
            aura_ast::Stmt::CellDef(c) => {
                let key = DefKey {
                    uri: uri.clone(),
                    name: c.name.node.clone(),
                    span: c.name.span,
                    kind: "cell",
                };
                defs.push(DefInfo {
                    range: range_from_source_span(text, c.name.span),
                    key,
                });
            }
            aura_ast::Stmt::ExternCell(c) => {
                let key = DefKey {
                    uri: uri.clone(),
                    name: c.name.node.clone(),
                    span: c.name.span,
                    kind: "extern_cell",
                };
                defs.push(DefInfo {
                    range: range_from_source_span(text, c.name.span),
                    key,
                });
            }
            aura_ast::Stmt::TypeAlias(t) => {
                let key = DefKey {
                    uri: uri.clone(),
                    name: t.name.node.clone(),
                    span: t.name.span,
                    kind: "type",
                };
                defs.push(DefInfo {
                    range: range_from_source_span(text, t.name.span),
                    key,
                });
            }
            aura_ast::Stmt::TraitDef(t) => {
                let key = DefKey {
                    uri: uri.clone(),
                    name: t.name.node.clone(),
                    span: t.name.span,
                    kind: "trait",
                };
                defs.push(DefInfo {
                    range: range_from_source_span(text, t.name.span),
                    key,
                });
            }
            aura_ast::Stmt::RecordDef(r) => {
                let key = DefKey {
                    uri: uri.clone(),
                    name: r.name.node.clone(),
                    span: r.name.span,
                    kind: "record",
                };
                defs.push(DefInfo {
                    range: range_from_source_span(text, r.name.span),
                    key,
                });
            }
            aura_ast::Stmt::EnumDef(e) => {
                let key = DefKey {
                    uri: uri.clone(),
                    name: e.name.node.clone(),
                    span: e.name.span,
                    kind: "enum",
                };
                defs.push(DefInfo {
                    range: range_from_source_span(text, e.name.span),
                    key,
                });
            }
            _ => {}
        }
    }

    // Scope-aware pass for locals + refs.
    let mut scopes: Vec<HashMap<String, DefKey>> = vec![HashMap::new()];
    let mut global_by_name: HashMap<String, Vec<DefKey>> = HashMap::new();
    for d in &defs {
        global_by_name
            .entry(d.key.name.clone())
            .or_default()
            .push(d.key.clone());
    }

    fn resolve_name(
        scopes: &[HashMap<String, DefKey>],
        globals: &HashMap<String, Vec<DefKey>>,
        name: &str,
    ) -> Option<DefKey> {
        for scope in scopes.iter().rev() {
            if let Some(k) = scope.get(name) {
                return Some(k.clone());
            }
        }
        globals.get(name).and_then(|v| v.first()).cloned()
    }

    fn add_def(
        defs: &mut Vec<DefInfo>,
        scopes: &mut Vec<HashMap<String, DefKey>>, 
        uri: &Url,
        text: &str,
        name: &aura_ast::Ident,
        kind: &'static str,
    ) {
        let key = DefKey {
            uri: uri.clone(),
            name: name.node.clone(),
            span: name.span,
            kind,
        };
        scopes
            .last_mut()
            .expect("scope")
            .insert(name.node.clone(), key.clone());
        defs.push(DefInfo {
            range: range_from_source_span(text, name.span),
            key,
        });
    }

    fn walk_expr(
        refs: &mut Vec<RefInfo>,
        scopes: &[HashMap<String, DefKey>],
        globals: &HashMap<String, Vec<DefKey>>,
        uri: &Url,
        text: &str,
        expr: &aura_ast::Expr,
    ) {
        use aura_ast::ExprKind;
        match &expr.kind {
            ExprKind::Ident(id) => {
                if let Some(key) = resolve_name(scopes, globals, &id.node) {
                    refs.push(RefInfo {
                        key,
                        range: range_from_source_span(text, id.span),
                    });
                }
            }
            ExprKind::Unary { expr: inner, .. } => walk_expr(refs, scopes, globals, uri, text, inner),
            ExprKind::Binary { left, right, .. } => {
                walk_expr(refs, scopes, globals, uri, text, left);
                walk_expr(refs, scopes, globals, uri, text, right);
            }
            ExprKind::Member { base, .. } => {
                walk_expr(refs, scopes, globals, uri, text, base);
            }
            ExprKind::Call { callee, args, trailing } => {
                walk_expr(refs, scopes, globals, uri, text, callee);
                for a in args {
                    match a {
                        aura_ast::CallArg::Positional(e) => walk_expr(refs, scopes, globals, uri, text, e),
                        aura_ast::CallArg::Named { value, .. } => walk_expr(refs, scopes, globals, uri, text, value),
                    }
                }
                if let Some(b) = trailing {
                    // trailing block has its own scope
                    let mut scopes2 = scopes.to_vec();
                    scopes2.push(HashMap::new());
                    walk_block(refs, &mut Vec::new(), &mut scopes2, globals, uri, text, b);
                }
            }
            ExprKind::Lambda { body, .. } => {
                let mut scopes2 = scopes.to_vec();
                scopes2.push(HashMap::new());
                walk_block(refs, &mut Vec::new(), &mut scopes2, globals, uri, text, body);
            }
            ExprKind::Flow { left, right, .. } => {
                walk_expr(refs, scopes, globals, uri, text, left);
                walk_expr(refs, scopes, globals, uri, text, right);
            }
            ExprKind::StyleLit { fields } => {
                for (_, v) in fields {
                    walk_expr(refs, scopes, globals, uri, text, v);
                }
            }
            ExprKind::RecordLit { fields, .. } => {
                for (_, v) in fields {
                    walk_expr(refs, scopes, globals, uri, text, v);
                }
            }
            ExprKind::ForAll { binders, body } | ExprKind::Exists { binders, body } => {
                // Quantifier binders introduce a nested scope.
                let mut scopes2 = scopes.to_vec();
                let mut qscope: HashMap<String, DefKey> = HashMap::new();
                for b in binders {
                    qscope.insert(
                        b.name.node.clone(),
                        DefKey {
                            uri: uri.clone(),
                            name: b.name.node.clone(),
                            span: b.name.span,
                            kind: "quant",
                        },
                    );
                }
                scopes2.push(qscope);
                walk_expr(refs, &scopes2, globals, uri, text, body);
            }
            ExprKind::IntLit(_) | ExprKind::StringLit(_) => {}
        }
    }

    fn walk_block(
        refs: &mut Vec<RefInfo>,
        defs: &mut Vec<DefInfo>,
        scopes: &mut Vec<HashMap<String, DefKey>>,
        globals: &HashMap<String, Vec<DefKey>>,
        uri: &Url,
        text: &str,
        block: &aura_ast::Block,
    ) {
        scopes.push(HashMap::new());
        for s in &block.stmts {
            walk_stmt(refs, defs, scopes, globals, uri, text, s);
        }
        if let Some(y) = &block.yield_expr {
            walk_expr(refs, scopes, globals, uri, text, y);
        }
        scopes.pop();
    }

    fn walk_stmt(
        refs: &mut Vec<RefInfo>,
        defs: &mut Vec<DefInfo>,
        scopes: &mut Vec<HashMap<String, DefKey>>,
        globals: &HashMap<String, Vec<DefKey>>,
        uri: &Url,
        text: &str,
        stmt: &aura_ast::Stmt,
    ) {
        match stmt {
            aura_ast::Stmt::MacroDef(m) => {
                // Treat macro name as a def.
                add_def(defs, scopes, uri, text, &m.name, "macro");

                // Params are binders for the macro body.
                scopes.push(HashMap::new());
                for p in &m.params {
                    add_def(defs, scopes, uri, text, p, "param");
                }
                walk_block(refs, defs, scopes, globals, uri, text, &m.body);
                scopes.pop();
            }
            aura_ast::Stmt::MacroCall(m) => {
                if let Some(key) = resolve_name(scopes, globals, &m.name.node) {
                    refs.push(RefInfo {
                        key,
                        range: range_from_source_span(text, m.name.span),
                    });
                }
                for a in &m.args {
                    walk_expr(refs, scopes, globals, uri, text, a);
                }
            }
            aura_ast::Stmt::StrandDef(sd) => {
                // RHS references resolve against current scope.
                walk_expr(refs, scopes, globals, uri, text, &sd.expr);
                // Then define name for subsequent statements.
                add_def(defs, scopes, uri, text, &sd.name, "val");
            }
            aura_ast::Stmt::Assign(a) => {
                // Assign target is a reference.
                if let Some(key) = resolve_name(scopes, globals, &a.target.node) {
                    refs.push(RefInfo {
                        key,
                        range: range_from_source_span(text, a.target.span),
                    });
                }
                walk_expr(refs, scopes, globals, uri, text, &a.expr);
            }
            aura_ast::Stmt::Prop(p) => {
                walk_expr(refs, scopes, globals, uri, text, &p.expr);
            }
            aura_ast::Stmt::ExprStmt(e) => walk_expr(refs, scopes, globals, uri, text, e),
            aura_ast::Stmt::Requires(r) => walk_expr(refs, scopes, globals, uri, text, &r.expr),
            aura_ast::Stmt::Ensures(e) => walk_expr(refs, scopes, globals, uri, text, &e.expr),
            aura_ast::Stmt::Assert(a) => walk_expr(refs, scopes, globals, uri, text, &a.expr),
            aura_ast::Stmt::Assume(a) => walk_expr(refs, scopes, globals, uri, text, &a.expr),
            aura_ast::Stmt::If(i) => {
                walk_expr(refs, scopes, globals, uri, text, &i.cond);
                walk_block(refs, defs, scopes, globals, uri, text, &i.then_block);
                if let Some(b) = &i.else_block {
                    walk_block(refs, defs, scopes, globals, uri, text, b);
                }
            }
            aura_ast::Stmt::Match(m) => {
                walk_expr(refs, scopes, globals, uri, text, &m.scrutinee);
                for arm in &m.arms {
                    walk_block(refs, defs, scopes, globals, uri, text, &arm.body);
                }
            }
            aura_ast::Stmt::While(w) => {
                walk_expr(refs, scopes, globals, uri, text, &w.cond);
                if let Some(inv) = &w.invariant {
                    walk_expr(refs, scopes, globals, uri, text, inv);
                }
                if let Some(dec) = &w.decreases {
                    walk_expr(refs, scopes, globals, uri, text, dec);
                }
                walk_block(refs, defs, scopes, globals, uri, text, &w.body);
            }
            aura_ast::Stmt::CellDef(c) => {
                // Function name def already added as global; treat it as a def occurrence too.
                let key = DefKey {
                    uri: uri.clone(),
                    name: c.name.node.clone(),
                    span: c.name.span,
                    kind: "cell",
                };
                refs.push(RefInfo {
                    key: key.clone(),
                    range: range_from_source_span(text, c.name.span),
                });

                scopes.push(HashMap::new());
                for p in &c.params {
                    add_def(defs, scopes, uri, text, &p.name, "param");
                }
                walk_block(refs, defs, scopes, globals, uri, text, &c.body);
                scopes.pop();
            }
            aura_ast::Stmt::ExternCell(c) => {
                let key = DefKey {
                    uri: uri.clone(),
                    name: c.name.node.clone(),
                    span: c.name.span,
                    kind: "extern_cell",
                };
                refs.push(RefInfo {
                    key,
                    range: range_from_source_span(text, c.name.span),
                });
            }
            aura_ast::Stmt::TypeAlias(t) => {
                let key = DefKey {
                    uri: uri.clone(),
                    name: t.name.node.clone(),
                    span: t.name.span,
                    kind: "type",
                };
                refs.push(RefInfo {
                    key,
                    range: range_from_source_span(text, t.name.span),
                });
            }
            aura_ast::Stmt::TraitDef(t) => add_def(defs, scopes, uri, text, &t.name, "trait"),
            aura_ast::Stmt::RecordDef(r) => add_def(defs, scopes, uri, text, &r.name, "record"),
            aura_ast::Stmt::EnumDef(e) => add_def(defs, scopes, uri, text, &e.name, "enum"),
            aura_ast::Stmt::FlowBlock(fb) => {
                // Treat flow block name as a def in current scope.
                add_def(defs, scopes, uri, text, &fb.name, "flow");
                walk_block(refs, defs, scopes, globals, uri, text, &fb.body);
            }
            aura_ast::Stmt::UnsafeBlock(ub) => {
                walk_block(refs, defs, scopes, globals, uri, text, &ub.body);
            }
            aura_ast::Stmt::Layout(lb) => walk_block(refs, defs, scopes, globals, uri, text, &lb.body),
            aura_ast::Stmt::Render(rb) => walk_block(refs, defs, scopes, globals, uri, text, &rb.body),
            aura_ast::Stmt::Import(_) => {}
        }
    }

    for stmt in &program.stmts {
        walk_stmt(&mut refs, &mut defs, &mut scopes, &global_by_name, uri, text, stmt);
    }

    (defs, refs)
}

type SolverThreadResult<T> = std::result::Result<T, String>;

#[derive(Clone)]
struct SolverWorker {
    tx: mpsc::UnboundedSender<SolverJob>,
}

enum SolverJob {
    ComputeDiagnostics {
        uri: Url,
        text: String,
        resp: oneshot::Sender<SolverThreadResult<Vec<Diagnostic>>>,
    },

    VerifyIncremental {
        uri: Url,
        text: String,
        program: aura_ast::Program,
        dep_hash_for_stmt: String,
        manifest: Option<PathBuf>,
        manifest_plugins: Vec<PluginManifest>,
        requested_plugins: Vec<String>,
        base_key: String,
        file_hash: String,
        profile_for_verify: String,
        cache_enabled: bool,
        scope_is_affected: bool,
        affected_offsets: Vec<(usize, usize)>,
        cache_snapshot: ProofCacheEntry,
        resp: oneshot::Sender<SolverThreadResult<(Vec<Diagnostic>, ProofCacheEntry, String, String, u64, u64, Option<bool>)>>,
    },
}

impl SolverWorker {
    fn spawn() -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<SolverJob>();

        std::thread::Builder::new()
            .name("aura-z3-worker".to_string())
            .spawn(move || {
                let mut prover = aura_verify::Z3Prover::new();

                while let Some(job) = rx.blocking_recv() {
                    match job {
                        SolverJob::ComputeDiagnostics { uri, text, resp } => {
                            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                compute_diagnostics(&uri, &text, &mut prover)
                            }))
                            .map_err(|panic| {
                                if let Some(s) = panic.downcast_ref::<&str>() {
                                    format!("aura-z3-worker panic: {s}")
                                } else if let Some(s) = panic.downcast_ref::<String>() {
                                    format!("aura-z3-worker panic: {s}")
                                } else {
                                    "aura-z3-worker panic".to_string()
                                }
                            });

                            let _ = resp.send(res);
                        }

                        SolverJob::VerifyIncremental {
                            uri,
                            text,
                            program,
                            dep_hash_for_stmt,
                            manifest,
                            manifest_plugins,
                            requested_plugins,
                            base_key,
                            file_hash,
                            profile_for_verify,
                            cache_enabled,
                            scope_is_affected,
                            affected_offsets,
                            cache_snapshot,
                            resp,
                        } => {
                            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                run_incremental_verify_stage(
                                    &uri,
                                    &text,
                                    &program,
                                    &dep_hash_for_stmt,
                                    manifest.as_ref(),
                                    &manifest_plugins,
                                    &requested_plugins,
                                    &base_key,
                                    &file_hash,
                                    &profile_for_verify,
                                    cache_enabled,
                                    scope_is_affected,
                                    &affected_offsets,
                                    cache_snapshot,
                                    &mut prover,
                                )
                            }))
                            .map_err(|panic| {
                                if let Some(s) = panic.downcast_ref::<&str>() {
                                    format!("aura-z3-worker panic: {s}")
                                } else if let Some(s) = panic.downcast_ref::<String>() {
                                    format!("aura-z3-worker panic: {s}")
                                } else {
                                    "aura-z3-worker panic".to_string()
                                }
                            });

                            let _ = resp.send(res);
                        }
                    }
                }
            })
            .expect("spawn aura-z3-worker thread");

        Self { tx }
    }

    async fn compute_diagnostics(&self, uri: Url, text: String) -> SolverThreadResult<Vec<Diagnostic>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(SolverJob::ComputeDiagnostics {
                uri,
                text,
                resp: resp_tx,
            })
            .map_err(|_| "aura-z3-worker channel closed".to_string())?;

        resp_rx
            .await
            .map_err(|_| "aura-z3-worker dropped response".to_string())?
    }

    async fn verify_incremental(
        &self,
        uri: Url,
        text: String,
        program: aura_ast::Program,
        dep_hash_for_stmt: String,
        manifest: Option<PathBuf>,
        manifest_plugins: Vec<PluginManifest>,
        requested_plugins: Vec<String>,
        base_key: String,
        file_hash: String,
        profile_for_verify: String,
        cache_enabled: bool,
        scope_is_affected: bool,
        affected_offsets: Vec<(usize, usize)>,
        cache_snapshot: ProofCacheEntry,
    ) -> SolverThreadResult<(Vec<Diagnostic>, ProofCacheEntry, String, String, u64, u64, Option<bool>)> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(SolverJob::VerifyIncremental {
                uri,
                text,
                program,
                dep_hash_for_stmt,
                manifest,
                manifest_plugins,
                requested_plugins,
                base_key,
                file_hash,
                profile_for_verify,
                cache_enabled,
                scope_is_affected,
                affected_offsets,
                cache_snapshot,
                resp: resp_tx,
            })
            .map_err(|_| "aura-z3-worker channel closed".to_string())?;

        resp_rx
            .await
            .map_err(|_| "aura-z3-worker dropped response".to_string())?
    }
}

struct Backend {
    client: Client,
    docs: RwLock<HashMap<Url, String>>,
    proofs_next_id: AtomicU64,
    proofs_tasks: Arc<Mutex<HashMap<u64, (Url, tokio::task::JoinHandle<()>)>>>,
    proof_cache: Arc<RwLock<HashMap<String, ProofCacheEntry>>>,
    workspace_root: RwLock<Option<PathBuf>>,
    aura_client_caps: RwLock<AuraClientCaps>,
    solver: SolverWorker,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct ProofCacheEntry {
    // Full diagnostics keyed by file hash.
    full_by_file: HashMap<String, Vec<Diagnostic>>,
    // Per-top-level-unit diagnostics keyed by unit hash.
    stmt_diags: HashMap<String, Vec<Diagnostic>>,
    // Cached diagnostics for Lumina geometry/aesthetics, keyed by a UI hash.
    ui_by_hash: HashMap<String, Vec<Diagnostic>>,
}

fn run_incremental_verify_stage(
    uri: &Url,
    text: &str,
    program: &aura_ast::Program,
    dep_hash_for_stmt: &str,
    manifest: Option<&PathBuf>,
    manifest_plugins: &[PluginManifest],
    requested_plugins: &[String],
    base_key: &str,
    file_hash: &str,
    profile_for_verify: &str,
    cache_enabled: bool,
    scope_is_affected: bool,
    affected_offsets: &[(usize, usize)],
    cache_snapshot: ProofCacheEntry,
    prover: &mut aura_verify::Z3Prover,
) -> (Vec<Diagnostic>, ProofCacheEntry, String, String, u64, u64, Option<bool>) {
    // Incremental: verify per top-level unit and reuse cached unit results.
    let mut cache_entry = cache_snapshot;

    let mut local_stmt_cache_hits: u64 = 0;
    let mut local_stmt_cache_misses: u64 = 0;
    let mut local_ui_cache_hit: Option<bool> = None;

    let mut diags: Vec<Diagnostic> = Vec::new();
    let source_path = uri.to_file_path().ok();

    let mut nexus = aura_nexus::NexusContext::default();
    nexus.insert(NexusFileContext {
        source_path,
        manifest_path: manifest.map(|p| p.to_path_buf()),
    });

    // Always run the fast non-Z3 verifier pass (range alias checks).
    if let Err(err) = aura_verify::verify_program(program, prover) {
        diags.push(diagnostic_from_verify_error(uri, text, err));
        return (
            diags,
            cache_entry,
            base_key.to_string(),
            file_hash.to_string(),
            local_stmt_cache_hits,
            local_stmt_cache_misses,
            local_ui_cache_hit,
        );
    }

    let mut live_stmt_hashes: Vec<String> = Vec::new();
    let mut ui_hash_input = String::new();

    // Compute stable statement hashes that incorporate intra-file call dependencies.
    let stmt_hashes = compute_checkable_stmt_merkle_hashes(program, text, dep_hash_for_stmt);

    let overlaps_affected = |stmt_start: usize, stmt_end: usize| -> bool {
        if affected_offsets.is_empty() {
            return true;
        }
        affected_offsets
            .iter()
            .any(|(a, b)| stmt_start < *b && *a < stmt_end)
    };

    for (stmt_idx, stmt) in program.stmts.iter().enumerate() {
        let (is_checkable, is_ui) = match stmt {
            aura_ast::Stmt::CellDef(_) => (true, true),
            aura_ast::Stmt::FlowBlock(_) => (true, false),
            aura_ast::Stmt::Layout(_) => (true, true),
            aura_ast::Stmt::Render(_) => (true, true),
            _ => (false, false),
        };
        if !is_checkable {
            continue;
        }

        let sp = match stmt {
            aura_ast::Stmt::CellDef(s) => s.span,
            aura_ast::Stmt::FlowBlock(s) => s.span,
            aura_ast::Stmt::Layout(s) => s.span,
            aura_ast::Stmt::Render(s) => s.span,
            _ => continue,
        };
        let start = sp.offset();
        let end = start.saturating_add(sp.len());

        if scope_is_affected && !overlaps_affected(start, end) {
            continue;
        }

        let stmt_hash = stmt_hashes
            .get(stmt_idx)
            .and_then(|x| x.as_ref())
            .map(|x| x.hash.clone())
            .unwrap_or_else(|| {
                let slice = text.get(start..end).unwrap_or("");
                let kind = match stmt {
                    aura_ast::Stmt::CellDef(_) => "cell",
                    aura_ast::Stmt::FlowBlock(_) => "flow",
                    aura_ast::Stmt::Layout(_) => "layout",
                    aura_ast::Stmt::Render(_) => "render",
                    _ => "stmt",
                };
                sha256_hex(&format!("{}:{}", kind, slice))
            });
        live_stmt_hashes.push(stmt_hash.clone());
        if is_ui {
            ui_hash_input.push_str(&stmt_hash);
            ui_hash_input.push('\n');
        }

        if cache_enabled {
            if let Some(cached) = cache_entry.stmt_diags.get(&stmt_hash).cloned() {
                local_stmt_cache_hits += 1;
                diags.extend(cached);
                continue;
            }
        }

        local_stmt_cache_misses += 1;

        let stmt_diags = match requested_plugins {
            xs if xs == ["aura-ai"] => {
                let plugins = (aura_plugin_ai::AuraAiPlugin::new(),);
                let mut out: Vec<Diagnostic> = Vec::new();
                match aura_verify::verify::verify_stmt_z3(stmt, prover, &plugins, &mut nexus) {
                    Ok(()) => {}
                    Err(err) => out.push(diagnostic_from_verify_error(uri, text, err)),
                }
                let proofs = aura_nexus::drain_proofs(&mut nexus);
                for p in proofs {
                    out.push(diagnostic_from_proof_note(
                        uri,
                        text,
                        &p,
                        manifest.map(|p| p.as_path()),
                        manifest_plugins,
                    ));
                }
                out
            }
            xs if xs == ["aura-iot"] => {
                let plugins = (aura_plugin_iot::AuraIotPlugin::new(),);
                let mut out: Vec<Diagnostic> = Vec::new();
                match aura_verify::verify::verify_stmt_z3(stmt, prover, &plugins, &mut nexus) {
                    Ok(()) => {}
                    Err(err) => out.push(diagnostic_from_verify_error(uri, text, err)),
                }
                let proofs = aura_nexus::drain_proofs(&mut nexus);
                for p in proofs {
                    out.push(diagnostic_from_proof_note(
                        uri,
                        text,
                        &p,
                        manifest.map(|p| p.as_path()),
                        manifest_plugins,
                    ));
                }
                out
            }
            xs
                if xs.len() == 2
                    && ((xs[0] == "aura-ai" && xs[1] == "aura-iot")
                        || (xs[0] == "aura-iot" && xs[1] == "aura-ai")) =>
            {
                let plugins = (
                    aura_plugin_iot::AuraIotPlugin::new(),
                    aura_plugin_ai::AuraAiPlugin::new(),
                );
                let mut out: Vec<Diagnostic> = Vec::new();
                match aura_verify::verify::verify_stmt_z3(stmt, prover, &plugins, &mut nexus) {
                    Ok(()) => {}
                    Err(err) => out.push(diagnostic_from_verify_error(uri, text, err)),
                }
                let proofs = aura_nexus::drain_proofs(&mut nexus);
                for p in proofs {
                    out.push(diagnostic_from_proof_note(
                        uri,
                        text,
                        &p,
                        manifest.map(|p| p.as_path()),
                        manifest_plugins,
                    ));
                }
                out
            }
            other => vec![diagnostic_from_verify_error(
                uri,
                text,
                aura_verify::VerifyError {
                    message: format!(
                        "unsupported Nexus plugin set: {:?}. Supported built-ins: ['aura-iot', 'aura-ai']",
                        other
                    ),
                    span: SourceSpan::new(SourceOffset::from(0usize), 0usize),
                    model: None,
                    meta: None,
                },
            )],
        };

        if cache_enabled {
            cache_entry
                .stmt_diags
                .insert(stmt_hash.clone(), stmt_diags.clone());
        }
        diags.extend(stmt_diags);
    }

    // Evict stmt cache entries that no longer exist in the current program.
    if cache_enabled {
        let live: std::collections::BTreeSet<String> = live_stmt_hashes.into_iter().collect();
        cache_entry.stmt_diags.retain(|k, _| live.contains(k));
    }

    // Cache Lumina geometry/aesthetics by a UI hash.
    // In `fast` profile, skip these heavier whole-program checks.
    if profile_for_verify != "fast" {
        let ui_hash = sha256_hex(&ui_hash_input);
        if cache_enabled {
            if let Some(cached) = cache_entry.ui_by_hash.get(&ui_hash).cloned() {
                local_ui_cache_hit = Some(true);
                diags.extend(cached);
            } else {
                local_ui_cache_hit = Some(false);
                let mut ui_diags: Vec<Diagnostic> = Vec::new();
                if let Err(err) =
                    aura_verify::geometry::verify_lumina_geometry(program, prover.ctx(), &mut nexus)
                {
                    ui_diags.push(diagnostic_from_verify_error(uri, text, err));
                }
                if let Err(err) =
                    aura_verify::geometry::verify_lumina_aesthetics(program, prover.ctx(), &mut nexus)
                {
                    ui_diags.push(diagnostic_from_verify_error(uri, text, err));
                }
                cache_entry.ui_by_hash.insert(ui_hash, ui_diags.clone());
                diags.extend(ui_diags);
            }
        } else {
            let mut ui_diags: Vec<Diagnostic> = Vec::new();
            if let Err(err) =
                aura_verify::geometry::verify_lumina_geometry(program, prover.ctx(), &mut nexus)
            {
                ui_diags.push(diagnostic_from_verify_error(uri, text, err));
            }
            if let Err(err) =
                aura_verify::geometry::verify_lumina_aesthetics(program, prover.ctx(), &mut nexus)
            {
                ui_diags.push(diagnostic_from_verify_error(uri, text, err));
            }
            diags.extend(ui_diags);
        }
    }

    (
        diags,
        cache_entry,
        base_key.to_string(),
        file_hash.to_string(),
        local_stmt_cache_hits,
        local_stmt_cache_misses,
        local_ui_cache_hit,
    )
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            docs: RwLock::new(HashMap::new()),
            proofs_next_id: AtomicU64::new(1),
            proofs_tasks: Arc::new(Mutex::new(HashMap::new())),
            proof_cache: Arc::new(RwLock::new(HashMap::new())),
            workspace_root: RwLock::new(None),
            aura_client_caps: RwLock::new(AuraClientCaps::default()),
            solver: SolverWorker::spawn(),
        }
    }

    async fn aura_caps_snapshot(&self) -> AuraClientCaps {
        self.aura_client_caps.read().await.clone()
    }

    async fn should_send_phase(&self, phase: &str) -> bool {
        let caps = self.aura_caps_snapshot().await;
        match &caps.phases {
            None => true,
            Some(phases) => phases.iter().any(|p| p == phase),
        }
    }

    async fn workspace_root_for(&self, uri: &Url) -> Option<PathBuf> {
        if let Some(p) = find_aura_toml_for_uri(uri) {
            return p.parent().map(|x| x.to_path_buf());
        }
        self.workspace_root.read().await.clone().or_else(|| {
            uri.to_file_path()
                .ok()
                .and_then(|p| p.parent().map(|x| x.to_path_buf()))
        })
    }

    async fn load_persistent_proof_cache(&self) {
        if !proof_cache_persist_enabled() {
            return;
        }
        let root = self.workspace_root.read().await.clone();
        let Some(root) = root else {
            return;
        };
        let path = proof_cache_path_for_root(&root);

        if proof_cache_clear_on_start() {
            let _ = fs::remove_file(&path);
            return;
        }

        // Best-effort: never fail LSP init on cache I/O.
        let data = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return,
        };
        let store: ProofCacheStoreV1 = match serde_json::from_str(&data) {
            Ok(s) => s,
            Err(_) => return,
        };
        if store.version != PROOF_CACHE_STORE_VERSION {
            return;
        }

        let mut cache = self.proof_cache.write().await;
        *cache = store.entries;
    }

    fn persist_proof_cache_update(cache_path: Option<PathBuf>, base_key: String, entry: ProofCacheEntry) {
        if !proof_cache_persist_enabled() {
            return;
        }
        let Some(path) = cache_path else {
            return;
        };
        let Some(parent) = path.parent() else {
            return;
        };
        let _ = fs::create_dir_all(parent);

        let mut store: ProofCacheStoreV1 = match fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<ProofCacheStoreV1>(&s).ok())
        {
            Some(s) if s.version == PROOF_CACHE_STORE_VERSION => s,
            _ => ProofCacheStoreV1 {
                version: PROOF_CACHE_STORE_VERSION,
                entries: HashMap::new(),
            },
        };

        store.entries.insert(base_key.clone(), entry);

        // Bound growth (best-effort). Preserve the just-written base_key.
        let max_entries = proof_cache_store_max_entries().max(1);
        if store.entries.len() > max_entries {
            let mut keys: Vec<String> = store.entries.keys().cloned().collect();
            keys.sort();
            for k in keys {
                if store.entries.len() <= max_entries {
                    break;
                }
                if k == base_key {
                    continue;
                }
                store.entries.remove(&k);
            }
        }

        let Ok(json) = serde_json::to_string(&store) else {
            return;
        };

        // Atomic-ish write: write temp then rename.
        let tmp = path.with_file_name(format!("{}.tmp.{}", PROOF_CACHE_FILE_NAME, std::process::id()));
        if fs::write(&tmp, json).is_ok() {
            let _ = fs::remove_file(&path);
            let _ = fs::rename(&tmp, &path);
            let _ = fs::remove_file(&tmp);
        }
    }

    async fn emit_proofs_stream(&self, ev: ProofsStreamEvent) {
        let _ = self
            .client
            .send_notification::<AuraProofsStreamNotification>(ev)
            .await;
    }

    async fn get_text_for_uri(&self, uri: &Url) -> String {
        // Prefer in-memory text (open editor), fall back to reading from disk.
        {
            let docs = self.docs.read().await;
            if let Some(t) = docs.get(uri) {
                return t.clone();
            }
        }

        uri.to_file_path()
            .ok()
            .and_then(|p| fs::read_to_string(p).ok())
            .unwrap_or_default()
    }

    async fn proofs(&self, params: ProofsParams) -> Result<ProofsResponse> {
        let uri = params.uri;

        let text = self.get_text_for_uri(&uri).await;

        let diagnostics = match self.solver.compute_diagnostics(uri.clone(), text.clone()).await {
            Ok(d) => d,
            Err(msg) => vec![Diagnostic {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: position_from_offset(&text, text.len()),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("aura::lsp".to_string())),
                code_description: None,
                source: Some("aura".to_string()),
                message: msg,
                related_information: None,
                tags: None,
                data: None,
            }],
        };

        Ok(ProofsResponse { uri, diagnostics })
    }

    async fn proofs_stream_start(&self, params: ProofsStreamStartParams) -> Result<ProofsStreamStartResponse> {
        let uri = params.uri;
        let profile = params.profile.unwrap_or_else(|| "thorough".to_string());
        let scope = params.scope.unwrap_or_else(|| "full".to_string());
        let scope_is_affected = scope == "affected";
        let affected_ranges = params.ranges.unwrap_or_default();
        let text = self.get_text_for_uri(&uri).await;

        let workspace_root = self.workspace_root_for(&uri).await;
        let cache_path = workspace_root.as_deref().map(proof_cache_path_for_root);
        let file_id = file_id_for_cache(workspace_root.as_deref(), &uri);
        let dep_hash = dep_hash_from_imports(workspace_root.as_deref(), &text);

        let id = self.proofs_next_id.fetch_add(1, Ordering::Relaxed);
        let uri2 = uri.clone();
        let client2 = self.client.clone();
        let tasks = Arc::clone(&self.proofs_tasks);
        let proof_cache = Arc::clone(&self.proof_cache);
        let solver = self.solver.clone();

        let aura_caps = self.aura_caps_snapshot().await;
        let telemetry_enabled = aura_caps.telemetry.proof_timings || aura_caps.telemetry.proof_cache;

        // Immediately notify start.
        self.emit_proofs_stream(ProofsStreamEvent {
            id,
            uri: uri.clone(),
            state: "start".to_string(),
            phase: None,
            diagnostics: None,
            telemetry: None,
            error: None,
        })
        .await;

        let handle = tokio::spawn(async move {
            let cache_enabled = proof_cache_enabled();
            let send = |ev: ProofsStreamEvent| async {
                let _ = client2
                    .send_notification::<AuraProofsStreamNotification>(ev)
                    .await;
            };

            let t_start = std::time::Instant::now();
            let mut t_parse_ms: Option<u128> = None;
            let mut t_sema_ms: Option<u128> = None;
            let mut t_normalize_ms: Option<u128> = None;
            let mut t_z3_ms: Option<u128> = None;

            let mut full_cache_hit = false;
            let mut stmt_cache_hits: u64 = 0;
            let mut stmt_cache_misses: u64 = 0;
            let mut ui_cache_hit: Option<bool> = None;

            let augmented = match aura_sdk::augment_source_with_default_std(&text) {
                Ok(t) => t,
                Err(_) => text,
            };

            let affected_offsets: Vec<(usize, usize)> = if scope_is_affected {
                affected_ranges
                    .iter()
                    .map(|r| {
                        let start = offset_from_position(&augmented, r.start);
                        let end = offset_from_position(&augmented, r.end);
                        (start.min(end), start.max(end))
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let affected_offsets_for_verify = affected_offsets.clone();

            // Prepare stable cache keys.
            let manifest = find_aura_toml_for_uri(&uri2);
            let manifest_text = manifest
                .as_deref()
                .and_then(|p| fs::read_to_string(p).ok())
                .unwrap_or_default();
            let manifest_hash = sha256_hex(&manifest_text);

            let manifest_plugins = manifest
                .as_deref()
                .and_then(load_manifest_plugins)
                .unwrap_or_default();

            let requested_plugins = match requested_plugins_from_manifest(&manifest_plugins) {
                Ok(p) => p,
                Err(e) => {
                    let diags = vec![diagnostic_from_verify_error(&uri2, &augmented, e)];
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "done".to_string(),
                        phase: None,
                        diagnostics: Some(diags),
                        telemetry: None,
                        error: None,
                    })
                    .await;
                    let mut map = tasks.lock().await;
                    map.remove(&id);
                    return;
                }
            };

            let solver_config = if profile == "fast" { "z3-fast" } else { "z3-thorough" };
            let base_key = sha256_hex(&format!(
                "file={};manifest_hash={};plugins={:?};solver={}",
                file_id,
                manifest_hash,
                requested_plugins,
                solver_config
            ));

            let file_hash = sha256_hex(&augmented);
            let full_key = sha256_hex(&format!("file={};deps={}", file_hash, dep_hash));

            // Full cache hit: return immediately (only for full runs).
            if cache_enabled && !scope_is_affected {
                if let Some(diags) = {
                    let cache = proof_cache.read().await;
                    cache
                        .get(&base_key)
                        .and_then(|e| e.full_by_file.get(&full_key))
                        .cloned()
                } {
                    full_cache_hit = true;
                    let telemetry = if telemetry_enabled {
                        Some(json!({
                            "profile": profile,
                            "scope": scope,
                            "cache": {
                                "fullHit": true,
                                "stmtHits": 0,
                                "stmtMisses": 0,
                                "uiHit": null,
                            },
                            "timingsMs": {
                                "total": t_start.elapsed().as_millis(),
                            }
                        }))
                    } else {
                        None
                    };
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "done".to_string(),
                        phase: None,
                        diagnostics: Some(diags),
                        telemetry,
                        error: None,
                    })
                    .await;
                    let mut map = tasks.lock().await;
                    map.remove(&id);
                    return;
                }
            }

            if let Some(phase_name) = Some("parse".to_string()) {
                // Respect client-advertised supported phases (best-effort).
                // If client did not advertise phases, we assume all are supported.
                let should_send = match &aura_caps.phases {
                    None => true,
                    Some(phases) => phases.iter().any(|p| p == "parse"),
                };
                if should_send {
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "phase".to_string(),
                        phase: Some(phase_name),
                        diagnostics: None,
                        telemetry: None,
                        error: None,
                    })
                    .await;
                }
            }

            let parse_res = tokio::task::spawn_blocking({
                let t = augmented.clone();
                move || aura_parse::parse_source(&t)
            })
            .await;

            t_parse_ms = Some(t_start.elapsed().as_millis());

            let program = match parse_res {
                Ok(Ok(p)) => p,
                Ok(Err(e)) => {
                    let diags = vec![diagnostic_from_miette(&uri2, &augmented, e)];
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "done".to_string(),
                        phase: None,
                        diagnostics: Some(diags),
                        telemetry: None,
                        error: None,
                    })
                    .await;
                    let mut map = tasks.lock().await;
                    map.remove(&id);
                    return;
                }
                Err(e) => {
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "error".to_string(),
                        phase: None,
                        diagnostics: None,
                        telemetry: None,
                        error: Some(format!("parse task failed: {e}")),
                    })
                    .await;
                    let mut map = tasks.lock().await;
                    map.remove(&id);
                    return;
                }
            };

            {
                let should_send = match &aura_caps.phases {
                    None => true,
                    Some(phases) => phases.iter().any(|p| p == "sema"),
                };
                if should_send {
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "phase".to_string(),
                        phase: Some("sema".to_string()),
                        diagnostics: None,
                        telemetry: None,
                        error: None,
                    })
                    .await;
                }
            }

            let sema_res = tokio::task::spawn_blocking({
                let program = program.clone();
                move || {
                    let mut checker = aura_core::Checker::new();
                    checker.check_program(&program).map_err(miette::Report::from)
                }
            })
            .await;

            t_sema_ms = Some(t_start.elapsed().as_millis());

            match sema_res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    let diags = vec![diagnostic_from_miette(&uri2, &augmented, e)];
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "done".to_string(),
                        phase: None,
                        diagnostics: Some(diags),
                        telemetry: None,
                        error: None,
                    })
                    .await;
                    let mut map = tasks.lock().await;
                    map.remove(&id);
                    return;
                }
                Err(e) => {
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "error".to_string(),
                        phase: None,
                        diagnostics: None,
                        telemetry: None,
                        error: Some(format!("sema task failed: {e}")),
                    })
                    .await;
                    let mut map = tasks.lock().await;
                    map.remove(&id);
                    return;
                }
            }

            {
                let should_send = match &aura_caps.phases {
                    None => true,
                    Some(phases) => phases.iter().any(|p| p == "normalize"),
                };
                if should_send {
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "phase".to_string(),
                        phase: Some("normalize".to_string()),
                        diagnostics: None,
                        telemetry: None,
                        error: None,
                    })
                    .await;
                }
            }

            let normalize_res = tokio::task::spawn_blocking({
                let program = program.clone();
                move || aura_core::lower_program(&program).map(|_| ()).map_err(miette::Report::from)
            })
            .await;

            t_normalize_ms = Some(t_start.elapsed().as_millis());

            match normalize_res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    let diags = vec![diagnostic_from_miette(&uri2, &augmented, e)];
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "done".to_string(),
                        phase: None,
                        diagnostics: Some(diags),
                        telemetry: None,
                        error: None,
                    })
                    .await;
                    let mut map = tasks.lock().await;
                    map.remove(&id);
                    return;
                }
                Err(e) => {
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "error".to_string(),
                        phase: None,
                        diagnostics: None,
                        telemetry: None,
                        error: Some(format!("normalize task failed: {e}")),
                    })
                    .await;
                    let mut map = tasks.lock().await;
                    map.remove(&id);
                    return;
                }
            }

            {
                let should_send = match &aura_caps.phases {
                    None => true,
                    Some(phases) => phases.iter().any(|p| p == "z3"),
                };
                if should_send {
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "phase".to_string(),
                        phase: Some("z3".to_string()),
                        diagnostics: None,
                        telemetry: None,
                        error: None,
                    })
                    .await;
                }
            }

            let cache_snapshot = if cache_enabled {
                let cache = proof_cache.read().await;
                cache.get(&base_key).cloned().unwrap_or_default()
            } else {
                ProofCacheEntry::default()
            };

            let verify_res = solver.verify_incremental(
                    uri2.clone(),
                    augmented.clone(),
                    program,
                    dep_hash.clone(),
                    manifest.clone(),
                    manifest_plugins.clone(),
                    requested_plugins.clone(),
                    base_key.clone(),
                    file_hash.clone(),
                    profile.clone(),
                    cache_enabled,
                    scope_is_affected,
                    affected_offsets_for_verify,
                    cache_snapshot,
                )
                .await;

            t_z3_ms = Some(t_start.elapsed().as_millis());

            match verify_res {
                Ok((diags, mut cache_entry, base_key, file_hash, stmt_hits, stmt_misses, ui_hit)) => {
                    stmt_cache_hits = stmt_hits;
                    stmt_cache_misses = stmt_misses;
                    ui_cache_hit = ui_hit;
                    // Update caches (best-effort). Only full runs populate full-file cache.
                    if cache_enabled && !scope_is_affected {
                        let full_key = sha256_hex(&format!("file={};deps={}", file_hash, dep_hash));
                        cache_entry.full_by_file.insert(full_key, diags.clone());
                        if cache_entry.full_by_file.len() > 8 {
                            // Avoid unbounded growth on repeated edits.
                            cache_entry.full_by_file.clear();
                        }
                    }

                    let base_key_for_persist = base_key.clone();
                    if cache_enabled {
                        let mut cache = proof_cache.write().await;
                        cache.insert(base_key, cache_entry);
                    }

                    // Persist updated cache so IDE restarts stay warm (best-effort).
                    if cache_enabled && proof_cache_persist_enabled() {
                        let cache_path2 = cache_path.clone();
                        let base_key2 = base_key_for_persist;
                        let entry2 = {
                            let cache = proof_cache.read().await;
                            cache.get(&base_key2).cloned()
                        };
                        if let Some(entry2) = entry2 {
                            tokio::task::spawn_blocking(move || {
                                Backend::persist_proof_cache_update(cache_path2, base_key2, entry2);
                            });
                        }
                    }

                    let telemetry = if telemetry_enabled {
                        Some(json!({
                            "profile": profile,
                            "scope": scope,
                            "cache": {
                                "fullHit": full_cache_hit,
                                "stmtHits": stmt_cache_hits,
                                "stmtMisses": stmt_cache_misses,
                                "uiHit": ui_cache_hit,
                            },
                            "timingsMs": {
                                "parse": t_parse_ms,
                                "sema": t_sema_ms,
                                "normalize": t_normalize_ms,
                                "z3": t_z3_ms,
                                "total": t_start.elapsed().as_millis(),
                            }
                        }))
                    } else {
                        None
                    };

                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "done".to_string(),
                        phase: None,
                        diagnostics: Some(diags),
                        telemetry,
                        error: None,
                    })
                    .await;
                }
                Err(e) => {
                    send(ProofsStreamEvent {
                        id,
                        uri: uri2.clone(),
                        state: "error".to_string(),
                        phase: None,
                        diagnostics: None,
                        telemetry: None,
                        error: Some(format!("z3 task failed: {e}")),
                    })
                    .await;
                }
            }

            let mut map = tasks.lock().await;
            map.remove(&id);
        });

        {
            let mut map = self.proofs_tasks.lock().await;
            map.insert(id, (uri.clone(), handle));
        }

        Ok(ProofsStreamStartResponse { id })
    }

    async fn proofs_stream_cancel(&self, params: ProofsStreamCancelParams) -> Result<()> {
        let id = params.id;
        let task = {
            let mut map = self.proofs_tasks.lock().await;
            map.remove(&id)
        };

        let cancelled_uri = if let Some((u, t)) = task {
            t.abort();
            u
        } else {
            Url::parse("untitled:///cancelled.aura").unwrap_or_else(|_| Url::parse("untitled:///cancelled").unwrap())
        };

        self.emit_proofs_stream(ProofsStreamEvent {
            id,
            uri: cancelled_uri,
            state: "cancelled".to_string(),
            phase: None,
            diagnostics: None,
            telemetry: None,
            error: None,
        })
        .await;

        Ok(())
    }

    async fn proof_cache_clear(&self, _params: ProofCacheClearParams) -> Result<ProofCacheClearResponse> {
        {
            let mut cache = self.proof_cache.write().await;
            cache.clear();
        }

        if let Some(root) = self.workspace_root.read().await.clone() {
            let path = proof_cache_path_for_root(&root);
            let _ = fs::remove_file(path);
        }

        Ok(ProofCacheClearResponse { cleared: true })
    }

    async fn publish_for(&self, uri: &Url) {
        let text = {
            let docs = self.docs.read().await;
            docs.get(uri).cloned().unwrap_or_default()
        };

        let diags = match self
            .solver
            .compute_diagnostics(uri.clone(), text.clone())
            .await
        {
            Ok(d) => d,
            Err(msg) => vec![Diagnostic {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: position_from_offset(&text, text.len()),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(DIAG_INTERNAL_ERROR.to_string())),
                code_description: diagnostic_code_href(DIAG_INTERNAL_ERROR)
                    .map(|href| CodeDescription { href }),
                source: Some("aura".to_string()),
                message: msg,
                related_information: None,
                tags: None,
                data: Some(json!({
                    "stable_code": DIAG_INTERNAL_ERROR,
                    "phase": "publish_diagnostics",
                })),
            }],
        };

        self.client.publish_diagnostics(uri.clone(), diags, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        {
            let mut root = self.workspace_root.write().await;
            *root = params
                .root_uri
                .as_ref()
                .and_then(|u| u.to_file_path().ok())
                .or_else(|| params.root_path.as_ref().map(PathBuf::from));
        }

        // Warm-start proof caching across IDE restarts (best-effort).
        self.load_persistent_proof_cache().await;

        // Capture client Aura protocol capabilities for negotiation.
        {
            let mut caps = self.aura_client_caps.write().await;
            *caps = parse_aura_client_caps(&params);
        }

        let legend = SemanticTokensLegend {
            token_types: vec![
                SemanticTokenType::KEYWORD,
                SemanticTokenType::STRING,
                SemanticTokenType::NUMBER,
                SemanticTokenType::OPERATOR,
                SemanticTokenType::VARIABLE,
                SemanticTokenType::FUNCTION,
                SemanticTokenType::TYPE,
                SemanticTokenType::COMMENT,
            ],
            token_modifiers: vec![],
        };

        let caps = self.aura_caps_snapshot().await;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..CompletionOptions::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: Some(false),
                    },
                })),
                inlay_hint_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: Some(false),
                        },
                        legend,
                        range: Some(true),
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                    }),
                ),
                experimental: Some(json!({
                    "auraProtocolVersion": AURA_LSP_PROTOCOL_VERSION,
                    "auraNegotiated": {
                        "protocolVersion": caps.protocol_version,
                        "phases": caps.phases,
                        "telemetry": {
                            "proofTimings": caps.telemetry.proof_timings,
                            "proofCache": caps.telemetry.proof_cache,
                        }
                    }
                })),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "aura-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let _ = self
            .client
            .log_message(MessageType::INFO, "Aura LSP initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        {
            let mut docs = self.docs.write().await;
            docs.insert(uri.clone(), text);
        }
        self.publish_for(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        // For now, accept full-text updates; many clients send full text anyway.
        if let Some(change) = params.content_changes.into_iter().last() {
            let mut docs = self.docs.write().await;
            docs.insert(uri.clone(), change.text);
        }

        self.publish_for(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        {
            let mut docs = self.docs.write().await;
            docs.remove(&uri);
        }
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let text = {
            let docs = self.docs.read().await;
            docs.get(&uri).cloned()
        }
        .or_else(|| {
            uri.to_file_path()
                .ok()
                .and_then(|p| fs::read_to_string(p).ok())
        })
        .unwrap_or_default();

        let offset = offset_from_position(&text, pos);
        let prefix = ident_prefix_at(&text, offset);

        let keywords = [
            "axiom",
            "assume",
            "def",
            "else",
            "exists",
            "false",
            "forall",
            "if",
            "import",
            "let",
            "match",
            "module",
            "proof",
            "theorem",
            "true",
            "type",
        ];

        let mut items: Vec<CompletionItem> = Vec::new();

        for &kw in &keywords {
            if prefix.is_empty() || kw.starts_with(&prefix) {
                items.push(CompletionItem {
                    label: kw.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    ..CompletionItem::default()
                });
            }
        }

        for ident in collect_identifiers(&text) {
            if ident == prefix {
                continue;
            }
            if prefix.is_empty() || ident.starts_with(&prefix) {
                items.push(CompletionItem {
                    label: ident,
                    kind: Some(CompletionItemKind::VARIABLE),
                    ..CompletionItem::default()
                });
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let text = self.get_text_for_uri(&uri).await;

        // Find the identifier under cursor by scanning parsed refs.
        let (defs, refs) = collect_file_symbols(&uri, &text);
        let mut chosen: Option<DefKey> = None;

        for r in &refs {
            if range_contains_position(r.range, pos) {
                chosen = Some(r.key.clone());
                break;
            }
        }
        // If cursor is on a definition name, use that.
        if chosen.is_none() {
            for d in &defs {
                if range_contains_position(d.range, pos) {
                    chosen = Some(d.key.clone());
                    break;
                }
            }
        }

        let Some(key) = chosen else {
            return Ok(None);
        };

        // If definition is in this file, return it directly.
        if key.uri == uri {
            let loc = Location {
                uri: key.uri.clone(),
                range: range_from_source_span(&text, key.span),
            };
            return Ok(Some(GotoDefinitionResponse::Scalar(loc)));
        }

        // Otherwise, do a workspace scan for matching def.
        let root = match self.workspace_root_for(&uri).await {
            Some(r) => r,
            None => return Ok(None),
        };

        for file in list_aura_files(&root) {
            let Some(furi) = file_uri_from_path(&file) else { continue };
            let ftext = fs::read_to_string(&file).unwrap_or_default();
            let (fdefs, _) = collect_file_symbols(&furi, &ftext);
            for d in fdefs {
                if d.key.name == key.name && d.key.kind == key.kind {
                    let loc = Location {
                        uri: furi,
                        range: d.range,
                    };
                    return Ok(Some(GotoDefinitionResponse::Scalar(loc)));
                }
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let include_decl = params.context.include_declaration;
        let text = self.get_text_for_uri(&uri).await;

        let (defs, refs) = collect_file_symbols(&uri, &text);
        let mut target: Option<DefKey> = None;

        for r in &refs {
            if range_contains_position(r.range, pos) {
                target = Some(r.key.clone());
                break;
            }
        }
        if target.is_none() {
            for d in &defs {
                if range_contains_position(d.range, pos) {
                    target = Some(d.key.clone());
                    break;
                }
            }
        }
        let Some(target) = target else { return Ok(Some(vec![])) };

        let root = match self.workspace_root_for(&uri).await {
            Some(r) => r,
            None => return Ok(Some(vec![])),
        };

        let mut out: Vec<Location> = Vec::new();
        for file in list_aura_files(&root) {
            let Some(furi) = file_uri_from_path(&file) else { continue };
            let ftext = fs::read_to_string(&file).unwrap_or_default();
            let (fdefs, frefs) = collect_file_symbols(&furi, &ftext);

            if include_decl {
                for d in fdefs {
                    if d.key.name == target.name && d.key.kind == target.kind {
                        out.push(Location { uri: furi.clone(), range: d.range });
                    }
                }
            }

            for r in frefs {
                if r.key.name == target.name && r.key.kind == target.kind {
                    out.push(Location { uri: furi.clone(), range: r.range });
                }
            }
        }

        Ok(Some(out))
    }

    async fn prepare_rename(&self, params: TextDocumentPositionParams) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let pos = params.position;
        let text = self.get_text_for_uri(&uri).await;
        let offset = offset_from_position(&text, pos);

        // Use identifiers collected from lexer as a fallback.
        let name = ident_prefix_at(&text, offset);
        if name.is_empty() || !is_valid_ident_name(&name) {
            return Ok(None);
        }

        // Best-effort: compute the exact range by expanding to identifier boundaries.
        let start = offset.saturating_sub(name.len());
        let span = SourceSpan::new(SourceOffset::from(start), name.len());
        let range = range_from_source_span(&text, span);

        Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
            range,
            placeholder: name,
        }))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;

        if !is_valid_ident_name(&new_name) {
            return Ok(None);
        }

        let text = self.get_text_for_uri(&uri).await;

        // Identify target symbol.
        let (defs, refs) = collect_file_symbols(&uri, &text);
        let mut target: Option<DefKey> = None;
        for r in &refs {
            if range_contains_position(r.range, pos) {
                target = Some(r.key.clone());
                break;
            }
        }
        if target.is_none() {
            for d in &defs {
                if range_contains_position(d.range, pos) {
                    target = Some(d.key.clone());
                    break;
                }
            }
        }
        let Some(target) = target else { return Ok(None) };

        // Safety: disallow global rename collision for global kinds.
        let root = match self.workspace_root_for(&uri).await {
            Some(r) => r,
            None => return Ok(None),
        };

        if matches!(target.kind, "cell" | "extern_cell" | "type") {
            for file in list_aura_files(&root) {
                let ftext = fs::read_to_string(&file).unwrap_or_default();
                let Some(furi) = file_uri_from_path(&file) else { continue };
                let (fdefs, _) = collect_file_symbols(&furi, &ftext);
                for d in fdefs {
                    if d.key.kind == target.kind && d.key.name == new_name {
                        // Name already taken.
                        return Ok(None);
                    }
                }
            }
        }

        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

        for file in list_aura_files(&root) {
            let Some(furi) = file_uri_from_path(&file) else { continue };
            let ftext = fs::read_to_string(&file).unwrap_or_default();
            let (fdefs, frefs) = collect_file_symbols(&furi, &ftext);

            for d in fdefs {
                if d.key.name == target.name && d.key.kind == target.kind {
                    changes.entry(furi.clone()).or_default().push(TextEdit {
                        range: d.range,
                        new_text: new_name.clone(),
                    });
                }
            }
            for r in frefs {
                if r.key.name == target.name && r.key.kind == target.kind {
                    changes.entry(furi.clone()).or_default().push(TextEdit {
                        range: r.range,
                        new_text: new_name.clone(),
                    });
                }
            }
        }

        if changes.is_empty() {
            return Ok(None);
        }

        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let text = self.get_text_for_uri(&uri).await;

        let program = match aura_parse::parse_source(&text) {
            Ok(p) => p,
            Err(_) => return Ok(Some(vec![])),
        };

        // Build a checker to access known function parameter names.
        let mut checker = aura_core::Checker::new();
        let _ = checker.check_program(&program);

        let mut hints: Vec<InlayHint> = Vec::new();

        fn walk_expr_for_hints(
            hints: &mut Vec<InlayHint>,
            checker: &aura_core::Checker,
            text: &str,
            expr: &aura_ast::Expr,
        ) {
            use aura_ast::ExprKind;
            match &expr.kind {
                ExprKind::Call { callee, args, .. } => {
                    if let Some(name) = expr_callee_name(callee) {
                        if let Some(param_names) = checker.function_param_names(&name) {
                            for (idx, arg) in args.iter().enumerate() {
                                if idx >= param_names.len() {
                                    break;
                                }
                                if let aura_ast::CallArg::Positional(e) = arg {
                                    let label = InlayHintLabel::String(format!("{}:", param_names[idx]));
                                    hints.push(InlayHint {
                                        position: position_from_offset(text, e.span.offset()),
                                        label,
                                        kind: Some(InlayHintKind::PARAMETER),
                                        text_edits: None,
                                        tooltip: None,
                                        padding_left: Some(true),
                                        padding_right: Some(true),
                                        data: None,
                                    });
                                }
                            }
                        }
                    }
                    // Recurse.
                    walk_expr_for_hints(hints, checker, text, callee);
                    for a in args {
                        match a {
                            aura_ast::CallArg::Positional(e) => walk_expr_for_hints(hints, checker, text, e),
                            aura_ast::CallArg::Named { value, .. } => walk_expr_for_hints(hints, checker, text, value),
                        }
                    }
                }
                ExprKind::Unary { expr: inner, .. } => walk_expr_for_hints(hints, checker, text, inner),
                ExprKind::Binary { left, right, .. } => {
                    walk_expr_for_hints(hints, checker, text, left);
                    walk_expr_for_hints(hints, checker, text, right);
                }
                ExprKind::Member { base, .. } => walk_expr_for_hints(hints, checker, text, base),
                ExprKind::Flow { left, right, .. } => {
                    walk_expr_for_hints(hints, checker, text, left);
                    walk_expr_for_hints(hints, checker, text, right);
                }
                ExprKind::Lambda { body, .. } => {
                    for s in &body.stmts {
                        walk_stmt_for_hints(hints, checker, text, s);
                    }
                    if let Some(y) = &body.yield_expr {
                        walk_expr_for_hints(hints, checker, text, y);
                    }
                }
                ExprKind::StyleLit { fields } => {
                    for (_, v) in fields {
                        walk_expr_for_hints(hints, checker, text, v);
                    }
                }
                ExprKind::RecordLit { fields, .. } => {
                    for (_, v) in fields {
                        walk_expr_for_hints(hints, checker, text, v);
                    }
                }
                ExprKind::ForAll { body, .. } | ExprKind::Exists { body, .. } => {
                    walk_expr_for_hints(hints, checker, text, body)
                }
                ExprKind::Ident(_) | ExprKind::IntLit(_) | ExprKind::StringLit(_) => {}
            }
        }

        fn walk_stmt_for_hints(
            hints: &mut Vec<InlayHint>,
            checker: &aura_core::Checker,
            text: &str,
            stmt: &aura_ast::Stmt,
        ) {
            match stmt {
                aura_ast::Stmt::MacroDef(m) => {
                    for s in &m.body.stmts {
                        walk_stmt_for_hints(hints, checker, text, s);
                    }
                    if let Some(y) = &m.body.yield_expr {
                        walk_expr_for_hints(hints, checker, text, y);
                    }
                }
                aura_ast::Stmt::MacroCall(m) => {
                    for a in &m.args {
                        walk_expr_for_hints(hints, checker, text, a);
                    }
                }
                aura_ast::Stmt::StrandDef(sd) => {
                    // Type hint for obvious RHS.
                    if sd.ty.is_none() {
                        let ty = match &sd.expr.kind {
                            aura_ast::ExprKind::IntLit(_) => Some("u32"),
                            aura_ast::ExprKind::StringLit(_) => Some("String"),
                            aura_ast::ExprKind::Call { .. } => None,
                            _ => None,
                        };

                        if let Some(ty) = ty {
                            hints.push(InlayHint {
                                position: position_from_offset(text, sd.name.span.offset() + sd.name.span.len()),
                                label: InlayHintLabel::String(format!(": {ty}")),
                                kind: Some(InlayHintKind::TYPE),
                                text_edits: None,
                                tooltip: None,
                                padding_left: Some(true),
                                padding_right: Some(true),
                                data: None,
                            });
                        }
                    }
                    walk_expr_for_hints(hints, checker, text, &sd.expr);
                }
                aura_ast::Stmt::Assign(a) => walk_expr_for_hints(hints, checker, text, &a.expr),
                aura_ast::Stmt::Prop(p) => walk_expr_for_hints(hints, checker, text, &p.expr),
                aura_ast::Stmt::ExprStmt(e) => walk_expr_for_hints(hints, checker, text, e),
                aura_ast::Stmt::Requires(r) => walk_expr_for_hints(hints, checker, text, &r.expr),
                aura_ast::Stmt::Ensures(e) => walk_expr_for_hints(hints, checker, text, &e.expr),
                aura_ast::Stmt::Assert(a) => walk_expr_for_hints(hints, checker, text, &a.expr),
                aura_ast::Stmt::Assume(a) => walk_expr_for_hints(hints, checker, text, &a.expr),
                aura_ast::Stmt::If(i) => {
                    walk_expr_for_hints(hints, checker, text, &i.cond);
                    for s in &i.then_block.stmts {
                        walk_stmt_for_hints(hints, checker, text, s);
                    }
                    if let Some(y) = &i.then_block.yield_expr {
                        walk_expr_for_hints(hints, checker, text, y);
                    }
                    if let Some(b) = &i.else_block {
                        for s in &b.stmts {
                            walk_stmt_for_hints(hints, checker, text, s);
                        }
                        if let Some(y) = &b.yield_expr {
                            walk_expr_for_hints(hints, checker, text, y);
                        }
                    }
                }
                aura_ast::Stmt::Match(m) => {
                    walk_expr_for_hints(hints, checker, text, &m.scrutinee);
                    for arm in &m.arms {
                        for s in &arm.body.stmts {
                            walk_stmt_for_hints(hints, checker, text, s);
                        }
                        if let Some(y) = &arm.body.yield_expr {
                            walk_expr_for_hints(hints, checker, text, y);
                        }
                    }
                }
                aura_ast::Stmt::While(w) => {
                    walk_expr_for_hints(hints, checker, text, &w.cond);
                    if let Some(inv) = &w.invariant {
                        walk_expr_for_hints(hints, checker, text, inv);
                    }
                    if let Some(dec) = &w.decreases {
                        walk_expr_for_hints(hints, checker, text, dec);
                    }
                    for s in &w.body.stmts {
                        walk_stmt_for_hints(hints, checker, text, s);
                    }
                    if let Some(y) = &w.body.yield_expr {
                        walk_expr_for_hints(hints, checker, text, y);
                    }
                }
                aura_ast::Stmt::CellDef(c) => {
                    for s in &c.body.stmts {
                        walk_stmt_for_hints(hints, checker, text, s);
                    }
                    if let Some(y) = &c.body.yield_expr {
                        walk_expr_for_hints(hints, checker, text, y);
                    }
                }
                aura_ast::Stmt::FlowBlock(fb) => {
                    for s in &fb.body.stmts {
                        walk_stmt_for_hints(hints, checker, text, s);
                    }
                    if let Some(y) = &fb.body.yield_expr {
                        walk_expr_for_hints(hints, checker, text, y);
                    }
                }
                aura_ast::Stmt::Layout(lb) => {
                    for s in &lb.body.stmts {
                        walk_stmt_for_hints(hints, checker, text, s);
                    }
                    if let Some(y) = &lb.body.yield_expr {
                        walk_expr_for_hints(hints, checker, text, y);
                    }
                }
                aura_ast::Stmt::Render(rb) => {
                    for s in &rb.body.stmts {
                        walk_stmt_for_hints(hints, checker, text, s);
                    }
                    if let Some(y) = &rb.body.yield_expr {
                        walk_expr_for_hints(hints, checker, text, y);
                    }
                }
                aura_ast::Stmt::UnsafeBlock(ub) => {
                    for s in &ub.body.stmts {
                        walk_stmt_for_hints(hints, checker, text, s);
                    }
                    if let Some(y) = &ub.body.yield_expr {
                        walk_expr_for_hints(hints, checker, text, y);
                    }
                }
                aura_ast::Stmt::Import(_)
                | aura_ast::Stmt::TypeAlias(_)
                | aura_ast::Stmt::TraitDef(_)
                | aura_ast::Stmt::RecordDef(_)
                | aura_ast::Stmt::EnumDef(_)
                | aura_ast::Stmt::ExternCell(_) => {}
            }
        }

        for stmt in &program.stmts {
            walk_stmt_for_hints(&mut hints, &checker, &text, stmt);
        }

        Ok(Some(hints))
    }

    async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let text = self.get_text_for_uri(&uri).await;
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: semantic_tokens_for_source(&text),
        })))
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri;
        let text = self.get_text_for_uri(&uri).await;
        let data = semantic_tokens_for_source(&text);
        Ok(Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }
}

fn semantic_token_type_index(t: &SemanticTokenType) -> u32 {
    match t {
        x if *x == SemanticTokenType::KEYWORD => 0,
        x if *x == SemanticTokenType::STRING => 1,
        x if *x == SemanticTokenType::NUMBER => 2,
        x if *x == SemanticTokenType::OPERATOR => 3,
        x if *x == SemanticTokenType::VARIABLE => 4,
        x if *x == SemanticTokenType::FUNCTION => 5,
        x if *x == SemanticTokenType::TYPE => 6,
        _ => 4,
    }
}

fn semantic_tokens_for_source(text: &str) -> Vec<SemanticToken> {
    // Minimal semantic tokens based on aura-lex spans and heuristics.
    let tokens = match aura_lex::Lexer::new(text).lex() {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    let mut out: Vec<(u32, u32, u32, u32, u32)> = Vec::new();
    for t in tokens {
        let span = t.span;
        let start = position_from_offset(text, span.offset());
        let line = start.line;
        let col = start.character;
        let len = span.len() as u32;
        if len == 0 {
            continue;
        }

        let tok_type = match &t.kind {
            aura_lex::TokenKind::String(_) => SemanticTokenType::STRING,
            aura_lex::TokenKind::Int(_) => SemanticTokenType::NUMBER,
            aura_lex::TokenKind::Ident(_) => SemanticTokenType::VARIABLE,
            aura_lex::TokenKind::KwImport
            | aura_lex::TokenKind::KwVal
            | aura_lex::TokenKind::KwCell
            | aura_lex::TokenKind::KwExtern
            | aura_lex::TokenKind::KwType
            | aura_lex::TokenKind::KwYield
            | aura_lex::TokenKind::KwMut
            | aura_lex::TokenKind::KwIf
            | aura_lex::TokenKind::KwElse
            | aura_lex::TokenKind::KwMatch
            | aura_lex::TokenKind::KwWhile
            | aura_lex::TokenKind::KwInvariant
            | aura_lex::TokenKind::KwLayout
            | aura_lex::TokenKind::KwRender => SemanticTokenType::KEYWORD,
            aura_lex::TokenKind::Arrow
            | aura_lex::TokenKind::TildeArrow
            | aura_lex::TokenKind::ColonColon
            | aura_lex::TokenKind::Colon
            | aura_lex::TokenKind::Eq
            | aura_lex::TokenKind::EqEq
            | aura_lex::TokenKind::Neq
            | aura_lex::TokenKind::Lt
            | aura_lex::TokenKind::Gt
            | aura_lex::TokenKind::Le
            | aura_lex::TokenKind::Ge
            | aura_lex::TokenKind::Plus
            | aura_lex::TokenKind::Minus
            | aura_lex::TokenKind::Star
            | aura_lex::TokenKind::Slash
            | aura_lex::TokenKind::AndAnd
            | aura_lex::TokenKind::OrOr
            | aura_lex::TokenKind::Bang
            | aura_lex::TokenKind::Dot
            | aura_lex::TokenKind::DotDot
            | aura_lex::TokenKind::Comma => SemanticTokenType::OPERATOR,
            _ => continue,
        };

        out.push((line, col, len, semantic_token_type_index(&tok_type), 0));
    }

    // Sort by position.
    out.sort_by_key(|(l, c, ..)| (*l, *c));

    // Delta encode.
    let mut encoded: Vec<SemanticToken> = Vec::new();
    let mut prev_line: u32 = 0;
    let mut prev_col: u32 = 0;
    let mut first = true;
    for (line, col, len, ty, mods) in out {
        let delta_line = if first { line } else { line - prev_line };
        let delta_start = if first {
            col
        } else if delta_line == 0 {
            col - prev_col
        } else {
            col
        };
        first = false;
        prev_line = line;
        prev_col = col;
        encoded.push(SemanticToken {
            delta_line,
            delta_start,
            length: len,
            token_type: ty,
            token_modifiers_bitset: mods,
        });
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stmt_slice_for_test<'a>(text: &'a str, stmt: &aura_ast::Stmt) -> &'a str {
        let sp = match stmt {
            aura_ast::Stmt::CellDef(s) => s.span,
            aura_ast::Stmt::FlowBlock(s) => s.span,
            aura_ast::Stmt::Layout(s) => s.span,
            aura_ast::Stmt::Render(s) => s.span,
            _ => return "",
        };
        let start = sp.offset();
        let end = start.saturating_add(sp.len());
        text.get(start..end).unwrap_or("")
    }

    fn hash_for_cell_named(text: &str, cell_name: &str, dep_hash: &str) -> String {
        let program = aura_parse::parse_source(text).expect("parse");
        let hashes = compute_checkable_stmt_merkle_hashes(&program, text, dep_hash);
        for (i, stmt) in program.stmts.iter().enumerate() {
            let slice = stmt_slice_for_test(text, stmt);
            if slice.trim_start().starts_with(&format!("cell {cell_name}")) {
                return hashes[i]
                    .as_ref()
                    .map(|x| x.hash.clone())
                    .unwrap_or_else(|| sha256_hex(""));
            }
        }
        panic!("cell not found: {cell_name}");
    }

    #[test]
    fn proof_cache_store_roundtrips() {
        let diag = Diagnostic {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 1,
                },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("aura::test".to_string())),
            code_description: None,
            source: Some("aura".to_string()),
            message: "boom".to_string(),
            related_information: None,
            tags: None,
            data: None,
        };

        let mut entry = ProofCacheEntry::default();
        entry
            .full_by_file
            .insert("k".to_string(), vec![diag.clone()]);

        let mut entries = HashMap::new();
        entries.insert("base".to_string(), entry);

        let store = ProofCacheStoreV1 {
            version: PROOF_CACHE_STORE_VERSION,
            entries,
        };

        let s = serde_json::to_string(&store).expect("serialize");
        let back: ProofCacheStoreV1 = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(back.version, PROOF_CACHE_STORE_VERSION);
        assert_eq!(back.entries.len(), 1);
        assert!(back
            .entries
            .get("base")
            .unwrap()
            .full_by_file
            .contains_key("k"));
    }

    #[test]
    fn file_id_is_workspace_relative_when_possible() {
        let root = tempfile::tempdir().expect("tempdir");
        let p = root.path().join("src").join("main.aura");
        fs::create_dir_all(p.parent().unwrap()).expect("mkdir");
        fs::write(&p, "").expect("write");
        let uri = Url::from_file_path(&p).unwrap();
        let id = file_id_for_cache(Some(root.path()), &uri);
        assert_eq!(id, "src/main.aura");
    }

    #[test]
    fn merkle_stmt_hash_changes_when_callee_changes() {
        let t1 = r#"
cell b() ->:
    yield 1

cell a() ->:
    val x: u32 = b()
    yield x
"#;

        let t2 = r#"
cell b() ->:
    yield 2

cell a() ->:
    val x: u32 = b()
    yield x
"#;

        let h1 = hash_for_cell_named(t1, "a", "deps0");
        let h2 = hash_for_cell_named(t2, "a", "deps0");
        assert_ne!(h1, h2, "caller stmt hash should change when callee changes");
    }

    #[test]
    fn merkle_stmt_hash_stable_when_unrelated_stmt_changes() {
        let t1 = r#"
cell b() ->:
    yield 1

cell c() ->:
    yield 10

cell a() ->:
    val x: u32 = b()
    yield x
"#;

        let t2 = r#"
cell b() ->:
    yield 1

cell c() ->:
    yield 11

cell a() ->:
    val x: u32 = b()
    yield x
"#;

        let h1 = hash_for_cell_named(t1, "a", "deps0");
        let h2 = hash_for_cell_named(t2, "a", "deps0");
        assert_eq!(h1, h2, "caller stmt hash should not change when unrelated stmt changes");
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(Backend::new)
        .custom_method("aura/proofs", Backend::proofs)
        .custom_method("aura/proofsStreamStart", Backend::proofs_stream_start)
        .custom_method("aura/proofsStreamCancel", Backend::proofs_stream_cancel)
        .custom_method("aura/proofCacheClear", Backend::proof_cache_clear)
        .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}
