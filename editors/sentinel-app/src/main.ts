import "./style.css";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readDir, readTextFile, watch, writeTextFile } from "@tauri-apps/plugin-fs";
import * as path from "@tauri-apps/api/path";
import { listen } from "@tauri-apps/api/event";

import { basicSetup } from "codemirror";
import { Compartment, EditorState, StateEffect, StateField } from "@codemirror/state";
import {
  Decoration,
  DecorationSet,
  EditorView,
  ViewPlugin,
  ViewUpdate,
  hoverTooltip,
  showTooltip,
  Tooltip,
  WidgetType,
} from "@codemirror/view";
import { StreamLanguage } from "@codemirror/language";
import { oneDark } from "@codemirror/theme-one-dark";
import { autocompletion, CompletionContext } from "@codemirror/autocomplete";

import {
  diagBaseKey,
  diagSortKey,
  diagStableId,
  mergeDiagnostics,
  normalizeDiagnostics,
  sevLabel,
} from "./lib/diagnostics";
import { linkifyConsoleLine } from "./lib/consoleLinkify";
import { escapeHtml } from "./lib/html";
import { diffProofSnapshots, renderStructuredProofs } from "./lib/proofsView";
import { buildTrustedCoreReport, renderTrustedCoreReportHtml } from "./lib/trustedCoreReport";

type LspPosition = { line: number; character: number };
type LspRange = { start: LspPosition; end: LspPosition };

type Diagnostic = {
  id?: string;
  range: LspRange;
  severity?: number;
  code?: string | number;
  source?: string;
  message: string;
  // LSP extras (optional)
  data?: any;
  relatedInformation?: Array<{
    location: { uri: string; range: LspRange };
    message: string;
  }>;
};

type TextEdit = {
  range: LspRange;
  newText: string;
};

type IncrementalChange = {
  range: LspRange;
  text: string;
  range_length?: number | null;
};

type WorkspaceEdit = {
  // Most servers use `changes`. Some use `documentChanges`.
  changes?: Record<string, TextEdit[]>;
  documentChanges?: Array<{ textDocument: { uri: string; version?: number | null }; edits: TextEdit[] }>;
};

type CodeAction = {
  title: string;
  edit?: WorkspaceEdit;
  command?: unknown;
  kind?: string;
  isPreferred?: boolean;
};

type CodeActionOrCommand = CodeAction | { title: string; command: string; arguments?: unknown[] };

type CodeActionsResponse = {
  actions: CodeActionOrCommand[];
};

type NavLocation = {
  uri: string;
  range: LspRange;
};

type OutlineItem = {
  name: string;
  kind: number;
  range: LspRange;
  selection_range: LspRange;
  children: OutlineItem[];
};

type DocumentSymbolsResponse = {
  items: OutlineItem[];
};

type ProofsResponse = {
  uri: string;
  diagnostics: Diagnostic[];
};

type ProofStreamPayload = {
  id: number;
  uri: string;
  state: string;
  phase?: string;
  diagnostics?: Diagnostic[];
  telemetry?: any;
  error?: string;
};

// sevLabel imported

type DocState = {
  id: string;
  title?: string;
  path?: string;
  uri?: string;
  text: string;
  lastSavedText: string;
  isDirty: boolean;
  lspOpened: boolean;
  pendingLspChanges: IncrementalChange[];
  dirtyProofRanges: LspRange[];
  lspDiagnostics: Diagnostic[];
  proofsDiagnostics: Diagnostic[];
  mergedDiagnostics: Diagnostic[];
  diagnosticsTimeline: Array<{
    ts: number;
    label: string;
    count: number;
    added: number;
    removed: number;
    sev?: { errors: number; warns: number; infos: number; hints: number };
    detail?: string;
    telemetry?: any;
  }>;
  outlineItems: OutlineItem[];
};

let docs: DocState[] = [];
let activeDocId: string | undefined;

let renamingDocId: string | undefined;
let renamingTabValue = "";

let sdkRootPath: string | undefined;

function activeDoc(): DocState | undefined {
  return docs.find((d) => d.id === activeDocId);
}

function ensureActiveDoc(): DocState {
  if (!activeDocId || !activeDoc()) {
    const d: DocState = {
      id: crypto.randomUUID(),
      text: "",
      lastSavedText: "",
      isDirty: false,
      lspOpened: false,
      pendingLspChanges: [],
      dirtyProofRanges: [],
      lspDiagnostics: [],
      proofsDiagnostics: [],
      mergedDiagnostics: [],
      diagnosticsTimeline: [],
      outlineItems: [],
    };
    docs = [d];
    activeDocId = d.id;
  }
  return activeDoc()!;
}

function setActiveDoc(id: string) {
  activeDocId = id;
  updateEditability();
}
let lspStarted = false;
let lspAutoRestarting = false;
let isRefreshing = false;
let activeProofStreamId: number | undefined;
let lastError: string | undefined;

let isProgrammaticDocSet = false;

let openActionsEl: HTMLElement | null = null;

let diffEnabled = false;
let diffLeftView: EditorView | null = null;
let diffSyncing = false;

function currentPath() {
  return activeDoc()?.path;
}

function currentUri() {
  return activeDoc()?.uri;
}

function makeUntitledUri(id: string) {
  // Must be a valid URL; parsed by Rust backend via Url::parse.
  return `untitled:///${encodeURIComponent(id)}.aura`;
}

type SearchMatch = {
  range: LspRange;
  line: number;
  col: number;
  preview: string;
};

type FolderMatch = SearchMatch & {
  filePath: string;
};

let searchMatches: SearchMatch[] = [];
let folderMatches: FolderMatch[] = [];

let projectRootPath: string | undefined;

type ProofSnapshot = {
  ts: number;
  items: Diagnostic[];
};

const proofHistoryByUri = new Map<string, ProofSnapshot[]>();
let selectedProofId: string | undefined;
let lastProofDelta: { added: number; removed: number; changed: number } | undefined;

type Settings = {
  theme: "default" | "oneDark";
  formatOnSave: boolean;
  proofMode: "auto" | "manual";
  proofProfile: "fast" | "thorough";
  proofDebounceMs: number;
};

const defaultSettings: Settings = {
  theme: "oneDark",
  formatOnSave: false,
  proofMode: "auto",
  proofProfile: "thorough",
  proofDebounceMs: 450,
};

function loadSettings(): Settings {
  try {
    const globalKey = "aura.sentinel.settings";
    const key = projectRootPath ? `aura.sentinel.settings:${projectRootPath}` : globalKey;
    const raw = localStorage.getItem(key) ?? (projectRootPath ? localStorage.getItem(globalKey) : null);
    if (!raw) return { ...defaultSettings };
    const j = JSON.parse(raw) as any;
    const theme = j?.theme === "default" || j?.theme === "oneDark" ? j.theme : defaultSettings.theme;
    const formatOnSave = typeof j?.formatOnSave === "boolean" ? j.formatOnSave : defaultSettings.formatOnSave;
    const proofMode = j?.proofMode === "manual" || j?.proofMode === "auto" ? j.proofMode : defaultSettings.proofMode;
    const proofProfile = j?.proofProfile === "fast" || j?.proofProfile === "thorough" ? j.proofProfile : defaultSettings.proofProfile;
    const proofDebounceMs = Number.isFinite(j?.proofDebounceMs) ? Math.max(0, Math.min(5000, Number(j.proofDebounceMs))) : defaultSettings.proofDebounceMs;
    return { theme, formatOnSave, proofMode, proofProfile, proofDebounceMs };
  } catch {
    return { ...defaultSettings };
  }
}

function saveSettings(s: Settings) {
  const globalKey = "aura.sentinel.settings";
  const key = projectRootPath ? `aura.sentinel.settings:${projectRootPath}` : globalKey;
  localStorage.setItem(key, JSON.stringify(s));
}

let settings: Settings = loadSettings();

const themeCompartment = new Compartment();
const editabilityCompartment = new Compartment();

function themeExtensionFor(s: Settings) {
  return s.theme === "oneDark" ? oneDark : [];
}

function applyThemeToEditors() {
  view.dispatch({ effects: themeCompartment.reconfigure(themeExtensionFor(settings)) });
  if (diffEnabled) enableDiff(true);
}

// --- CodeMirror: Aura syntax (minimal) ---

const auraLanguage = StreamLanguage.define({
  name: "aura",
  startState() {
    return { inString: false } as { inString: boolean };
  },
  token(stream, state) {
    if (stream.sol() && stream.peek() === "#") {
      stream.skipToEnd();
      return "comment";
    }

    if (state.inString) {
      while (!stream.eol()) {
        const ch = stream.next();
        if (ch === "\\") {
          stream.next();
          continue;
        }
        if (ch === '"') {
          state.inString = false;
          break;
        }
      }
      return "string";
    }

    if (stream.peek() === '"') {
      stream.next();
      state.inString = true;
      return "string";
    }

    if (stream.match(/^\d+/, true)) return "number";

    if (stream.match(/^(cell|val|mut|layout|render|import|if|else|for|while|return|break|continue)\b/, true)) {
      return "keyword";
    }

    if (stream.match(/^~>/, true) || stream.match(/^->/, true) || stream.match(/^==|^!=|^<=|^>=/, true)) {
      return "operator";
    }

    if (stream.match(/^[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_][A-Za-z0-9_]*)*/, true)) {
      return "variableName";
    }

    stream.next();
    return null;
  },
});

// --- Diagnostics decorations ---

const setDiagnosticsEffect = StateEffect.define<Diagnostic[]>();
const setSearchMatchesEffect = StateEffect.define<LspRange[]>();
const setTraceRangesEffect = StateEffect.define<LspRange[]>();
const setProvenRangesEffect = StateEffect.define<LspRange[]>();

function diagClass(d: Diagnostic) {
  if ((d.source ?? "") === "aura-nexus") return "cm-diag-proven";
  // LSP severity: 1 Error, 2 Warning, 3 Info, 4 Hint
  if (d.severity === 1) return "cm-diag-error";
  if (d.severity === 2) return "cm-diag-warn";
  return "cm-diag-info";
}

// diag helpers imported

function formatTimestamp(ts: number) {
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return String(ts);
  }
}

function findDeepestSymbolNameForRange(doc: DocState, range: LspRange): string {
  if (!doc.outlineItems?.length) return "(no symbols)";
  const line0 = range.start.line;
  const char0 = range.start.character;
  const paths = flattenOutline(doc.outlineItems);
  const matches: OutlineItem[][] = [];
  for (const { path } of paths) {
    const r = path[path.length - 1].range;
    const afterStart = line0 > r.start.line || (line0 === r.start.line && char0 >= r.start.character);
    const beforeEnd = line0 < r.end.line || (line0 === r.end.line && char0 <= r.end.character);
    if (afterStart && beforeEnd) matches.push(path);
  }
  matches.sort((a, b) => b.length - a.length);
  const best = matches[0];
  return best?.[best.length - 1]?.name ?? "(global)";
}

function formatCounterexampleValue(v: any): string {
  if (v === null || v === undefined) return "";
  if (typeof v === "string") return v;
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  try {
    return JSON.stringify(v);
  } catch {
    return String(v);
  }
}

function prettyProofData(data: any): string {
  try {
    const cx = data?.counterexample;
    const mapped = cx?.mapped;
    const schema = String(cx?.schema ?? mapped?.schema ?? "").trim();

    if (mapped && Array.isArray(mapped.bindings)) {
      const lines: string[] = [];
      if (schema) lines.push(`counterexample.schema: ${schema}`);

      const bindings = mapped.bindings as any[];
      lines.push("bindings:");
      for (const b of bindings.slice(0, 40)) {
        const name = String(b?.name ?? "").trim();
        if (!name) continue;
        const ty = String(b?.aura_type ?? b?.auraType ?? "").trim();
        const value = formatCounterexampleValue(b?.value);
        lines.push(`  ${name}${ty ? `: ${ty}` : ""} = ${value}`);
      }

      const injections = Array.isArray(mapped.injections) ? (mapped.injections as any[]) : [];
      if (injections.length) {
        lines.push("injections:");
        for (const inj of injections.slice(0, 20)) {
          const text = String(inj?.text ?? "").trim();
          if (!text) continue;
          lines.push(`  ${text}`);
        }
      }

      return lines.join("\n");
    }
  } catch {
    // fall through
  }

  try {
    return JSON.stringify(data, null, 2);
  } catch {
    return String(data);
  }
}

function renderExplainPanel(doc: DocState, diag: Diagnostic | undefined) {
  if (!diag) return `<div class="meta">Select a proof obligation to see details.</div>`;

  const line = diag.range.start.line + 1;
  const col = diag.range.start.character + 1;
  const title = escapeHtml(formatDiagTitle(diag));
  const msg = escapeHtml(String(diag.message ?? ""));
  const loc = escapeHtml(`${line}:${col}`);
  const src = escapeHtml(String(diag.source ?? ""));
  const code = escapeHtml(String(diag.code ?? ""));

  let extra = "";
  const data = (diag as any).data;
  if (data !== undefined) {
    try {
      extra = `<div class="meta" style="margin-top:8px;">Data / Counterexample (if provided)</div><pre class="proofPre">${escapeHtml(
        prettyProofData(data)
      )}</pre>`;
    } catch {
      // ignore
    }
  }

  let related = "";
  const rel = (diag as any).relatedInformation;
  if (Array.isArray(rel) && rel.length) {
    related = `
      <div class="meta" style="margin-top:8px;">Related information</div>
      <div class="proofRelated">
        ${rel
          .slice(0, 20)
          .map((ri: any) => {
            const m = escapeHtml(String(ri?.message ?? ""));
            const r = ri?.location?.range;
            const where = r
              ? `${Number(r.start?.line ?? 0) + 1}:${Number(r.start?.character ?? 0) + 1}`
              : "";
            return `<div class="meta">${escapeHtml(where)} ${m}</div>`;
          })
          .join("\n")}
      </div>
    `;
  }

  return `
    <div class="proofExplain">
      <div class="tipTitle">${title}</div>
      <div class="meta">${loc} · ${src} ${code}</div>
      <div class="proofMsg">${msg}</div>
      ${related}
      ${extra}
    </div>
  `;
}

// proofs helpers imported

async function exportTrustedCoreReport(format: "json" | "html") {
  const doc = ensureActiveDoc();
  if (!doc.uri) return;

  const baseName = doc.path ? doc.path.split("\\").pop() ?? "aura" : "aura";
  const stem = baseName.endsWith(".aura") ? baseName.slice(0, -5) : baseName;
  const ext = format === "json" ? "json" : "html";
  const defaultPath = doc.path ? `${doc.path}.${format === "json" ? "trusted-core.json" : "trusted-core.html"}` : undefined;

  const outPath = await save({
    title: `Export Trusted-core Report (${format.toUpperCase()})`,
    defaultPath: defaultPath ?? `${stem}.trusted-core.${ext}`,
    filters: [{ name: format.toUpperCase(), extensions: [ext] }],
  });
  if (!outPath) return;

  const report = buildTrustedCoreReport({
    uri: doc.uri,
    path: doc.path ?? null,
    projectRoot: projectRootPath ?? null,
    settings,
    lspDiagnostics: doc.lspDiagnostics,
    proofsDiagnostics: doc.proofsDiagnostics,
    mergedDiagnostics: doc.mergedDiagnostics,
    timelineDelta: lastProofDelta ?? null,
  });

  if (format === "json") {
    await writeTextFile(String(outPath), JSON.stringify(report, null, 2));
    setStatus("Exported report (JSON)");
    return;
  }

  const html = renderTrustedCoreReportHtml(report, formatDiagTitle);

  await writeTextFile(String(outPath), html);
  setStatus("Exported report (HTML)");
}

