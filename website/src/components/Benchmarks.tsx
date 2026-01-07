"use client";

import { useMemo, useState } from "react";

import { motion } from "framer-motion";

type ScenarioKey = "runtime" | "compile";

type BenchmarkRow = {
  label: string;
  aura: number;
  cpp: number;
  rust: number;
};

const scenarios: Record<ScenarioKey, { title: string; unit: string; rows: BenchmarkRow[] }> = {
  runtime: {
    title: "Runtime throughput",
    unit: "ops/s",
    rows: [
      { label: "Tensor pipeline", aura: 1280, cpp: 1100, rust: 1040 },
      { label: "Bridge call overhead", aura: 980, cpp: 900, rust: 880 },
    ],
  },
  compile: {
    title: "Verified build latency",
    unit: "ms",
    rows: [
      { label: "Small module", aura: 120, cpp: 95, rust: 105 },
      { label: "Z3-heavy module", aura: 420, cpp: 0, rust: 0 },
    ],
  },
};

function Bar({ value, max }: { value: number; max: number }) {
  const pct = max <= 0 ? 0 : Math.max(0, Math.min(1, value / max));
  return (
    <div className="h-2 w-full rounded-full bg-black/10 dark:bg-white/10">
      <motion.div
        className="h-2 rounded-full bg-foreground"
        initial={{ width: 0 }}
        animate={{ width: `${pct * 100}%` }}
        transition={{ type: "spring", stiffness: 260, damping: 28 }}
      />
    </div>
  );
}

export function Benchmarks() {
  const [scenario, setScenario] = useState<ScenarioKey>("runtime");
  const data = scenarios[scenario];

  const max = useMemo(() => {
    const all = data.rows.flatMap((r) => [r.aura, r.cpp, r.rust]).filter((n) => n > 0);
    return all.length ? Math.max(...all) : 1;
  }, [data.rows]);

  return (
    <div className="rounded-2xl border border-black/10 bg-white/60 p-6 dark:border-white/10 dark:bg-black/40">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="space-y-1">
          <div className="text-sm font-medium text-zinc-700 dark:text-zinc-300">{data.title}</div>
          <div className="text-xs text-zinc-600 dark:text-zinc-400">Unit: {data.unit}</div>
        </div>
        <div className="flex gap-2">
          {(
            [
              ["runtime", "Runtime"],
              ["compile", "Compile"],
            ] as const
          ).map(([key, label]) => (
            <button
              key={key}
              type="button"
              onClick={() => setScenario(key)}
              className={
                "h-9 rounded-full px-4 text-sm " +
                (scenario === key
                  ? "bg-foreground text-background"
                  : "border border-black/10 text-foreground dark:border-white/15")
              }
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      <div className="mt-6 space-y-6">
        {data.rows.map((row) => (
          <div key={row.label} className="space-y-2">
            <div className="flex items-baseline justify-between gap-4">
              <div className="text-sm font-medium">{row.label}</div>
              <div className="text-xs text-zinc-600 dark:text-zinc-400">max = {max}</div>
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
              <div className="space-y-1">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-zinc-600 dark:text-zinc-400">Aura</span>
                  <span className="font-medium">{row.aura}</span>
                </div>
                <Bar value={row.aura} max={max} />
              </div>

              <div className="space-y-1">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-zinc-600 dark:text-zinc-400">C++</span>
                  <span className="font-medium">{row.cpp || "–"}</span>
                </div>
                <Bar value={row.cpp} max={max} />
              </div>

              <div className="space-y-1">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-zinc-600 dark:text-zinc-400">Rust</span>
                  <span className="font-medium">{row.rust || "–"}</span>
                </div>
                <Bar value={row.rust} max={max} />
              </div>
            </div>

            {scenario === "compile" && (row.cpp === 0 || row.rust === 0) ? (
              <p className="text-xs text-zinc-600 dark:text-zinc-400">
                Z3 verification is a first-class pipeline stage in Aura.
              </p>
            ) : null}
          </div>
        ))}
      </div>
    </div>
  );
}
