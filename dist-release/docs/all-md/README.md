# Aura (2026 Edition)

Aura is a proof-driven programming language and tooling stack:
- A fast compiler + Dev-VM for iteration
- Z3-backed verification for contracts, invariants, and safety properties
- An LSP (`aura-lsp`) + the Aura Sentinel IDE UX (proof streaming, explainable counterexamples)
- Automated FFI binding generation (Aura-Bindgen)

This repo is the full workspace (language + verifier + IDE integration + SDK packaging).

## Quick links
- Getting started docs: [sdk/docs/README.md](sdk/docs/README.md)
- Z3 Gate protocol (Sentinel ↔ LSP): [sdk/docs/z3-gate.md](sdk/docs/z3-gate.md)
- Verifier guide: [sdk/docs/verifier-guide.md](sdk/docs/verifier-guide.md)
- LSP stability contract: [sdk/docs/lsp-stability.md](sdk/docs/lsp-stability.md)
- Debug protocol: [docs/debug-protocol.md](docs/debug-protocol.md)
- Roadmap: [ROADMAP.md](ROADMAP.md)

## What Aura looks like

Aura uses indentation-based blocks and a `cell` entry point. Example (from the SDK welcome program):

```aura
import aura::lumina

cell main():
    layout:
        VStack(alignment: "center") {
            render: Text(text: "Welcome to the Aura Universe!", style: PixarModern())
            render: Button(label: "Start Creating") {
                on_click: ~> { log("Aura SDK is Active.") }
            }
        }
```

Refinement types (and verifier-friendly style) look like:

```aura
val mut i: u32[0..5000] = 0
while i < count invariant i <= count:
    i = i + 1
```

## Getting started (from source)

Prereqs:
- Rust toolchain (stable)
- Optional: Z3 installed (to enable verification) and LLVM (to enable LLVM backend)

### Run the CLI

Show help:

```bash
cargo run -p aura -- --help
```

Build (type-check + lower + emit backend artifacts):

```bash
cargo run -p aura -- build main.aura
```

Run (Dev-VM/Hybrid is supported):

```bash
cargo run -p aura -- run main.aura
```

Verify (Z3 gate when enabled):

```bash
cargo run -p aura --features z3 -- verify main.aura --smt-profile fast
```

Notes:
- `--smt-profile` supports `fast`, `ci`, `thorough`.
- Optional incremental solver mode (keep Z3 warm inside a run): set `AURA_Z3_INCREMENTAL=1`.

### Run the language server (LSP)

```bash
cargo run -p aura-lsp
```

Proof streaming and counterexample payloads follow the Z3 Gate contract in [sdk/docs/z3-gate.md](sdk/docs/z3-gate.md).

### Run Aura Sentinel (desktop app)

The desktop app is a Tauri shell in [editors/sentinel-app](editors/sentinel-app).

```bash
cd editors/sentinel-app
npm install
npm run tauri:dev
```

## Verification UX (Explainable failures)

When a proof fails, Aura emits structured `Diagnostic.data` payloads for IDEs:
- A minimal model slice (`data.counterexample.slice`)
- A best-effort mapping of bindings back into source with inline “ghost text” injections
- A versioned counterexample mapping schema (`aura.counterexample.v2`)

See protocol details in [sdk/docs/z3-gate.md](sdk/docs/z3-gate.md).

## Interop: Aura-Bindgen (C headers → Aura shims)

Aura can generate bindings from C headers:

```bash
cargo run -p aura -- bindgen --header demo.h --out build/bindgen
```

Optional: enable best-effort refined type mapping (ranges/nullability) in generated shims:

```bash
cargo run -p aura -- bindgen --header demo.h --out build/bindgen --refine-types
```

## Repository layout (high level)

Core crates:
- `aura`: CLI (build/run/verify/test/fmt/lint/pkg/bindgen)
- `aura-parse`, `aura-lex`, `aura-ast`, `aura-core`: frontend pipeline
- `aura-verify`: Z3 verifier + counterexample metadata
- `aura-lsp`: language server + proof streaming
- `aura-stdlib`, `aura-rt`, `aura-interpret`: stdlib/runtime/Dev-VM

Tooling:
- `tools/release/release.py`: builds distributable SDK zip into `dist/`
- `docs/`: stability, UB boundaries, debug protocol, release channels

## Releases / Packages (how to produce artifacts)

This repo can build distributable artifacts locally even if GitHub Releases/Packages are empty:

```bash
python tools/release/release.py --help
```

Release policy notes live in [docs/release-channels.md](docs/release-channels.md).

## Suggested GitHub “About” metadata

You set these in GitHub repo settings (not in code), but here are good defaults:
- **Description**: “Aura is a proof-driven programming language + IDE with Z3-backed verification, a Dev-VM, and automated FFI binding generation.”
- **Topics**: `programming-language`, `compiler`, `formal-methods`, `verification`, `z3`, `lsp`, `rust`, `tauri`, `ffi`
- **Website**: publish the site under `website/` (when you have a URL)
