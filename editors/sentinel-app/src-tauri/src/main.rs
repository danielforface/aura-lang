#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use anyhow::Context;
use lsp_types::{
    notification::Notification,
    notification::{DidChangeTextDocument, DidOpenTextDocument},
    CodeActionContext, CodeActionOrCommand, CodeActionParams, CodeActionResponse, Diagnostic,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, PublishDiagnosticsParams,
    TextDocumentContentChangeEvent, TextDocumentItem, VersionedTextDocumentIdentifier,
    TextDocumentIdentifier,
    DocumentSymbol, DocumentSymbolParams, Location, Position, Range, ReferenceContext, ReferenceParams,
    RenameParams, SymbolInformation, TextDocumentPositionParams, WorkspaceEdit,
    DocumentFormattingParams, FormattingOptions,
    CompletionItem, CompletionParams, CompletionResponse,
    Hover, HoverParams,
    SignatureHelp, SignatureHelpParams,
};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::Mutex;
use tauri::Emitter;
use tauri::Manager;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProofsParams {
    uri: Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProofsResponse {
    uri: Url,
    diagnostics: Vec<lsp_types::Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodeActionsResponse {
    actions: Vec<CodeActionOrCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NavLocation {
    uri: String,
    range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentSymbolsResponse {
    items: Vec<OutlineItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OutlineItem {
    name: String,
    kind: u32,
    range: Range,
    selection_range: Range,
    children: Vec<OutlineItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IncrementalChange {
    range: Range,
    text: String,
    range_length: Option<u32>,
}

#[derive(Default)]
struct LspState {
    client: Option<Arc<LspClient>>,
    root: Option<PathBuf>,
}

#[derive(Default)]
struct ProcState {
    next_id: u64,
    procs: HashMap<u64, AuraProc>,
}

struct AuraProc {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
}

struct TermProc {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
}

#[derive(Default)]
struct TermState {
    next_id: u64,
    procs: HashMap<u64, TermProc>,
}


#[derive(Debug, Clone, Serialize)]
struct AuraOutputPayload {
    id: u64,
    stream: String,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
struct AuraExitPayload {
    id: u64,
    code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
struct TermOutputPayload {
    id: u64,
    stream: String,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
struct TermExitPayload {
    id: u64,
    code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
struct PublishDiagnosticsPayload {
    uri: String,
    diagnostics: Vec<lsp_types::Diagnostic>,
}

struct LspClient {
    app: tauri::AppHandle,
    project_root: PathBuf,
    stdin: Arc<Mutex<ChildStdin>>,
    child: Arc<Mutex<Child>>,
    alive: Arc<AtomicBool>,
    pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
    next_id: AtomicU64,
    versions: Arc<Mutex<HashMap<Url, i32>>>,
    latest_diagnostics: Arc<Mutex<HashMap<Url, Vec<lsp_types::Diagnostic>>>>,
}

impl LspClient {
    async fn start(app: tauri::AppHandle, project_root: &Path) -> anyhow::Result<Self> {
        let lsp_bin = resolve_aura_lsp_binary(project_root)?;

        let mut child = Command::new(lsp_bin)
            .current_dir(project_root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .context("failed to spawn aura-lsp")?;

        let stdin = child.stdin.take().context("missing stdin")?;
        let stdout = child.stdout.take().context("missing stdout")?;

        let client = LspClient {
            app,
            project_root: project_root.to_path_buf(),
            stdin: Arc::new(Mutex::new(stdin)),
            child: Arc::new(Mutex::new(child)),
            alive: Arc::new(AtomicBool::new(true)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            versions: Arc::new(Mutex::new(HashMap::new())),
            latest_diagnostics: Arc::new(Mutex::new(HashMap::new())),
        };

        client.spawn_reader(stdout);

        // Initialize handshake (advertise basic LSP client capabilities).
        client
            .send_request(
                "initialize",
                serde_json::json!({
                    "processId": std::process::id(),
                    "rootUri": Url::from_directory_path(project_root).ok(),
                    "capabilities": {
                        "experimental": {
                            "aura": {
                                "protocolVersion": 1,
                                "phases": ["parse", "sema", "normalize", "z3"],
                                "telemetry": {
                                    "proofTimings": true,
                                    "proofCache": true
                                }
                            }
                        },
                        "workspace": { "configuration": true },
                        "textDocument": {
                            "synchronization": {
                                "dynamicRegistration": false,
                                "willSave": false,
                                "willSaveWaitUntil": false,
                                "didSave": true
                            },
                            "completion": {
                                "dynamicRegistration": false,
                                "completionItem": { "snippetSupport": false }
                            },
                            "hover": { "dynamicRegistration": false },
                            "signatureHelp": {
                                "dynamicRegistration": false,
                                "signatureInformation": { "documentationFormat": ["markdown", "plaintext"] }
                            }
                        }
                    }
                }),
            )
            .await?;
        client.send_notification("initialized", serde_json::json!({})).await?;

        let _ = client.app.emit(
            "lsp/status",
            serde_json::json!({
                "state": "started",
                "root": client.project_root.display().to_string()
            }),
        );

        Ok(client)
    }

    fn spawn_reader(&self, mut stdout: ChildStdout) {
        let pending = Arc::clone(&self.pending);
        let app = self.app.clone();
        let latest = Arc::clone(&self.latest_diagnostics);
        let alive = Arc::clone(&self.alive);
        tauri::async_runtime::spawn(async move {
            loop {
                match read_message(&mut stdout).await {
                    Ok(Some(v)) => {
                        // Notifications (no id)
                        if let Some(method) = v.get("method").and_then(|m| m.as_str()) {
                            if method == "textDocument/publishDiagnostics" {
                                if let Some(params) = v.get("params") {
                                    if let Ok(pd) = serde_json::from_value::<PublishDiagnosticsParams>(params.clone()) {
                                        {
                                            let mut map = latest.lock().await;
                                            map.insert(pd.uri.clone(), pd.diagnostics.clone());
                                        }
                                        let payload = PublishDiagnosticsPayload {
                                            uri: pd.uri.to_string(),
                                            diagnostics: pd.diagnostics,
                                        };
                                        let _ = app.emit("lsp/diagnostics", payload);
                                    }
                                }
                                continue;
                            }

                            // Z3 Gate proof stream (custom protocol from aura-lsp)
                            if method == "aura/proofsStream" {
                                if let Some(params) = v.get("params") {
                                    let _ = app.emit("proofs/stream", params.clone());
                                }
                                continue;
                            }
                        }

                        if let Some(id) = v.get("id").and_then(|id| id.as_u64()) {
                            let tx = {
                                let mut map = pending.lock().await;
                                map.remove(&id)
                            };
                            if let Some(tx) = tx {
                                let _ = tx.send(v);
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }

            alive.store(false, Ordering::Relaxed);
            let _ = app.emit("lsp/status", serde_json::json!({"state": "stopped"}));
        });
    }

    async fn send_notification(&self, method: &str, params: serde_json::Value) -> anyhow::Result<()> {
        if !self.alive.load(Ordering::Relaxed) {
            anyhow::bail!("lsp not running");
        }
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        let mut stdin = self.stdin.lock().await;
        write_message(&mut *stdin, &msg).await?;
        Ok(())
    }

    async fn send_request(&self, method: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        if !self.alive.load(Ordering::Relaxed) {
            anyhow::bail!("lsp not running");
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        let mut stdin = self.stdin.lock().await;
        write_message(&mut *stdin, &msg).await?;

        let v = rx.await.context("lsp request canceled")?;
        if v.get("error").is_some() {
            anyhow::bail!("lsp error: {}", v);
        }
        Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null))
    }

    fn root(&self) -> &Path {
        &self.project_root
    }

    async fn shutdown_and_kill(&self) {
        // Best-effort graceful shutdown.
        let _ = self.send_request("shutdown", serde_json::json!({})).await;
        let _ = self.send_notification("exit", serde_json::json!({})).await;

        let mut child = self.child.lock().await;
        let _ = child.kill().await;
        self.alive.store(false, Ordering::Relaxed);

        let _ = self.app.emit("lsp/status", serde_json::json!({"state": "stopped"}));
    }

    async fn did_open_or_change(&self, uri: Url, text: String) -> anyhow::Result<()> {
        let version = {
            let mut versions = self.versions.lock().await;
            let e = versions.entry(uri.clone()).or_insert(0);
            *e += 1;
            *e
        };

        if version == 1 {
            let params = DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "aura".to_string(),
                    version,
                    text,
                },
            };
            self.send_notification(DidOpenTextDocument::METHOD, serde_json::to_value(params)?).await?;
        } else {
            let params = DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri: uri.clone(), version },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text,
                }],
            };
            self.send_notification(DidChangeTextDocument::METHOD, serde_json::to_value(params)?).await?;
        }

        Ok(())
    }

    async fn did_open(&self, uri: Url, text: String) -> anyhow::Result<()> {
        {
            let mut versions = self.versions.lock().await;
            versions.insert(uri.clone(), 1);
        }

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "aura".to_string(),
                version: 1,
                text,
            },
        };
        self.send_notification(DidOpenTextDocument::METHOD, serde_json::to_value(params)?).await?;
        Ok(())
    }

    async fn did_change_incremental(&self, uri: Url, changes: Vec<IncrementalChange>) -> anyhow::Result<()> {
        if changes.is_empty() {
            return Ok(());
        }

        let version = {
            let mut versions = self.versions.lock().await;
            let e = versions.entry(uri.clone()).or_insert(1);
            *e += 1;
            *e
        };

        let content_changes = changes
            .into_iter()
            .map(|c| TextDocumentContentChangeEvent {
                range: Some(c.range),
                range_length: c.range_length.map(|x| x as u32),
                text: c.text,
            })
            .collect();

        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri: uri.clone(), version },
            content_changes,
        };
        self.send_notification(DidChangeTextDocument::METHOD, serde_json::to_value(params)?).await?;
        Ok(())
    }

    async fn proofs(&self, uri: Url) -> anyhow::Result<ProofsResponse> {
        let result = self
            .send_request(
                "aura/proofs",
                serde_json::to_value(ProofsParams { uri: uri.clone() })?,
            )
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    async fn proofs_stream_start(
        &self,
        uri: Url,
        profile: Option<String>,
        scope: Option<String>,
        ranges: Option<Vec<lsp_types::Range>>,
    ) -> anyhow::Result<u64> {
        let result = self
            .send_request(
                "aura/proofsStreamStart",
                serde_json::json!({
                    "uri": uri,
                    "profile": profile,
                    "scope": scope,
                    "ranges": ranges,
                }),
            )
            .await?;
        let id = result
            .get("id")
            .and_then(|x| x.as_u64())
            .context("missing id from aura/proofsStreamStart")?;
        Ok(id)
    }

    async fn proofs_stream_cancel(&self, id: u64) -> anyhow::Result<()> {
        let _ = self
            .send_request(
                "aura/proofsStreamCancel",
                serde_json::json!({"id": id}),
            )
            .await?;
        Ok(())
    }

    async fn proof_cache_clear(&self, uri: Option<Url>) -> anyhow::Result<bool> {
        let result = self
            .send_request(
                "aura/proofCacheClear",
                serde_json::json!({
                    "uri": uri,
                }),
            )
            .await?;
        Ok(result
            .get("cleared")
            .and_then(|x| x.as_bool())
            .unwrap_or(true))
    }

    async fn completion(&self, uri: Url, position: Position) -> anyhow::Result<Vec<CompletionItem>> {
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };

        let result = self
            .send_request("textDocument/completion", serde_json::to_value(params)?)
            .await?;

        let resp: Option<CompletionResponse> = serde_json::from_value(result)?;
        let items = match resp {
            None => vec![],
            Some(CompletionResponse::Array(arr)) => arr,
            Some(CompletionResponse::List(list)) => list.items,
        };
        Ok(items)
    }

    async fn hover(&self, uri: Url, position: Position) -> anyhow::Result<Option<Hover>> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position,
            },
            work_done_progress_params: Default::default(),
        };

        let result = self
            .send_request("textDocument/hover", serde_json::to_value(params)?)
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    async fn signature_help(&self, uri: Url, position: Position) -> anyhow::Result<Option<SignatureHelp>> {
        let params = SignatureHelpParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position,
            },
            work_done_progress_params: Default::default(),
            context: None,
        };

        let result = self
            .send_request("textDocument/signatureHelp", serde_json::to_value(params)?)
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    async fn code_actions(
        &self,
        uri: Url,
        range: Range,
        diagnostics: Vec<Diagnostic>,
    ) -> anyhow::Result<CodeActionsResponse> {
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range,
            context: CodeActionContext {
                diagnostics,
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self
            .send_request("textDocument/codeAction", serde_json::to_value(params)?)
            .await?;

        let resp: CodeActionResponse = serde_json::from_value(result).unwrap_or_default();
        Ok(CodeActionsResponse { actions: resp })
    }

    async fn goto_definition(&self, uri: Url, position: Position) -> anyhow::Result<Vec<NavLocation>> {
        let params = lsp_types::GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self
            .send_request("textDocument/definition", serde_json::to_value(params)?)
            .await?;

        Ok(extract_nav_locations(result))
    }

    async fn references(&self, uri: Url, position: Position) -> anyhow::Result<Vec<NavLocation>> {
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position,
            },
            context: ReferenceContext {
                include_declaration: true,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self
            .send_request("textDocument/references", serde_json::to_value(params)?)
            .await?;

        let locs: Vec<Location> = serde_json::from_value(result).unwrap_or_default();
        Ok(locs
            .into_iter()
            .map(|l| NavLocation {
                uri: l.uri.to_string(),
                range: l.range,
            })
            .collect())
    }

    async fn rename(&self, uri: Url, position: Position, new_name: String) -> anyhow::Result<WorkspaceEdit> {
        let params = RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position,
            },
            new_name,
            work_done_progress_params: Default::default(),
        };

        let result = self
            .send_request("textDocument/rename", serde_json::to_value(params)?)
            .await?;

        Ok(serde_json::from_value(result).unwrap_or_default())
    }

    async fn document_symbols(&self, uri: Url) -> anyhow::Result<Vec<OutlineItem>> {
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self
            .send_request("textDocument/documentSymbol", serde_json::to_value(params)?)
            .await?;

        // result can be: DocumentSymbol[] or SymbolInformation[]
        if let Ok(ds) = serde_json::from_value::<Vec<DocumentSymbol>>(result.clone()) {
            return Ok(ds.into_iter().map(outline_from_document_symbol).collect());
        }
        if let Ok(si) = serde_json::from_value::<Vec<SymbolInformation>>(result) {
            return Ok(si.into_iter().map(outline_from_symbol_information).collect());
        }
        Ok(vec![])
    }

    async fn format_document(&self, uri: Url, tab_size: u32, insert_spaces: bool) -> anyhow::Result<Vec<lsp_types::TextEdit>> {
        let params = DocumentFormattingParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            options: FormattingOptions {
                tab_size,
                insert_spaces,
                trim_trailing_whitespace: None,
                insert_final_newline: None,
                trim_final_newlines: None,
                properties: Default::default(),
            },
            work_done_progress_params: Default::default(),
        };

        let result = self
            .send_request("textDocument/formatting", serde_json::to_value(params)?)
            .await?;

        let edits: Option<Vec<lsp_types::TextEdit>> = serde_json::from_value(result).unwrap_or(None);
        Ok(edits.unwrap_or_default())
    }
}

