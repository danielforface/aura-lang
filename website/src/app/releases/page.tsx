import Link from "next/link";

export default function Releases() {
  return (
    <div className="space-y-8">
      <section className="rounded-2xl border border-black/10 bg-white/60 p-8 text-black dark:border-white/10 dark:bg-black/40 dark:text-white">
        <h1 className="text-balance text-3xl font-semibold tracking-tight">Release Notes</h1>
        <p className="mt-2 text-sm leading-6 text-zinc-700 dark:text-zinc-300">
          Major milestones and updates for the Aura language, SDK, and tooling.
        </p>
      </section>

      {/* v0.2.0 */}
      <section className="rounded-2xl border border-black/10 bg-white/60 p-8 text-black dark:border-white/10 dark:bg-black/40 dark:text-white">
        <div className="space-y-4">
          <div>
            <h2 className="text-2xl font-semibold">v0.2.0 — Explainable Verification</h2>
            <p className="text-sm text-zinc-600 dark:text-zinc-400">January 2026</p>
          </div>

          <div className="space-y-3 text-sm leading-6">
            <div>
              <h3 className="font-semibold">Verification UX Overhaul</h3>
              <ul className="mt-2 space-y-1 ml-4 list-disc text-zinc-700 dark:text-zinc-300">
                <li>
                  <strong>Typed counterexamples:</strong> Z3 models now carry type information (ranges for integers, bool values).
                  Values are extracted as structured JSON and mapped back to source identifiers.
                </li>
                <li>
                  <strong>Variable trace:</strong> Best-effort tracking of variable definitions and assignments.
                  Identifies which bindings are relevant to a failed assertion.
                </li>
                <li>
                  <strong>UNSAT core logic trace:</strong> Interactive proof steps showing why assertions must fail.
                  Span-linked core entries jump directly to failing code.
                </li>
                <li>
                  <strong>Counterexample schema v2:</strong> Improved mapping format with inline injection hints
                  for easier debugging in Sentinel IDE.
                </li>
              </ul>
            </div>

            <div>
              <h3 className="font-semibold">Sentinel IDE Enhancements</h3>
              <ul className="mt-2 space-y-1 ml-4 list-disc text-zinc-700 dark:text-zinc-300">
                <li>Proofs panel now renders typed values, variable traces, and logic trace breadcrumbs.</li>
                <li>Quick hints and suggestions for fixing common proof failures.</li>
                <li>Better diagnostics de-duplication and related-info navigation.</li>
              </ul>
            </div>

            <div>
              <h3 className="font-semibold">CI & Build Improvements</h3>
              <ul className="mt-2 space-y-1 ml-4 list-disc text-zinc-700 dark:text-zinc-300">
                <li>
                  <strong>Trusted Core audit baseline:</strong> CI now enforces a &quot;trusted core report&quot; diffs
                  against a known-good baseline, catching drift in verified modules.
                </li>
                <li>Reproducible, deterministic release builds with attestation metadata.</li>
                <li>All platform SDKs (Windows, macOS, Linux) built and packaged consistently.</li>
              </ul>
            </div>

            <div>
              <h3 className="font-semibold">Refined Types & Bindgen</h3>
              <ul className="mt-2 space-y-1 ml-4 list-disc text-zinc-700 dark:text-zinc-300">
                <li>
                  <code className="bg-zinc-200 dark:bg-zinc-800 px-1 rounded">aura bindgen --refine-types</code> maps
                  C integer ranges and nullability into Aura refined-type annotations.
                </li>
              </ul>
            </div>

            <div>
              <h3 className="font-semibold">Performance & Stability</h3>
              <ul className="mt-2 space-y-1 ml-4 list-disc text-zinc-700 dark:text-zinc-300">
                <li>Incremental proof cache with file hash + dependency tracking.</li>
                <li>High-speed Dev-VM interpreter for sub-second edit-run loops.</li>
                <li>Differential testing between Dev-VM and native LLVM backends.</li>
              </ul>
            </div>

            <div className="border-t border-zinc-300 dark:border-zinc-700 pt-4">
              <p className="text-zinc-700 dark:text-zinc-300">
                <strong>Roadmap:</strong> v0.2 is the &quot;first solid daily driver&quot; milestone from our roadmap.
                <Link
                  href="/docs"
                  className="text-blue-600 dark:text-blue-400 hover:underline"
                >
                  {" "}
                  Read the full roadmap
                </Link>
                {" "}for planned features in v0.3+ (ownership model, effect system, packages/registry).
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* v0.1.0 */}
      <section className="rounded-2xl border border-black/10 bg-white/60 p-8 text-black dark:border-white/10 dark:bg-black/40 dark:text-white">
        <div className="space-y-4">
          <div>
            <h2 className="text-2xl font-semibold">v0.1.0 — Foundation Release</h2>
            <p className="text-sm text-zinc-600 dark:text-zinc-400">Initial public release</p>
          </div>

          <div className="space-y-3 text-sm leading-6">
            <ul className="space-y-1 ml-4 list-disc text-zinc-700 dark:text-zinc-300">
              <li>Core language syntax and module system.</li>
              <li>Z3-backed verification with proof streaming.</li>
              <li>Aura Sentinel IDE (VS Code extension and desktop app).</li>
              <li>SDK packaging and distribution.</li>
              <li>Aura-Bindgen FFI automation.</li>
              <li>Standard library with core, math, collections, io, and more.</li>
            </ul>
          </div>
        </div>
      </section>
    </div>
  );
}