async function exportProofsJson() {
  const doc = ensureActiveDoc();
  if (!doc.uri) return;

  const baseName = doc.path ? doc.path.split("\\").pop() ?? "aura" : "aura";
  const stem = baseName.endsWith(".aura") ? baseName.slice(0, -5) : baseName;
  const defaultPath = doc.path ? `${doc.path}.proofs.json` : undefined;

  const outPath = await save({
    title: "Export Proofs (JSON)",
    defaultPath: defaultPath ?? `${stem}.proofs.json`,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!outPath) return;

  const proofData = {
    uri: doc.uri,
    path: doc.path,
    diagnostics: doc.mergedDiagnostics,
    timestamp: new Date().toISOString(),
  };

  await writeTextFile(String(outPath), JSON.stringify(proofData, null, 2));
  setStatus("Exported proofs (JSON)");
}

// escapeHtml imported

function fileUriToPath(uri: string): string | undefined {
  try {
    const u = new URL(uri);
    if (u.protocol !== "file:") return undefined;
    let p = decodeURIComponent(u.pathname);
    // Windows file URL: /C:/path
    if (/^\/[A-Za-z]:\//.test(p)) p = p.slice(1);
    return p.replaceAll("/", "\\");
  } catch {
    return undefined;
  }
}

function formatDiagTitle(d: Diagnostic) {
  const sev = sevLabel(d.severity);
  const src = d.source ?? "";
  const code = d.code ?? "";
  const parts = [sev, src, String(code)].filter((x) => String(x).trim().length);
  return parts.join(" · ");
}

function toDocPos(view: EditorView, line0: number, char0: number) {
  const lineNumber = Math.max(1, Math.min(view.state.doc.lines, line0 + 1));
  const line = view.state.doc.line(lineNumber);
  return Math.max(line.from, Math.min(line.to, line.from + Math.max(0, char0)));
}

const diagnosticsPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet = Decoration.none;
    current: Diagnostic[] = [];

    constructor(view: EditorView) {
      void view;
    }

    update(update: ViewUpdate) {
      for (const e of update.transactions.flatMap((t) => t.effects)) {
        if (e.is(setDiagnosticsEffect)) {
          this.current = e.value;
          this.decorations = buildDecorations(update.view, this.current);
        }
      }

      if (update.docChanged) {
        this.decorations = buildDecorations(update.view, this.current);
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  }
);

const searchPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet = Decoration.none;
    current: LspRange[] = [];

    constructor(view: EditorView) {
      void view;
    }

    update(update: ViewUpdate) {
      for (const e of update.transactions.flatMap((t) => t.effects)) {
        if (e.is(setSearchMatchesEffect)) {
          this.current = e.value;
          this.decorations = buildSearchDecorations(update.view, this.current);
        }
      }
      if (update.docChanged) {
        this.decorations = buildSearchDecorations(update.view, this.current);
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  }
);

const tracePlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet = Decoration.none;
    current: LspRange[] = [];

    constructor(view: EditorView) {
      void view;
    }

    update(update: ViewUpdate) {
      for (const e of update.transactions.flatMap((t) => t.effects)) {
        if (e.is(setTraceRangesEffect)) {
          this.current = e.value;
          this.decorations = buildTraceDecorations(update.view, this.current);
        }
      }
      if (update.docChanged) {
        this.decorations = buildTraceDecorations(update.view, this.current);
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  }
);

const trustDimmingPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet = Decoration.none;
    proven: LspRange[] = [];

    constructor(view: EditorView) {
      void view;
    }

    update(update: ViewUpdate) {
      for (const e of update.transactions.flatMap((t) => t.effects)) {
        if (e.is(setProvenRangesEffect)) {
          this.proven = e.value;
          this.decorations = buildTrustDimmingDecorations(update.view, this.proven);
        }
      }

      if (update.docChanged) {
        this.decorations = buildTrustDimmingDecorations(update.view, this.proven);
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  }
);

function buildSearchDecorations(view: EditorView, ranges: LspRange[]): DecorationSet {
  const out: any[] = [];
  for (const r of ranges) {
    const from = toDocPos(view, r.start.line, r.start.character);
    const to = toDocPos(view, r.end.line, r.end.character);
    if (to <= from) continue;
    out.push(
      Decoration.mark({
        class: "cm-search-hit",
      }).range(from, to)
    );
  }
  return Decoration.set(out, true);
}

function buildTraceDecorations(view: EditorView, ranges: LspRange[]): DecorationSet {
  const out: any[] = [];
  for (const r of ranges) {
    const from = toDocPos(view, r.start.line, r.start.character);
    const to = toDocPos(view, r.end.line, r.end.character);
    if (to <= from) continue;
    out.push(
      Decoration.mark({
        class: "cm-trace-hit",
      }).range(from, to)
    );
  }
  return Decoration.set(out, true);
}

function buildTrustDimmingDecorations(view: EditorView, proven: LspRange[]): DecorationSet {
  type Span = { from: number; to: number };

  const docLen = view.state.doc.length;
  if (!docLen) return Decoration.none;

  const provenSpans: Span[] = [];
  for (const r of proven ?? []) {
    const from = toDocPos(view, r.start.line, r.start.character);
    const to = toDocPos(view, r.end.line, r.end.character);
    if (to <= from) continue;
    provenSpans.push({ from, to });
  }

  if (!provenSpans.length) return Decoration.none;

  provenSpans.sort((a, b) => a.from - b.from || a.to - b.to);

  // Merge overlaps.
  const merged: Span[] = [];
  for (const s of provenSpans) {
    const last = merged[merged.length - 1];
    if (!last || s.from > last.to) {
      merged.push({ ...s });
    } else {
      last.to = Math.max(last.to, s.to);
    }
  }

  const dim: Span[] = [];
  let cur = 0;
  for (const s of merged) {
    if (s.from > cur) dim.push({ from: cur, to: s.from });
    cur = Math.max(cur, s.to);
  }
  if (cur < docLen) dim.push({ from: cur, to: docLen });

  const out: any[] = [];
  for (const s of dim) {
    if (s.to <= s.from) continue;
    out.push(
      Decoration.mark({
        class: "cm-dim",
      }).range(s.from, s.to)
    );
  }

  return Decoration.set(out, true);
}

function provenRangesForDoc(doc: DocState): LspRange[] {
  const out: LspRange[] = [];
  for (const d of doc.mergedDiagnostics ?? []) {
    if ((d.source ?? "") === "aura-nexus") out.push(d.range);
  }
  return out;
}

function traceRangesForDiagnostic(diag: Diagnostic | undefined): LspRange[] {
  if (!diag) return [];

  const ranges: LspRange[] = [];
  ranges.push(diag.range);

  const rel = (diag as any).relatedInformation;
  if (Array.isArray(rel)) {
    for (const ri of rel) {
      const r = ri?.location?.range;
      if (r && r.start && r.end) ranges.push(r as LspRange);
    }
  }

  // De-dup and cap.
  const seen = new Set<string>();
  const dedup: LspRange[] = [];
  for (const r of ranges) {
    const k = JSON.stringify(r);
    if (seen.has(k)) continue;
    seen.add(k);
    dedup.push(r);
    if (dedup.length >= 80) break;
  }
  return dedup;
}

function applySelectedProofHighlights(doc: DocState) {
  if (!selectedProofId) {
    view.dispatch({ effects: setTraceRangesEffect.of([]) });
    return;
  }
  const diag = (doc.mergedDiagnostics ?? []).find((d) => (d.id ?? diagStableId(d)) === selectedProofId);
  const ranges = traceRangesForDiagnostic(diag);
  view.dispatch({ effects: setTraceRangesEffect.of(ranges) });
}

function buildDecorations(view: EditorView, diags: Diagnostic[]): DecorationSet {
  const ranges: any[] = [];

  class InlineValueWidget extends WidgetType {
    text: string;
    constructor(text: string) {
      super();
      this.text = text;
    }
    toDOM() {
      const span = document.createElement("span");
      span.className = "cm-cex-inline";
      span.textContent = this.text;
      return span;
    }
    ignoreEvent() {
      return true;
    }
  }

  const seenInjections = new Set<string>();
  for (const d of diags) {
    const from = toDocPos(view, d.range.start.line, d.range.start.character);
    const to = toDocPos(view, d.range.end.line, d.range.end.character);
    if (to <= from) continue;
    ranges.push(
      Decoration.mark({
        class: diagClass(d),
      }).range(from, to)
    );

    // Inline counterexample values (ghost text) if provided by the LSP.
    const data = (d as any).data;
    const mapped = data?.counterexample?.mapped;
    const injections = Array.isArray(mapped?.injections) ? mapped.injections : undefined;
    if (injections && injections.length) {
      for (const inj of injections.slice(0, 12)) {
        const r = inj?.range;
        const text = String(inj?.text ?? "").trim();
        if (!r || !r.start || !r.end || !text) continue;
        const pos = toDocPos(view, r.end.line, r.end.character);
        const key = `${pos}:${text}`;
        if (seenInjections.has(key)) continue;
        seenInjections.add(key);
        ranges.push(
          Decoration.widget({
            widget: new InlineValueWidget(text),
            side: 1,
          }).range(pos)
        );
      }
    }
  }
  return Decoration.set(ranges, true);
}

function diagAtPos(view: EditorView, pos: number): Diagnostic | undefined {
  // Prefer the most severe diagnostic that covers this position.
  let best: Diagnostic | undefined;
  const doc = ensureActiveDoc();
  for (const d of doc.mergedDiagnostics) {
    const from = toDocPos(view, d.range.start.line, d.range.start.character);
    const to = toDocPos(view, d.range.end.line, d.range.end.character);
    if (pos >= from && pos <= to) {
      if (!best) {
        best = d;
        continue;
      }
      const a = best.severity ?? 99;
      const b = d.severity ?? 99;
      if (b < a) best = d;
    }
  }
  return best;
}

const diagnosticsHover = hoverTooltip((view, pos) => {
  const d = diagAtPos(view, pos);
  if (!d) return null;

  const line = d.range.start.line + 1;
  const col = d.range.start.character + 1;
  const title = escapeHtml(formatDiagTitle(d));
  const msg = escapeHtml(d.message ?? "");

  return {
    pos,
    end: pos,
    above: true,
    create() {
      const dom = document.createElement("div");
      dom.className = "cm-tooltip-aura";
      dom.innerHTML = `
        <div class="tipTitle">${title}</div>
        <div class="tipMeta">${line}:${col}</div>
        <div class="tipMsg">${msg}</div>
      `;
      return { dom };
    },
  };
});

function offsetToLspPos(doc: EditorState["doc"], offset: number) {
  const line = doc.lineAt(offset);
  return { line: line.number - 1, character: offset - line.from };
}

function hoverContentsToPlainText(contents: any): string {
  if (!contents) return "";
  if (typeof contents === "string") return contents;
  // MarkupContent
  if (typeof contents === "object" && typeof contents.value === "string") return contents.value;
  // MarkedString {language,value}
  if (typeof contents === "object" && typeof contents.value === "string") return contents.value;
  if (Array.isArray(contents)) return contents.map((c) => hoverContentsToPlainText(c)).filter(Boolean).join("\n\n");
  return "";
}

const serverHover = hoverTooltip(async (view, pos) => {
  // Prefer diagnostic hover when present.
  if (diagAtPos(view, pos)) return null;

  const doc = ensureActiveDoc();
  if (!doc.uri) return null;

  try {
    await ensureLspStarted();
    const p = offsetToLspPos(view.state.doc, pos);
    const res = await invoke<any>("hover", { uri: doc.uri, line: p.line, character: p.character });
    const contents = res?.contents;
    const txt = hoverContentsToPlainText(contents).trim();
    if (!txt) return null;

    const safe = escapeHtml(txt);
    return {
      pos,
      end: pos,
      above: true,
      create() {
        const dom = document.createElement("div");
        dom.className = "cm-tooltip-aura";
        dom.innerHTML = `<div class="tipMsg">${safe.replace(/\n/g, "<br/>")}</div>`;
        return { dom };
      },
    };
  } catch {
    return null;
  }
});

const setSignatureTooltipEffect = StateEffect.define<Tooltip | null>();
const signatureTooltipField = StateField.define<Tooltip | null>({
  create: () => null,
  update: (value, tr) => {
    for (const e of tr.effects) {
      if (e.is(setSignatureTooltipEffect)) return e.value;
    }
    return value;
  },
  provide: (f) => showTooltip.from(f),
});

async function requestSignatureHelp() {
  const doc = ensureActiveDoc();
  if (!doc.uri) return;
  try {
    await ensureLspStarted();
    const p = currentLspPosition();
    const res = await invoke<any>("signature_help", { uri: doc.uri, line: p.line, character: p.character });
    const signatures = Array.isArray(res?.signatures) ? res.signatures : [];
    if (!signatures.length) {
      view.dispatch({ effects: setSignatureTooltipEffect.of(null) });
      return;
    }

    const activeIndex = typeof res?.activeSignature === "number" ? res.activeSignature : 0;
    const sig = signatures[Math.max(0, Math.min(activeIndex, signatures.length - 1))];
    const label = String(sig?.label ?? "").trim();
    const docText = hoverContentsToPlainText(sig?.documentation).trim();
    const body = escapeHtml([label, docText].filter(Boolean).join("\n\n"));
    if (!body) {
      view.dispatch({ effects: setSignatureTooltipEffect.of(null) });
      return;
    }

    const pos = view.state.selection.main.head;
    const tt: Tooltip = {
      pos,
      above: false,
      strictSide: true,
      create() {
        const dom = document.createElement("div");
        dom.className = "cm-tooltip-aura";
        dom.innerHTML = `<div class="tipMsg">${body.replace(/\n/g, "<br/>")}</div>`;
        return { dom };
      },
    };

    view.dispatch({ effects: setSignatureTooltipEffect.of(tt) });
  } catch {
    // ignore
  }
}

const lspCompletionSource = async (context: CompletionContext) => {
  const doc = ensureActiveDoc();
  if (!doc.uri) return null;

  const before = context.matchBefore(/[A-Za-z_][A-Za-z0-9_\-]*/);
  if (!context.explicit && !before) return null;

  try {
    await ensureLspStarted();
    const p = offsetToLspPos(context.state.doc, context.pos);
    const items = await invoke<any[]>("completion", { uri: doc.uri, line: p.line, character: p.character });
    if (!Array.isArray(items) || !items.length) return null;

    const options = items
      .map((it) => {
        const label = String(it?.label ?? "");
        if (!label) return null;
        const detail = typeof it?.detail === "string" ? it.detail : undefined;
        const insertText = typeof it?.insertText === "string" ? it.insertText : label;
        return {
          label,
          detail,
          apply: insertText,
          type: typeof it?.kind === "number" ? String(it.kind) : undefined,
        };
      })
      .filter(Boolean) as any[];

    if (!options.length) return null;
    return {
      from: before ? before.from : context.pos,
      options,
      validFor: /[A-Za-z0-9_\-]*/,
    };
  } catch {
    return null;
  }
};

const app = document.querySelector<HTMLDivElement>("#app")!;
app.innerHTML = `
  <div class="container">
    <div class="menubar" id="menubar">
      <div class="menuLeft" role="menubar" aria-label="Menu">
        <div class="appTitle">Aura Sentinel</div>
        <button class="menuTop" type="button" data-menu="file" aria-haspopup="true" aria-expanded="false">File</button>
        <button class="menuTop" type="button" data-menu="run" aria-haspopup="true" aria-expanded="false">Run</button>
        <button class="menuTop" type="button" data-menu="tools" aria-haspopup="true" aria-expanded="false">Tools</button>
      </div>
      <div class="menuRight">
        <div class="breadcrumbs" id="crumbs"></div>
        <div class="status" id="status">Idle</div>
      </div>
      <div class="menuDropdown" id="menuDropdown" style="display:none" role="menu"></div>

      <div class="commandButtons" style="display:none">
        <button id="newFile" type="button">New</button>
        <button id="open" type="button">Open</button>
        <button id="openFolder" type="button">Open Folder</button>
        <button id="save" type="button">Save</button>
        <button id="saveAs" type="button">Save As</button>
        <button id="searchFocus" type="button">Search</button>
        <button id="searchInFile" type="button">Search In File</button>
        <button id="searchInFolder" type="button">Search In Folder</button>
        <button id="proofs" type="button">Proofs</button>
        <button id="proofsAffected" type="button" title="Re-run proofs only for recently edited regions">Affected</button>
        <button id="proofProfile" type="button" title="Toggle proof profile (fast vs thorough)">Profile: Thorough</button>
        <button id="diff" type="button">Diff</button>
        <button id="build" type="button">Build</button>
        <button id="run" type="button">Run</button>
        <button id="runDev" type="button">Run (Dev)</button>
        <button id="stopRun" type="button">Stop</button>
        <button id="tests" type="button">Tests</button>
        <button id="pkg" type="button">Pkg</button>
      </div>
    </div>
    <div class="tabs" id="tabs"></div>

    <div class="sidebar sidebarLeft">
      <details class="sideSection" open>
        <summary>Explorer</summary>
        <div class="sideContent" id="filesOut">
          <div class="meta" id="workspaceLabel">No folder open.</div>
          <div class="recent" id="recentOut" style="margin-top:10px;"></div>
          <div class="fileTree" id="fileTree" style="margin-top:10px;"></div>
        </div>
      </details>

      <details class="sideSection" style="margin-top:12px;">
        <summary>Search</summary>
        <div class="sideContent" id="searchOut">
          <div class="searchRow">
            <input id="searchQuery" class="searchInput" placeholder="Search…" />
            <button id="searchFile" class="miniBtn" type="button">In File</button>
            <button id="searchFolder" class="miniBtn" type="button">In Folder</button>
          </div>
          <div class="meta">Ctrl+F in file · Ctrl+Shift+F folder</div>
          <div id="searchResults" style="margin-top:10px;"></div>
        </div>
      </details>
    </div>

    <div class="main">
      <div class="editorRow">
        <div class="diffPane" id="diffPane"><div id="diffLeft" class="diffLeft"></div></div>
        <div class="editorWrap" id="editor"></div>
        <div class="minimap" id="minimap" title="Minimap (click to jump)">
          <div class="minimapViewport" id="minimapViewport"></div>
          <pre class="minimapText" id="minimapText"></pre>
        </div>
      </div>
    </div>
    <div class="sidebar sidebarRight">
      <details class="sideSection" open>
        <summary>Analysis</summary>
        <details class="sideItem" open>
          <summary>Proofs</summary>
          <div class="sideContent" id="proofsOut">
            <div class="meta">Open an .aura file and click Proofs.</div>
          </div>
        </details>
        <details class="sideItem">
          <summary>Outline</summary>
          <div class="sideContent" id="outlineOut">
            <div class="meta">Symbols will appear after opening a file.</div>
          </div>
        </details>
        <details class="sideItem">
          <summary>Navigation</summary>
          <div class="sideContent" id="navOut">
            <div class="meta">F12: definition · Shift+F12: references · F2: rename</div>
          </div>
        </details>
      </details>

      <details class="sideSection" open style="margin-top:12px;">
        <summary>Packages</summary>
        <div class="sideContent" id="pkgOut">
          <div class="pkgRow">
            <label class="settingsLabel" for="pkgName">Package</label>
            <select id="pkgName" class="settingsControl">
              <option value="raylib">raylib</option>
              <option value="onnxruntime">onnxruntime</option>
            </select>
          </div>
          <div class="pkgRow">
            <label class="settingsLabel" for="pkgVersion">Version (optional)</label>
            <input id="pkgVersion" class="settingsControl" placeholder="latest" />
          </div>
          <div class="pkgRow">
            <label class="settingsLabel" for="pkgUrl">URL (optional)</label>
            <input id="pkgUrl" class="settingsControl" placeholder="https://…/artifact.zip" />
          </div>
          <div class="settingsRow">
            <label class="settingsLabel"><input type="checkbox" id="pkgForce" /> Force</label>
          </div>
          <div class="settingsRow">
            <label class="settingsLabel"><input type="checkbox" id="pkgNoSmoke" /> No smoke test</label>
          </div>
          <div class="pkgActions">
            <button id="pkgAdd" class="miniBtn" type="button">Add</button>
            <button id="pkgRefresh" class="miniBtn" type="button">Refresh Graph</button>
          </div>
          <div class="pkgActions" style="margin-top:6px;">
            <button id="bindgenRun" class="miniBtn" type="button">Bindgen…</button>
            <button id="bindgenOpenReport" class="miniBtn" type="button">Trusted Boundary</button>
          </div>
          <div class="meta" id="pkgMeta">Dependency graph loads from <code>aura.lock</code>.</div>
          <div id="pkgGraph" class="pkgGraph" style="margin-top:10px;"></div>
          <div id="bindgenOut" class="pkgGraph" style="margin-top:10px;"></div>
        </div>
      </details>

      <details class="sideSection" open style="margin-top:12px;">
        <summary>System</summary>
        <details class="sideItem" open>
          <summary>Tests</summary>
          <div class="sideContent" id="testsOut">
            <div class="meta">Click Tests to run tests in <code>tests/</code>.</div>
            <div id="testsResults" style="margin-top:10px;"></div>
          </div>
        </details>
        <details class="sideItem">
          <summary>Settings</summary>
          <div class="sideContent" id="settingsOut">
            <div class="settingsRow">
              <label class="settingsLabel" for="settingsTheme">Theme</label>
              <select id="settingsTheme" class="settingsControl">
                <option value="oneDark">One Dark</option>
                <option value="default">Default</option>
              </select>
            </div>
            <div class="settingsRow">
              <label class="settingsLabel"><input type="checkbox" id="settingsFormatOnSave" /> Format on save</label>
            </div>
            <div class="settingsRow">
              <label class="settingsLabel" for="settingsProofMode">Proof mode</label>
              <select id="settingsProofMode" class="settingsControl">
                <option value="auto">Auto</option>
                <option value="manual">Manual</option>
              </select>
            </div>
            <div class="settingsRow">
              <label class="settingsLabel" for="settingsProofDebounce">Proof debounce (ms)</label>
              <input id="settingsProofDebounce" class="settingsControl" type="number" min="0" max="5000" step="50" />
            </div>
            <div class="meta">Auto proofs run while typing; manual runs on Proofs/Save.</div>
          </div>
        </details>
      </details>
    </div>

    <div class="panel" id="panel">
      <div class="panelHeader">
        <div class="panelTabs">
          <button class="panelTab active" id="panelTabTerminal" type="button">Terminal</button>
          <button class="panelTab" id="panelTabProblems" type="button">Problems</button>
          <button class="panelTab" id="panelTabDebug" type="button">Debug</button>
          <button class="panelTab" id="panelTabPerf" type="button">Perf</button>
        </div>
        <div class="consoleActions">
          <button id="consoleClear" class="miniBtn" type="button">Clear</button>
        </div>
      </div>

      <div class="panelBody">
        <div class="panelView" id="terminalView">
          <div class="consoleBody" id="consoleBody"></div>
          <div class="terminalBar">
            <input id="terminalInput" class="terminalInput" placeholder=">" spellcheck="false" autocomplete="off" />
          </div>
          <div class="meta" id="consoleMeta">No output yet.</div>
        </div>
        <div class="panelView" id="problemsView" style="display:none">
          <div class="problemsBody" id="problemsOut"></div>
        </div>
        <div class="panelView" id="debugView" style="display:none">
          <div class="meta" style="margin-bottom:8px;">Dev-VM debugger (step, breakpoints, watches)</div>
          <div style="display:flex; gap:8px; flex-wrap:wrap; margin-bottom:10px;">
            <button id="dbgEnable" class="miniBtn" type="button">Enable</button>
            <button id="dbgPause" class="miniBtn" type="button">Pause</button>
            <button id="dbgContinue" class="miniBtn" type="button">Continue</button>
            <button id="dbgStep" class="miniBtn" type="button">Step</button>
            <button id="dbgToggleBp" class="miniBtn" type="button">Toggle BP @ cursor</button>
            <button id="dbgClearBps" class="miniBtn" type="button">Clear BPs</button>
          </div>

          <div class="meta"><b>Breakpoints</b></div>
          <div style="display:flex; gap:8px; margin-top:6px;">
            <input id="dbgBpLine" class="settingsControl" type="number" min="1" placeholder="line" style="max-width:110px;" />
            <input id="dbgBpCond" class="settingsControl" type="text" placeholder="condition (optional)" />
            <button id="dbgBpAdd" class="miniBtn" type="button">Add</button>
          </div>
          <div id="dbgBps" style="margin-top:8px;"></div>

          <div class="meta" style="margin-top:12px;"><b>Watches</b></div>
          <div style="display:flex; gap:8px; margin-top:6px;">
            <input id="dbgWatchExpr" class="settingsControl" type="text" placeholder="expression" />
            <button id="dbgWatchAdd" class="miniBtn" type="button">Add</button>
            <button id="dbgWatchClear" class="miniBtn" type="button">Clear</button>
          </div>
          <div id="dbgWatches" style="margin-top:8px;"></div>

          <div class="meta" style="margin-top:12px;"><b>Stopped</b></div>
          <div id="dbgStopped" style="margin-top:6px;"></div>
        </div>

        <div class="panelView" id="perfView" style="display:none">
          <div class="meta" style="margin-bottom:8px;">Profiler timeline + flamegraph (Dev-VM)</div>
          <div style="display:flex; gap:8px; flex-wrap:wrap; margin-bottom:10px;">
            <button id="perfExportFolded" class="miniBtn" type="button">Export flame.folded</button>
            <button id="perfExportTimeline" class="miniBtn" type="button">Export timeline.json</button>
          </div>
          <div id="perfOut"></div>
        </div>
      </div>
    </div>
  </div>

  <div class="paletteOverlay" id="palette" style="display:none">
    <div class="palettePanel" role="dialog" aria-modal="true">
      <div class="paletteTitle" id="paletteTitle">Command Palette</div>
      <input id="paletteInput" class="paletteInput" placeholder="Type to filter…" />
      <div class="paletteList" id="paletteList"></div>
      <div class="meta" style="margin-top:8px;">Esc close · ↑/↓ navigate · Enter run</div>
    </div>
  </div>
`;

const editorHost = document.querySelector<HTMLDivElement>("#editor")!;
const workspaceLabelEl = document.querySelector<HTMLDivElement>("#workspaceLabel")!;
const recentOutEl = document.querySelector<HTMLDivElement>("#recentOut")!;
const fileTreeEl = document.querySelector<HTMLDivElement>("#fileTree")!;
const proofsOut = document.querySelector<HTMLDivElement>("#proofsOut")!;
const navOut = document.querySelector<HTMLDivElement>("#navOut")!;
const outlineOut = document.querySelector<HTMLDivElement>("#outlineOut")!;
const statusEl = document.querySelector<HTMLDivElement>("#status")!;
const crumbsEl = document.querySelector<HTMLDivElement>("#crumbs")!;
const tabsEl = document.querySelector<HTMLDivElement>("#tabs")!;
const diffBtn = document.querySelector<HTMLButtonElement>("#diff")!;
const proofsAffectedBtn = document.querySelector<HTMLButtonElement>("#proofsAffected")!;
const proofProfileBtn = document.querySelector<HTMLButtonElement>("#proofProfile")!;
const buildBtn = document.querySelector<HTMLButtonElement>("#build")!;
const runBtn = document.querySelector<HTMLButtonElement>("#run")!;
const runDevBtn = document.querySelector<HTMLButtonElement>("#runDev")!;
const stopRunBtn = document.querySelector<HTMLButtonElement>("#stopRun")!;
const testsBtn = document.querySelector<HTMLButtonElement>("#tests")!;
const pkgBtn = document.querySelector<HTMLButtonElement>("#pkg")!;
const diffPaneEl = document.querySelector<HTMLDivElement>("#diffPane")!;
const diffLeftHost = document.querySelector<HTMLDivElement>("#diffLeft")!;

const minimapEl = document.querySelector<HTMLDivElement>("#minimap")!;
const minimapViewportEl = document.querySelector<HTMLDivElement>("#minimapViewport")!;
const minimapTextEl = document.querySelector<HTMLPreElement>("#minimapText")!;

const searchQueryEl = document.querySelector<HTMLInputElement>("#searchQuery")!;
const searchResultsEl = document.querySelector<HTMLDivElement>("#searchResults")!;
const searchFileBtn = document.querySelector<HTMLButtonElement>("#searchFile")!;
const searchFolderBtn = document.querySelector<HTMLButtonElement>("#searchFolder")!;

const paletteOverlayEl = document.querySelector<HTMLDivElement>("#palette")!;
const paletteTitleEl = document.querySelector<HTMLDivElement>("#paletteTitle")!;
const paletteInputEl = document.querySelector<HTMLInputElement>("#paletteInput")!;
const paletteListEl = document.querySelector<HTMLDivElement>("#paletteList")!;

void listen<ProofStreamPayload>("proofs/stream", (event) => {
  const payload = event.payload;
  const doc = ensureActiveDoc();
  if (!doc.uri) return;
  if (payload.uri && payload.uri !== doc.uri) return;
  if (activeProofStreamId !== undefined && payload.id !== activeProofStreamId) return;

  if (payload.state === "start") {
    setStatus("Proofs: running…");
    return;
  }

  if (payload.state === "phase") {
    const p = payload.phase ? ` (${payload.phase})` : "";
    setStatus(`Proofs: running…${p}`);
    return;
  }

  if (payload.state === "cancelled") {
    return;
  }

  if (payload.state === "error") {
    activeProofStreamId = undefined;
    activeProofRunMeta = undefined;
    lastError = payload.error ?? "Proofs error";
    proofsOut.innerHTML = `<div class="meta">Proofs error: ${escapeHtml(lastError)}</div>`;
    setStatus("Proofs: error");
    return;
  }

  if (payload.state === "done") {
    activeProofStreamId = undefined;

    const incoming = normalizeDiagnostics(payload.diagnostics ?? []);
    const meta = activeProofRunMeta;
    activeProofRunMeta = undefined;

    const beforeIds = new Set((doc.mergedDiagnostics ?? []).map((d) => d.id ?? diagStableId(d)));

    if (meta?.scope === "affected" && meta.ranges.length) {
      // Merge: replace proof diagnostics that overlap affected ranges.
      const kept = (doc.proofsDiagnostics ?? []).filter((d) => !diagOverlapsAnyRange(d, meta.ranges));
      doc.proofsDiagnostics = normalizeDiagnostics([...kept, ...incoming]);
      // Clear dirty ranges we just reran.
      doc.dirtyProofRanges = [];
    } else {
      doc.proofsDiagnostics = incoming;
      // Full run = clean slate.
      doc.dirtyProofRanges = [];
    }
    doc.lspDiagnostics = normalizeDiagnostics(doc.lspDiagnostics);
    doc.mergedDiagnostics = mergeDiagnostics(doc.lspDiagnostics, doc.proofsDiagnostics);

    // Proven/untrusted dimming follows merged diagnostics.
    view.dispatch({ effects: setProvenRangesEffect.of(provenRangesForDoc(doc)) });

    // Diagnostics timeline (simple change tracking per proof run).
    const afterIds = new Set((doc.mergedDiagnostics ?? []).map((d) => d.id ?? diagStableId(d)));
    let added = 0;
    let removed = 0;
    for (const id of afterIds) if (!beforeIds.has(id)) added++;
    for (const id of beforeIds) if (!afterIds.has(id)) removed++;
    const label = meta?.scope === "affected" ? "Proofs (affected)" : "Proofs (full)";
    const nextEntry = {
      ts: Date.now(),
      label,
      count: doc.mergedDiagnostics.length,
      added,
      removed,
      sev: sevCounts(doc.mergedDiagnostics),
      detail: meta ? `profile=${meta.profile}` : undefined,
      telemetry: payload.telemetry,
    };
    doc.diagnosticsTimeline = [nextEntry, ...(doc.diagnosticsTimeline ?? [])].slice(0, 30);

    const hist = proofHistoryByUri.get(doc.uri) ?? [];
    const prev = hist.length ? hist[hist.length - 1].items : undefined;
    lastProofDelta = diffProofSnapshots(prev, doc.proofsDiagnostics);
    const nextHist = [...hist, { ts: Date.now(), items: doc.proofsDiagnostics }].slice(-20);
    proofHistoryByUri.set(doc.uri, nextHist);

    proofsOut.innerHTML = renderStructuredProofs(doc, { selectedProofId, lastProofDelta, lastTelemetry: payload.telemetry });
    view.dispatch({ effects: setDiagnosticsEffect.of(doc.mergedDiagnostics) });
    applySelectedProofHighlights(doc);
    setStatus(doc.mergedDiagnostics.length ? `Issues: ${doc.mergedDiagnostics.length}` : "Proofs: clean");
  }
});

function initMenuBar() {
  const dropdown = document.querySelector<HTMLDivElement>("#menuDropdown")!;
  const menubar = document.querySelector<HTMLDivElement>("#menubar")!;

  const menus: Record<string, Array<{ label: string; clickId: string }>> = {
    file: [
      { label: "New File", clickId: "newFile" },
      { label: "Open…", clickId: "open" },
      { label: "Open Folder…", clickId: "openFolder" },
      { label: "Save", clickId: "save" },
      { label: "Save As…", clickId: "saveAs" },
    ],
    run: [
      { label: "Build", clickId: "build" },
      { label: "Run", clickId: "run" },
      { label: "Run (Dev)", clickId: "runDev" },
      { label: "Stop", clickId: "stopRun" },
      { label: "Tests", clickId: "tests" },
    ],
    tools: [
      { label: "Proofs", clickId: "proofs" },
      { label: "Proofs: Affected Regions", clickId: "proofsAffected" },
      { label: "Proofs: Toggle Profile", clickId: "proofProfile" },
      { label: "Proofs: Clear Cache", clickId: "clearProofCache" },
      { label: "Export Proofs (JSON)", clickId: "exportProofs" },
      { label: "Export Trusted-Core Report (JSON)", clickId: "exportTrustedCoreReportJson" },
      { label: "Export Trusted-Core Report (HTML)", clickId: "exportTrustedCoreReportHtml" },
      { label: "---", clickId: "" },
      { label: "Diff", clickId: "diff" },
      { label: "Packages", clickId: "pkg" },
      { label: "Search", clickId: "searchFocus" },
      { label: "Search In File", clickId: "searchInFile" },
      { label: "Search In Folder", clickId: "searchInFolder" },
    ],
  };

  function closeAll() {
    dropdown.style.display = "none";
    for (const b of Array.from(document.querySelectorAll<HTMLButtonElement>(".menuTop[data-menu]"))) {
      b.setAttribute("aria-expanded", "false");
    }
  }

  function openMenu(btn: HTMLButtonElement, name: string) {
    const items = menus[name] ?? [];
    dropdown.innerHTML = items
      .map((it) => {
        const enc = encodeURIComponent(JSON.stringify(it));
        return `<button class="menuCmd" type="button" role="menuitem" data-cmd="${enc}">${escapeHtml(it.label)}</button>`;
      })
      .join("\n");

    const r = btn.getBoundingClientRect();
    const mb = menubar.getBoundingClientRect();
    dropdown.style.left = `${Math.max(6, r.left - mb.left)}px`;
    dropdown.style.top = `${Math.max(0, r.bottom - mb.top)}px`;
    dropdown.style.display = "block";
    for (const b of Array.from(document.querySelectorAll<HTMLButtonElement>(".menuTop[data-menu]"))) {
      b.setAttribute("aria-expanded", b === btn ? "true" : "false");
    }
  }

  menubar.addEventListener("click", (ev) => {
    const t = ev.target as HTMLElement;
    const btn = t.closest<HTMLButtonElement>(".menuTop[data-menu]");
    if (btn) {
      ev.preventDefault();
      const name = btn.dataset.menu ?? "";
      if (!name) return;
      const isOpen = dropdown.style.display !== "none" && btn.getAttribute("aria-expanded") === "true";
      if (isOpen) {
        closeAll();
      } else {
        openMenu(btn, name);
      }
      return;
    }

    const cmdBtn = t.closest<HTMLButtonElement>(".menuCmd[data-cmd]");
    if (cmdBtn) {
      ev.preventDefault();
      try {
        const it = JSON.parse(decodeURIComponent(cmdBtn.dataset.cmd ?? "")) as { label: string; clickId: string };
        if (it.clickId === "") {
          // Separator; do nothing
        } else if (it.clickId === "exportProofs") {
          void exportProofsJson();
        } else if (it.clickId === "exportTrustedCoreReportJson") {
          void exportTrustedCoreReport("json");
        } else if (it.clickId === "exportTrustedCoreReportHtml") {
          void exportTrustedCoreReport("html");
        } else {
          const target = document.querySelector<HTMLButtonElement>(`#${CSS.escape(it.clickId)}`);
          if (target) {
            target.click();
          }
        }
      } catch {
        // ignore
      } finally {
        closeAll();
      }
      return;
    }
  });

  document.addEventListener("click", (ev) => {
    const t = ev.target as HTMLElement;
    if (t.closest("#menubar")) return;
    closeAll();
  });
}

initMenuBar();

const settingsThemeEl = document.querySelector<HTMLSelectElement>("#settingsTheme")!;
const settingsFormatOnSaveEl = document.querySelector<HTMLInputElement>("#settingsFormatOnSave")!;
const settingsProofModeEl = document.querySelector<HTMLSelectElement>("#settingsProofMode")!;
const settingsProofDebounceEl = document.querySelector<HTMLInputElement>("#settingsProofDebounce")!;

const consoleBodyEl = document.querySelector<HTMLDivElement>("#consoleBody")!;
const consoleMetaEl = document.querySelector<HTMLDivElement>("#consoleMeta")!;
const consoleClearBtn = document.querySelector<HTMLButtonElement>("#consoleClear")!;

const terminalInputEl = document.querySelector<HTMLInputElement>("#terminalInput")!;
const problemsOutEl = document.querySelector<HTMLDivElement>("#problemsOut")!;
const panelTabTerminalBtn = document.querySelector<HTMLButtonElement>("#panelTabTerminal")!;
const panelTabProblemsBtn = document.querySelector<HTMLButtonElement>("#panelTabProblems")!;
const panelTabDebugBtn = document.querySelector<HTMLButtonElement>("#panelTabDebug")!;
const panelTabPerfBtn = document.querySelector<HTMLButtonElement>("#panelTabPerf")!;
const terminalViewEl = document.querySelector<HTMLDivElement>("#terminalView")!;
const problemsViewEl = document.querySelector<HTMLDivElement>("#problemsView")!;
const debugViewEl = document.querySelector<HTMLDivElement>("#debugView")!;
const perfViewEl = document.querySelector<HTMLDivElement>("#perfView")!;

const dbgEnableBtn = document.querySelector<HTMLButtonElement>("#dbgEnable")!;
const dbgPauseBtn = document.querySelector<HTMLButtonElement>("#dbgPause")!;
const dbgContinueBtn = document.querySelector<HTMLButtonElement>("#dbgContinue")!;
const dbgStepBtn = document.querySelector<HTMLButtonElement>("#dbgStep")!;
const dbgToggleBpBtn = document.querySelector<HTMLButtonElement>("#dbgToggleBp")!;
const dbgClearBpsBtn = document.querySelector<HTMLButtonElement>("#dbgClearBps")!;

const dbgBpLineEl = document.querySelector<HTMLInputElement>("#dbgBpLine")!;
const dbgBpCondEl = document.querySelector<HTMLInputElement>("#dbgBpCond")!;
const dbgBpAddBtn = document.querySelector<HTMLButtonElement>("#dbgBpAdd")!;
const dbgBpsEl = document.querySelector<HTMLDivElement>("#dbgBps")!;

const dbgWatchExprEl = document.querySelector<HTMLInputElement>("#dbgWatchExpr")!;
const dbgWatchAddBtn = document.querySelector<HTMLButtonElement>("#dbgWatchAdd")!;
const dbgWatchClearBtn = document.querySelector<HTMLButtonElement>("#dbgWatchClear")!;
const dbgWatchesEl = document.querySelector<HTMLDivElement>("#dbgWatches")!;
const dbgStoppedEl = document.querySelector<HTMLDivElement>("#dbgStopped")!;

const perfExportFoldedBtn = document.querySelector<HTMLButtonElement>("#perfExportFolded")!;
const perfExportTimelineBtn = document.querySelector<HTMLButtonElement>("#perfExportTimeline")!;
const perfOutEl = document.querySelector<HTMLDivElement>("#perfOut")!;

const testsResultsEl = document.querySelector<HTMLDivElement>("#testsResults")!;

const pkgNameEl = document.querySelector<HTMLSelectElement>("#pkgName")!;
const pkgVersionEl = document.querySelector<HTMLInputElement>("#pkgVersion")!;
const pkgUrlEl = document.querySelector<HTMLInputElement>("#pkgUrl")!;
const pkgForceEl = document.querySelector<HTMLInputElement>("#pkgForce")!;
const pkgNoSmokeEl = document.querySelector<HTMLInputElement>("#pkgNoSmoke")!;
const pkgAddBtn = document.querySelector<HTMLButtonElement>("#pkgAdd")!;
const pkgRefreshBtn = document.querySelector<HTMLButtonElement>("#pkgRefresh")!;
const pkgGraphEl = document.querySelector<HTMLDivElement>("#pkgGraph")!;
const pkgMetaEl = document.querySelector<HTMLDivElement>("#pkgMeta")!;
const bindgenRunBtn = document.querySelector<HTMLButtonElement>("#bindgenRun")!;
const bindgenOpenReportBtn = document.querySelector<HTMLButtonElement>("#bindgenOpenReport")!;
const bindgenOutEl = document.querySelector<HTMLDivElement>("#bindgenOut")!;

function toggleProofProfile() {
  settings.proofProfile = settings.proofProfile === "fast" ? "thorough" : "fast";
  saveSettings(settings);
  updateProofProfileUi();
  setStatus(`Proofs: profile = ${settings.proofProfile}`);
}

function applySettingsToUi() {
  settingsThemeEl.value = settings.theme;
  settingsFormatOnSaveEl.checked = settings.formatOnSave;
  settingsProofModeEl.value = settings.proofMode;
  settingsProofDebounceEl.value = String(settings.proofDebounceMs);
}

applySettingsToUi();

function updateProofProfileUi() {
  if (!proofProfileBtn) return;
  const label = settings.proofProfile === "fast" ? "Fast" : "Thorough";
  proofProfileBtn.textContent = `Profile: ${label}`;
}

updateProofProfileUi();

function posLt(a: LspPosition, b: LspPosition) {
  return a.line < b.line || (a.line === b.line && a.character < b.character);
}

function posLeq(a: LspPosition, b: LspPosition) {
  return a.line < b.line || (a.line === b.line && a.character <= b.character);
}

function rangeOverlaps(a: LspRange, b: LspRange) {
  // Treat ranges as [start, end) per LSP.
  return posLt(a.start, b.end) && posLt(b.start, a.end);
}

function normalizeRange(r: LspRange): LspRange {
  if (posLeq(r.start, r.end)) return r;
  return { start: r.end, end: r.start };
}

function mergeRanges(ranges: LspRange[]): LspRange[] {
  const normalized = ranges.map(normalizeRange);
  normalized.sort((a, b) => (a.start.line === b.start.line ? a.start.character - b.start.character : a.start.line - b.start.line));
  const out: LspRange[] = [];
  for (const r of normalized) {
    const last = out[out.length - 1];
    if (!last) {
      out.push(r);
      continue;
    }

    // Merge if overlapping or touching.
    const touches = posLeq(r.start, last.end) && posLeq(last.start, r.end);
    if (touches && (rangeOverlaps(last, r) || posLeq(r.start, last.end))) {
      const end = posLt(last.end, r.end) ? r.end : last.end;
      out[out.length - 1] = { start: last.start, end };
    } else {
      out.push(r);
    }
  }
  return out;
}

function diagOverlapsAnyRange(d: Diagnostic, ranges: LspRange[]) {
  const dr = d.range as any as LspRange;
  return ranges.some((r) => rangeOverlaps(dr, r));
}

type ActiveProofRunMeta = {
  scope: "full" | "affected";
  profile: "fast" | "thorough";
  ranges: LspRange[];
};

let activeProofRunMeta: ActiveProofRunMeta | undefined;

type AuraStream = "stdout" | "stderr";
type AuraOutputPayload = { id: number; stream: AuraStream; text: string };
type AuraExitPayload = { id: number; code: number | null };

type TermStream = "stdout" | "stderr";
type TermOutputPayload = { id: number; stream: TermStream; text: string };
type TermExitPayload = { id: number; code: number | null };

let consoleBufferByStream: Record<AuraStream, string> = { stdout: "", stderr: "" };
let consoleLines = 0;
const consoleMaxLines = 2500;
let lastAuraProcId: number | null = null;
let lastAuraExitCode: number | null = null;
let lastAuraExitTs: number | null = null;

let terminalSessionId: number | null = null;
let terminalBufferByStream: Record<TermStream, string> = { stdout: "", stderr: "" };

type BottomPanelTab = "terminal" | "problems" | "debug" | "perf";
let bottomPanelTab: BottomPanelTab = "terminal";

type DebugBreakpoint = { line: number; condition?: string | null };
type DebugWatch = { expr: string };

type DebugStoppedEvent = {
  event: "stopped";
  reason: string;
  file: string;
  line: number;
  col: number;
  env: Record<string, string>;
  watches: Array<{ expr: string; value?: string | null; error?: string | null }>;
};

type PerfReportEvent = {
  event: "perfReport";
  report: {
    timeline: Array<{ line: number; col: number; kind: string; dur_ns: number }>;
    flame_folded: string;
    memory: Record<string, number>;
  };
};

type NativeLaunchEvent = { event: "nativeLaunch"; exe: string };
type NativeExitEvent = { event: "nativeExit"; code?: number | null };
type HelloEvent = { event: "hello"; protocol: number; capabilities: string[] };
type TerminatedEvent = { event: "terminated"; target: string };
type AuraDebugEvent =
  | DebugStoppedEvent
  | PerfReportEvent
  | NativeLaunchEvent
  | NativeExitEvent
  | HelloEvent
  | TerminatedEvent
  | { event: string; [k: string]: any };

let dbgEnabled = false;
let dbgBreakpoints: DebugBreakpoint[] = [];
let dbgWatches: string[] = [];
let dbgLastStopped: DebugStoppedEvent | undefined;
let dbgLastNativeExe: string | undefined;
let dbgLastNativeExit: number | null | undefined;
let dbgHelloCaps: Set<string> | undefined;
let dbgLastTerminated: { target: string; ts: number } | undefined;
let perfLastReport: PerfReportEvent["report"] | undefined;

const auraCapturedOutputById = new Map<number, string>();
const auraExitWaiters = new Map<number, (code: number | null) => void>();

function normalizeNewlines(s: string) {
  return s.replace(/\r\n/g, "\n");
}

async function auraSend(cmd: any) {
  if (!lastAuraProcId) return;
  try {
    await invoke("aura_send", { id: lastAuraProcId, data: JSON.stringify(cmd) + "\n" });
  } catch (e) {
    consoleAppendLineHtml("stderr", `Debug send failed: ${String(e)}`);
  }
}

function cursorLine1(): number {
  try {
    // CodeMirror lines are 1-based.
    return view.state.doc.lineAt(view.state.selection.main.head).number;
  } catch {
    return 1;
  }
}

function renderDebugPanel() {
  const bps = dbgBreakpoints
    .slice()
    .sort((a, b) => a.line - b.line)
    .map((bp) => {
      const cond = (bp.condition ?? "").trim();
      const condHtml = cond ? ` <span class="meta">if</span> <code>${escapeHtml(cond)}</code>` : "";
      const enc = encodeURIComponent(JSON.stringify(bp));
      return `<div class="meta">line <b>${bp.line}</b>${condHtml} <button class="miniBtn" data-dbg-bp-del="${enc}">Remove</button></div>`;
    })
    .join("\n");
  dbgBpsEl.innerHTML = bps || `<div class="meta">(none)</div>`;

  const ws = dbgWatches
    .slice()
    .map((w) => {
      const enc = encodeURIComponent(w);
      return `<div class="meta"><code>${escapeHtml(w)}</code> <button class="miniBtn" data-dbg-watch-del="${enc}">Remove</button></div>`;
    })
    .join("\n");
  dbgWatchesEl.innerHTML = ws || `<div class="meta">(none)</div>`;

  if (!dbgLastStopped) {
    const termLine = dbgLastTerminated
      ? `<div class="meta">Terminated: <code>${escapeHtml(dbgLastTerminated.target)}</code></div>`
      : "";
    const nativeLine = dbgLastNativeExe
      ? `<div class="meta">Native: <code>${escapeHtml(dbgLastNativeExe)}</code>${
          dbgLastNativeExit !== undefined ? ` (exit ${escapeHtml(String(dbgLastNativeExit))})` : ""
        }</div>`
      : "";
    dbgStoppedEl.innerHTML = termLine || nativeLine || `<div class="meta">(not stopped)</div>`;
    return;
  }

  const envEntries = Object.entries(dbgLastStopped.env ?? {}).slice(0, 120);
  envEntries.sort((a, b) => a[0].localeCompare(b[0]));
  const envHtml = envEntries
    .map(([k, v]) => `<div class="meta"><code>${escapeHtml(k)}</code> = <code>${escapeHtml(String(v))}</code></div>`)
    .join("\n");

  const watchHtml = (dbgLastStopped.watches ?? [])
    .map((w) => {
      if (w.error) {
        return `<div class="meta"><code>${escapeHtml(w.expr)}</code> = <span class="meta">error</span> <code>${escapeHtml(String(w.error))}</code></div>`;
      }
      return `<div class="meta"><code>${escapeHtml(w.expr)}</code> = <code>${escapeHtml(String(w.value ?? ""))}</code></div>`;
    })
    .join("\n");

  dbgStoppedEl.innerHTML = `
    <div class="meta">${escapeHtml(dbgLastStopped.reason)} @ ${escapeHtml(dbgLastStopped.file)}:${dbgLastStopped.line}:${dbgLastStopped.col}</div>
    ${dbgLastNativeExe ? `<div class="meta">Native: <code>${escapeHtml(dbgLastNativeExe)}</code>${dbgLastNativeExit !== undefined ? ` (exit ${escapeHtml(String(dbgLastNativeExit))})` : ""}</div>` : ""}
    <div class="meta" style="margin-top:8px;"><b>Watches</b></div>
    ${watchHtml || `<div class="meta">(none)</div>`}
    <div class="meta" style="margin-top:10px;"><b>Env</b></div>
    ${envHtml || `<div class="meta">(empty)</div>`}
  `;
}

function renderPerfPanel() {
  if (!perfLastReport) {
    perfOutEl.innerHTML = `<div class="meta">(no perf report yet)</div>`;
    return;
  }

  const memHelp: Record<string, string> = {
    env_bindings: "number of env bindings",
    env_keys_bytes: "bytes of env binding keys",
    values_total: "live values tracked",
    values_int: "live ints",
    values_bool: "live bools",
    values_str: "live strings",
    values_style: "live style maps",
    style_entries: "style map entries",
    string_bytes: "UTF-8 bytes in strings",
    stdout_bytes: "captured stdout bytes",
    callbacks: "live UI callbacks",
    verify_cache_entries: "verification cache entries",
    perf_total_ns: "timeline total (ns)",
  };

  const mem = perfLastReport.memory ?? {};
  const memLines = Object.entries(mem)
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([k, v]) => {
      const help = memHelp[k] ? ` <span class="meta">(${escapeHtml(memHelp[k])})</span>` : "";
      return `<div class="meta"><code>${escapeHtml(k)}</code>: <code>${escapeHtml(String(v))}</code>${help}</div>`;
    })
    .join("\n");

  const timelineCount = (perfLastReport.timeline ?? []).length;
  const flamePreview = (perfLastReport.flame_folded ?? "").split("\n").slice(0, 60).join("\n");
  perfOutEl.innerHTML = `
    <div class="meta">Timeline events: <b>${timelineCount}</b></div>
    <div class="meta" style="margin-top:8px;"><b>Memory</b></div>
    ${memLines || `<div class="meta">(none)</div>`}
    <div class="meta" style="margin-top:10px;"><b>Flamegraph (folded preview)</b></div>
    <pre class="meta" style="white-space:pre-wrap;">${escapeHtml(flamePreview || "(empty)")}</pre>
  `;
}

function tryHandleAuraDebugLine(line: string): boolean {
  const prefix = "AURA_DEBUG_EVENT ";
  if (!line.startsWith(prefix)) return false;
  const raw = line.slice(prefix.length).trim();
  if (!raw) return true;
  try {
    const ev = JSON.parse(raw) as AuraDebugEvent;
    if (ev.event === "hello") {
      const caps = (ev as HelloEvent).capabilities ?? [];
      dbgHelloCaps = new Set(caps);
      dbgEnabled = true;
      renderDebugPanel();
    } else if (ev.event === "stopped") {
      dbgLastStopped = ev as DebugStoppedEvent;
      dbgEnabled = true;
      renderDebugPanel();
    } else if (ev.event === "perfReport") {
      perfLastReport = (ev as PerfReportEvent).report;
      renderPerfPanel();
    } else if (ev.event === "nativeLaunch") {
      dbgLastNativeExe = (ev as NativeLaunchEvent).exe;
      renderDebugPanel();
    } else if (ev.event === "nativeExit") {
      dbgLastNativeExit = (ev as NativeExitEvent).code ?? null;
      renderDebugPanel();
    } else if (ev.event === "terminated") {
      dbgLastStopped = undefined;
      dbgLastTerminated = { target: String((ev as TerminatedEvent).target ?? "unknown"), ts: Date.now() };
      dbgEnabled = true;
      renderDebugPanel();
    }
  } catch {
    // ignore malformed events
  }
  return true;
}

function consoleClear() {
  consoleBufferByStream = { stdout: "", stderr: "" };
  consoleLines = 0;
  consoleBodyEl.innerHTML = "";
  consoleMetaEl.textContent = "No output yet.";
}

function formatConsoleMeta() {
  if (!lastAuraProcId) return "No output yet.";
  if (lastAuraExitTs == null) return `Running (pid ${lastAuraProcId})…`;
  const code = lastAuraExitCode;
  const label = code === 0 ? "ok" : code == null ? "error" : `exit ${code}`;
  return `Last: pid ${lastAuraProcId} · ${label}`;
}

function auraIsRunning(): boolean {
  return !!lastAuraProcId && lastAuraExitTs == null;
}

async function runAuraStop() {
  if (!auraIsRunning() || !lastAuraProcId) {
    setStatus("Run: nothing to stop");
    return;
  }
  try {
    setBottomPanelTab("terminal");
    consoleAppendLineHtml("stderr", `== stop (pid ${lastAuraProcId}) ==`);

    // Best-effort: ask Aura to terminate before we kill the Aura process.
    // Gate on capabilities if we have them; otherwise, still try.
    if (!dbgHelloCaps || dbgHelloCaps.has("native.terminate")) {
      void auraSend({ cmd: "terminate" });
    }

    await invoke("aura_kill", { id: lastAuraProcId });
    setStatus("Run: stopping…");
  } catch (e) {
    setStatus("Run: stop failed");
    consoleAppendLineHtml("stderr", `Stop failed: ${String(e)}`);
  }
}

function sevCounts(diags: Diagnostic[]): { errors: number; warns: number; infos: number; hints: number } {
  let errors = 0;
  let warns = 0;
  let infos = 0;
  let hints = 0;
  for (const d of diags ?? []) {
    const s = d.severity ?? 0;
    if (s === 1) errors++;
    else if (s === 2) warns++;
    else if (s === 3) infos++;
    else if (s === 4) hints++;
  }
  return { errors, warns, infos, hints };
}

function consoleAppendLineHtml(stream: AuraStream, line: string) {
  const cls = stream === "stderr" ? "consoleLine stderr" : "consoleLine";
  const html = linkifyConsoleLine(line);
  consoleBodyEl.insertAdjacentHTML("beforeend", `<div class="${cls}">${html}</div>`);
  consoleLines++;

  if (consoleLines > consoleMaxLines) {
    // Drop old lines to keep DOM manageable.
    const kids = consoleBodyEl.children;
    const drop = Math.min(200, kids.length);
    for (let i = 0; i < drop; i++) {
      consoleBodyEl.removeChild(consoleBodyEl.firstElementChild!);
    }
    consoleLines -= drop;
  }

  // Keep scrolled to bottom.
  consoleBodyEl.scrollTop = consoleBodyEl.scrollHeight;
}

function setBottomPanelTab(tab: BottomPanelTab) {
  bottomPanelTab = tab;
  panelTabTerminalBtn.classList.toggle("active", tab === "terminal");
  panelTabProblemsBtn.classList.toggle("active", tab === "problems");
  panelTabDebugBtn.classList.toggle("active", tab === "debug");
  panelTabPerfBtn.classList.toggle("active", tab === "perf");
  terminalViewEl.style.display = tab === "terminal" ? "block" : "none";
  problemsViewEl.style.display = tab === "problems" ? "block" : "none";
  debugViewEl.style.display = tab === "debug" ? "block" : "none";
  perfViewEl.style.display = tab === "perf" ? "block" : "none";
  if (tab === "terminal") {
    terminalInputEl.focus();
  }
}

async function terminalEnsureStarted() {
  if (terminalSessionId != null) return;
  if (!isSdkEnabledForWorkspace()) return;
  try {
    terminalSessionId = await invoke<number>("terminal_start", { cwd: workspaceRootPath });
  } catch (e) {
    consoleAppendLineHtml("stderr", `Terminal failed to start: ${String(e)}`);
  }
}

async function terminalSendLine(line: string) {
  const v = (line ?? "").trimEnd();
  if (!v) return;
  await terminalEnsureStarted();
  if (terminalSessionId == null) return;
  try {
    await invoke("terminal_send", { id: terminalSessionId, data: v + "\n" });
  } catch (e) {
    consoleAppendLineHtml("stderr", `Terminal send failed: ${String(e)}`);
  }
}

// linkifyConsoleLine imported

function waitForAuraExit(id: number): Promise<number | null> {
  return new Promise((resolve) => {
    auraExitWaiters.set(id, (code) => resolve(code));
  });
}

async function getProjectRootForTools(): Promise<string> {
  await ensureProjectRootUpToDate();
  const repoRoot = await path.resolve("..", "..");
  const hint = workspaceRootPath ?? (currentPath() ? await path.dirname(currentPath()!) : undefined);
  return projectRootPath ?? hint ?? repoRoot;
}

function argsToDisplay(args: string[]): string {
  return args
    .map((a) => (a.includes(" ") ? JSON.stringify(a) : a))
    .join(" ");
}

async function auraSpawn(args: string[], label?: string): Promise<number> {
  const projectRoot = await getProjectRootForTools();
  const id = await invoke<number>("aura_spawn", { projectRoot, args });
  lastAuraProcId = id;
  lastAuraExitCode = null;
  lastAuraExitTs = null;
  auraCapturedOutputById.set(id, "");

  const header = label ? `== ${label} ==` : `== aura ${argsToDisplay(args)} ==`;
  consoleAppendLineHtml("stdout", header);
  consoleMetaEl.textContent = formatConsoleMeta();
  return id;
}

async function runAuraBuild() {
  setBottomPanelTab("terminal");
  const root = await getProjectRootForTools();
  const doc = ensureActiveDoc();
  if (!doc.path && doc.uri) {
    const ok = await saveActiveDocAs();
    if (!ok) return;
  }
  const docPath = currentPath();
  const target = docPath && docPath.toLowerCase().endsWith(".aura") ? docPath : await path.join(root, "main.aura");
  await auraSpawn(["build", target], "Build");
}

async function runAuraRun() {
  setBottomPanelTab("terminal");
  const root = await getProjectRootForTools();
  const doc = ensureActiveDoc();
  if (!doc.path && doc.uri) {
    const ok = await saveActiveDocAs();
    if (!ok) return;
  }
  const docPath = currentPath();
  const target = docPath && docPath.toLowerCase().endsWith(".aura") ? docPath : await path.join(root, "main.aura");
  await auraSpawn(["run", target], "Run");
}

async function runAuraRunDev() {
  setBottomPanelTab("terminal");
  if (auraIsRunning()) {
    await runAuraStop();
  }
  const root = await getProjectRootForTools();
  const doc = ensureActiveDoc();
  if (!doc.path && doc.uri) {
    const ok = await saveActiveDocAs();
    if (!ok) return;
  }
  const docPath = currentPath();
  const target = docPath && docPath.toLowerCase().endsWith(".aura") ? docPath : await path.join(root, "main.aura");
  await auraSpawn(["run", target, "--mode", "avm", "--hot"], "Run (Dev)");
}

type BindgenTrustedBoundaryReport = {
  tool: string;
  headers: string[];
  generated_shim: string;
  link: { lib_dirs: string[]; libs: string[]; c_sources: string[]; runtime_dlls: string[] };
  discovered_fns: Array<{ name: string; params: Array<[string, string]>; ret: string }>;
  notes: string[];
};

async function readBindgenReport(projectRoot: string): Promise<{ outDir: string; reportPath: string; report: BindgenTrustedBoundaryReport } | null> {
  const outDir = await path.join(projectRoot, "build", "bindgen");
  const reportPath = await path.join(outDir, "bindgen.trusted-boundary.json");
  try {
    const raw = await readTextFile(reportPath);
    const report = JSON.parse(raw) as BindgenTrustedBoundaryReport;
    if (!report || typeof report.tool !== "string") return null;
    return { outDir, reportPath, report };
  } catch {
    return null;
  }
}

function renderBindgenReportHtml(x: { outDir: string; reportPath: string; report: BindgenTrustedBoundaryReport }): string {
  const r = x.report;
  const fnCount = Array.isArray(r.discovered_fns) ? r.discovered_fns.length : 0;
  const libs = (r.link?.libs ?? []).slice(0, 20);
  const headers = (r.headers ?? []).slice(0, 8);
  const notes = (r.notes ?? []).slice(0, 6);
  const fnPreview = (r.discovered_fns ?? []).slice(0, 8);

  const rows = fnPreview
    .map((f) => {
      const params = (f.params ?? []).map((p) => `${p?.[0] ?? "_"}: ${p?.[1] ?? "?"}`).join(", ");
      return `<div class="meta"><code>${escapeHtml(String(f.name))}(${escapeHtml(params)}) -> ${escapeHtml(String(f.ret ?? ""))}</code></div>`;
    })
    .join("\n");

  return `
    <div class="meta">Bindgen trusted-boundary report</div>
    <div class="meta">Tool: ${escapeHtml(String(r.tool ?? ""))}</div>
    <div class="meta">Out: ${escapeHtml(x.outDir)}</div>
    <div class="meta">Report: ${escapeHtml(x.reportPath)}</div>
    <div class="meta">Shim: ${escapeHtml(String(r.generated_shim ?? ""))}</div>
    <div class="meta" style="margin-top:8px;"><b>Headers</b></div>
    ${(headers.length ? headers : ["(none)"]).map((h) => `<div class="meta">${escapeHtml(String(h))}</div>`).join("\n")}
    <div class="meta" style="margin-top:8px;"><b>Link</b></div>
    ${(libs.length ? libs : ["(none)"]).map((l) => `<div class="meta">lib: ${escapeHtml(String(l))}</div>`).join("\n")}
    <div class="meta" style="margin-top:8px;"><b>Discovered functions</b> (${fnCount})</div>
    ${rows || `<div class="meta">(none)</div>`}
    ${fnCount > fnPreview.length ? `<div class="meta">…and ${fnCount - fnPreview.length} more</div>` : ""}
    <div class="meta" style="margin-top:8px;"><b>Notes</b></div>
    ${(notes.length ? notes : ["(none)"]).map((n) => `<div class="meta">${escapeHtml(String(n))}</div>`).join("\n")}
  `;
}

async function refreshBindgenPanel() {
  const projectRoot = await getProjectRootForTools();
  const parsed = await readBindgenReport(projectRoot);
  if (!parsed) {
    bindgenOutEl.innerHTML = `<div class="meta">No bindgen report found. Run Bindgen…</div>`;
    return;
  }
  bindgenOutEl.innerHTML = renderBindgenReportHtml(parsed);
}

async function runAuraBindgenUi() {
  const projectRoot = await getProjectRootForTools();
  const selected = await open({
    title: "Select C header(s) for Aura-Bindgen",
    multiple: true,
    directory: false,
    filters: [{ name: "C/C++ headers", extensions: ["h", "hpp", "hh"] }],
  });
  if (!selected) return;
  const headers = Array.isArray(selected) ? selected : [selected];
  if (!headers.length) return;

  const outDir = await path.join(projectRoot, "build", "bindgen");
  const shimPath = await path.join(outDir, "bridge.aura");

  let oldShim = "";
  try {
    oldShim = await readTextFile(shimPath);
  } catch {
    oldShim = "";
  }

  const args: string[] = ["bindgen"];
  for (const h of headers) {
    args.push("--header", h);
  }
  args.push("--out", outDir);

  setBottomPanelTab("terminal");
  const id = await auraSpawn(args, "Bindgen");
  const code = await waitForAuraExit(id);
  if (code !== 0) {
    setStatus("Bindgen: failed");
    void refreshBindgenPanel();
    return;
  }

  setStatus("Bindgen: done");
  void refreshBindgenPanel();

  // Preview diff: show shim changes (old vs new).
  try {
    await openPath(shimPath);
    const doc = ensureActiveDoc();
    doc.lastSavedText = oldShim;
    if (!diffEnabled) enableDiff(true);
  } catch {
    // ignore
  }
}

async function openBindgenReportInEditor() {
  const projectRoot = await getProjectRootForTools();
  const parsed = await readBindgenReport(projectRoot);
  if (!parsed) {
    setStatus("Bindgen: no report");
    void refreshBindgenPanel();
    return;
  }
  await openPath(parsed.reportPath);
}

type TestResult = { filePath: string; status: "pass" | "fail" | "snap-created" | "snap-mismatch"; detail?: string };
let lastTestResults: TestResult[] = [];

function renderTestsResults() {
  if (!lastTestResults.length) {
    testsResultsEl.innerHTML = "";
    return;
  }
  const rows = lastTestResults
    .map((r) => {
      const badge =
        r.status === "pass"
          ? `<span class="badge">PASS</span>`
          : r.status === "snap-created"
            ? `<span class="badge">SNAP</span>`
            : `<span class="badge">FAIL</span>`;
      const detail = r.detail ? `<div class="meta">${escapeHtml(r.detail)}</div>` : "";
      const encPath = encodeURIComponent(JSON.stringify(r.filePath));
      return `<div class="testRow" data-path="${encPath}" title="Open file">${badge}${escapeHtml(r.filePath)}${detail}</div>`;
    })
    .join("\n");
  testsResultsEl.innerHTML = rows;
}

async function listAuraFilesRecursive(dir: string, depth: number): Promise<string[]> {
  if (depth > 8) return [];
  let entries: { name: string; isDirectory: boolean; isFile: boolean }[] = [];
  try {
    entries = (await readDir(dir)) as any;
  } catch {
    return [];
  }
  const out: string[] = [];
  for (const e of entries) {
    if (!e?.name) continue;
    const full = await path.join(dir, e.name);
    if (e.isDirectory) {
      out.push(...(await listAuraFilesRecursive(full, depth + 1)));
    } else if (e.isFile) {
      if (full.toLowerCase().endsWith(".aura")) out.push(full);
    }
  }
  return out;
}

async function runAuraTests() {
  const projectRoot = await getProjectRootForTools();
  const testsDir = await path.join(projectRoot, "tests");

  let testFiles: string[] = [];
  try {
    testFiles = await listAuraFilesRecursive(testsDir, 0);
  } catch {
    testFiles = [];
  }

  if (!testFiles.length) {
    lastTestResults = [];
    renderTestsResults();
    consoleAppendLineHtml("stderr", "No tests found. Create tests/*.aura");
    setStatus("Tests: none");
    return;
  }

  lastTestResults = [];
  renderTestsResults();
  setStatus(`Tests: running (${testFiles.length})…`);

  for (const tf of testFiles) {
    const id = await auraSpawn(["build", tf, "--mode", "avm"], `Test: ${tf}`);
    const code = await waitForAuraExit(id);
    const out = normalizeNewlines(auraCapturedOutputById.get(id) ?? "");

    const snapPath = `${tf}.snap`;
    let status: TestResult["status"] = code === 0 ? "pass" : "fail";
    let detail = code === 0 ? undefined : `exit ${code}`;

    try {
      const existing = normalizeNewlines(await readTextFile(snapPath));
      if (existing !== out) {
        await writeTextFile(`${snapPath}.new`, out);
        status = "snap-mismatch";
        detail = `snapshot mismatch (wrote ${snapPath}.new)`;
      }
    } catch {
      // Snapshot missing: create it.
      try {
        await writeTextFile(snapPath, out);
        status = code === 0 ? "snap-created" : "fail";
        detail = code === 0 ? `snapshot created (${snapPath})` : detail;
      } catch {
        // ignore snapshot write failures
      }
    }

    lastTestResults.push({ filePath: tf, status, detail });
    renderTestsResults();
  }

  const failed = lastTestResults.filter((r) => r.status === "fail" || r.status === "snap-mismatch").length;
  setStatus(failed ? `Tests: ${failed} failed` : "Tests: ok");
}

type LockedPkg = { name: string; version?: string; url?: string; sha256?: string };

function parseAuraLock(text: string): LockedPkg[] {
  const lines = normalizeNewlines(text).split("\n");
  const pkgs: LockedPkg[] = [];
  let cur: LockedPkg | null = null;

  for (const raw of lines) {
    const line = raw.trim();
    if (!line) continue;
    const m = /^\[packages\.([^\]]+)\]$/.exec(line);
    if (m) {
      if (cur) pkgs.push(cur);
      cur = { name: m[1] };
      continue;
    }
    const kv = /^([A-Za-z0-9_\-]+)\s*=\s*"(.*)"\s*$/.exec(line);
    if (kv && cur) {
      const key = kv[1];
      const val = kv[2];
      if (key === "version") cur.version = val;
      if (key === "url") cur.url = val;
      if (key === "sha256") cur.sha256 = val;
    }
  }
  if (cur) pkgs.push(cur);
  return pkgs;
}

