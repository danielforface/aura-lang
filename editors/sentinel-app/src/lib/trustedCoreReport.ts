import type { DiagnosticLike } from "./diagnostics";
import { escapeHtml } from "./html";

export type TrustedCoreReportSettings = {
  theme: string;
  formatOnSave: boolean;
  proofMode: string;
  proofDebounceMs: number;
};

export type TrustedCoreReport = {
  kind: "aura-sentinel.trusted-core-report";
  generatedAt: string;
  uri: string;
  path: string | null;
  projectRoot: string | null;
  settings: TrustedCoreReportSettings;
  lspDiagnostics: DiagnosticLike[];
  proofsDiagnostics: DiagnosticLike[];
  mergedDiagnostics: DiagnosticLike[];
  timelineDelta: { added: number; removed: number; changed: number } | null;
};

export function buildTrustedCoreReport(args: {
  generatedAt?: string;
  uri: string;
  path?: string | null;
  projectRoot?: string | null;
  settings: TrustedCoreReportSettings;
  lspDiagnostics: DiagnosticLike[];
  proofsDiagnostics: DiagnosticLike[];
  mergedDiagnostics: DiagnosticLike[];
  timelineDelta?: { added: number; removed: number; changed: number } | null;
}): TrustedCoreReport {
  return {
    kind: "aura-sentinel.trusted-core-report",
    generatedAt: args.generatedAt ?? new Date().toISOString(),
    uri: args.uri,
    path: args.path ?? null,
    projectRoot: args.projectRoot ?? null,
    settings: args.settings,
    lspDiagnostics: args.lspDiagnostics,
    proofsDiagnostics: args.proofsDiagnostics,
    mergedDiagnostics: args.mergedDiagnostics,
    timelineDelta: args.timelineDelta ?? null,
  };
}

export function renderTrustedCoreReportHtml(report: TrustedCoreReport, formatDiagTitle: (d: DiagnosticLike) => string): string {
  const esc = (s: string) => escapeHtml(s).replace(/\n/g, "<br/>");
  const rows = (report.mergedDiagnostics ?? [])
    .map((d) => {
      const line = d.range.start.line + 1;
      const col = d.range.start.character + 1;
      return `<div style="margin:8px 0;"><div><b>${esc(formatDiagTitle(d))}</b> <span style="opacity:.75;">${esc(
        `${line}:${col}`
      )}</span></div><div>${esc(String(d.message ?? ""))}</div></div>`;
    })
    .join("\n");

  return `<!doctype html>
  <html><head><meta charset="utf-8"/>
  <title>Trusted-core report</title>
  <style>body{font-family:system-ui,Segoe UI,Arial,sans-serif;margin:20px} code,pre{background:#f4f4f4;padding:8px;border-radius:8px;display:block;overflow:auto} .meta{opacity:.75}</style>
  </head><body>
    <h1>Trusted-core report</h1>
    <div class="meta">Generated: ${esc(report.generatedAt)}</div>
    <div class="meta">URI: ${esc(report.uri)}</div>
    <div class="meta">Project root: ${esc(String(report.projectRoot ?? ""))}</div>
    <h2>Settings</h2>
    <pre>${esc(JSON.stringify(report.settings, null, 2))}</pre>
    <h2>Timeline delta</h2>
    <pre>${esc(JSON.stringify(report.timelineDelta, null, 2))}</pre>
    <h2>Diagnostics</h2>
    ${rows || `<div class="meta">No diagnostics.</div>`}
  </body></html>`;
}
