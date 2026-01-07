export type LspRangeLike = {
  start: { line: number; character: number };
  end: { line: number; character: number };
};

export type DiagnosticLike = {
  id?: string;
  range: LspRangeLike;
  severity?: number;
  code?: string | number;
  source?: string;
  message: string;
};

export function sevLabel(s?: number): string {
  // LSP DiagnosticSeverity: 1 Error, 2 Warning, 3 Information, 4 Hint
  switch (s) {
    case 1:
      return "Error";
    case 2:
      return "Warn";
    case 3:
      return "Info";
    case 4:
      return "Hint";
    default:
      return "";
  }
}

export function diagSortKey(d: DiagnosticLike): number {
  const sev = d.severity ?? 99;
  return sev * 1_000_000 + d.range.start.line * 1_000 + d.range.start.character;
}

export function diagStableId(d: DiagnosticLike): string {
  const r = d.range;
  const sev = d.severity ?? 99;
  const code = d.code ?? "";
  const src = d.source ?? "";
  const msg = d.message ?? "";
  return `${r.start.line}:${r.start.character}-${r.end.line}:${r.end.character}|${sev}|${String(code)}|${String(src)}|${msg}`;
}

export function diagBaseKey(d: DiagnosticLike): string {
  const r = d.range;
  const sev = d.severity ?? 99;
  const code = d.code ?? "";
  const src = d.source ?? "";
  return `${r.start.line}:${r.start.character}-${r.end.line}:${r.end.character}|${sev}|${String(code)}|${String(src)}`;
}

export function normalizeDiagnostics<T extends DiagnosticLike>(diags: T[]): Array<T & { id: string }> {
  return (diags ?? []).map((d) => ({ ...d, id: d.id ?? diagStableId(d) }));
}

export function mergeDiagnostics<T extends DiagnosticLike>(lsp: T[], proofs: T[]): Array<T & { id: string }> {
  const out = new Map<string, T & { id: string }>();

  const add = (d: T) => {
    const id = d.id ?? diagStableId(d);
    const next = { ...d, id };
    const cur = out.get(id);
    if (!cur) {
      out.set(id, next);
      return;
    }

    const a = cur.severity ?? 99;
    const b = next.severity ?? 99;
    // Prefer the more severe diagnostic (lower number).
    if (b < a) out.set(id, next);
  };

  for (const d of normalizeDiagnostics(lsp)) add(d as T);
  for (const d of normalizeDiagnostics(proofs)) add(d as T);
  return [...out.values()].sort((a, b) => diagSortKey(a) - diagSortKey(b));
}