function renderPkgGraph(pkgs: LockedPkg[]) {
  const locked = pkgs
    .map((p) => {
      const v = p.version ? `@${p.version}` : "";
      const url = p.url ? `<div class="meta">${escapeHtml(p.url)}</div>` : "";
      return `<div class="pkgNode"><span class="badge">DEP</span>${escapeHtml(p.name + v)}${url}</div>`;
    })
    .join("\n");

  const lockedHtml = locked
    ? `<div class="meta">Locked packages</div>${locked}`
    : `<div class="meta">No locked packages.</div>`;

  pkgGraphEl.innerHTML = lockedHtml;
}

function parseAuraImports(text: string): string[] {
  const out: string[] = [];
  const lines = (text ?? "").split(/\r?\n/);
  for (const line of lines) {
    const m = /^\s*import\s+([A-Za-z0-9_:\-]+)\s*$/.exec(line);
    if (m) out.push(m[1]);
  }
  return [...new Set(out)].sort((a, b) => a.localeCompare(b));
}

async function buildWorkspaceImportGraphHtml(): Promise<string> {
  const root = workspaceRootPath;
  if (!root) return "";
  const files = await ensureWorkspaceFiles();
  if (!files.length) return `<div class="meta">No workspace .aura files.</div>`;

  const rows: string[] = [];
  const limit = 120;
  for (const f of files.slice(0, limit)) {
    let imports: string[] = [];
    try {
      const txt = await readTextFile(f);
      imports = parseAuraImports(txt);
    } catch {
      imports = [];
    }

    const rel = f.startsWith(root) ? f.slice(root.length).replace(/^\\+/, "") : f;
    const enc = encodeURIComponent(JSON.stringify(f));
    const impHtml = imports.length
      ? imports.slice(0, 14).map((im) => `<div class="meta">import ${escapeHtml(im)}</div>`).join("\n")
      : `<div class="meta">(no imports)</div>`;
    const more = imports.length > 14 ? `<div class="meta">…and ${imports.length - 14} more</div>` : "";
    rows.push(`<div class="pkgNode" data-open-path="${enc}"><span class="badge">FILE</span>${escapeHtml(rel)}${impHtml}${more}</div>`);
  }

  const trunc = files.length > limit ? `<div class="meta">Showing ${limit}/${files.length} files.</div>` : "";
  return `<div class="meta" style="margin-top:10px;">Workspace import graph</div>${trunc}${rows.join("\n")}`;
}

