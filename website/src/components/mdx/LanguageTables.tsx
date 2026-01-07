export function KeywordTable() {
  const keywords = [
    "cell",
    "val",
    "mut",
    "type",
    "import",
    "if",
    "else",
    "while",
    "invariant",
    "yield",
  ];

  return (
    <div className="not-prose my-6 overflow-hidden rounded-2xl border border-black/10 dark:border-white/10">
      <div className="border-b border-black/10 bg-white/60 px-4 py-2 text-xs font-semibold tracking-wide text-zinc-700 dark:border-white/10 dark:bg-black/40 dark:text-zinc-200">
        Keywords
      </div>
      <div className="grid grid-cols-2 gap-2 bg-white/60 p-4 text-sm dark:bg-black/40 sm:grid-cols-3 lg:grid-cols-4">
        {keywords.map((k) => (
          <code
            key={k}
            className="rounded-md border border-black/10 bg-black/5 px-2 py-1 font-mono dark:border-white/10 dark:bg-white/10"
          >
            {k}
          </code>
        ))}
      </div>
    </div>
  );
}

export function OperatorTable() {
  const rows: Array<{ op: string; meaning: string; notes?: string }> = [
    { op: "=", meaning: "assignment", notes: "only for existing bindings" },
    { op: ":", meaning: "type ascription / annotation", notes: "used in bindings and signatures" },
    { op: "->", meaning: "flow (sync)", notes: "sequencing + capability transfer" },
    { op: "~>", meaning: "flow (async)", notes: "sequencing + async capability" },
    { op: "==", meaning: "equality", notes: "boolean result" },
    { op: "!=" , meaning: "inequality", notes: "boolean result" },
    { op: "<, <=, >, >=", meaning: "comparison", notes: "boolean result" },
    { op: "+, -, *, /", meaning: "arithmetic", notes: "integer arithmetic (prototype)" },
    { op: "and, or, not", meaning: "boolean logic", notes: "(spelled out in docs/examples)" },
  ];

  return (
    <div className="not-prose my-6 overflow-hidden rounded-2xl border border-black/10 dark:border-white/10">
      <div className="border-b border-black/10 bg-white/60 px-4 py-2 text-xs font-semibold tracking-wide text-zinc-700 dark:border-white/10 dark:bg-black/40 dark:text-zinc-200">
        Operators
      </div>
      <div className="overflow-x-auto bg-white/60 dark:bg-black/40">
        <table className="w-full border-collapse text-left text-sm">
          <thead>
            <tr className="border-b border-black/10 dark:border-white/10">
              <th className="px-4 py-2 font-semibold">Token</th>
              <th className="px-4 py-2 font-semibold">Meaning</th>
              <th className="px-4 py-2 font-semibold">Notes</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr
                key={r.op}
                className="border-b border-black/5 last:border-b-0 dark:border-white/10"
              >
                <td className="px-4 py-2">
                  <code className="rounded-md border border-black/10 bg-black/5 px-2 py-1 font-mono dark:border-white/10 dark:bg-white/10">
                    {r.op}
                  </code>
                </td>
                <td className="px-4 py-2">{r.meaning}</td>
                <td className="px-4 py-2 text-zinc-700 dark:text-zinc-300">
                  {r.notes ?? ""}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
