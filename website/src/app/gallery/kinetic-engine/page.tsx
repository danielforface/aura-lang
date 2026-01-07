export default function KineticEnginePage() {
  return (
    <div className="space-y-4">
      <h1 className="text-3xl font-semibold tracking-tight">Kinetic Engine</h1>
      <p className="max-w-2xl text-sm leading-6 text-zinc-700 dark:text-zinc-300">
        A Raylib-powered demo that proves Aura can swallow native libraries through the Universal
        Bridge and run with zero-config dependency setup.
      </p>
      <div className="rounded-2xl border border-black/10 bg-white/60 p-6 dark:border-white/10 dark:bg-black/40">
        <div className="text-sm font-medium">Highlights</div>
        <ul className="mt-3 list-disc pl-5 text-sm text-zinc-700 dark:text-zinc-300">
          <li>Header-driven extern generation</li>
          <li>Manifest-based linking via aura.toml</li>
          <li>Native artifacts auto-installed into deps/</li>
        </ul>
      </div>
    </div>
  );
}
