"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const cp = __importStar(require("node:child_process"));
const fs = __importStar(require("node:fs"));
const os = __importStar(require("node:os"));
const path = __importStar(require("node:path"));
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
let client;
let proofsPanel;
function escapeHtml(s) {
    return s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/\"/g, "&quot;")
        .replace(/'/g, "&#39;");
}
function formatSeverity(sev) {
    switch (sev) {
        case vscode.DiagnosticSeverity.Error:
            return "Error";
        case vscode.DiagnosticSeverity.Warning:
            return "Warning";
        case vscode.DiagnosticSeverity.Information:
            return "Info";
        case vscode.DiagnosticSeverity.Hint:
            return "Hint";
        default:
            return "";
    }
}
async function renderProofsPanel(docUri) {
    if (!client) {
        vscode.window.showErrorMessage("Aura: language server not running");
        return;
    }
    const uri = docUri ?? vscode.window.activeTextEditor?.document.uri;
    if (!uri) {
        vscode.window.showErrorMessage("Aura: no active document");
        return;
    }
    const res = await client.sendRequest("aura/proofs", {
        uri: uri.toString(),
    });
    const rows = res.diagnostics
        .map((d) => {
        const sev = formatSeverity(d.severity);
        const line = d.range.start.line + 1;
        const col = d.range.start.character + 1;
        const code = typeof d.code === "string" ? d.code : typeof d.code === "number" ? d.code.toString() : "";
        return `<tr>
        <td>${escapeHtml(sev)}</td>
        <td>${escapeHtml(code)}</td>
        <td>${escapeHtml(`${line}:${col}`)}</td>
        <td>${escapeHtml(d.message ?? "")}</td>
      </tr>`;
    })
        .join("\n");
    const html = `<!DOCTYPE html>
  <html>
    <head>
      <meta charset="utf-8" />
      <meta name="viewport" content="width=device-width, initial-scale=1" />
      <style>
        body { font-family: var(--vscode-font-family); font-size: var(--vscode-font-size); color: var(--vscode-foreground); }
        h2 { margin: 0 0 8px 0; font-weight: 600; }
        .meta { margin: 0 0 12px 0; opacity: 0.8; }
        table { width: 100%; border-collapse: collapse; }
        th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--vscode-editorWidget-border); vertical-align: top; }
        th { opacity: 0.9; }
        .toolbar { margin: 10px 0 12px 0; display: flex; gap: 8px; }
        button { color: var(--vscode-button-foreground); background: var(--vscode-button-background); border: 0; padding: 6px 10px; cursor: pointer; }
        button:hover { background: var(--vscode-button-hoverBackground); }
      </style>
    </head>
    <body>
      <h2>Aura Sentinel: Proofs</h2>
      <div class="meta">${escapeHtml(res.uri)}</div>
      <div class="toolbar">
        <button id="refresh">Refresh</button>
      </div>
      <table>
        <thead>
          <tr>
            <th>Severity</th>
            <th>Code</th>
            <th>At</th>
            <th>Message</th>
          </tr>
        </thead>
        <tbody>
          ${rows || "<tr><td colspan=\"4\">No proofs/diagnostics</td></tr>"}
        </tbody>
      </table>
      <script>
        const vscode = acquireVsCodeApi();
        document.getElementById('refresh').addEventListener('click', () => vscode.postMessage({ type: 'refresh' }));
      </script>
    </body>
  </html>`;
    if (!proofsPanel) {
        proofsPanel = vscode.window.createWebviewPanel("auraProofs", "Aura Proofs", vscode.ViewColumn.Beside, { enableScripts: true });
        proofsPanel.onDidDispose(() => {
            proofsPanel = undefined;
        });
        proofsPanel.webview.onDidReceiveMessage(async (msg) => {
            if (msg?.type === "refresh") {
                await renderProofsPanel(uri);
            }
        });
    }
    proofsPanel.webview.html = html;
    proofsPanel.reveal(vscode.ViewColumn.Beside);
}
function parseNexusProofMeta(d) {
    const code = d.code;
    const codeStr = typeof code === "string" ? code : typeof code === "number" ? String(code) : undefined;
    if (!codeStr || !codeStr.startsWith("nexus:")) {
        return undefined;
    }
    const parts = codeStr.split(";");
    const head = parts.shift() ?? "";
    const plugin = head.slice("nexus:".length).trim();
    if (!plugin)
        return undefined;
    const meta = { plugin };
    for (const p of parts) {
        const [k, v] = p.split("=", 2);
        if (!k || v === undefined)
            continue;
        const key = k.trim();
        const val = v.trim();
        if (key === "kind")
            meta.kind = val;
        if (key === "mask")
            meta.mask = val;
        if (key === "range")
            meta.range = val;
    }
    return meta;
}
function findNexusDiagnosticAt(doc, pos) {
    const ds = vscode.languages.getDiagnostics(doc.uri);
    for (const d of ds) {
        const meta = parseNexusProofMeta(d);
        if (!meta)
            continue;
        if (d.range.contains(pos))
            return d;
    }
    return undefined;
}
function findAuraToml(startPath) {
    let cur = fs.statSync(startPath).isDirectory() ? startPath : path.dirname(startPath);
    while (true) {
        const cand = path.join(cur, "aura.toml");
        if (fs.existsSync(cand))
            return cand;
        const parent = path.dirname(cur);
        if (parent === cur)
            return undefined;
        cur = parent;
    }
}
function parsePluginsFromAuraToml(raw) {
    const blocks = raw.match(/\{[^}]*\}/g) ?? [];
    const out = [];
    for (const b of blocks) {
        const mName = /name\s*=\s*"([^"]+)"/i.exec(b);
        if (!mName)
            continue;
        const name = mName[1];
        const mTrusted = /trusted\s*=\s*(true|false)/i.exec(b);
        const trusted = mTrusted ? mTrusted[1].toLowerCase() === "true" : undefined;
        out.push({ name, trusted, source: "manifest" });
    }
    return out;
}
function activePluginsForWorkspace() {
    const editor = vscode.window.activeTextEditor;
    const start = editor?.document?.uri?.fsPath;
    const ws = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const auraToml = start ? findAuraToml(start) : ws ? findAuraToml(ws) : undefined;
    if (auraToml && fs.existsSync(auraToml)) {
        const raw = fs.readFileSync(auraToml, "utf8");
        const parsed = parsePluginsFromAuraToml(raw);
        if (parsed.length > 0) {
            return parsed;
        }
    }
    return [
        { name: "aura-iot", trusted: undefined, source: "default" },
        { name: "aura-ai", trusted: undefined, source: "default" },
    ];
}
class NexusPluginItem extends vscode.TreeItem {
    constructor(row) {
        super(row.name, vscode.TreeItemCollapsibleState.None);
        if (row.source === "default") {
            this.description = "active (default)";
        }
        else if (row.trusted === true) {
            this.description = "active (trusted)";
        }
        else if (row.trusted === false) {
            this.description = "active (NOT trusted)";
        }
        else {
            this.description = "active";
        }
    }
}
class NexusPluginProvider {
    _onDidChangeTreeData = new vscode.EventEmitter();
    onDidChangeTreeData = this._onDidChangeTreeData.event;
    refresh() {
        this._onDidChangeTreeData.fire(undefined);
    }
    getTreeItem(element) {
        return element;
    }
    getChildren() {
        const rows = activePluginsForWorkspace();
        return Promise.resolve(rows.map((r) => new NexusPluginItem(r)));
    }
}
function pickCargoPath(config) {
    const configured = String(config.get("cargoPath") ?? "").trim();
    if (configured) {
        return configured;
    }
    const home = os.homedir();
    const candidates = [
        process.env.CARGO ?? "",
        process.platform === "win32"
            ? path.join(home, ".cargo", "bin", "cargo.exe")
            : path.join(home, ".cargo", "bin", "cargo"),
        "cargo",
    ].filter(Boolean);
    for (const c of candidates) {
        if (path.isAbsolute(c) && fs.existsSync(c)) {
            return c;
        }
    }
    return "cargo";
}
function envAuraHome() {
    const v = (process.env.AURA_HOME ?? "").trim();
    return v ? v : undefined;
}
function whichBinary(name) {
    // Best-effort PATH lookup.
    try {
        const cmd = process.platform === "win32" ? "where" : "which";
        const r = cp.spawnSync(cmd, [name], { encoding: "utf8" });
        if (r.status === 0) {
            const first = String(r.stdout || "").split(/\r?\n/).map((s) => s.trim()).find(Boolean);
            if (first && fs.existsSync(first))
                return first;
        }
    }
    catch {
        // ignore
    }
    return undefined;
}
function workspaceBinaryCandidates(exeName) {
    const roots = vscode.workspace.workspaceFolders?.map((w) => w.uri.fsPath) ?? [];
    const out = [];
    for (const r of roots) {
        // Client projects may vendor AuraSDK binaries directly.
        out.push(path.join(r, "bin", exeName));
        // Common portable folder layout.
        out.push(path.join(r, "AuraSDK", "bin", exeName));
        // Optional hidden folder layout.
        out.push(path.join(r, ".aura", "bin", exeName));
    }
    return out;
}
function auraLspBinaryPath(workspaceRoot) {
    const exe = process.platform === "win32" ? "aura-lsp.exe" : "aura-lsp";
    return path.join(workspaceRoot, "target", "debug", exe);
}
function auraLspBinaryPathRelease(workspaceRoot) {
    const exe = process.platform === "win32" ? "aura-lsp.exe" : "aura-lsp";
    return path.join(workspaceRoot, "target", "release", exe);
}
function resolveAuraLspBinary(context, workspaceRoot) {
    const exe = process.platform === "win32" ? "aura-lsp.exe" : "aura-lsp";
    // Client workspace: prefer a vendored binary in the project.
    for (const cand of workspaceBinaryCandidates(exe)) {
        if (fs.existsSync(cand))
            return cand;
    }
    // AuraSDK install: prefer AURA_HOME/bin.
    const home = envAuraHome();
    if (home) {
        const fromHome = path.join(home, "bin", exe);
        if (fs.existsSync(fromHome))
            return fromHome;
    }
    // AuraSDK install: allow PATH lookup.
    const fromPath = whichBinary(exe);
    if (fromPath)
        return fromPath;
    const debugPath = auraLspBinaryPath(workspaceRoot);
    if (fs.existsSync(debugPath)) {
        return debugPath;
    }
    const releasePath = auraLspBinaryPathRelease(workspaceRoot);
    if (fs.existsSync(releasePath)) {
        return releasePath;
    }
    // Fallback: allow running a bundled binary from the extension folder during development.
    const bundled = path.join(context.extensionPath, process.platform === "win32" ? "aura-lsp.exe" : "aura-lsp");
    return bundled;
}
function resolveAuraBinary(context, config, workspaceRoot) {
    const configured = String(config.get("auraPath") ?? "").trim();
    if (configured && fs.existsSync(configured)) {
        return configured;
    }
    const exe = process.platform === "win32" ? "aura.exe" : "aura";
    // Client workspace: prefer a vendored binary in the project.
    for (const cand of workspaceBinaryCandidates(exe)) {
        if (fs.existsSync(cand))
            return cand;
    }
    const home = envAuraHome();
    if (home) {
        const fromHome = path.join(home, "bin", exe);
        if (fs.existsSync(fromHome))
            return fromHome;
    }
    const fromPath = whichBinary(exe);
    if (fromPath)
        return fromPath;
    // Dev fallback: run via cargo.
    return pickCargoPath(config);
}
function auraInterpretBinaryPath(workspaceRoot, profile) {
    const exe = process.platform === "win32" ? "aura-interpret.exe" : "aura-interpret";
    return path.join(workspaceRoot, "target", profile, exe);
}
async function buildAuraInterpret(context, cargoPath, workspaceRoot) {
    await new Promise((resolve, reject) => {
        const child = cp.spawn(cargoPath, ["build", "-p", "aura-interpret", "--release"], {
            cwd: workspaceRoot,
            env: process.env,
        });
        const output = vscode.window.createOutputChannel("Aura Sentinel");
        context.subscriptions.push(output);
        child.stdout.on("data", (d) => output.append(d.toString()));
        child.stderr.on("data", (d) => output.append(d.toString()));
        child.on("error", reject);
        child.on("exit", (code) => {
            if (code === 0)
                resolve();
            else
                reject(new Error(`cargo build (aura-interpret) failed with code ${code}`));
        });
    });
}
function replHtml() {
    return `<!doctype html>
  <html>
    <head>
      <meta charset="utf-8" />
      <meta name="viewport" content="width=device-width, initial-scale=1" />
      <style>
        body { font-family: var(--vscode-font-family); padding: 12px; }
        .row { display: flex; gap: 8px; }
        input { flex: 1; padding: 8px; border: 1px solid var(--vscode-input-border); background: var(--vscode-input-background); color: var(--vscode-input-foreground); }
        button { padding: 8px 10px; }
        pre { white-space: pre-wrap; background: var(--vscode-editor-background); border: 1px solid var(--vscode-editorWidget-border); padding: 10px; margin-top: 10px; }
        .flash-ok { outline: 2px solid var(--vscode-testing-iconPassed); }
        @keyframes glow { 0% { box-shadow: 0 0 0 rgba(0,0,0,0);} 50% { box-shadow: 0 0 10px rgba(0,255,120,0.35);} 100% { box-shadow: 0 0 0 rgba(0,0,0,0);} }
        .glow { animation: glow 220ms ease-out; }
      </style>
    </head>
    <body>
      <div class="row">
        <input id="line" placeholder="Aura REPL (Z3-gated)…" />
        <button id="run">Run</button>
      </div>
      <pre id="out"></pre>
      <script>
        const vscode = acquireVsCodeApi();
        const line = document.getElementById('line');
        const out = document.getElementById('out');
        const run = document.getElementById('run');

        function submit() {
          const text = String(line.value || '');
          if (!text.trim()) return;
          vscode.postMessage({ type: 'eval', line: text });
          line.value = '';
        }

        run.addEventListener('click', submit);
        line.addEventListener('keydown', (e) => {
          if (e.key === 'Enter') submit();
        });

        window.addEventListener('message', (event) => {
          const msg = event.data;
          if (!msg || msg.type !== 'result') return;
          out.textContent = msg.text;
          if (msg.verified) {
            out.classList.remove('glow');
            void out.offsetWidth;
            out.classList.add('glow');
          }
        });
      </script>
    </body>
  </html>`;
}
async function buildAuraLsp(context, cargoPath, workspaceRoot, status) {
    status.text = "Sentinel: Building aura-lsp...";
    status.show();
    await new Promise((resolve, reject) => {
        const child = cp.spawn(cargoPath, ["build", "-p", "aura-lsp"], {
            cwd: workspaceRoot,
            env: process.env,
        });
        const output = vscode.window.createOutputChannel("Aura Sentinel");
        context.subscriptions.push(output);
        child.stdout.on("data", (d) => output.append(d.toString()));
        child.stderr.on("data", (d) => output.append(d.toString()));
        child.on("error", reject);
        child.on("exit", (code) => {
            if (code === 0) {
                resolve();
            }
            else {
                reject(new Error(`cargo build failed with code ${code}`));
            }
        });
    });
}
function computeProofStateForActiveDoc() {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== "aura") {
        return { state: "idle", errors: 0, warnings: 0 };
    }
    const diags = vscode.languages.getDiagnostics(editor.document.uri);
    let errors = 0;
    let warnings = 0;
    for (const d of diags) {
        if (d.severity === vscode.DiagnosticSeverity.Error) {
            errors += 1;
        }
        else if (d.severity === vscode.DiagnosticSeverity.Warning) {
            warnings += 1;
        }
    }
    const state = errors > 0 ? "failed" : "verified";
    return { state, errors, warnings };
}
function updateStatusBar(status, state) {
    if (state === "idle") {
        status.text = "Sentinel: Idle";
        return;
    }
    if (state === "computing") {
        status.text = "Sentinel: Computing Proof...";
        return;
    }
    const { errors, warnings } = computeProofStateForActiveDoc();
    if (state === "failed") {
        status.text = warnings > 0 ? `Sentinel: Proof Failed (${errors}E, ${warnings}W)` : `Sentinel: Proof Failed (${errors}E)`;
    }
    else {
        status.text = warnings > 0 ? `Sentinel: Verified (${warnings}W)` : "Sentinel: Verified";
    }
}
function pickPrimaryDiagnosticsByLine(ds) {
    const errorsByLine = new Map();
    const warningsByLine = new Map();
    for (const d of ds) {
        const line = d.range.start.line;
        if (d.severity === vscode.DiagnosticSeverity.Error) {
            if (!errorsByLine.has(line)) {
                errorsByLine.set(line, d);
            }
        }
        else if (d.severity === vscode.DiagnosticSeverity.Warning) {
            if (!warningsByLine.has(line) && !errorsByLine.has(line)) {
                warningsByLine.set(line, d);
            }
        }
    }
    return { errors: [...errorsByLine.values()], warnings: [...warningsByLine.values()] };
}
function refreshDecorations(errorDeco, warningDeco, iotVerifiedDeco, aiVerifiedDeco) {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== "aura") {
        return;
    }
    const diags = vscode.languages.getDiagnostics(editor.document.uri);
    const { errors, warnings } = pickPrimaryDiagnosticsByLine(diags);
    const verifiedIot = [];
    const verifiedAi = [];
    for (const d of diags) {
        const meta = parseNexusProofMeta(d);
        if (!meta || meta.kind !== "verified")
            continue;
        const p = meta.plugin.toLowerCase();
        if (p.includes("iot"))
            verifiedIot.push(d);
        else if (p.includes("ai"))
            verifiedAi.push(d);
    }
    const toOptions = (d) => ({
        range: d.range,
        hoverMessage: new vscode.MarkdownString(d.message),
    });
    editor.setDecorations(errorDeco, errors.map(toOptions));
    editor.setDecorations(warningDeco, warnings.map(toOptions));
    // Plugin-verified overlays (gutter icons).
    editor.setDecorations(iotVerifiedDeco, verifiedIot.map(toOptions));
    editor.setDecorations(aiVerifiedDeco, verifiedAi.map(toOptions));
}
async function activate(context) {
    const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (!workspaceRoot) {
        return;
    }
    const cfg = vscode.workspace.getConfiguration("aura");
    const cargoPath = pickCargoPath(cfg);
    const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    status.text = "Sentinel: Starting...";
    status.show();
    context.subscriptions.push(status);
    const errorDeco = vscode.window.createTextEditorDecorationType({
        isWholeLine: true,
        gutterIconPath: vscode.Uri.file(path.join(context.extensionPath, "media", "error.svg")),
        overviewRulerLane: vscode.OverviewRulerLane.Right,
        overviewRulerColor: new vscode.ThemeColor("editorError.foreground"),
        before: {
            contentText: "✖ ",
            color: new vscode.ThemeColor("editorError.foreground"),
            margin: "0 8px 0 0",
        },
    });
    const warningDeco = vscode.window.createTextEditorDecorationType({
        isWholeLine: true,
        gutterIconPath: vscode.Uri.file(path.join(context.extensionPath, "media", "warning.svg")),
        overviewRulerLane: vscode.OverviewRulerLane.Right,
        overviewRulerColor: new vscode.ThemeColor("editorWarning.foreground"),
        before: {
            contentText: "⚠ ",
            color: new vscode.ThemeColor("editorWarning.foreground"),
            margin: "0 8px 0 0",
        },
    });
    const iotVerifiedDeco = vscode.window.createTextEditorDecorationType({
        gutterIconPath: vscode.Uri.file(path.join(context.extensionPath, "media", "chip.svg")),
    });
    const aiVerifiedDeco = vscode.window.createTextEditorDecorationType({
        gutterIconPath: vscode.Uri.file(path.join(context.extensionPath, "media", "neural.svg")),
    });
    context.subscriptions.push(errorDeco, warningDeco, iotVerifiedDeco, aiVerifiedDeco);
    const nexusProvider = new NexusPluginProvider();
    context.subscriptions.push(vscode.window.registerTreeDataProvider("auraNexusPlugins", nexusProvider));
    const serverCommand = resolveAuraLspBinary(context, workspaceRoot);
    // Only build aura-lsp from source when we are in a dev workspace without an installed binary.
    if (!fs.existsSync(serverCommand)) {
        try {
            await buildAuraLsp(context, cargoPath, workspaceRoot, status);
        }
        catch (e) {
            updateStatusBar(status, "failed");
            vscode.window.showErrorMessage(`Aura: failed to build aura-lsp. ${String(e)}`);
            return;
        }
    }
    if (!fs.existsSync(serverCommand)) {
        updateStatusBar(status, "failed");
        vscode.window.showErrorMessage(`Aura Sentinel: aura-lsp binary not found at ${serverCommand}`);
        return;
    }
    const serverOptions = {
        command: serverCommand,
        args: [],
        options: {
            cwd: workspaceRoot,
            env: {
                ...process.env,
                RUST_BACKTRACE: "1",
            },
        },
    };
    const clientOptions = {
        documentSelector: [{ language: "aura" }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher("**/*.aura"),
        },
    };
    client = new node_1.LanguageClient("aura", "Aura Language Server", serverOptions, clientOptions);
    context.subscriptions.push(client);
    client.start();
    const textSub = vscode.workspace.onDidChangeTextDocument((e) => {
        if (e.document.languageId === "aura") {
            updateStatusBar(status, "computing");
        }
    });
    context.subscriptions.push(textSub);
    const diagSub = vscode.languages.onDidChangeDiagnostics(() => {
        const { state } = computeProofStateForActiveDoc();
        updateStatusBar(status, state);
        refreshDecorations(errorDeco, warningDeco, iotVerifiedDeco, aiVerifiedDeco);
        nexusProvider.refresh();
    });
    context.subscriptions.push(diagSub);
    const activeEditorSub = vscode.window.onDidChangeActiveTextEditor(() => {
        const { state } = computeProofStateForActiveDoc();
        updateStatusBar(status, state);
        refreshDecorations(errorDeco, warningDeco, iotVerifiedDeco, aiVerifiedDeco);
        nexusProvider.refresh();
    });
    context.subscriptions.push(activeEditorSub);
    const tomlWatcher = vscode.workspace.createFileSystemWatcher("**/aura.toml");
    context.subscriptions.push(tomlWatcher);
    const onToml = () => nexusProvider.refresh();
    context.subscriptions.push(tomlWatcher.onDidCreate(onToml), tomlWatcher.onDidChange(onToml), tomlWatcher.onDidDelete(onToml));
    // Hover: show bitmask/range proven by plugins.
    context.subscriptions.push(vscode.languages.registerHoverProvider({ language: "aura" }, {
        provideHover(doc, pos) {
            const d = findNexusDiagnosticAt(doc, pos);
            if (!d)
                return undefined;
            const meta = parseNexusProofMeta(d);
            if (!meta)
                return undefined;
            const lines = [];
            lines.push(`**${d.message}**`);
            if (meta.mask)
                lines.push(`Bitmask: \`${meta.mask}\``);
            if (meta.range)
                lines.push(`Range: \`${meta.range}\``);
            const smt = d.relatedInformation?.find((ri) => ri.message.startsWith("SMT:"))?.message;
            if (smt) {
                lines.push("");
                lines.push(smt);
            }
            return new vscode.Hover(new vscode.MarkdownString(lines.join("\n")));
        }
    }));
    // Inlay hints: show inferred ranges next to verified accesses.
    context.subscriptions.push(vscode.languages.registerInlayHintsProvider({ language: "aura" }, {
        provideInlayHints(doc, _range, _token) {
            const ds = vscode.languages.getDiagnostics(doc.uri);
            const hints = [];
            for (const d of ds) {
                const meta = parseNexusProofMeta(d);
                if (!meta || meta.kind !== "verified" || !meta.range)
                    continue;
                // Prefer showing these only for IoT-ish proofs.
                if (!meta.plugin.toLowerCase().includes("iot"))
                    continue;
                const hint = new vscode.InlayHint(d.range.end, `Range [${meta.range}]`, vscode.InlayHintKind.Type);
                hint.paddingLeft = true;
                hints.push(hint);
            }
            return hints;
        }
    }));
    context.subscriptions.push(vscode.commands.registerCommand("aura.runActiveFile", async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            return;
        }
        const filePath = editor.document.uri.fsPath;
        if (!filePath.endsWith(".aura")) {
            vscode.window.showWarningMessage("Aura: active file is not a .aura file");
            return;
        }
        const auraPath = String(cfg.get("auraPath") ?? "").trim();
        const auraExe = resolveAuraBinary(context, cfg, workspaceRoot);
        const term = vscode.window.createTerminal("Aura Run");
        term.show();
        if (auraPath) {
            term.sendText(`\"${auraPath}\" run \"${filePath}\" --backend llvm --optimize full`);
            return;
        }
        if (path.basename(auraExe).startsWith("cargo")) {
            // Dev fallback: run via cargo from the workspace root.
            term.sendText(`\"${cargoPath}\" run -p aura --features z3,llvm -- run \"${filePath}\" --backend llvm --optimize full`);
        }
        else {
            term.sendText(`\"${auraExe}\" run \"${filePath}\" --backend llvm --optimize full`);
        }
    }));
    context.subscriptions.push(vscode.commands.registerCommand("aura.openRepl", async () => {
        const panel = vscode.window.createWebviewPanel("auraSentinelRepl", "Aura Sentinel REPL", vscode.ViewColumn.Beside, { enableScripts: true });
        panel.webview.html = replHtml();
        let sessionSource = "";
        panel.webview.onDidReceiveMessage(async (msg) => {
            if (!msg || msg.type !== "eval")
                return;
            const line = String(msg.line ?? "");
            sessionSource += line + "\n";
            try {
                await buildAuraInterpret(context, cargoPath, workspaceRoot);
                const exe = auraInterpretBinaryPath(workspaceRoot, "release");
                if (!fs.existsSync(exe)) {
                    throw new Error(`aura-interpret binary not found at ${exe}`);
                }
                context.subscriptions.push(vscode.commands.registerCommand("aura.showProofs", async () => {
                    await renderProofsPanel();
                }));
                context.subscriptions.push(vscode.workspace.onDidSaveTextDocument(async (doc) => {
                    if (!proofsPanel)
                        return;
                    if (doc.languageId !== "aura")
                        return;
                    await renderProofsPanel(doc.uri);
                }));
                const child = cp.spawn(exe, ["run", "--json"], {
                    cwd: workspaceRoot,
                    env: {
                        ...process.env,
                        RUST_BACKTRACE: "1",
                    },
                });
                let stdout = "";
                let stderr = "";
                child.stdout.on("data", (d) => (stdout += d.toString()));
                child.stderr.on("data", (d) => (stderr += d.toString()));
                child.stdin.write(sessionSource);
                child.stdin.end();
                const code = await new Promise((resolve) => child.on("exit", (c) => resolve(c ?? 1)));
                if (code !== 0) {
                    throw new Error(`aura-interpret failed (code ${code}). ${stderr}`);
                }
                const parsed = JSON.parse(stdout.trim());
                const text = [
                    parsed.stdout ? String(parsed.stdout) : "",
                    `=> ${String(parsed.value ?? "()")}`,
                ]
                    .filter(Boolean)
                    .join("\n");
                panel.webview.postMessage({ type: "result", verified: !!parsed.verified, text });
            }
            catch (e) {
                panel.webview.postMessage({ type: "result", verified: false, text: String(e) });
            }
        });
    }));
    const { state } = computeProofStateForActiveDoc();
    updateStatusBar(status, state);
    refreshDecorations(errorDeco, warningDeco, iotVerifiedDeco, aiVerifiedDeco);
    nexusProvider.refresh();
}
async function deactivate() {
    if (client) {
        await client.stop();
        client = undefined;
    }
}
//# sourceMappingURL=extension.js.map