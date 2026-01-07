import type { DiagnosticLike, LspRangeLike } from "./diagnostics";
import { diagSortKey, diagStableId, sevLabel } from "./diagnostics";
import { escapeHtml } from "./html";

export type OutlineItemLike = {
  name: string;
  range: LspRangeLike;
  selection_range: LspRangeLike;
  children: OutlineItemLike[];
};

export type DocForProofsLike = {
  mergedDiagnostics: DiagnosticLike[];
  outlineItems: OutlineItemLike[];
};

export type ProofDelta = { added: number; removed: number; changed: number };

export function flattenOutline(items: OutlineItemLike[], path: OutlineItemLike[] = []): Array<{ path: OutlineItemLike[]; item: OutlineItemLike }> {
  const out: Array<{ path: OutlineItemLike[]; item: OutlineItemLike }> = [];
  for (const item of items) {
    const next = [...path, item];
    out.push({ path: next, item });
    if (item.children?.length) out.push(...flattenOutline(item.children, next));
  }
  return out;
}

export function findDeepestSymbolNameForRange(doc: DocForProofsLike, range: LspRangeLike): string {
  if (!doc.outlineItems?.length) return "(no symbols)";
  const line0 = range.start.line;
  const char0 = range.start.character;
  const paths = flattenOutline(doc.outlineItems);
  const matches: OutlineItemLike[][] = [];
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

export function formatDiagTitle(d: DiagnosticLike): string {
  const sev = sevLabel(d.severity);
  const src = d.source ?? "";
  const code = d.code ?? "";
  const parts = [sev, src, String(code)].filter((x) => String(x).trim().length);
  return parts.join(" · ");
}

export function renderExplainPanel(diag: DiagnosticLike | undefined): string {
  if (!diag) return `<div class="meta">Select a proof obligation to see details.</div>`;

  const line = diag.range.start.line + 1;
  const col = diag.range.start.character + 1;
  const title = escapeHtml(formatDiagTitle(diag));
  const msg = escapeHtml(String(diag.message ?? ""));
  const loc = escapeHtml(`${line}:${col}`);
  const src = escapeHtml(String(diag.source ?? ""));
  const code = escapeHtml(String(diag.code ?? ""));

  const data = (diag as any).data;

  const renderValue = (v: unknown): string => {
    if (v === null || v === undefined) return `<span class="meta">(null)</span>`;

    // Most bindings come through as strings.
    if (typeof v === "string") {
      const s = v;
      const trimmed = s.trim();
      // If it looks like JSON, pretty print it.
      if ((trimmed.startsWith("{") && trimmed.endsWith("}")) || (trimmed.startsWith("[") && trimmed.endsWith("]"))) {
        try {
          const parsed = JSON.parse(trimmed);
          return `<pre class="proofPre">${escapeHtml(JSON.stringify(parsed, null, 2))}</pre>`;
        } catch {
          // fall through
        }
      }
      // Multi-line / long values: keep readable.
      if (s.includes("\n") || s.length > 120) {
        return `<pre class="proofPre">${escapeHtml(s)}</pre>`;
      }
      return escapeHtml(s);
    }

    // Best-effort pretty JSON for objects.
    try {
      return `<pre class="proofPre">${escapeHtml(JSON.stringify(v, null, 2))}</pre>`;
    } catch {
      return escapeHtml(String(v));
    }
  };

  const renderBindings = (titleText: string, bindings: any[]): string => {
    if (!Array.isArray(bindings) || !bindings.length) return "";
    const rows = bindings
      .slice(0, 50)
      .map((b: any) => {
        const nameRaw = String(b?.name ?? "");
        const name = escapeHtml(nameRaw);
        const value = renderValue(b?.value);
        const rel = b?.relevant ? `<span class="badge">relevant</span>` : "";
        const q = encodeURIComponent(nameRaw);
        const clickable = nameRaw.trim().length
          ? `<span class="proofLink" data-search-q="${q}" title="Highlight occurrences in editor"><b>${name}</b></span>`
          : `<b>${name}</b>`;
        return `<div class="meta">${rel} ${clickable} = ${value}</div>`;
      })
      .join("\n");
    return `
      <div class="meta" style="margin-top:8px;">${escapeHtml(titleText)}</div>
      <div class="proofRelated">${rows}</div>
    `;
  };

  let extra = "";
  if (data !== undefined && data !== null) {
    try {
      const modelText = typeof data?.model === "string" ? data.model : undefined;
      const slice = Array.isArray(data?.counterexample?.slice) ? data.counterexample.slice : undefined;
      const relevant = Array.isArray(data?.meta?.relevantBindings) ? data.meta.relevantBindings : undefined;
      const allBindings = Array.isArray(data?.meta?.bindings) ? data.meta.bindings : undefined;

      const mapped = data?.counterexample?.mapped;
      const mappedBindings = Array.isArray(mapped?.bindings) ? mapped.bindings : undefined;
      const mappedInjections = Array.isArray(mapped?.injections) ? mapped.injections : undefined;

      const injectionsHtml = Array.isArray(mappedInjections) && mappedInjections.length
        ? `
          <div class="meta" style="margin-top:8px;">Inline injections</div>
          <div class="proofRelated">
            ${mappedInjections
              .slice(0, 20)
              .map((inj: any) => `<div class="meta">${escapeHtml(String(inj?.text ?? ""))}</div>`)
              .join("\n")}
          </div>
        `
        : "";

      const structured = `${
        renderBindings("Counterexample slice", slice ?? []) ||
        renderBindings("Mapped bindings", mappedBindings ?? []) ||
        renderBindings("Relevant bindings", relevant ?? []) ||
        renderBindings("Bindings", allBindings ?? [])
      }${
        modelText
          ? `<div class="meta" style="margin-top:8px;">Model</div><pre class="proofPre">${escapeHtml(modelText)}</pre>`
          : ""
      }${injectionsHtml}`;

      const unsatCore = Array.isArray((data as any)?.unsat_core) ? (data as any).unsat_core : undefined;
      const interpolant = typeof (data as any)?.interpolant === "string" ? (data as any).interpolant : undefined;
      const reasoningTrace = (() => {
        const parts: string[] = [];
        if (Array.isArray(unsatCore) && unsatCore.length) {
          const lines = unsatCore
            .slice(0, 50)
            .map((s: any, i: number) => `<div class="meta">${escapeHtml(String(i + 1))}. ${escapeHtml(String(s))}</div>`)
            .join("\n");
          parts.push(`<div class="meta" style="margin-top:8px;">Reasoning trace (UNSAT core)</div><div class="proofRelated">${lines}</div>`);
        }
        if (interpolant && interpolant.trim().length) {
          parts.push(`<div class="meta" style="margin-top:8px;">Derived lemma</div><pre class="proofPre">${escapeHtml(interpolant)}</pre>`);
        }
        return parts.join("");
      })();

      extra = structured.trim().length
        ? `${structured}${reasoningTrace}`
        : `<div class="meta" style="margin-top:8px;">Data / Counterexample (if provided)</div><pre class="proofPre">${escapeHtml(
            JSON.stringify(data, null, 2)
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
            const rangeJson = r ? encodeURIComponent(JSON.stringify(r)) : "";
            const dataAttr = r ? ` data-jump-range="${rangeJson}" title="Jump to location"` : "";
            return `<div class="meta"${dataAttr}>${escapeHtml(where)} ${m}</div>`;
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

export function renderStructuredProofs(
  doc: DocForProofsLike,
  opts: {
    selectedProofId?: string;
    lastProofDelta?: ProofDelta;
    lastTelemetry?: any;
  }
): string {
  const all = doc.mergedDiagnostics ?? [];
  const groups = new Map<string, DiagnosticLike[]>();
  for (const d of all) {
    const name = findDeepestSymbolNameForRange(doc, d.range);
    const key = name || "(global)";
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(d);
  }

  const sortedGroups = [...groups.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  const selected = opts.selectedProofId ? all.find((d) => (d.id ?? diagStableId(d)) === opts.selectedProofId) : undefined;

  const delta = opts.lastProofDelta
    ? `<div class="meta">Timeline (since last proof run): +${opts.lastProofDelta.added} −${opts.lastProofDelta.removed} ~${opts.lastProofDelta.changed}</div>`
    : `<div class="meta">Timeline: first proof run for this file.</div>`;

  const cache = opts.lastTelemetry?.cache;
  const cacheLine = (() => {
    if (!cache) return "";
    const fullHit = !!cache.fullHit;
    const stmtHits = Number(cache.stmtHits ?? 0);
    const stmtMisses = Number(cache.stmtMisses ?? 0);
    const uiHit = cache.uiHit;
    const parts: string[] = [];
    if (fullHit) parts.push(`<span class="badge">cached</span> full hit`);
    else if (stmtHits > 0) parts.push(`<span class="badge">cache</span> stmt hits ${stmtHits}/${Math.max(1, stmtHits + stmtMisses)}`);
    if (uiHit === true) parts.push("ui hit");
    if (uiHit === false) parts.push("ui miss");
    if (!parts.length) return "";
    return `<div class="meta">${parts.join(" · ")}</div>`;
  })();

  const header = `
    <div class="proofHeader">
      <div class="proofActions">
        <button class="miniBtn" type="button" data-export="json">Export JSON</button>
        <button class="miniBtn" type="button" data-export="html">Export HTML</button>
      </div>
      ${delta}${cacheLine ? `\n      ${cacheLine}` : ""}
    </div>
  `;

  if (!all.length) {
    return `${header}<div class="meta">No proofs/diagnostics.</div>`;
  }

  const list = sortedGroups
    .map(([name, items]) => {
      const sorted = [...items].sort((a, b) => diagSortKey(a) - diagSortKey(b));
      const rows = sorted
        .map((d) => {
          const line = d.range.start.line + 1;
          const col = d.range.start.character + 1;
          const badge = sevLabel(d.severity);
          const code = d.code ?? "";
          const src = d.source ?? "";
          const msg = d.message ?? "";
          const safe = escapeHtml(String(msg));
          const rangeJson = encodeURIComponent(JSON.stringify(d.range));
          const id = d.id ?? diagStableId(d);
          const active = id === opts.selectedProofId ? " proofSelected" : "";
          return `
            <div class="diag${active}" data-range="${rangeJson}" data-diag-id="${encodeURIComponent(id)}">
              <div><span class="badge">${escapeHtml(badge)}</span> <span class="meta">${escapeHtml(String(src))} ${escapeHtml(String(code))}</span></div>
              <div class="meta">${line}:${col}</div>
              <div>${safe}</div>
              <div class="diagActions"><button class="miniBtn" data-qf="1">Quick Fix</button></div>
            </div>
          `;
        })
        .join("\n");

      return `
        <div class="diagSection">
          <div class="diagSectionTitle">${escapeHtml(name)} (${sorted.length})</div>
          ${rows}
        </div>
      `;
    })
    .join("\n");

  const explain = `
    <div class="diagSection">
      <div class="diagSectionTitle">Explain</div>
      ${renderExplainPanel(selected)}
    </div>
  `;

  return `${header}${list}${explain}`;
}

export function diagBaseKeyForDelta(d: DiagnosticLike): string {
  const r = d.range;
  const sev = d.severity ?? 99;
  const code = d.code ?? "";
  const src = d.source ?? "";
  return `${r.start.line}:${r.start.character}-${r.end.line}:${r.end.character}|${sev}|${String(code)}|${String(src)}`;
}

export function diffProofSnapshots(prev: DiagnosticLike[] | undefined, next: DiagnosticLike[]): ProofDelta {
  if (!prev) return { added: next.length, removed: 0, changed: 0 };

  const prevByBase = new Map<string, DiagnosticLike>();
  for (const d of prev) prevByBase.set(diagBaseKeyForDelta(d), d);

  const nextByBase = new Map<string, DiagnosticLike>();
  for (const d of next) nextByBase.set(diagBaseKeyForDelta(d), d);

  let added = 0;
  let removed = 0;
  let changed = 0;

  for (const [k, d] of nextByBase.entries()) {
    const p = prevByBase.get(k);
    if (!p) {
      added++;
      continue;
    }
    if ((p.message ?? "") !== (d.message ?? "")) changed++;
  }

  for (const k of prevByBase.keys()) {
    if (!nextByBase.has(k)) removed++;
  }

  return { added, removed, changed };
}