async function refreshPkgGraph() {
  const projectRoot = await getProjectRootForTools();
  const lockPath = await path.join(projectRoot, "aura.lock");
  try {
    const txt = await readTextFile(lockPath);
    const pkgs = parseAuraLock(txt);
    renderPkgGraph(pkgs);
    pkgMetaEl.textContent = `Loaded from ${lockPath}`;
  } catch {
    pkgGraphEl.innerHTML = `<div class="meta">No aura.lock found in project root.</div>`;
    pkgMetaEl.textContent = "Dependency graph loads from aura.lock.";
  }

  // Also show workspace import deps (best-effort).
  try {
    const html = await buildWorkspaceImportGraphHtml();
    if (html && html.trim().length) {
      pkgGraphEl.insertAdjacentHTML("beforeend", html);
    }
  } catch {
    // ignore
  }

  void refreshBindgenPanel();
}

async function runPkgAdd() {
  const pkg = String(pkgNameEl.value ?? "").trim();
  if (!pkg) return;

  const args: string[] = ["pkg", "add", pkg];
  const v = (pkgVersionEl.value ?? "").trim();
  const u = (pkgUrlEl.value ?? "").trim();
  if (v) args.push("--version", v);
  if (u) args.push("--url", u);
  if (pkgForceEl.checked) args.push("--force");
  if (pkgNoSmokeEl.checked) args.push("--no-smoke");

  const id = await auraSpawn(args, `Pkg add: ${pkg}`);
  await waitForAuraExit(id);
  void refreshPkgGraph();
}