fn extract_nav_locations(result: serde_json::Value) -> Vec<NavLocation> {
    // LSP definition can be Location | Location[] | LocationLink[] | null
    if result.is_null() {
        return vec![];
    }

    if let Ok(loc) = serde_json::from_value::<Location>(result.clone()) {
        return vec![NavLocation {
            uri: loc.uri.to_string(),
            range: loc.range,
        }];
    }

    // Try array of Location
    if let Ok(locs) = serde_json::from_value::<Vec<Location>>(result.clone()) {
        return locs
            .into_iter()
            .map(|l| NavLocation {
                uri: l.uri.to_string(),
                range: l.range,
            })
            .collect();
    }

    // Try LocationLink[] (extract targetUri + targetSelectionRange)
    if let Ok(links) = serde_json::from_value::<Vec<lsp_types::LocationLink>>(result) {
        return links
            .into_iter()
            .map(|l| NavLocation {
                uri: l.target_uri.to_string(),
                range: l.target_selection_range,
            })
            .collect();
    }

    vec![]
}

fn symbol_kind_number(k: lsp_types::SymbolKind) -> u32 {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32
}

fn outline_from_document_symbol(s: DocumentSymbol) -> OutlineItem {
    OutlineItem {
        name: s.name,
        kind: symbol_kind_number(s.kind),
        range: s.range,
        selection_range: s.selection_range,
        children: s.children.unwrap_or_default().into_iter().map(outline_from_document_symbol).collect(),
    }
}

