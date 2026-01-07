import Link from "next/link";

export default function Downloads() {
  return (
    <div className="space-y-6">
      <section className="rounded-2xl border border-black/10 bg-white/60 p-8 text-black dark:border-white/10 dark:bg-black/40 dark:text-white">
        <h1 className="text-balance text-3xl font-semibold tracking-tight">Download AuraSDK v0.2.0</h1>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-zinc-700 dark:text-zinc-300">
          AuraSDK is a self-contained, portable folder with <span className="font-medium">aura</span>
          , <span className="font-medium">aura-lsp</span>, the standard library, offline docs, and bundled
          Z3 DLLs on Windows.
        </p>

        <div className="mt-6 flex flex-col gap-3 sm:flex-row">
          <a
            className="inline-flex h-11 items-center justify-center rounded-full bg-foreground px-5 text-background"
            href="/downloads/aura-sdk.zip"
          >
            Download (latest)
          </a>
          <Link
            className="inline-flex h-11 items-center justify-center rounded-full border border-black/10 px-5 text-foreground dark:border-white/15"
            href="/docs/getting-started"
          >
            Getting Started
          </Link>
        </div>

        <div className="mt-6 space-y-2 text-sm leading-6 text-zinc-700 dark:text-zinc-300">
          <p>
            Build the zip locally with <span className="font-mono">tools/release/release.py</span> and
            drop it into <span className="font-mono">website/public/downloads</span> as
            <span className="font-mono"> aura-sdk.zip</span>.
          </p>
          <p>
            After extracting, run <span className="font-mono">install.ps1</span> (Windows) or
            <span className="font-mono"> install.sh</span> (Unix).
          </p>
        </div>
      </section>

      <section className="rounded-2xl border border-black/10 bg-white/60 p-8 text-black dark:border-white/10 dark:bg-black/40 dark:text-white">
        <h2 className="text-balance text-3xl font-semibold tracking-tight">Download Aura Sentinel v0.2.0</h2>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-zinc-700 dark:text-zinc-300">
          Aura Sentinel is the developer IDE layer for Aura: a VS Code extension bundling the Aura language server
          and proof-driven editor tools (live Z3 diagnostics, Proofs panel, Nexus plugin UX).
        </p>

        <div className="mt-6 flex flex-col gap-3 sm:flex-row">
          <a
            className="inline-flex h-11 items-center justify-center rounded-full bg-foreground px-5 text-background"
            href="/downloads/aura-sentinel.vsix"
          >
            Download VSIX (latest)
          </a>
          <Link
            className="inline-flex h-11 items-center justify-center rounded-full border border-black/10 px-5 text-foreground dark:border-white/15"
            href="/docs/lsp-and-sentinel"
          >
            Sentinel Docs
          </Link>
        </div>

        <div className="mt-6 space-y-2 text-sm leading-6 text-zinc-700 dark:text-zinc-300">
          <p>
            Build the VSIX locally with <span className="font-mono">tools/release/release.py --website</span>. The
            release script writes <span className="font-mono">aura-sentinel.vsix</span> into
            <span className="font-mono"> website/public/downloads</span>.
          </p>
          <p>
            Install it in VS Code via <span className="font-mono">Extensions: Install from VSIX...</span>.
          </p>
        </div>
      </section>

      <section className="rounded-2xl border border-black/10 bg-white/60 p-8 text-black dark:border-white/10 dark:bg-black/40 dark:text-white">
        <h2 className="text-balance text-3xl font-semibold tracking-tight">Download Aura Sentinel App v0.2.0</h2>
        <p className="mt-3 max-w-2xl text-sm leading-6 text-zinc-700 dark:text-zinc-300">
          Aura Sentinel App is the standalone desktop shell for Aura Sentinel. It reuses the same proof-driven LSP
          APIs (including <span className="font-mono">aura/proofs</span>) outside VS Code.
        </p>

        <div className="mt-6 flex flex-col gap-3 sm:flex-row">
          <a
            className="inline-flex h-11 items-center justify-center rounded-full bg-foreground px-5 text-background"
            href="/downloads/aura-sentinel-app.exe"
          >
            Download App (latest)
          </a>
          <a
            className="inline-flex h-11 items-center justify-center rounded-full border border-black/10 px-5 text-foreground dark:border-white/15"
            href="/downloads/aura-sentinel.msi"
          >
            Download MSI (latest)
          </a>
          <a
            className="inline-flex h-11 items-center justify-center rounded-full border border-black/10 px-5 text-foreground dark:border-white/15"
            href="/downloads/aura-sentinel-setup.exe"
          >
            Download Setup EXE (latest)
          </a>
        </div>

        <div className="mt-6 space-y-2 text-sm leading-6 text-zinc-700 dark:text-zinc-300">
          <p>
            Build it locally with <span className="font-mono">tools/release/release.py --sentinel-app --website</span>.
            The release script writes <span className="font-mono">aura-sentinel-app.exe</span> into
            <span className="font-mono"> website/public/downloads</span>.
          </p>
          <p>
            Installer builds are published as <span className="font-mono">aura-sentinel.msi</span> and
            <span className="font-mono"> aura-sentinel-setup.exe</span>.
          </p>
        </div>
      </section>
    </div>
  );
}