let workspaceRootPath: string | undefined;
let workspaceTreeHtml = "";

let workspaceFilesCache: { root: string; files: string[] } | null = null;

let unwatchWorkspace: (() => void) | null = null;
let lastWorkspaceWatchTick = 0;
let workspaceWatchTimer: number | undefined;
let workspaceWatchPending = new Set<string>();

function shouldIgnoreWatchPath(p: string): boolean {
  const s = String(p ?? "").toLowerCase().replace(/\//g, "\\");
  const needles = ["\\.git\\", "\\node_modules\\", "\\target\\", "\\dist\\", "\\build\\", "\\.aura\\cache\\"];
  return needles.some((n) => s.includes(n));
}

function scheduleWorkspaceRefresh(paths: string[]) {
  for (const p of paths) {
    if (!p) continue;
    if (shouldIgnoreWatchPath(p)) continue;
    workspaceWatchPending.add(p);
  }
  if (workspaceWatchTimer) return;
  workspaceWatchTimer = window.setTimeout(() => {
    workspaceWatchTimer = undefined;
    const pending = [...workspaceWatchPending];
    workspaceWatchPending.clear();
    if (!pending.length) return;

    const touchedAura = pending.some((p) => String(p).toLowerCase().endsWith(".aura"));
    const touchedLock = pending.some((p) => String(p).toLowerCase().endsWith("aura.lock"));
    const touchedBindgen = pending.some((p) => String(p).toLowerCase().endsWith("bindgen.trusted-boundary.json"));

    if (touchedAura) {
      workspaceFilesCache = null;
      void refreshFileTree();
      void refreshPkgGraph();
    }
    if (touchedLock) {
      void refreshPkgGraph();
    }
    if (touchedBindgen) {
      void refreshBindgenPanel();
    }
  }, 220);
}

async function startWorkspaceWatch(root: string) {
  try {
    if (unwatchWorkspace) {
      try {
        unwatchWorkspace();
      } catch {
        // ignore
      }
      unwatchWorkspace = null;
    }

    // Debounced watcher: coalesce bursts and refresh only relevant panels.
    unwatchWorkspace = await watch(
      root,
      async (ev) => {
        try {
          const now = Date.now();
          if (now - lastWorkspaceWatchTick < 150) return;
          lastWorkspaceWatchTick = now;

          const paths = Array.isArray((ev as any)?.paths) ? ((ev as any).paths as string[]) : [];
          if (!paths.length) return;

          scheduleWorkspaceRefresh(paths);
        } catch {
          // ignore watcher errors
        }
      },
      { recursive: true, delayMs: 250 }
    );
  } catch {
    // Watching is best-effort (depends on permissions/scope).
    unwatchWorkspace = null;
  }
}

type PaletteMode = "commands" | "files";
type PaletteItem = {
  label: string;
  detail?: string;
  run: () => void | Promise<void>;
};

let paletteOpen = false;
let paletteMode: PaletteMode = "commands";
let paletteAll: PaletteItem[] = [];
let paletteFiltered: PaletteItem[] = [];
let paletteIndex = 0;

function paletteSetOpen(open: boolean) {
  paletteOpen = open;
  paletteOverlayEl.style.display = paletteOpen ? "flex" : "none";
  if (paletteOpen) {
    paletteInputEl.focus();
    paletteInputEl.select();
  }
}

function paletteRender() {
  if (!paletteFiltered.length) {
    paletteListEl.innerHTML = `<div class="meta">No matches.</div>`;
    return;
  }
  const rows = paletteFiltered
    .slice(0, 100)
    .map((it, i) => {
      const active = i === paletteIndex ? "paletteItem active" : "paletteItem";
      const detail = it.detail ? `<div class="paletteDetail">${escapeHtml(it.detail)}</div>` : "";
      return `
        <div class="${active}" data-idx="${i}">
          <div class="paletteLabel">${escapeHtml(it.label)}</div>
          ${detail}
        </div>
      `;
    })
    .join("\n");
  paletteListEl.innerHTML = rows;
}

function paletteFilter() {
  const q = (paletteInputEl.value ?? "").trim().toLowerCase();
  if (!q) {
    paletteFiltered = [...paletteAll];
  } else {
    paletteFiltered = paletteAll
      .map((it) => {
        const hay = `${it.label} ${it.detail ?? ""}`.toLowerCase();
        const score = hay.startsWith(q) ? 0 : hay.includes(q) ? 1 : 99;
        return { it, score };
      })
      .filter((x) => x.score < 99)
      .sort((a, b) => a.score - b.score || a.it.label.localeCompare(b.it.label))
      .map((x) => x.it);
  }
  paletteIndex = 0;
  paletteRender();
}

async function ensureWorkspaceFiles(): Promise<string[]> {
  const root = workspaceRootPath;
  if (!root) return [];
  if (workspaceFilesCache && workspaceFilesCache.root === root) return workspaceFilesCache.files;
  const files = await collectAuraFiles(root, 900);
  workspaceFilesCache = { root, files };
  return files;
}

function openCommandPalette() {
  paletteMode = "commands";
  paletteTitleEl.textContent = "Command Palette";
  paletteInputEl.value = "";

  const commands: PaletteItem[] = [
    { label: "File: Open…", run: () => document.querySelector<HTMLButtonElement>("#open")?.click() },
    { label: "File: Open Folder…", run: () => document.querySelector<HTMLButtonElement>("#openFolder")?.click() },
    { label: "File: Save", run: () => document.querySelector<HTMLButtonElement>("#save")?.click() },
    { label: "Project: Build", run: () => void runAuraBuild() },
    { label: "Project: Run", run: () => void runAuraRun() },
    { label: "Project: Run (Dev)", detail: "Uses Dev-VM (avm) + hot loop", run: () => void runAuraRunDev() },
    { label: "Project: Stop", detail: "Stops the last running Aura process", run: () => void runAuraStop() },
    { label: "Project: Tests", run: () => void runAuraTests() },
    { label: "Pkg: Add Dependency", run: () => (pkgNameEl.focus(), void refreshPkgGraph()) },
    { label: "Bindgen: Generate from Header…", run: () => void runAuraBindgenUi() },
    { label: "Bindgen: Open Trusted-Boundary Report", run: () => void openBindgenReportInEditor() },
    { label: "Proofs: Run", run: () => document.querySelector<HTMLButtonElement>("#proofs")?.click() },
    { label: "Proofs: Rerun Affected", run: () => document.querySelector<HTMLButtonElement>("#proofsAffected")?.click() },
    { label: "Proofs: Toggle Profile (Fast/Thorough)", run: () => toggleProofProfile() },
    { label: "Proofs: Clear Cache", detail: "Clears verifier cache (RAM + disk)", run: () => void clearProofCache() },
    { label: "View: Toggle Diff", run: () => enableDiff(!diffEnabled) },
    { label: "Navigation: Go to Definition", run: () => void gotoDefinition() },
    { label: "Navigation: Find References", run: () => void findReferences() },
    { label: "Navigation: Rename Symbol", run: () => void renameSymbol() },
    { label: "Search: Focus", run: () => (searchQueryEl.focus(), searchQueryEl.select()) },
    { label: "Workspace: Refresh File Tree", run: () => void refreshFileTree() },
    { label: "LSP: Reindex (Restart)", run: () => void reindexLsp() },
  ];

  paletteAll = commands;
  paletteFiltered = [...paletteAll];
  paletteIndex = 0;
  paletteRender();
  paletteSetOpen(true);
}

async function openQuickOpen() {
  paletteMode = "files";
  paletteTitleEl.textContent = "Quick Open";
  paletteInputEl.value = "";

  if (!workspaceRootPath) {
    await openWorkspaceFolder();
  }
  if (!workspaceRootPath) {
    setStatus("Quick Open: open a folder first");
    return;
  }

  const files = await ensureWorkspaceFiles();
  const items: PaletteItem[] = files.map((p) => ({
    label: p.split("\\").pop() ?? p,
    detail: p,
    run: () => void openPath(p),
  }));

  paletteAll = items;
  paletteFiltered = [...paletteAll];
  paletteIndex = 0;
  paletteRender();
  paletteSetOpen(true);
}

type RecentProjects = {
  folders: string[];
};

function loadRecentProjects(): RecentProjects {
  try {
    const raw = localStorage.getItem("aura.sentinel.recentProjects");
    if (!raw) return { folders: [] };
    const parsed = JSON.parse(raw) as any;
    const folders = Array.isArray(parsed?.folders) ? parsed.folders.filter((x: any) => typeof x === "string") : [];
    return { folders };
  } catch {
    return { folders: [] };
  }
}

function saveRecentProjects(r: RecentProjects) {
  localStorage.setItem("aura.sentinel.recentProjects", JSON.stringify(r));
}

function addRecentFolder(p: string) {
  const r = loadRecentProjects();
  const next = [p, ...r.folders.filter((x) => x !== p)].slice(0, 10);
  saveRecentProjects({ folders: next });
}

function isSdkEnabledForWorkspace(): boolean {
  return !!workspaceRootPath && !!sdkRootPath && sdkRootPath === workspaceRootPath;
}

function canEdit(): boolean {
  const doc = activeDoc();
  if (!doc) return false;
  if (!isSdkEnabledForWorkspace()) return false;
  return !!doc.path;
}

function updateEditability() {
  try {
    view.dispatch({ effects: editabilityCompartment.reconfigure(EditorView.editable.of(canEdit())) });
  } catch {
    // ignore during early startup
  }
}

async function copyToClipboard(text: string): Promise<void> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
  } catch {
    // fall back
  }

  const ta = document.createElement("textarea");
  ta.value = text;
  ta.style.position = "fixed";
  ta.style.left = "-10000px";
  ta.style.top = "-10000px";
  document.body.appendChild(ta);
  ta.focus();
  ta.select();
  try {
    document.execCommand("copy");
  } catch {
    // ignore
  } finally {
    document.body.removeChild(ta);
  }
}

function renderProblemsPanel() {
  const doc = ensureActiveDoc();
  // Defensive de-duplication (mergedDiagnostics should already be unique).
  const dedup = new Map<string, Diagnostic>();
  for (const d of doc.mergedDiagnostics) {
    const id = d.id ?? diagStableId(d);
    const cur = dedup.get(id);
    if (!cur) {
      dedup.set(id, { ...d, id });
      continue;
    }
    const a = cur.severity ?? 99;
    const b = d.severity ?? 99;
    if (b < a) dedup.set(id, { ...d, id });
  }
  const list = [...dedup.values()].sort((a, b) => diagSortKey(a) - diagSortKey(b));

  const timeline = (doc.diagnosticsTimeline ?? []).slice(0, 30);
  const timelineHtml = timeline.length
    ? `
      <div class="meta">Diagnostics timeline</div>
      <div class="timeline">
        ${timeline
          .map((e) => {
            const t = new Date(e.ts);
            const time = t.toLocaleTimeString();
            const sev = e.sev ? ` · E${e.sev.errors} W${e.sev.warns} I${e.sev.infos} H${e.sev.hints}` : "";
            const detail = e.detail ? ` · ${escapeHtml(e.detail)}` : "";
            const tele =
              e.telemetry?.timingsMs?.total != null
                ? ` · ${escapeHtml(String(e.telemetry.timingsMs.total))}ms`
                : e.telemetry
                  ? ` · telemetry`
                  : "";
            return `<div class="timelineRow"><span class="badge">${escapeHtml(e.label)}</span> <span class="meta">${escapeHtml(time)} · ${e.count} issue(s) · +${e.added}/-${e.removed}${sev}${detail}${tele}</span></div>`;
          })
          .join("\n")}
      </div>
    `
    : "";

  if (!list.length) {
    problemsOutEl.innerHTML = timelineHtml || `<div class="meta">No problems.</div>`;
    return;
  }

  problemsOutEl.innerHTML =
    timelineHtml +
    list
    .slice(0, 800)
    .map((d) => {
      const line = d.range.start.line + 1;
      const col = d.range.start.character + 1;
      const badge = sevLabel(d.severity);
      const code = d.code ?? "";
      const src = d.source ?? "";
      const msg = d.message ?? "";
      const safe = escapeHtml(msg);
      const rangeJson = encodeURIComponent(JSON.stringify(d.range));
      const id = d.id ?? diagStableId(d);
      const hasData = (d as any).data !== undefined;
      return `
        <div class="diag" data-range="${rangeJson}" data-diag-id="${encodeURIComponent(id)}">
          <div><span class="badge">${escapeHtml(badge)}</span> <span class="meta">${escapeHtml(String(src))} ${escapeHtml(String(code))}</span></div>
          <div class="meta">${line}:${col}</div>
          <div>${safe}</div>
          <div class="diagActions">
            <button class="miniBtn" data-qf="1">Quick Fix</button>
            <button class="miniBtn" data-copy="msg">Copy</button>
            ${hasData ? `<button class="miniBtn" data-copy="json">Copy JSON</button>` : ""}
          </div>
        </div>
      `;
    })
    .join("\n");
}

function renderRecents() {
  const r = loadRecentProjects();
  if (!r.folders.length) {
    recentOutEl.innerHTML = `<div class="meta">Recent projects will appear here.</div>`;
    return;
  }
  recentOutEl.innerHTML = `
    <div class="meta">Recent projects</div>
    ${r.folders
      .map((p) => {
        const json = encodeURIComponent(JSON.stringify(p));
        return `<div class="recentRow" data-folder="${json}" title="Open folder">${escapeHtml(p)}</div>`;
      })
      .join("\n")}
  `;
}

function setWorkspaceRoot(p: string | undefined) {
  workspaceRootPath = p;
  workspaceFilesCache = null;
  if (!workspaceRootPath) {
    workspaceLabelEl.textContent = "No folder open.";
    workspaceTreeHtml = "";
    fileTreeEl.innerHTML = "";
    if (unwatchWorkspace) {
      try {
        unwatchWorkspace();
      } catch {
        // ignore
      }
      unwatchWorkspace = null;
    }
    renderRecents();
    updateEditability();
    return;
  }
  const sdkTag = isSdkEnabledForWorkspace() ? " (SDK)" : "";
  workspaceLabelEl.textContent = workspaceRootPath + sdkTag;
  addRecentFolder(workspaceRootPath);
  renderRecents();
  void refreshFileTree();
  void startWorkspaceWatch(workspaceRootPath);
  // Warm caches for quick-open/search.
  void ensureWorkspaceFiles();

  updateEditability();

  // If the project root changes, restart/reindex the LSP and reload per-project settings.
  void (async () => {
    if (!isSdkEnabledForWorkspace()) return;
    const prev = projectRootPath;
    await ensureProjectRootUpToDate();
    if (lspStarted && projectRootPath && projectRootPath !== prev) {
      await reindexLsp();
    }
  })();
}

function setSdkRoot(p: string | undefined) {
  sdkRootPath = p;
  if (sdkRootPath) localStorage.setItem("aura.sentinel.sdkRoot", sdkRootPath);
  else localStorage.removeItem("aura.sentinel.sdkRoot");

  lspStarted = false;
  terminalSessionId = null;
  terminalBufferByStream = { stdout: "", stderr: "" };
  setWorkspaceRoot(workspaceRootPath);

  if (isSdkEnabledForWorkspace()) {
    void terminalEnsureStarted();
    void reindexLsp();
  }
}

async function refreshFileTree() {
  const root = workspaceRootPath;
  if (!root) {
    fileTreeEl.innerHTML = "";
    return;
  }
  fileTreeEl.innerHTML = `<div class="meta">Loading…</div>`;
  try {
    const html = await buildWorkspaceTreeHtml(root);
    workspaceTreeHtml = html;
    workspaceFilesCache = null;
    fileTreeEl.innerHTML = html || `<div class="meta">No .aura files.</div>`;
  } catch {
    fileTreeEl.innerHTML = `<div class="meta">Failed to read folder.</div>`;
  }
}

async function buildWorkspaceTreeHtml(root: string) {
  const limit = 600;
  let count = 0;

  async function walk(dir: string, depth: number): Promise<string> {
    if (count >= limit) return "";
    if (depth > 8) return "";

    let entries: { name: string; isDirectory: boolean; isFile: boolean }[] = [];
    try {
      entries = (await readDir(dir)) as any;
    } catch {
      return "";
    }

    // Stable sort: dirs first, then files.
    const sorted = [...entries].sort((a, b) => {
      const ad = a?.isDirectory ? 0 : 1;
      const bd = b?.isDirectory ? 0 : 1;
      if (ad !== bd) return ad - bd;
      return String(a?.name ?? "").localeCompare(String(b?.name ?? ""));
    });

    const rows: string[] = [];
    for (const e of sorted) {
      if (!e?.name) continue;
      if (count >= limit) break;
      const full = await path.join(dir, e.name);
      if (e.isDirectory) {
        const child = await walk(full, depth + 1);
        // Hide empty folders.
        if (!child.trim()) continue;
        count++;
        rows.push(`
          <details class="treeDir" ${depth < 1 ? "open" : ""}>
            <summary>${escapeHtml(e.name)}</summary>
            <div class="treeChildren">${child}</div>
          </details>
        `);
      } else if (e.isFile) {
        if (!full.toLowerCase().endsWith(".aura")) continue;
        count++;
        const json = encodeURIComponent(JSON.stringify(full));
        rows.push(`<div class="treeFile" data-path="${json}" title="Open">${escapeHtml(e.name)}</div>`);
      }
    }
    return rows.join("\n");
  }

  return await walk(root, 0);
}

async function openWorkspaceFolder() {
  try {
    const selected = await open({
      title: "Open folder",
      directory: true,
      multiple: false,
    });
    if (!selected || Array.isArray(selected)) return;
    setWorkspaceRoot(selected);

    const ok = window.confirm(
      "Set this folder as Aura SDK workspace?\n\nOnly SDK workspaces enable editing, LSP, proofs, and full Aura tooling in this folder.",
    );
    if (ok) {
      setSdkRoot(selected);
      setStatus("Workspace set as SDK");
    } else {
      setSdkRoot(undefined);
      setStatus("Open folder: SDK not set");
    }
  } catch (e) {
    setStatus("Open folder: failed");
    consoleAppendLineHtml("stderr", `Open folder failed: ${String(e)}`);
  }
}

async function newUntitledDoc() {
  if (!workspaceRootPath) {
    await openWorkspaceFolder();
  }
  if (!workspaceRootPath) return;
  if (!isSdkEnabledForWorkspace()) {
    setStatus("New file: set folder as SDK first");
    return;
  }

  const cur = ensureActiveDoc();
  cur.text = getText();
  await flushIncrementalChanges();

  const nameRaw = window.prompt("New file name:", "main.aura");
  if (!nameRaw) return;
  const name = nameRaw.trim();
  if (!name) return;
  const fileName = name.toLowerCase().endsWith(".aura") ? name : `${name}.aura`;
  const filePath = await path.join(workspaceRootPath, fileName);

  try {
    await readTextFile(filePath);
    const overwrite = window.confirm(`File already exists:\n${fileName}\n\nOverwrite?`);
    if (!overwrite) return;
  } catch {
    // doesn't exist
  }

  await writeTextFile(filePath, "");

  const doc: DocState = {
    id: crypto.randomUUID(),
    path: filePath,
    uri: (await invoke<string>("file_uri", { path: filePath })) || undefined,
    text: "",
    lastSavedText: "",
    isDirty: false,
    lspOpened: false,
    pendingLspChanges: [],
    dirtyProofRanges: [],
    lspDiagnostics: [],
    proofsDiagnostics: [],
    mergedDiagnostics: [],
    diagnosticsTimeline: [],
    outlineItems: [],
  };

  docs.push(doc);
  setActiveDoc(doc.id);
  renderTabs();
  setText("");
  refreshMinimapText();
  view.dispatch({ effects: setDiagnosticsEffect.of([]) });
  renderProblems();
  renderProblemsPanel();
  renderOutline();
  renderSearchResults();

  try {
    await ensureDocOpenedInLsp(doc);
  } catch {
    // ignore
  }
  updateEditability();
  void refreshFileTree();
  setStatus(`New file: ${fileName}`);
}

async function saveActiveDocAs(): Promise<boolean> {
  const doc = ensureActiveDoc();
  try {
    const hintDir = workspaceRootPath ?? (doc.path ? await path.dirname(doc.path) : undefined);
    const defaultPath = doc.path ?? (hintDir ? await path.join(hintDir, "main.aura") : "main.aura");
    const outPath = await save({
      title: "Save Aura file",
      defaultPath,
      filters: [{ name: "Aura", extensions: ["aura"] }],
    });
    if (!outPath) return false;

    doc.text = getText();
    doc.path = String(outPath);
    doc.uri = (await invoke<string>("file_uri", { path: doc.path })) || doc.uri;
    doc.lastSavedText = doc.text;
    doc.isDirty = false;
    renderTabs();

    await writeTextFile(doc.path, doc.text);

    if (!workspaceRootPath && doc.path) {
      const dir = await path.dirname(doc.path);
      setWorkspaceRoot(dir);
    }
    if (doc.path) localStorage.setItem("aura.sentinel.lastFile", doc.path);

    if (doc.uri) {
      await ensureLspStarted();
      await invoke("did_open_text", { uri: doc.uri, text: doc.text });
      doc.lspOpened = true;
      doc.pendingLspChanges = [];
    }

    setStatus("Saved");
    void refreshProofs();
    void refreshSymbols();
    refreshMinimapText();
    if (diffEnabled) enableDiff(true);
    return true;
  } catch (e) {
    setStatus("Save As: failed");
    consoleAppendLineHtml("stderr", `Save As failed: ${String(e)}`);
    return false;
  }
}