fn outline_from_symbol_information(s: SymbolInformation) -> OutlineItem {
    let (range, selection_range) = match s.location {
        lsp_types::Location { range, .. } => (range, range),
    };
    OutlineItem {
        name: s.name,
        kind: symbol_kind_number(s.kind),
        range,
        selection_range,
        children: vec![],
    }
}

fn resolve_sidecar_near_current_exe(exe_name: &str) -> Option<PathBuf> {
    let cur = std::env::current_exe().ok()?;
    let exe_dir = cur.parent()?;

    let mut candidates: Vec<PathBuf> = Vec::new();

    // Common layouts (Windows/Linux): sidecar next to app executable.
    candidates.push(exe_dir.join(exe_name));

    // Some bundle layouts place resources/sidecars in a resources directory.
    candidates.push(exe_dir.join("resources").join(exe_name));
    candidates.push(exe_dir.join(".." ).join("resources").join(exe_name));

    // macOS app bundle layout: <App>.app/Contents/MacOS/<main>, resources at Contents/Resources.
    candidates.push(exe_dir.join(".." ).join("Resources").join(exe_name));

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }

    // Tauri sidecars are often suffixed with the target triple at build time.
    // Try to find an aura-lsp-* executable in the same directories.
    let dirs_to_scan = [
        exe_dir.to_path_buf(),
        exe_dir.join("resources"),
        exe_dir.join("..").join("resources"),
        exe_dir.join("..").join("Resources"),
    ];

    for d in dirs_to_scan {
        let Ok(read) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in read.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if cfg!(windows) {
                if name.starts_with("aura-lsp-") && name.ends_with(".exe") {
                    return Some(p);
                }
            } else if name.starts_with("aura-lsp-") {
                return Some(p);
            }
        }
    }

    None
}

