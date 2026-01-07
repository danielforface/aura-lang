"use client";

import { useState } from "react";

const proven = `cell main() -> u32:
  val x: u32 = 42
  yield x
`;

const unsafe = `cell main() -> u32:
  type Percentage = u32[0..100]
  val p: Percentage = 180
  yield p
`;

export default function PlaygroundPage() {
  const [mode, setMode] = useState<"proven" | "unsafe">("proven");

  const isProven = mode === "proven";
  const code = isProven ? proven : unsafe;

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h1 className="text-3xl font-semibold tracking-tight">Interactive Playground (Stub)</h1>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => setMode("proven")}
            className={
              "h-10 rounded-full px-4 text-sm " +
              (isProven
                ? "bg-foreground text-background"
                : "border border-black/10 text-foreground dark:border-white/15")
            }
          >
            Proven
          </button>
          <button
            type="button"
            onClick={() => setMode("unsafe")}
            className={
              "h-10 rounded-full px-4 text-sm " +
              (!isProven
                ? "bg-foreground text-background"
                : "border border-black/10 text-foreground dark:border-white/15")
            }
          >
            Unsafe
          </button>
        </div>
      </div>

      <div className="rounded-2xl border border-black/10 bg-white/60 p-6 dark:border-white/10 dark:bg-black/40">
        <div className="flex items-center justify-between">
          <div className="text-sm font-medium">Z3 Prover</div>
          <div
            className={
              "rounded-full px-3 py-1 text-xs font-medium " +
              (isProven
                ? "bg-black/5 text-foreground dark:bg-white/10"
                : "bg-black/5 text-foreground dark:bg-white/10")
            }
          >
            {isProven ? "Verified" : "Proof Failed"}
          </div>
        </div>
        <pre className="mt-4 overflow-x-auto rounded-xl bg-black/5 p-4 text-sm text-foreground dark:bg-white/10">
          <code>{code}</code>
        </pre>
        <p className="mt-4 text-sm text-zinc-700 dark:text-zinc-300">
          This is a UI stub: it simulates how the editor will show “Proven” vs “Unsafe” code.
        </p>
      </div>
    </div>
  );
}