function setStatus(text: string) {
  statusEl.textContent = text;
}

function setCrumbs(text: string) {
  crumbsEl.textContent = text;
}

function clamp(n: number, lo: number, hi: number) {
  return Math.max(lo, Math.min(hi, n));
}

function jumpTo(range: LspRange) {
  const start = toDocPos(view, range.start.line, range.start.character);
  const end = toDocPos(view, range.end.line, range.end.character);
  view.dispatch({
    selection: { anchor: start, head: end },
    scrollIntoView: true,
  });
  view.focus();
}

function getText() {
  return view.state.doc.toString();
}

function setText(text: string) {
  isProgrammaticDocSet = true;
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: text },
  });
  isProgrammaticDocSet = false;
}

function refreshMinimapText() {
  minimapTextEl.textContent = getText();
  updateMinimapViewport();
}

function updateMinimapViewport() {
  // Best-effort sync between CodeMirror scroll and minimap.
  if (!minimapEl || !minimapTextEl) return;

  const cm = view.scrollDOM;
  const docH = cm.scrollHeight;
  const viewH = cm.clientHeight;
  const scrollTop = cm.scrollTop;

  const mmH = minimapEl.clientHeight;
  const mmDocH = minimapTextEl.scrollHeight;

  const denom = Math.max(1, docH - viewH);
  const ratio = clamp(scrollTop / denom, 0, 1);

  const mmOffsetMax = Math.max(0, mmDocH - mmH);
  const mmOffset = ratio * mmOffsetMax;
  minimapTextEl.style.transform = `translateY(${-mmOffset}px)`;

  const viewportH = clamp((mmH * viewH) / Math.max(1, docH), 14, mmH);
  const viewportTop = ratio * Math.max(0, mmH - viewportH);
  minimapViewportEl.style.top = `${viewportTop}px`;
  minimapViewportEl.style.height = `${viewportH}px`;
}

function isWorkspaceEdit(x: unknown): x is WorkspaceEdit {
  if (!x || typeof x !== "object") return false;
  const w = x as any;
  return typeof w === "object" && (w.changes || w.documentChanges);
}

function toChange(view: EditorView, edit: TextEdit) {
  const from = toDocPos(view, edit.range.start.line, edit.range.start.character);
  const to = toDocPos(view, edit.range.end.line, edit.range.end.character);
  return { from, to, insert: edit.newText ?? "" };
}

function applyTextEdits(edits: TextEdit[]) {
  const changes = edits
    .map((e) => toChange(view, e))
    .sort((a, b) => (b.from - a.from) || (b.to - a.to));
  if (!changes.length) return;
  view.dispatch({ changes });
}

function applyWorkspaceEdit(edit: WorkspaceEdit) {
  // Prefer documentChanges if present.
  if (Array.isArray(edit.documentChanges)) {
    for (const dc of edit.documentChanges) {
      if (!dc?.textDocument?.uri) continue;
      const uri = currentUri();
      if (!uri || dc.textDocument.uri !== uri) continue;
      applyTextEdits(dc.edits ?? []);
    }
    return;
  }

  const uri = currentUri();
  if (edit.changes && uri) {
    const edits = edit.changes[uri];
    if (Array.isArray(edits)) applyTextEdits(edits);
  }
}

function clearSearchHighlights() {
  searchMatches = [];
  view.dispatch({ effects: setSearchMatchesEffect.of([]) });
}

function renderSearchResults() {
  const q = searchQueryEl.value ?? "";
  if (!q.trim()) {
    searchResultsEl.innerHTML = `<div class="meta">Type a query.</div>`;
    return;
  }

  if (folderMatches.length) {
    searchResultsEl.innerHTML = `
      <div class="meta">Folder matches: ${folderMatches.length}</div>
      ${folderMatches
        .map((m) => {
          const rangeJson = encodeURIComponent(JSON.stringify(m.range));
          const pathJson = encodeURIComponent(JSON.stringify(m.filePath));
          const safePrev = escapeHtml(m.preview);
          return `
            <div class="match" data-path="${pathJson}" data-range="${rangeJson}">
              <div class="meta">${escapeHtml(m.filePath)}</div>
              <div class="meta">${m.line}:${m.col}</div>
              <div>${safePrev}</div>
            </div>
          `;
        })
        .join("\n")}
    `;
    return;
  }

  searchResultsEl.innerHTML = `
    <div class="meta">Matches in file: ${searchMatches.length}</div>
    ${searchMatches
      .map((m) => {
        const rangeJson = encodeURIComponent(JSON.stringify(m.range));
        const safePrev = escapeHtml(m.preview);
        return `
          <div class="match" data-range="${rangeJson}">
            <div class="meta">${m.line}:${m.col}</div>
            <div>${safePrev}</div>
          </div>
        `;
      })
      .join("\n")}
  `;
}

function buildLineStarts(text: string) {
  const starts: number[] = [0];
  for (let i = 0; i < text.length; i++) {
    const ch = text.charCodeAt(i);
    if (ch === 10 /* \n */) starts.push(i + 1);
  }
  return starts;
}

function indexToLineCol(lineStarts: number[], idx: number) {
  // Binary search last start <= idx
  let lo = 0;
  let hi = lineStarts.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lineStarts[mid] <= idx) lo = mid + 1;
    else hi = mid - 1;
  }
  const line0 = Math.max(0, hi);
  const col = idx - lineStarts[line0];
  return { line0, col };
}

function previewForLine(text: string, lineStarts: number[], line0: number) {
  const start = lineStarts[line0] ?? 0;
  const end = line0 + 1 < lineStarts.length ? lineStarts[line0 + 1] - 1 : text.length;
  return text.slice(start, end).trim();
}

function searchInCurrentFile(query: string) {
  folderMatches = [];
  if (!query.trim()) {
    clearSearchHighlights();
    renderSearchResults();
    return;
  }

  const text = getText();
  const starts = buildLineStarts(text);
  const hits: SearchMatch[] = [];
  const ranges: LspRange[] = [];

  let idx = 0;
  while (idx <= text.length) {
    const found = text.indexOf(query, idx);
    if (found < 0) break;
    const { line0, col } = indexToLineCol(starts, found);
    const range: LspRange = {
      start: { line: line0, character: col },
      end: { line: line0, character: col + query.length },
    };
    hits.push({
      range,
      line: line0 + 1,
      col: col + 1,
      preview: previewForLine(text, starts, line0),
    });
    ranges.push(range);
    idx = found + Math.max(1, query.length);
    if (hits.length >= 500) break;
  }

  searchMatches = hits;
  view.dispatch({ effects: setSearchMatchesEffect.of(ranges) });
  renderSearchResults();
  setStatus(`Search: ${hits.length}`);

  if (hits[0]) jumpTo(hits[0].range);
}

async function collectAuraFiles(rootDir: string, limit = 500) {
  const out: string[] = [];
  const stack: string[] = [rootDir];
  while (stack.length && out.length < limit) {
    const dir = stack.pop()!;
    let entries: { name: string; isDirectory: boolean; isFile: boolean }[] = [];
    try {
      entries = (await readDir(dir)) as any;
    } catch {
      continue;
    }
    for (const e of entries) {
      if (!e?.name) continue;
      const p = await path.join(dir, e.name);
      if (e.isDirectory) {
        stack.push(p);
      } else if (e.isFile) {
        if (p.toLowerCase().endsWith(".aura")) out.push(p);
      }
      if (out.length >= limit) break;
    }
  }
  return out;
}

async function searchInFolder(query: string) {
  searchMatches = [];
  view.dispatch({ effects: setSearchMatchesEffect.of([]) });
  folderMatches = [];

  if (!query.trim()) {
    renderSearchResults();
    return;
  }

  setStatus("Folder search: scanning…");
  const rootDir = workspaceRootPath ?? (currentPath() ? await path.dirname(currentPath()!) : undefined);
  if (!rootDir) {
    searchResultsEl.innerHTML = `<div class="meta">Open a folder (or a file) first.</div>`;
    return;
  }
  const files = await collectAuraFiles(rootDir, 300);

  const results: FolderMatch[] = [];
  for (const filePath of files) {
    let content = "";
    try {
      content = await readTextFile(filePath);
    } catch {
      continue;
    }
    if (!content.includes(query)) continue;
    const starts = buildLineStarts(content);
    let idx = 0;
    while (idx <= content.length) {
      const found = content.indexOf(query, idx);
      if (found < 0) break;
      const { line0, col } = indexToLineCol(starts, found);
      const range: LspRange = {
        start: { line: line0, character: col },
        end: { line: line0, character: col + query.length },
      };
      results.push({
        filePath,
        range,
        line: line0 + 1,
        col: col + 1,
        preview: previewForLine(content, starts, line0),
      });
      idx = found + Math.max(1, query.length);
      if (results.length >= 200) break;
    }
    if (results.length >= 200) break;
  }

  folderMatches = results;
  renderSearchResults();
  setStatus(`Folder search: ${results.length}`);
}

function closeActionsMenu() {
  if (openActionsEl) {
    openActionsEl.remove();
    openActionsEl = null;
  }
}

async function showQuickFixesForDiagnostic(diag: Diagnostic, anchorEl: HTMLElement) {
  const uri = currentUri();
  if (!uri) return;
  closeActionsMenu();

  try {
    await ensureLspStarted();
    setStatus("Quick Fix: loading…");
    const res = await invoke<CodeActionsResponse>("code_actions", {
      uri,
      startLine: diag.range.start.line,
      startChar: diag.range.start.character,
      endLine: diag.range.end.line,
      endChar: diag.range.end.character,
      diagnostics: [diag],
    });

    const actions = (res?.actions ?? []).filter((a) => a && typeof (a as any).title === "string");
    const host = document.createElement("div");
    host.className = "actionsMenu";

    if (!actions.length) {
      host.innerHTML = `<div class="meta">No quick fixes.</div>`;
      anchorEl.insertAdjacentElement("afterend", host);
      openActionsEl = host;
      setStatus("Quick Fix: none");
      return;
    }

    const title = document.createElement("div");
    title.className = "actionsTitle";
    title.textContent = "Quick Fix";
    host.appendChild(title);

    for (const a of actions) {
      const btn = document.createElement("button");
      btn.className = "miniBtn";
      btn.type = "button";
      btn.textContent = (a as any).title;
      btn.addEventListener("click", async (ev) => {
        ev.preventDefault();
        ev.stopPropagation();

        const maybeEdit = (a as any).edit;
        if (!isWorkspaceEdit(maybeEdit)) {
          setStatus("Quick Fix: unsupported action");
          closeActionsMenu();
          return;
        }

        applyWorkspaceEdit(maybeEdit);
        closeActionsMenu();
        setStatus(`Applied: ${(a as any).title}`);
        await sendDidChange();
        void refreshProofs();
      });
      host.appendChild(btn);
    }

    anchorEl.insertAdjacentElement("afterend", host);
    openActionsEl = host;
    setStatus(`Quick Fix: ${actions.length} found`);
  } catch (e) {
    setStatus("Quick Fix: error");
    closeActionsMenu();
    lastError = String(e);
  }
}

async function ensureLspStarted() {
  if (lspStarted) return;
  if (!isSdkEnabledForWorkspace()) {
    throw new Error("LSP is disabled until the opened folder is set as SDK");
  }
  await ensureProjectRootUpToDate();
  const projectRoot = projectRootPath ?? sdkRootPath!;
  await invoke("start_lsp", { projectRoot });
  lspStarted = true;
}

async function detectAuraProjectRoot(hint: string): Promise<string> {
  // Walk upward looking for aura.toml, capped to avoid pathological loops.
  let cur = hint;
  for (let i = 0; i < 25; i++) {
    try {
      const cand = await path.join(cur, "aura.toml");
      await readTextFile(cand);
      return cur;
    } catch {
      // keep walking
    }
    const parent = await path.dirname(cur);
    if (!parent || parent === cur) break;
    cur = parent;
  }
  return hint;
}

async function ensureProjectRootUpToDate() {
  const hint = workspaceRootPath ?? (currentPath() ? await path.dirname(currentPath()!) : undefined);
  if (!hint) return;
  const root = await detectAuraProjectRoot(hint).catch(() => hint);
  if (root && root !== projectRootPath) {
    projectRootPath = root;
    // Reload per-project settings when project root changes.
    settings = loadSettings();
    applySettingsToUi();
    applyThemeToEditors();
    rebuildDebouncers();
  }
}

async function reindexLsp() {
  lspAutoRestarting = true;
  if (!isSdkEnabledForWorkspace()) {
    lspAutoRestarting = false;
    setStatus("LSP: set folder as SDK first");
    return;
  }
  await ensureProjectRootUpToDate();
  const projectRoot = projectRootPath ?? sdkRootPath!;

  try {
    await invoke("restart_lsp", { projectRoot });
    lspStarted = true;
    for (const d of docs) {
      d.lspOpened = false;
      d.pendingLspChanges = [];
    }
    // Re-open all open docs in the new server.
    const active = ensureActiveDoc();
    if (active.uri) active.text = getText();
    for (const d of docs) {
      if (!d.uri) continue;
      await ensureDocOpenedInLsp(d);
    }
    void refreshSymbols();
    void refreshProofs();
    setStatus("LSP: reindexed");
  } catch {
    setStatus("LSP: restart failed");
  } finally {
    lspAutoRestarting = false;
  }
}

async function ensureDocOpenedInLsp(doc: DocState) {
  if (!doc.uri) return;
  if (!isSdkEnabledForWorkspace()) return;
  try {
    await ensureLspStarted();
  } catch {
    return;
  }
  if (doc.lspOpened) return;
  await invoke("did_open_text", { uri: doc.uri, text: doc.text });
  doc.lspOpened = true;
}

function queueIncrementalChanges(update: ViewUpdate) {
  const doc = ensureActiveDoc();
  if (!doc.uri) return;

  const changes: IncrementalChange[] = [];
  update.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    const startLine = update.startState.doc.lineAt(fromA);
    const endLine = update.startState.doc.lineAt(toA);
    const range: LspRange = {
      start: { line: startLine.number - 1, character: fromA - startLine.from },
      end: { line: endLine.number - 1, character: toA - endLine.from },
    };
    changes.push({ range, text: inserted.toString(), range_length: null });
  });

  if (!changes.length) return;
  doc.pendingLspChanges.push(...changes);

  // Track dirty proof ranges for "rerun affected".
  doc.dirtyProofRanges = mergeRanges([...doc.dirtyProofRanges, ...changes.map((c) => c.range)]);
}

async function flushIncrementalChanges() {
  const doc = ensureActiveDoc();
  if (!doc.uri) return;
  if (!doc.pendingLspChanges.length) return;
  await ensureDocOpenedInLsp(doc);
  const changes = doc.pendingLspChanges.splice(0, doc.pendingLspChanges.length);
  await invoke("did_change_incremental", { uri: doc.uri, changes });
}

async function sendDidChange() {
  const uri = currentUri();
  if (!uri) return;
  await ensureLspStarted();
  await invoke("did_change_text", { uri, text: getText() });
}

function currentLspPosition() {
  const pos = view.state.selection.main.head;
  const line = view.state.doc.lineAt(pos);
  return { line: line.number - 1, character: pos - line.from };
}

function flattenOutline(items: OutlineItem[], path: OutlineItem[] = []): Array<{ path: OutlineItem[]; item: OutlineItem }> {
  const out: Array<{ path: OutlineItem[]; item: OutlineItem }> = [];
  for (const item of items) {
    const next = [...path, item];
    out.push({ path: next, item });
    if (item.children?.length) out.push(...flattenOutline(item.children, next));
  }
  return out;
}

function outlineAtPos(line0: number, char0: number): OutlineItem[] {
  // Choose the deepest symbol whose range contains the position.
  const matches: OutlineItem[][] = [];
  const doc = ensureActiveDoc();
  for (const { path } of flattenOutline(doc.outlineItems)) {
    const r = path[path.length - 1].range;
    const afterStart = line0 > r.start.line || (line0 === r.start.line && char0 >= r.start.character);
    const beforeEnd = line0 < r.end.line || (line0 === r.end.line && char0 <= r.end.character);
    if (afterStart && beforeEnd) matches.push(path);
  }
  matches.sort((a, b) => b.length - a.length);
  return matches[0] ?? [];
}

function renderOutline() {
  const doc = ensureActiveDoc();
  if (!doc.outlineItems.length) {
    outlineOut.innerHTML = `<div class="meta">No symbols.</div>`;
    return;
  }

  const rows: string[] = [];
  const walk = (items: OutlineItem[], depth: number) => {
    for (const s of items) {
      const pad = clamp(depth, 0, 8) * 12;
      const rangeJson = encodeURIComponent(JSON.stringify(s.selection_range ?? s.range));
      rows.push(
        `<div class="sym" data-range="${rangeJson}" style="padding-left:${pad}px">${escapeHtml(s.name)}</div>`
      );
      if (s.children?.length) walk(s.children, depth + 1);
    }
  };
  walk(doc.outlineItems, 0);
  outlineOut.innerHTML = rows.join("\n");
}

async function refreshSymbols() {
  const doc = ensureActiveDoc();
  if (!doc.uri) return;
  try {
    await ensureDocOpenedInLsp(doc);
    await flushIncrementalChanges();
    const res = await invoke<DocumentSymbolsResponse>("document_symbols", { uri: doc.uri });
    doc.outlineItems = res?.items ?? [];
    renderOutline();
    const p = currentLspPosition();
    const crumbPath = outlineAtPos(p.line, p.character);
    setCrumbs(crumbPath.map((x) => x.name).join(" > "));
  } catch {
    // ignore
  }
}

async function refreshProofs(opts?: { scope?: "full" | "affected"; ranges?: LspRange[] }) {
  const doc = ensureActiveDoc();
  if (!doc.uri) {
    proofsOut.innerHTML = `<div class="meta">No file open.</div>`;
    return;
  }

  const scope = opts?.scope ?? "full";
  const ranges = mergeRanges(opts?.ranges ?? []);

  if (isRefreshing) return;
  isRefreshing = true;
  lastError = undefined;

  try {
    // Make sure the server sees the latest buffer.
    doc.text = getText();
    await ensureDocOpenedInLsp(doc);
    await flushIncrementalChanges();

    if (activeProofStreamId !== undefined) {
      try {
        await invoke("proofs_stream_cancel", { id: activeProofStreamId });
      } catch {
        // ignore
      }
      activeProofStreamId = undefined;
      activeProofRunMeta = undefined;
    }

    activeProofRunMeta = {
      scope,
      profile: settings.proofProfile,
      ranges,
    };

    let id: number;
    try {
      id = await invoke<number>("proofs_stream_start_opts", {
        uri: doc.uri,
        profile: settings.proofProfile,
        scope,
        ranges,
      });
    } catch {
      // Back-compat fallback.
      id = await invoke<number>("proofs_stream_start", { uri: doc.uri });
      activeProofRunMeta = { scope: "full", profile: settings.proofProfile, ranges: [] };
    }
    activeProofStreamId = id;
    setStatus("Proofs: running…");
  } catch (e) {
    lastError = String(e);
    proofsOut.innerHTML = `<div class="meta">Proofs error: ${escapeHtml(lastError)}</div>`;
    setStatus("Proofs: error");
  } finally {
    isRefreshing = false;
  }
}

async function refreshProofsAffected() {
  const doc = ensureActiveDoc();
  if (!doc.uri) return;
  const ranges = mergeRanges(doc.dirtyProofRanges);
  if (!ranges.length) {
    setStatus("Proofs: nothing changed");
    return;
  }
  await refreshProofs({ scope: "affected", ranges });
}

async function clearProofCache() {
  if (!isSdkEnabledForWorkspace()) {
    setStatus("Proof cache: set folder as SDK first");
    return;
  }
  try {
    await ensureProjectRootUpToDate();
    await ensureLspStarted();
    const cleared = await invoke<boolean>("proof_cache_clear");
    setStatus(cleared ? "Proof cache: cleared" : "Proof cache: clear failed");
    void refreshProofs();
  } catch (e) {
    setStatus("Proof cache: clear failed");
    lastError = String(e);
  }
}

function debounce<T extends (...args: any[]) => void>(fn: T, ms: number): T {
  let t: number | undefined;
  return ((...args: any[]) => {
    if (t) window.clearTimeout(t);
    t = window.setTimeout(() => fn(...args), ms);
  }) as T;
}

let debouncedProofs = debounce(() => {
  void refreshProofs();
}, settings.proofDebounceMs);

let debouncedDidChange = debounce(() => {
  void flushIncrementalChanges();
}, 150);

let debouncedSymbols = debounce(() => {
  void refreshSymbols();
}, 400);

let debouncedMinimap = debounce(() => {
  refreshMinimapText();
}, 300);

function rebuildDebouncers() {
  debouncedProofs = debounce(() => {
    void refreshProofs();
  }, settings.proofDebounceMs);
}

async function gotoDefinition() {
  const uri = currentUri();
  if (!uri) return;
  await ensureDocOpenedInLsp(ensureActiveDoc());
  await flushIncrementalChanges();
  const p = currentLspPosition();
  const locs = await invoke<NavLocation[]>("goto_definition", {
    uri,
    line: p.line,
    character: p.character,
  });
  const first = (locs ?? [])[0];
  if (!first) {
    setStatus("Definition: not found");
    return;
  }

  if (first.uri === uri) {
    closeActionsMenu();
    jumpTo(first.range);
    setStatus("Definition: jumped");
    return;
  }

  const pth = fileUriToPath(first.uri);
  if (!pth) {
    setStatus("Definition: unsupported uri");
    return;
  }

  await openPath(pth);
  jumpTo(first.range);
  setStatus("Definition: opened");
}