fn resolve_aura_lsp_binary(project_root: &Path) -> anyhow::Result<PathBuf> {
    let exe = if cfg!(windows) { "aura-lsp.exe" } else { "aura-lsp" };

    if let Ok(p) = std::env::var("AURA_LSP_PATH") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Ok(pb);
        }
    }

    // Prefer bundled sidecar when running as an installed app.
    if let Some(p) = resolve_sidecar_near_current_exe(exe) {
        return Ok(p);
    }

    // Prefer repo builds if present.
    let candidates = [
        project_root.join("target").join("release").join(exe),
        project_root.join("target").join("debug").join(exe),
    ];

    for c in candidates {
        if c.exists() {
            return Ok(c);
        }
    }

    // Finally, try PATH. If it's not found, return a clear first-run error.
    let in_path = std::env::var_os("PATH")
        .unwrap_or_default()
        .to_string_lossy()
        .split(if cfg!(windows) { ';' } else { ':' })
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .map(|d| d.join(exe))
        .find(|p| p.is_file());

    if let Some(p) = in_path {
        return Ok(p);
    }

    anyhow::bail!(
        "aura-lsp not found. Expected a bundled sidecar near the app, a repo build under target/(debug|release), or an executable on PATH. You can also set AURA_LSP_PATH to the full path of aura-lsp."
    )
}

