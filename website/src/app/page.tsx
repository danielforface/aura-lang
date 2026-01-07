import Link from "next/link";

import { Benchmarks } from "@/components/Benchmarks";

export default function Home() {
  return (
    <div className="space-y-12">
      <section className="rounded-2xl border border-black/10 bg-white/60 p-8 text-black dark:border-white/10 dark:bg-black/40 dark:text-white">
        <div className="space-y-5">
          <p className="text-sm font-medium tracking-wide text-zinc-700 dark:text-zinc-300">
            Aura Portal
          </p>
          <h1 className="text-balance text-4xl font-semibold leading-tight tracking-tight sm:text-5xl">
            Aura: The First Mathematically Proven AI-Native Language.
          </h1>
          <p className="max-w-2xl text-pretty text-lg leading-7 text-zinc-700 dark:text-zinc-300">
            In 2026, memory safety is not enough. Aura adds logical safety: Z3-backed
            verification for invariants and shape-safe AI inference, compiled through an LLVM
            backend.
          </p>
          <p className="max-w-2xl text-pretty text-sm leading-6 text-zinc-700 dark:text-zinc-300">
            Latest milestone: <span className="font-medium">Lumina Sentinel</span> proves UI geometry
            before rendering pixels — then drives live Raylib frames with click callbacks routed back
            into the AVM.
          </p>
          <div className="flex flex-col gap-3 sm:flex-row">
            <Link
              className="inline-flex h-11 items-center justify-center rounded-full bg-foreground px-5 text-background"
              href="/docs/getting-started"
            >
              Read the Book
            </Link>
            <Link
              className="inline-flex h-11 items-center justify-center rounded-full border border-black/10 px-5 text-foreground dark:border-white/15"
              href="/downloads"
            >
              Download AuraSDK
            </Link>
            <Link
              className="inline-flex h-11 items-center justify-center rounded-full border border-black/10 px-5 text-foreground dark:border-white/15"
              href="/docs/lumina-sentinel"
            >
              Lumina Sentinel Deep Dive
            </Link>
          </div>
        </div>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-semibold tracking-tight">Benchmarks</h2>
        <p className="max-w-2xl text-sm leading-6 text-zinc-700 dark:text-zinc-300">
          Visual, interactive comparison of Aura vs C++ and Rust (illustrative).
        </p>
        <Benchmarks />
      </section>
    </div>
  );
}