async function findReferences() {
  const uri = currentUri();
  if (!uri) return;
  await ensureDocOpenedInLsp(ensureActiveDoc());
  await flushIncrementalChanges();
  const p = currentLspPosition();
  const locs = (await invoke<NavLocation[]>("find_references", {
    uri,
    line: p.line,
    character: p.character,
  })) as NavLocation[];

  const list = (locs ?? []).sort((a, b) => (a.uri === b.uri ? diagSortKey({ range: a.range } as any) - diagSortKey({ range: b.range } as any) : a.uri.localeCompare(b.uri)));
  if (!list.length) {
    navOut.innerHTML = `<div class="meta">No references.</div>`;
    setStatus("References: none");
    return;
  }

  navOut.innerHTML = list
    .map((l) => {
      const line = l.range.start.line + 1;
      const col = l.range.start.character + 1;
      const safeUri = escapeHtml(l.uri);
      const rangeJson = encodeURIComponent(JSON.stringify(l.range));
      const uriJson = encodeURIComponent(JSON.stringify(l.uri));
      return `
        <div class="ref" data-uri="${uriJson}" data-range="${rangeJson}">
          <div class="meta">${safeUri}</div>
          <div class="meta">${line}:${col}</div>
        </div>
      `;
    })
    .join("\n");
  setStatus(`References: ${list.length}`);
}

async function renameSymbol() {
  const uri = currentUri();
  if (!uri) return;
  const name = window.prompt("Rename to:");
  if (!name) return;

  await ensureDocOpenedInLsp(ensureActiveDoc());
  await flushIncrementalChanges();
  const p = currentLspPosition();
  const edit = await invoke<WorkspaceEdit>("rename_symbol", {
    uri,
    line: p.line,
    character: p.character,
    newName: name,
  });

  if (!isWorkspaceEdit(edit)) {
    setStatus("Rename: unsupported edit");
    return;
  }

  // Apply only current file edits for now.
  applyWorkspaceEdit(edit);
  setStatus("Rename: applied");
  const doc = ensureActiveDoc();
  doc.text = getText();
  doc.isDirty = doc.lastSavedText !== doc.text;
  renderTabs();
  await flushIncrementalChanges();
  void refreshProofs();
  void refreshSymbols();
}

function renderProblems() {
  const doc = ensureActiveDoc();
  const bySev = new Map<number, Diagnostic[]>();
  for (const d of doc.mergedDiagnostics) {
    const s = d.severity ?? 99;
    if (!bySev.has(s)) bySev.set(s, []);
    bySev.get(s)!.push(d);
  }

  const order = [1, 2, 3, 4, 99];
  const sections = order
    .filter((s) => (bySev.get(s)?.length ?? 0) > 0)
    .map((s) => {
      const list = (bySev.get(s) ?? []).sort((a, b) => diagSortKey(a) - diagSortKey(b));
      const label = s === 99 ? "Other" : sevLabel(s);
      const items = list
        .map((d) => {
          const line = d.range.start.line + 1;
          const col = d.range.start.character + 1;
          const badge = sevLabel(d.severity);
          const code = d.code ?? "";
          const src = d.source ?? "";
          const msg = d.message ?? "";
          const safe = escapeHtml(msg);
          const rangeJson = encodeURIComponent(JSON.stringify(d.range));
          return `
            <div class="diag" data-range="${rangeJson}">
              <div><span class="badge">${badge}</span> <span class="meta">${escapeHtml(String(src))} ${escapeHtml(String(code))}</span></div>
              <div class="meta">${line}:${col}</div>
              <div>${safe}</div>
              <div class="diagActions"><button class="miniBtn" data-qf="1">Quick Fix</button></div>
            </div>
          `;
        })
        .join("\n");

      return `
        <div class="diagSection">
          <div class="diagSectionTitle">${label} (${list.length})</div>
          ${items}
        </div>
      `;
    })
    .join("\n");

  proofsOut.innerHTML = sections || `<div class="meta">No proofs/diagnostics.</div>`;

  // Mirror a lightweight problems list into the bottom panel.
  renderProblemsPanel();
}

// Create CodeMirror view
const startState = EditorState.create({
  doc: "",
  extensions: [
    basicSetup,
    auraLanguage,
    editabilityCompartment.of(EditorView.editable.of(false)),
    diagnosticsPlugin,
    searchPlugin,
    tracePlugin,
    trustDimmingPlugin,
    diagnosticsHover,
    serverHover,
    autocompletion({ override: [lspCompletionSource] }),
    themeCompartment.of(themeExtensionFor(settings)),
    EditorView.updateListener.of((u) => {
      if (isProgrammaticDocSet) return;

      if (u.selectionSet) {
        const p = currentLspPosition();
        const crumbPath = outlineAtPos(p.line, p.character);
        setCrumbs(crumbPath.map((x) => x.name).join(" > "));
      }

      if (!u.docChanged) return;
      // Signature help: trigger when typing '(' or ','; clear on ')'.
      let inserted = "";
      u.changes.iterChanges((_fromA, _toA, _fromB, _toB, insertedText) => {
        inserted += insertedText.sliceString(0);
      });
      if (inserted.includes(")")) {
        view.dispatch({ effects: setSignatureTooltipEffect.of(null) });
      } else if (inserted.includes("(") || inserted.includes(",")) {
        void requestSignatureHelp();
      }
      const doc = ensureActiveDoc();
      if (!doc.uri) return;
      queueIncrementalChanges(u);
      doc.text = getText();
      doc.isDirty = doc.lastSavedText !== doc.text;
      renderTabs();
      setStatus("Typing…");
      debouncedDidChange();
      if (settings.proofMode === "auto") debouncedProofs();
      debouncedSymbols();
      debouncedMinimap();
    }),
  ],
});

const view = new EditorView({
  state: startState,
  parent: editorHost,
});

// Keep minimap in sync with scrolling.
view.scrollDOM.addEventListener(
  "scroll",
  () => {
    // Avoid layout thrash: schedule for next frame.
    window.requestAnimationFrame(updateMinimapViewport);
  },
  { passive: true }
);

minimapEl.addEventListener("click", (ev) => {
  const rect = minimapEl.getBoundingClientRect();
  const y = clamp(ev.clientY - rect.top, 0, rect.height);
  const ratio = rect.height > 0 ? y / rect.height : 0;

  const cm = view.scrollDOM;
  const docH = cm.scrollHeight;
  const viewH = cm.clientHeight;
  const target = ratio * Math.max(0, docH - viewH);
  cm.scrollTop = target;
});

searchFileBtn.addEventListener("click", () => {
  searchInCurrentFile(searchQueryEl.value ?? "");
});

searchFolderBtn.addEventListener("click", () => {
  void searchInFolder(searchQueryEl.value ?? "");
});

searchQueryEl.addEventListener("keydown", (ev) => {
  if (ev.key === "Enter") {
    ev.preventDefault();
    searchInCurrentFile(searchQueryEl.value ?? "");
  }
});

searchResultsEl.addEventListener("click", async (ev) => {
  const target = ev.target as HTMLElement;
  const el = target.closest<HTMLElement>(".match");
  if (!el) return;
  const encRange = el.getAttribute("data-range");
  if (!encRange) return;
  try {
    const range = JSON.parse(decodeURIComponent(encRange)) as LspRange;
    const encPath = el.getAttribute("data-path");
    if (encPath) {
      const p = JSON.parse(decodeURIComponent(encPath)) as string;
      const curPath = currentPath();
      if (p && p !== curPath) {
        await openPath(p);
      }
    }
    jumpTo(range);
  } catch {
    // ignore
  }
});

document.querySelector<HTMLButtonElement>("#open")!.addEventListener("click", async () => {
  try {
    const selected = await open({
      title: "Open Aura file",
      filters: [{ name: "Aura", extensions: ["aura"] }],
      multiple: false,
    });
    if (!selected || Array.isArray(selected)) return;

    await openPath(selected);
  } catch (e) {
    setStatus("Open: failed");
    consoleAppendLineHtml("stderr", `Open file failed: ${String(e)}`);
  }
});

document.querySelector<HTMLButtonElement>("#openFolder")!.addEventListener("click", async () => {
  await openWorkspaceFolder();
});

// Optional legacy button (may not be present depending on layout).
document.querySelector<HTMLButtonElement>("#openFolder2")?.addEventListener("click", async () => {
  await openWorkspaceFolder();
});

document.querySelector<HTMLButtonElement>("#save")!.addEventListener("click", async () => {
  const doc = ensureActiveDoc();
  if (!doc.path) {
    await saveActiveDocAs();
    return;
  }

  async function maybeFormatOnSave() {
    if (!settings.formatOnSave) return;
    if (!doc.uri) return;
    try {
      // Ensure LSP sees latest text.
      doc.text = getText();
      await ensureLspStarted();
      await invoke("did_change_text", { uri: doc.uri, text: doc.text });
      const edits = await invoke<TextEdit[]>("format_document", {
        uri: doc.uri,
        tabSize: 2,
        insertSpaces: true,
      });
      if (Array.isArray(edits) && edits.length) {
        applyTextEdits(edits);
        // Sync doc state after edits.
        doc.text = getText();
      }
    } catch {
      // ignore format errors
    }
  }

  await maybeFormatOnSave();

  doc.text = getText();
  await writeTextFile(doc.path, doc.text);
  doc.lastSavedText = doc.text;
  doc.isDirty = false;
  renderTabs();
  setStatus("Saved");
  void refreshProofs();
  void refreshSymbols();
  refreshMinimapText();
  if (diffEnabled) enableDiff(true);
});

document.querySelector<HTMLButtonElement>("#saveAs")!.addEventListener("click", async () => {
  await saveActiveDocAs();
});

document.querySelector<HTMLButtonElement>("#newFile")!.addEventListener("click", async () => {
  await newUntitledDoc();
});

document.querySelector<HTMLButtonElement>("#searchFocus")?.addEventListener("click", () => {
  searchQueryEl.focus();
  searchQueryEl.select();
});

document.querySelector<HTMLButtonElement>("#searchInFile")?.addEventListener("click", () => {
  searchQueryEl.focus();
  searchInCurrentFile(searchQueryEl.value ?? "");
});

document.querySelector<HTMLButtonElement>("#searchInFolder")?.addEventListener("click", () => {
  searchQueryEl.focus();
  void searchInFolder(searchQueryEl.value ?? "");
});

document.querySelector<HTMLButtonElement>("#proofs")!.addEventListener("click", async () => {
  await refreshProofs();
});

document.querySelector<HTMLButtonElement>("#proofsAffected")!.addEventListener("click", async () => {
  await refreshProofsAffected();
});

document.querySelector<HTMLButtonElement>("#proofProfile")!.addEventListener("click", () => {
  toggleProofProfile();
});

diffBtn.addEventListener("click", () => {
  enableDiff(!diffEnabled);
});

buildBtn.addEventListener("click", () => {
  void runAuraBuild();
});

runBtn.addEventListener("click", () => {
  void runAuraRun();
});

runDevBtn.addEventListener("click", () => {
  void runAuraRunDev();
});

stopRunBtn.addEventListener("click", () => {
  void runAuraStop();
});

testsBtn.addEventListener("click", () => {
  void runAuraTests();
});

pkgBtn.addEventListener("click", () => {
  pkgNameEl.focus();
  void refreshPkgGraph();
});

consoleClearBtn.addEventListener("click", () => {
  consoleClear();
});

panelTabTerminalBtn.addEventListener("click", () => setBottomPanelTab("terminal"));
panelTabProblemsBtn.addEventListener("click", () => {
  setBottomPanelTab("problems");
  renderProblemsPanel();
});

panelTabDebugBtn.addEventListener("click", () => {
  setBottomPanelTab("debug");
  renderDebugPanel();
});

panelTabPerfBtn.addEventListener("click", () => {
  setBottomPanelTab("perf");
  renderPerfPanel();
});

dbgEnableBtn.addEventListener("click", () => {
  dbgEnabled = true;
  void auraSend({ cmd: "enable", startPaused: false, perf: true });
  void auraSend({ cmd: "setBreakpoints", breakpoints: dbgBreakpoints });
  void auraSend({ cmd: "setWatches", watches: dbgWatches.map((expr) => ({ expr })) });
  setBottomPanelTab("debug");
  renderDebugPanel();
});

dbgPauseBtn.addEventListener("click", () => {
  void auraSend({ cmd: "pause" });
});

dbgContinueBtn.addEventListener("click", () => {
  void auraSend({ cmd: "continue" });
});

dbgStepBtn.addEventListener("click", () => {
  void auraSend({ cmd: "step" });
});

dbgToggleBpBtn.addEventListener("click", () => {
  const line = cursorLine1();
  const idx = dbgBreakpoints.findIndex((b) => b.line === line);
  if (idx >= 0) {
    dbgBreakpoints = dbgBreakpoints.filter((b) => b.line !== line);
  } else {
    dbgBreakpoints = [...dbgBreakpoints, { line }];
  }
  void auraSend({ cmd: "setBreakpoints", breakpoints: dbgBreakpoints });
  renderDebugPanel();
});

dbgClearBpsBtn.addEventListener("click", () => {
  dbgBreakpoints = [];
  void auraSend({ cmd: "setBreakpoints", breakpoints: [] });
  renderDebugPanel();
});

dbgBpAddBtn.addEventListener("click", () => {
  const line = Number(dbgBpLineEl.value);
  if (!Number.isFinite(line) || line <= 0) return;
  const cond = (dbgBpCondEl.value ?? "").trim();
  dbgBreakpoints = dbgBreakpoints.filter((b) => b.line !== line);
  dbgBreakpoints = [...dbgBreakpoints, { line, condition: cond.length ? cond : null }];
  dbgBpLineEl.value = "";
  dbgBpCondEl.value = "";
  void auraSend({ cmd: "setBreakpoints", breakpoints: dbgBreakpoints });
  renderDebugPanel();
});

dbgBpsEl.addEventListener("click", (ev) => {
  const t = ev.target as HTMLElement;
  const btn = t.closest<HTMLButtonElement>("button[data-dbg-bp-del]");
  if (!btn) return;
  try {
    const bp = JSON.parse(decodeURIComponent(btn.dataset.dbgBpDel ?? "")) as DebugBreakpoint;
    dbgBreakpoints = dbgBreakpoints.filter((b) => b.line !== bp.line);
    void auraSend({ cmd: "setBreakpoints", breakpoints: dbgBreakpoints });
    renderDebugPanel();
  } catch {
    // ignore
  }
});

dbgWatchAddBtn.addEventListener("click", () => {
  const expr = (dbgWatchExprEl.value ?? "").trim();
  if (!expr) return;
  dbgWatches = [...dbgWatches.filter((w) => w !== expr), expr];
  dbgWatchExprEl.value = "";
  void auraSend({ cmd: "setWatches", watches: dbgWatches.map((e) => ({ expr: e })) });
  renderDebugPanel();
});

dbgWatchClearBtn.addEventListener("click", () => {
  dbgWatches = [];
  void auraSend({ cmd: "setWatches", watches: [] });
  renderDebugPanel();
});

dbgWatchesEl.addEventListener("click", (ev) => {
  const t = ev.target as HTMLElement;
  const btn = t.closest<HTMLButtonElement>("button[data-dbg-watch-del]");
  if (!btn) return;
  try {
    const expr = decodeURIComponent(btn.dataset.dbgWatchDel ?? "");
    dbgWatches = dbgWatches.filter((w) => w !== expr);
    void auraSend({ cmd: "setWatches", watches: dbgWatches.map((e) => ({ expr: e })) });
    renderDebugPanel();
  } catch {
    // ignore
  }
});

perfExportFoldedBtn.addEventListener("click", () => {
  void (async () => {
    if (!perfLastReport) return;
    const root = await getProjectRootForTools();
    const outPath = await path.join(root, "perf.flame.folded");
    await writeTextFile(outPath, perfLastReport.flame_folded ?? "");
    consoleAppendLineHtml("stdout", `Wrote ${escapeHtml(outPath)}`);
    setBottomPanelTab("perf");
  })();
});

perfExportTimelineBtn.addEventListener("click", () => {
  void (async () => {
    if (!perfLastReport) return;
    const root = await getProjectRootForTools();
    const outPath = await path.join(root, "perf.timeline.json");
    await writeTextFile(outPath, JSON.stringify(perfLastReport.timeline ?? [], null, 2));
    consoleAppendLineHtml("stdout", `Wrote ${escapeHtml(outPath)}`);
    setBottomPanelTab("perf");
  })();
});

terminalInputEl.addEventListener("keydown", (ev) => {
  if (ev.key === "Enter") {
    ev.preventDefault();
    const v = terminalInputEl.value ?? "";
    terminalInputEl.value = "";
    void terminalSendLine(v);
  }
});

consoleBodyEl.addEventListener("click", async (ev) => {
  const target = ev.target as HTMLElement;
  const link = target.closest<HTMLAnchorElement>(".consoleLink");
  if (!link) return;
  ev.preventDefault();

  try {
    const p = decodeURIComponent(link.getAttribute("data-path") ?? "");
    const line = Number(decodeURIComponent(link.getAttribute("data-line") ?? ""));
    const col = Number(decodeURIComponent(link.getAttribute("data-col") ?? ""));
    if (!p || !Number.isFinite(line) || !Number.isFinite(col)) return;
    await openPath(p);
    const r: LspRange = {
      start: { line: Math.max(0, line - 1), character: Math.max(0, col - 1) },
      end: { line: Math.max(0, line - 1), character: Math.max(0, col - 1) },
    };
    jumpTo(r);
  } catch {
    // ignore
  }
});

testsResultsEl.addEventListener("click", async (ev) => {
  const target = ev.target as HTMLElement;
  const row = target.closest<HTMLElement>(".testRow");
  if (!row) return;
  const enc = row.getAttribute("data-path");
  if (!enc) return;
  try {
    const p = JSON.parse(decodeURIComponent(enc)) as string;
    if (p) await openPath(p);
  } catch {
    // ignore
  }
});

pkgAddBtn.addEventListener("click", () => {
  void runPkgAdd();
});

pkgRefreshBtn.addEventListener("click", () => {
  void refreshPkgGraph();
});

bindgenRunBtn.addEventListener("click", () => {
  void runAuraBindgenUi();
});

bindgenOpenReportBtn.addEventListener("click", () => {
  void openBindgenReportInEditor();
});

pkgGraphEl.addEventListener("click", async (ev) => {
  const target = ev.target as HTMLElement;
  const row = target.closest<HTMLElement>("[data-open-path]");
  if (!row) return;
  const enc = row.getAttribute("data-open-path");
  if (!enc) return;
  try {
    const p = JSON.parse(decodeURIComponent(enc)) as string;
    if (p) await openPath(p);
  } catch {
    // ignore
  }
});

paletteOverlayEl.addEventListener("click", (ev) => {
  const target = ev.target as HTMLElement;
  if (target === paletteOverlayEl) {
    paletteSetOpen(false);
  }
});

paletteInputEl.addEventListener("input", () => {
  paletteFilter();
});

paletteListEl.addEventListener("click", (ev) => {
  const target = ev.target as HTMLElement;
  const row = target.closest<HTMLElement>(".paletteItem");
  if (!row) return;
  const idx = Number(row.getAttribute("data-idx") ?? "0");
  const item = paletteFiltered[idx];
  if (!item) return;
  void Promise.resolve(item.run()).finally(() => paletteSetOpen(false));
});

settingsThemeEl.addEventListener("change", () => {
  const v = settingsThemeEl.value === "default" ? "default" : "oneDark";
  settings = { ...settings, theme: v };
  saveSettings(settings);
  applyThemeToEditors();
});

settingsFormatOnSaveEl.addEventListener("change", () => {
  settings = { ...settings, formatOnSave: settingsFormatOnSaveEl.checked };
  saveSettings(settings);
});

settingsProofModeEl.addEventListener("change", () => {
  const v = settingsProofModeEl.value === "manual" ? "manual" : "auto";
  settings = { ...settings, proofMode: v };
  saveSettings(settings);
});

settingsProofDebounceEl.addEventListener("change", () => {
  const n = Number(settingsProofDebounceEl.value);
  const proofDebounceMs = Number.isFinite(n) ? Math.max(0, Math.min(5000, Math.floor(n))) : defaultSettings.proofDebounceMs;
  settings = { ...settings, proofDebounceMs };
  saveSettings(settings);
  rebuildDebouncers();
});

proofsOut.addEventListener("click", (ev) => {
  const target = ev.target as HTMLElement;

  // Binding navigation links (counterexample values): highlight occurrences.
  const searchLink = target.closest<HTMLElement>("[data-search-q]");
  if (searchLink) {
    ev.preventDefault();
    ev.stopPropagation();
    const enc = searchLink.getAttribute("data-search-q");
    if (!enc) return;
    try {
      const q = decodeURIComponent(enc);
      // Highlight occurrences without mutating the search UI.
      const text = getText();
      const ranges: LspRange[] = [];
      let idx = 0;
      const limit = 200;
      while (idx <= text.length && ranges.length < limit) {
        const found = text.indexOf(q, idx);
        if (found < 0) break;
        const before = text.slice(0, found);
        const line0 = before.split("\n").length - 1;
        const lastNl = before.lastIndexOf("\n");
        const col = found - (lastNl >= 0 ? lastNl + 1 : 0);
        ranges.push({
          start: { line: line0, character: col },
          end: { line: line0, character: col + q.length },
        });
        idx = found + Math.max(1, q.length);
      }
      view.dispatch({ effects: setSearchMatchesEffect.of(ranges) });
      if (ranges[0]) jumpTo(ranges[0]);
      setStatus(`Highlight: ${q}`);
    } catch {
      // ignore
    }
    return;
  }

  // Jump links from the Explain panel related info.
  const jump = target.closest<HTMLElement>("[data-jump-range]");
  if (jump) {
    ev.preventDefault();
    ev.stopPropagation();
    const enc = jump.getAttribute("data-jump-range");
    if (!enc) return;
    try {
      const range = JSON.parse(decodeURIComponent(enc)) as LspRange;
      closeActionsMenu();
      jumpTo(range);
    } catch {
      // ignore
    }
    return;
  }

  // Export buttons
  const exportBtn = target.closest<HTMLElement>("[data-export]");
  if (exportBtn) {
    ev.preventDefault();
    const kind = exportBtn.getAttribute("data-export") === "html" ? "html" : "json";
    void exportTrustedCoreReport(kind);
    return;
  }

  // Quick Fix button inside a diagnostic entry.
  if (target && target.closest && target.closest("[data-qf='1']")) {
    ev.preventDefault();
    ev.stopPropagation();
    const diagEl = target.closest<HTMLElement>(".diag");
    if (!diagEl) return;
    const enc = diagEl.getAttribute("data-range");
    if (!enc) return;
    try {
      const range = JSON.parse(decodeURIComponent(enc)) as LspRange;
      const doc = ensureActiveDoc();
      const diag = doc.mergedDiagnostics.find((d) => JSON.stringify(d.range) === JSON.stringify(range));
      if (!diag) return;
      void showQuickFixesForDiagnostic(diag, diagEl);
    } catch {
      // ignore
    }
    return;
  }

  const diagEl = target.closest<HTMLElement>(".diag");
  if (!diagEl) return;

  const idEnc = diagEl.getAttribute("data-diag-id");
  if (idEnc) {
    try {
      selectedProofId = decodeURIComponent(idEnc);
      const doc = ensureActiveDoc();
      const lastTelemetry = (doc.diagnosticsTimeline ?? [])[0]?.telemetry;
      proofsOut.innerHTML = renderStructuredProofs(doc, { selectedProofId, lastProofDelta, lastTelemetry });
      applySelectedProofHighlights(doc);
    } catch {
      // ignore
    }
  }
  const enc = diagEl.getAttribute("data-range");
  if (!enc) return;
  try {
    const range = JSON.parse(decodeURIComponent(enc)) as LspRange;
    closeActionsMenu();
    jumpTo(range);
  } catch {
    // ignore
  }
});