async fn read_message<R: AsyncRead + Unpin>(reader: &mut R) -> anyhow::Result<Option<serde_json::Value>> {
    // Read headers until \r\n\r\n
    let mut header_bytes = Vec::new();
    let mut buf = [0u8; 1];
    while reader.read_exact(&mut buf).await.is_ok() {
        header_bytes.push(buf[0]);
        if header_bytes.ends_with(b"\r\n\r\n") {
            break;
        }
        if header_bytes.len() > 32 * 1024 {
            anyhow::bail!("LSP header too large");
        }
    }

    if header_bytes.is_empty() {
        return Ok(None);
    }

    let header_str = String::from_utf8_lossy(&header_bytes);
    let mut content_len: usize = 0;
    for line in header_str.split("\r\n") {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_len = rest.trim().parse::<usize>().unwrap_or(0);
        }
    }

    if content_len == 0 {
        return Ok(None);
    }

    let mut content = vec![0u8; content_len];
    reader.read_exact(&mut content).await?;
    let v: serde_json::Value = serde_json::from_slice(&content)?;
    Ok(Some(v))
}

async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, msg: &serde_json::Value) -> anyhow::Result<()> {
    let content = serde_json::to_vec(msg)?;
    let header = format!("Content-Length: {}\r\n\r\n", content.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(&content).await?;
    writer.flush().await?;
    Ok(())
}

#[tauri::command]
async fn file_uri(path: String) -> Result<String, String> {
    Url::from_file_path(PathBuf::from(path))
        .map(|u| u.to_string())
        .map_err(|_| "invalid path".to_string())
}

fn detect_aura_project_root(hint: &Path) -> PathBuf {
    let mut cur: Option<&Path> = Some(hint);
    while let Some(p) = cur {
        if std::fs::metadata(p.join("aura.toml")).is_ok() {
            return p.to_path_buf();
        }
        cur = p.parent();
    }
    hint.to_path_buf()
}

fn resolve_aura_binary(project_root: &Path) -> PathBuf {
    let exe = if cfg!(windows) { "aura.exe" } else { "aura" };

    // Prefer SDK / local builds when running inside this repo.
    let candidates = [
        project_root.join("dist").join("AuraSDK").join("bin").join(exe),
        project_root.join("target").join("release").join(exe),
        project_root.join("target").join("debug").join(exe),
        project_root.join(exe),
    ];

    for c in candidates {
        if c.is_file() {
            return c;
        }
    }

    // Fall back to PATH.
    PathBuf::from(exe)
}

fn spawn_pipe_reader<R: AsyncRead + Unpin + Send + 'static>(
    app: tauri::AppHandle,
    id: u64,
    stream: &'static str,
    mut reader: R,
) {
    tauri::async_runtime::spawn(async move {
        let mut buf = vec![0u8; 8 * 1024];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app.emit(
                        "aura/output",
                        AuraOutputPayload {
                            id,
                            stream: stream.to_string(),
                            text,
                        },
                    );
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_term_reader<R: AsyncRead + Unpin + Send + 'static>(
    app: tauri::AppHandle,
    id: u64,
    stream: &'static str,
    mut reader: R,
) {
    tauri::async_runtime::spawn(async move {
        let mut buf = vec![0u8; 8 * 1024];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app.emit(
                        "term/output",
                        TermOutputPayload {
                            id,
                            stream: stream.to_string(),
                            text,
                        },
                    );
                }
                Err(_) => break,
            }
        }
    });
}

#[tauri::command]
async fn aura_spawn(app: tauri::AppHandle, project_root: String, args: Vec<String>) -> Result<u64, String> {
    let hint = PathBuf::from(project_root);
    let root = detect_aura_project_root(&hint);
    let bin = resolve_aura_binary(&root);

    let mut child = Command::new(bin)
        .current_dir(&root)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .env("AURA_DEBUG_PROTOCOL", "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let stdin = child.stdin.take().ok_or_else(|| "missing stdin".to_string())?;

    let stdout = child.stdout.take().ok_or_else(|| "missing stdout".to_string())?;
    let stderr = child.stderr.take().ok_or_else(|| "missing stderr".to_string())?;

    let id = {
        let state = app.state::<Mutex<ProcState>>();
        let mut st = state.lock().await;
        st.next_id += 1;
        let id = st.next_id;
        st.procs.insert(
            id,
            AuraProc {
                child,
                stdin: Arc::new(Mutex::new(stdin)),
            },
        );
        id
    };

    spawn_pipe_reader(app.clone(), id, "stdout", stdout);
    spawn_pipe_reader(app.clone(), id, "stderr", stderr);

    // Watch for exit and emit an event when done.
    let app_for_watch = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let exit_code = {
                let state = app_for_watch.state::<Mutex<ProcState>>();
                let mut st = state.lock().await;
                let Some(child) = st.procs.get_mut(&id) else {
                    return;
                };

                match child.child.try_wait() {
                    Ok(Some(status)) => {
                        let code = status.code();
                        st.procs.remove(&id);
                        Some(code)
                    }
                    Ok(None) => None,
                    Err(_) => {
                        st.procs.remove(&id);
                        Some(None)
                    }
                }
            };

            if let Some(code) = exit_code {
                let _ = app_for_watch.emit("aura/exit", AuraExitPayload { id, code });
                return;
            }

            tokio::time::sleep(Duration::from_millis(75)).await;
        }
    });

    Ok(id)
}

#[tauri::command]
async fn aura_send(app: tauri::AppHandle, id: u64, data: String) -> Result<(), String> {
    let stdin = {
        let state = app.state::<Mutex<ProcState>>();
        let st = state.lock().await;
        let Some(p) = st.procs.get(&id) else {
            return Err("missing process".to_string());
        };
        Arc::clone(&p.stdin)
    };

    let mut stdin = stdin.lock().await;
    stdin.write_all(data.as_bytes()).await.map_err(|e| e.to_string())?;
    stdin.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn aura_kill(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let state = app.state::<Mutex<ProcState>>();
    let mut st = state.lock().await;
    let Some(child) = st.procs.get_mut(&id) else {
        return Ok(());
    };
    child.child.kill().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn terminal_start(app: tauri::AppHandle, cwd: Option<String>) -> Result<u64, String> {
    let cwd = cwd.unwrap_or_else(|| "".to_string());
    let cwd_path = if cwd.trim().is_empty() { None } else { Some(PathBuf::from(cwd)) };

    let (bin, args): (String, Vec<String>) = if cfg!(windows) {
        (
            "powershell.exe".to_string(),
            vec![
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-Command".to_string(),
                "-".to_string(),
            ],
        )
    } else {
        ("bash".to_string(), vec![])
    };

    let mut cmd = Command::new(bin);
    if let Some(cwd_path) = cwd_path.as_ref() {
        cmd.current_dir(cwd_path);
    }
    cmd.args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let stdin = child.stdin.take().ok_or_else(|| "missing stdin".to_string())?;
    let stdout = child.stdout.take().ok_or_else(|| "missing stdout".to_string())?;
    let stderr = child.stderr.take().ok_or_else(|| "missing stderr".to_string())?;

    let id = {
        let state = app.state::<Mutex<TermState>>();
        let mut st = state.lock().await;
        st.next_id += 1;
        let id = st.next_id;
        st.procs.insert(
            id,
            TermProc {
                child,
                stdin: Arc::new(Mutex::new(stdin)),
            },
        );
        id
    };

    spawn_term_reader(app.clone(), id, "stdout", stdout);
    spawn_term_reader(app.clone(), id, "stderr", stderr);

    // Watch for exit and emit an event.
    let app_for_watch = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let exit_code = {
                let state = app_for_watch.state::<Mutex<TermState>>();
                let mut st = state.lock().await;
                let Some(tp) = st.procs.get_mut(&id) else {
                    return;
                };

                match tp.child.try_wait() {
                    Ok(Some(status)) => {
                        let code = status.code();
                        st.procs.remove(&id);
                        Some(code)
                    }
                    Ok(None) => None,
                    Err(_) => {
                        st.procs.remove(&id);
                        Some(None)
                    }
                }
            };

            if let Some(code) = exit_code {
                let _ = app_for_watch.emit("term/exit", TermExitPayload { id, code });
                return;
            }

            tokio::time::sleep(Duration::from_millis(75)).await;
        }
    });

    Ok(id)
}