problemsOutEl.addEventListener("click", (ev) => {
  const target = ev.target as HTMLElement;

  // Copy actions
  const copyBtn = target.closest<HTMLElement>("[data-copy]");
  if (copyBtn) {
    ev.preventDefault();
    ev.stopPropagation();
    const kind = copyBtn.getAttribute("data-copy");
    const diagEl = target.closest<HTMLElement>(".diag");
    if (!diagEl) return;
    const idEnc = diagEl.getAttribute("data-diag-id");
    if (!idEnc) return;
    const id = decodeURIComponent(idEnc);
    const doc = ensureActiveDoc();
    const diag = doc.mergedDiagnostics.find((d) => (d.id ?? diagStableId(d)) === id);
    if (!diag) return;
    if (kind === "json") {
      void copyToClipboard(JSON.stringify((diag as any).data ?? null, null, 2)).then(() => setStatus("Copied JSON"));
      return;
    }
    void copyToClipboard(String(diag.message ?? "")).then(() => setStatus("Copied"));
    return;
  }

  if (target && target.closest && target.closest("[data-qf='1']")) {
    ev.preventDefault();
    ev.stopPropagation();
    const diagEl = target.closest<HTMLElement>(".diag");
    if (!diagEl) return;
    const enc = diagEl.getAttribute("data-range");
    if (!enc) return;
    try {
      const range = JSON.parse(decodeURIComponent(enc)) as LspRange;
      const doc = ensureActiveDoc();
      const diag = doc.mergedDiagnostics.find((d) => JSON.stringify(d.range) === JSON.stringify(range));
      if (!diag) return;
      void showQuickFixesForDiagnostic(diag, diagEl);
    } catch {
      // ignore
    }
    return;
  }

  const diagEl = target.closest<HTMLElement>(".diag");
  if (!diagEl) return;
  const enc = diagEl.getAttribute("data-range");
  if (!enc) return;
  try {
    const range = JSON.parse(decodeURIComponent(enc)) as LspRange;
    closeActionsMenu();
    jumpTo(range);
  } catch {
    // ignore
  }
});

document.addEventListener("click", (ev) => {
  if (!openActionsEl) return;
  const t = ev.target as HTMLElement;
  if (t && (t === openActionsEl || openActionsEl.contains(t))) return;
  closeActionsMenu();
});

async function openPath(p: string) {
  try {
    // If already open, switch tabs.
    const existing = docs.find((d) => d.path === p);
    if (existing) {
      await switchToDoc(existing.id);
      return;
    }

    const uri = (await invoke<string>("file_uri", { path: p })) || undefined;
    const txt = await readTextFile(p);
    const doc: DocState = {
      id: crypto.randomUUID(),
      path: p,
      uri,
      text: txt,
      lastSavedText: txt,
      isDirty: false,
      lspOpened: false,
      pendingLspChanges: [],
      dirtyProofRanges: [],
      lspDiagnostics: [],
      proofsDiagnostics: [],
      mergedDiagnostics: [],
      diagnosticsTimeline: [],
      outlineItems: [],
    };

    docs.push(doc);
    setActiveDoc(doc.id);
    renderTabs();

    // If no workspace is set, default to this file's folder.
    if (!workspaceRootPath && doc.path) {
      const dir = await path.dirname(doc.path);
      setWorkspaceRoot(dir);
    }

    localStorage.setItem("aura.sentinel.lastFile", p);

    setText(doc.text);
    refreshMinimapText();
    setStatus("Idle");
    if (doc.uri) {
      await ensureDocOpenedInLsp(doc);
      void refreshProofs();
      void refreshSymbols();
    }
    proofsOut.innerHTML = `<div class="meta">Loaded: ${p}</div>`;
    if (diffEnabled) enableDiff(true);
  } catch (e) {
    setStatus("Open: error");
    consoleAppendLineHtml("stderr", `Open failed: ${String(e)}`);
  }
}

async function switchToDoc(id: string) {
  const next = docs.find((d) => d.id === id);
  if (!next) return;

  // Persist current doc state
  const cur = ensureActiveDoc();
  cur.text = getText();
  await flushIncrementalChanges();

  setActiveDoc(id);
  renderTabs();

  setText(next.text);
  refreshMinimapText();
  view.dispatch({ effects: setDiagnosticsEffect.of(next.mergedDiagnostics) });
  view.dispatch({ effects: setSearchMatchesEffect.of([]) });
  renderProblems();
  renderOutline();
  renderSearchResults();

  if (next.uri) {
    await ensureDocOpenedInLsp(next);
    void refreshSymbols();
  }
  if (diffEnabled) enableDiff(true);
}

function closeDoc(id: string) {
  if (docs.length <= 1) return;
  const idx = docs.findIndex((d) => d.id === id);
  if (idx < 0) return;
  const wasActive = docs[idx].id === activeDocId;
  docs.splice(idx, 1);
  if (wasActive) {
    const next = docs[Math.max(0, idx - 1)];
    activeDocId = next.id;
    void switchToDoc(next.id);
  }
  renderTabs();
}

function renderTabs() {
  ensureActiveDoc();
  const items = docs
    .map((d) => {
      const name = d.title?.trim() || (d.path ? d.path.split("\\").pop() ?? d.path : "Untitled");
      const dirty = d.isDirty ? "<span class=\"tabDot\" title=\"Unsaved\"></span>" : "";
      const active = d.id === activeDocId ? "tab active" : "tab";
      const idJson = encodeURIComponent(d.id);

      const isRenaming = renamingDocId === d.id;
      const nameHtml = isRenaming
        ? `<input class="tabRename" data-rename="1" value="${escapeHtml(renamingTabValue || name)}" />`
        : `<span class="tabName" data-name="1">${escapeHtml(name)}</span>`;
      return `
        <div class="${active}" data-id="${idJson}">
          ${dirty}${nameHtml}
          <button class="tabClose" data-close="1" title="Close">×</button>
        </div>
      `;
    })
    .join("\n");
  tabsEl.innerHTML = items;

  if (renamingDocId) {
    requestAnimationFrame(() => {
      const input = tabsEl.querySelector<HTMLInputElement>(`.tab[data-id="${encodeURIComponent(renamingDocId!)}"] .tabRename`);
      input?.focus();
      input?.select();
    });
  }
}

tabsEl.addEventListener("click", (ev) => {
  const target = ev.target as HTMLElement;
  if (target.closest("[data-rename='1']")) return;
  const tab = target.closest<HTMLElement>(".tab");
  if (!tab) return;
  const id = tab.getAttribute("data-id");
  if (!id) return;
  const docId = decodeURIComponent(id);

  if (target.closest("[data-close='1']")) {
    ev.preventDefault();
    ev.stopPropagation();
    closeDoc(docId);
    return;
  }

  void switchToDoc(docId);
});

tabsEl.addEventListener("dblclick", (ev) => {
  const target = ev.target as HTMLElement;
  const name = target.closest<HTMLElement>("[data-name='1']");
  if (!name) return;
  const tab = target.closest<HTMLElement>(".tab");
  if (!tab) return;
  const id = tab.getAttribute("data-id");
  if (!id) return;
  const docId = decodeURIComponent(id);
  const doc = docs.find((d) => d.id === docId);
  if (!doc) return;

  renamingDocId = docId;
  renamingTabValue = (doc.title?.trim() || (doc.path ? doc.path.split("\\").pop() ?? doc.path : "Untitled")) ?? "";
  renderTabs();
});

tabsEl.addEventListener("keydown", (ev) => {
  const target = ev.target as HTMLElement;
  const input = target.closest<HTMLInputElement>(".tabRename");
  if (!input) return;

  if (ev.key === "Escape") {
    ev.preventDefault();
    renamingDocId = undefined;
    renamingTabValue = "";
    renderTabs();
    return;
  }

  if (ev.key === "Enter") {
    ev.preventDefault();
    input.blur();
  }
});

tabsEl.addEventListener("input", (ev) => {
  const target = ev.target as HTMLElement;
  const input = target.closest<HTMLInputElement>(".tabRename");
  if (!input) return;
  renamingTabValue = input.value;
});

tabsEl.addEventListener(
  "focusout",
  (ev) => {
    const target = ev.target as HTMLElement;
    const input = target.closest<HTMLInputElement>(".tabRename");
    if (!input) return;
    const tab = input.closest<HTMLElement>(".tab");
    if (!tab) return;
    const id = tab.getAttribute("data-id");
    if (!id) return;
    const docId = decodeURIComponent(id);
    const doc = docs.find((d) => d.id === docId);
    if (!doc) return;

    const nextTitle = (input.value ?? "").trim();
    doc.title = nextTitle || undefined;
    renamingDocId = undefined;
    renamingTabValue = "";
    renderTabs();
  },
  true,
);

recentOutEl.addEventListener("click", (ev) => {
  const target = ev.target as HTMLElement;
  const row = target.closest<HTMLElement>(".recentRow");
  if (!row) return;
  const enc = row.getAttribute("data-folder");
  if (!enc) return;
  try {
    const folder = JSON.parse(decodeURIComponent(enc)) as string;
    if (!folder) return;
    setWorkspaceRoot(folder);
  } catch {
    // ignore
  }
});

fileTreeEl.addEventListener("click", async (ev) => {
  const target = ev.target as HTMLElement;
  const file = target.closest<HTMLElement>(".treeFile");
  if (!file) return;
  const enc = file.getAttribute("data-path");
  if (!enc) return;
  try {
    const p = JSON.parse(decodeURIComponent(enc)) as string;
    if (!p) return;
    await openPath(p);
  } catch {
    // ignore
  }
});

outlineOut.addEventListener("click", (ev) => {
  const target = ev.target as HTMLElement;
  const el = target.closest<HTMLElement>(".sym");
  if (!el) return;
  const enc = el.getAttribute("data-range");
  if (!enc) return;
  try {
    const range = JSON.parse(decodeURIComponent(enc)) as LspRange;
    jumpTo(range);
  } catch {
    // ignore
  }
});

navOut.addEventListener("click", async (ev) => {
  const target = ev.target as HTMLElement;
  const el = target.closest<HTMLElement>(".ref");
  if (!el) return;
  const encRange = el.getAttribute("data-range");
  const encUri = el.getAttribute("data-uri");
  if (!encRange || !encUri) return;
  try {
    const range = JSON.parse(decodeURIComponent(encRange)) as LspRange;
    const uri = JSON.parse(decodeURIComponent(encUri)) as string;
    if (uri === currentUri()) {
      jumpTo(range);
      return;
    }
    const pth = fileUriToPath(uri);
    if (!pth) return;
    await openPath(pth);
    jumpTo(range);
  } catch {
    // ignore
  }
});

// Keyboard shortcuts: F12 definition, Shift+F12 references, F2 rename, Ctrl+S save.
document.addEventListener("keydown", (ev) => {
  if (paletteOpen) {
    if (ev.key === "Escape") {
      ev.preventDefault();
      paletteSetOpen(false);
      return;
    }
    if (ev.key === "ArrowDown") {
      ev.preventDefault();
      paletteIndex = Math.min(paletteIndex + 1, Math.max(0, Math.min(99, paletteFiltered.length - 1)));
      paletteRender();
      return;
    }
    if (ev.key === "ArrowUp") {
      ev.preventDefault();
      paletteIndex = Math.max(0, paletteIndex - 1);
      paletteRender();
      return;
    }
    if (ev.key === "Enter") {
      ev.preventDefault();
      const item = paletteFiltered[paletteIndex];
      if (!item) return;
      void Promise.resolve(item.run()).finally(() => paletteSetOpen(false));
      return;
    }
    return;
  }

  if ((ev.ctrlKey || ev.metaKey) && ev.shiftKey && (ev.key === "p" || ev.key === "P")) {
    ev.preventDefault();
    openCommandPalette();
    return;
  }

  if ((ev.ctrlKey || ev.metaKey) && !ev.shiftKey && (ev.key === "p" || ev.key === "P")) {
    ev.preventDefault();
    void openQuickOpen();
    return;
  }

  if (ev.key === "F12" && !ev.shiftKey && !ev.ctrlKey && !ev.altKey) {
    ev.preventDefault();
    void gotoDefinition();
    return;
  }
  if (ev.key === "F12" && ev.shiftKey) {
    ev.preventDefault();
    void findReferences();
    return;
  }
  if (ev.key === "F2" && !ev.ctrlKey && !ev.altKey) {
    ev.preventDefault();
    void renameSymbol();
    return;
  }

  // Run/stop shortcuts: F5 run-dev, Shift+F5 stop.
  if (ev.key === "F5" && !ev.ctrlKey && !ev.altKey) {
    ev.preventDefault();
    if (ev.shiftKey) {
      void runAuraStop();
    } else {
      void runAuraRunDev();
    }
    return;
  }

  if ((ev.ctrlKey || ev.metaKey) && ev.shiftKey && ev.code === "Space") {
    ev.preventDefault();
    void requestSignatureHelp();
    return;
  }
  if ((ev.ctrlKey || ev.metaKey) && (ev.key === "s" || ev.key === "S")) {
    ev.preventDefault();
    document.querySelector<HTMLButtonElement>("#save")?.click();
  }

  if ((ev.ctrlKey || ev.metaKey) && (ev.key === "f" || ev.key === "F")) {
    ev.preventDefault();
    searchQueryEl.focus();
    searchQueryEl.select();
  }

  if ((ev.ctrlKey || ev.metaKey) && ev.shiftKey && (ev.key === "f" || ev.key === "F")) {
    ev.preventDefault();
    searchQueryEl.focus();
    searchQueryEl.select();
    void searchInFolder(searchQueryEl.value ?? "");
  }
});

// Stream LSP publishDiagnostics from the backend
type PublishDiagnosticsPayload = { uri: string; diagnostics: Diagnostic[] };
void listen<PublishDiagnosticsPayload>("lsp/diagnostics", (event) => {
  if (!event.payload) return;

  // Route diagnostics to the matching doc.
  const doc = docs.find((d) => d.uri === event.payload!.uri);
  if (!doc) return;

  const beforeIds = new Set((doc.mergedDiagnostics ?? []).map((d) => d.id ?? diagStableId(d)));

  doc.lspDiagnostics = normalizeDiagnostics(event.payload.diagnostics ?? []);
  doc.proofsDiagnostics = normalizeDiagnostics(doc.proofsDiagnostics);
  doc.mergedDiagnostics = mergeDiagnostics(doc.lspDiagnostics, doc.proofsDiagnostics);

  const afterIds = new Set((doc.mergedDiagnostics ?? []).map((d) => d.id ?? diagStableId(d)));
  let added = 0;
  let removed = 0;
  for (const id of afterIds) if (!beforeIds.has(id)) added++;
  for (const id of beforeIds) if (!afterIds.has(id)) removed++;
  const nextEntry = {
    ts: Date.now(),
    label: "LSP",
    count: doc.mergedDiagnostics.length,
    added,
    removed,
    sev: sevCounts(doc.mergedDiagnostics),
  };
  doc.diagnosticsTimeline = [nextEntry, ...(doc.diagnosticsTimeline ?? [])].slice(0, 30);

  if (doc.id === activeDocId) {
    view.dispatch({ effects: setDiagnosticsEffect.of(doc.mergedDiagnostics) });
    view.dispatch({ effects: setProvenRangesEffect.of(provenRangesForDoc(doc)) });
    applySelectedProofHighlights(doc);
    renderProblems();
  }
});

// Stream Aura CLI output from the backend
void listen<AuraOutputPayload>("aura/output", (event) => {
  const p = event.payload;
  if (!p) return;

  lastAuraProcId = p.id;
  lastAuraExitTs = null;
  lastAuraExitCode = null;

  // Capture raw output for snapshot testing.
  auraCapturedOutputById.set(p.id, (auraCapturedOutputById.get(p.id) ?? "") + p.text);

  const cur = consoleBufferByStream[p.stream] ?? "";
  const combined = normalizeNewlines(cur + p.text);
  const parts = combined.split("\n");
  const tail = parts.pop() ?? "";
  consoleBufferByStream[p.stream] = tail;
  for (const line of parts) {
    if (!tryHandleAuraDebugLine(line)) {
      consoleAppendLineHtml(p.stream, line);
    }
  }

  consoleMetaEl.textContent = formatConsoleMeta();
});

// Stream interactive terminal output from the backend
void listen<TermOutputPayload>("term/output", (event) => {
  const p = event.payload;
  if (!p) return;

  const cur = terminalBufferByStream[p.stream] ?? "";
  const combined = normalizeNewlines(cur + p.text);
  const parts = combined.split("\n");
  const tail = parts.pop() ?? "";
  terminalBufferByStream[p.stream] = tail;
  for (const line of parts) {
    consoleAppendLineHtml(p.stream, line);
  }
});

void listen<TermExitPayload>("term/exit", (event) => {
  const p = event.payload;
  if (!p) return;
  if (terminalSessionId === p.id) {
    terminalSessionId = null;
  }

  // Flush any remaining partial terminal line buffers.
  for (const stream of ["stdout", "stderr"] as TermStream[]) {
    const tail = terminalBufferByStream[stream];
    if (tail && tail.trim().length) {
      consoleAppendLineHtml(stream, tail);
    }
    terminalBufferByStream[stream] = "";
  }

  consoleAppendLineHtml("stderr", `Terminal exited (${p.code ?? "?"})`);
});

void listen<AuraExitPayload>("aura/exit", (event) => {
  const p = event.payload;
  if (!p) return;

  lastAuraProcId = p.id;
  lastAuraExitCode = p.code;
  lastAuraExitTs = Date.now();

  // Flush any remaining partial line buffers.
  for (const stream of ["stdout", "stderr"] as AuraStream[]) {
    const tail = consoleBufferByStream[stream];
    if (tail && tail.trim().length) {
      consoleAppendLineHtml(stream, tail);
    }
    consoleBufferByStream[stream] = "";
  }

  consoleMetaEl.textContent = formatConsoleMeta();
  const waiter = auraExitWaiters.get(p.id);
  if (waiter) {
    auraExitWaiters.delete(p.id);
    waiter(p.code);
  }
});

type LspStatusPayload = { state: "started" | "stopped"; root?: string };
let lspAutoRestartTimer: number | undefined;

void listen<LspStatusPayload>("lsp/status", (event) => {
  const p = event.payload;
  if (!p) return;
  if (p.state === "started") {
    lspStarted = true;
    return;
  }

  if (p.state === "stopped") {
    lspStarted = false;
    if (lspAutoRestarting) return;
    if (lspAutoRestartTimer) window.clearTimeout(lspAutoRestartTimer);
    lspAutoRestartTimer = window.setTimeout(() => {
      void reindexLsp();
    }, 400);
  }
});

// Load persisted SDK selection (if any) and apply initial gating.
sdkRootPath = localStorage.getItem("aura.sentinel.sdkRoot") || undefined;
updateEditability();

// Reopen last file best-effort.
const last = localStorage.getItem("aura.sentinel.lastFile");
ensureActiveDoc();
renderTabs();
if (last) void openPath(last);

function enableDiff(on: boolean) {
  diffEnabled = on;
  diffBtn.classList.toggle("active", diffEnabled);
  diffPaneEl.style.display = diffEnabled ? "block" : "none";
  minimapEl.style.display = diffEnabled ? "none" : "block";

  if (!diffEnabled) {
    if (diffLeftView) {
      diffLeftView.destroy();
      diffLeftView = null;
    }
    return;
  }

  const doc = ensureActiveDoc();
  const leftState = EditorState.create({
    doc: doc.lastSavedText ?? "",
    extensions: [
      basicSetup,
      auraLanguage,
      themeExtensionFor(settings),
      EditorView.editable.of(false),
      EditorView.updateListener.of(() => void 0),
    ],
  });

  if (diffLeftView) {
    diffLeftView.setState(leftState);
  } else {
    diffLeftView = new EditorView({ state: leftState, parent: diffLeftHost });
    // Sync scroll between panes.
    diffLeftView.scrollDOM.addEventListener(
      "scroll",
      () => {
        if (diffSyncing) return;
        diffSyncing = true;
        view.scrollDOM.scrollTop = diffLeftView!.scrollDOM.scrollTop;
        window.requestAnimationFrame(() => (diffSyncing = false));
      },
      { passive: true }
    );

    view.scrollDOM.addEventListener(
      "scroll",
      () => {
        if (!diffEnabled || !diffLeftView) return;
        if (diffSyncing) return;
        diffSyncing = true;
        diffLeftView.scrollDOM.scrollTop = view.scrollDOM.scrollTop;
        window.requestAnimationFrame(() => (diffSyncing = false));
      },
      { passive: true }
    );
  }
}