#[tauri::command]
async fn terminal_send(app: tauri::AppHandle, id: u64, data: String) -> Result<(), String> {
    let stdin = {
        let state = app.state::<Mutex<TermState>>();
        let st = state.lock().await;
        st.procs
            .get(&id)
            .map(|tp| Arc::clone(&tp.stdin))
            .ok_or_else(|| "terminal not running".to_string())?
    };

    let mut w = stdin.lock().await;
    w.write_all(data.as_bytes()).await.map_err(|e| e.to_string())?;
    w.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn terminal_kill(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let state = app.state::<Mutex<TermState>>();
    let mut st = state.lock().await;
    let Some(tp) = st.procs.get_mut(&id) else {
        return Ok(());
    };
    tp.child.kill().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn start_lsp(app: tauri::AppHandle, project_root: String) -> Result<(), String> {
    let hint = PathBuf::from(project_root);
    let root = detect_aura_project_root(&hint);
    let state = app.state::<Mutex<LspState>>();
    let mut st = state.lock().await;

    if let Some(existing) = st.client.as_ref() {
        if st.root.as_deref() == Some(existing.root()) {
            return Ok(());
        }

        existing.shutdown_and_kill().await;
        st.client = None;
        st.root = None;
    }

    let client = Arc::new(LspClient::start(app.clone(), &root).await.map_err(|e| e.to_string())?);
    st.client = Some(client);
    st.root = Some(root);
    Ok(())
}

#[tauri::command]
async fn restart_lsp(app: tauri::AppHandle, project_root: String) -> Result<(), String> {
    let hint = PathBuf::from(project_root);
    let root = detect_aura_project_root(&hint);

    let state = app.state::<Mutex<LspState>>();
    let mut st = state.lock().await;
    if let Some(existing) = st.client.take() {
        existing.shutdown_and_kill().await;
    }
    st.root = None;

    let client = Arc::new(LspClient::start(app.clone(), &root).await.map_err(|e| e.to_string())?);
    st.client = Some(client);
    st.root = Some(root);
    Ok(())
}

#[tauri::command]
async fn did_change_text(app: tauri::AppHandle, uri: String, text: String) -> Result<(), String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .did_open_or_change(uri, text)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn did_open_text(app: tauri::AppHandle, uri: String, text: String) -> Result<(), String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client.did_open(uri, text).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn did_change_incremental(
    app: tauri::AppHandle,
    uri: String,
    changes: Vec<IncrementalChange>,
) -> Result<(), String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .did_change_incremental(uri, changes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn proofs_for_text(app: tauri::AppHandle, uri: String, _text: String) -> Result<ProofsResponse, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };
    client.proofs(uri).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn proofs_stream_start(app: tauri::AppHandle, uri: String) -> Result<u64, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .proofs_stream_start(uri, None, None, None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn proofs_stream_start_opts(
    app: tauri::AppHandle,
    uri: String,
    profile: Option<String>,
    scope: Option<String>,
    ranges: Option<Vec<lsp_types::Range>>,
) -> Result<u64, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .proofs_stream_start(uri, profile, scope, ranges)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn proofs_stream_cancel(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };
    client
        .proofs_stream_cancel(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn proof_cache_clear(app: tauri::AppHandle) -> Result<bool, String> {
    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };
    client
        .proof_cache_clear(None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn code_actions(
    app: tauri::AppHandle,
    uri: String,
    start_line: u32,
    start_char: u32,
    end_line: u32,
    end_char: u32,
    diagnostics: Vec<lsp_types::Diagnostic>,
) -> Result<CodeActionsResponse, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    let range = lsp_types::Range {
        start: lsp_types::Position {
            line: start_line,
            character: start_char,
        },
        end: lsp_types::Position {
            line: end_line,
            character: end_char,
        },
    };

    client
        .code_actions(uri, range, diagnostics)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn goto_definition(
    app: tauri::AppHandle,
    uri: String,
    line: u32,
    character: u32,
) -> Result<Vec<NavLocation>, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .goto_definition(
            uri,
            Position {
                line,
                character,
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn find_references(
    app: tauri::AppHandle,
    uri: String,
    line: u32,
    character: u32,
) -> Result<Vec<NavLocation>, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .references(
            uri,
            Position {
                line,
                character,
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn completion(
    app: tauri::AppHandle,
    uri: String,
    line: u32,
    character: u32,
) -> Result<Vec<CompletionItem>, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .completion(
            uri,
            Position {
                line,
                character,
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn hover(
    app: tauri::AppHandle,
    uri: String,
    line: u32,
    character: u32,
) -> Result<Option<Hover>, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .hover(
            uri,
            Position {
                line,
                character,
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn signature_help(
    app: tauri::AppHandle,
    uri: String,
    line: u32,
    character: u32,
) -> Result<Option<SignatureHelp>, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .signature_help(
            uri,
            Position {
                line,
                character,
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn rename_symbol(
    app: tauri::AppHandle,
    uri: String,
    line: u32,
    character: u32,
    new_name: String,
) -> Result<WorkspaceEdit, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .rename(
            uri,
            Position {
                line,
                character,
            },
            new_name,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn document_symbols(app: tauri::AppHandle, uri: String) -> Result<DocumentSymbolsResponse, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    let items = client.document_symbols(uri).await.map_err(|e| e.to_string())?;
    Ok(DocumentSymbolsResponse { items })
}

#[tauri::command]
async fn format_document(
    app: tauri::AppHandle,
    uri: String,
    tab_size: u32,
    insert_spaces: bool,
) -> Result<Vec<lsp_types::TextEdit>, String> {
    let uri = Url::parse(&uri).map_err(|e| e.to_string())?;

    let state = app.state::<Mutex<LspState>>();
    let client = {
        let st = state.lock().await;
        st.client
            .as_ref()
            .cloned()
            .ok_or_else(|| "LSP not started".to_string())?
    };

    client
        .format_document(uri, tab_size, insert_spaces)
        .await
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_fs::init())
        .manage(Mutex::new(LspState::default()))
        .manage(Mutex::new(ProcState::default()))
        .manage(Mutex::new(TermState::default()))
        .invoke_handler(tauri::generate_handler![
            file_uri,
            start_lsp,
            restart_lsp,
            aura_spawn,
            aura_send,
            aura_kill,
            terminal_start,
            terminal_send,
            terminal_kill,
            did_change_text,
            did_open_text,
            did_change_incremental,
            proofs_for_text,
            proofs_stream_start,
            proofs_stream_start_opts,
            proofs_stream_cancel,
            proof_cache_clear,
            code_actions,
            goto_definition,
            find_references,
            rename_symbol,
            completion,
            hover,
            signature_help,
            document_symbols,
            format_document
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, AsyncWriteExt};

    #[tokio::test]
    async fn jsonrpc_roundtrip_message_framing() {
        let (mut a, mut b) = duplex(16 * 1024);

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "aura/proofs",
            "params": {"uri": "file:///demo.aura"}
        });

        write_message(&mut a, &msg).await.unwrap();
        let got = read_message(&mut b).await.unwrap().unwrap();
        assert_eq!(got, msg);
    }

    #[tokio::test]
    async fn read_message_returns_none_on_empty_stream() {
        let (mut a, mut b) = duplex(1024);
        a.shutdown().await.unwrap();
        let got = read_message(&mut b).await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn read_message_rejects_huge_header() {
        let (mut a, mut b) = duplex(64 * 1024);
        // Write >32KB without terminator
        a.write_all(&vec![b'a'; 33 * 1024]).await.unwrap();
        let err = read_message(&mut b).await.err().unwrap();
        assert!(err.to_string().to_lowercase().contains("header"));
    }
}
