# Aura Sentinel App — Roadmap (Standalone)

This checklist is derived from the “Big upgrade roadmap” you provided and is meant to be executed step-by-step.

## Editor & UX (core IDE feel)

- [x] Hover tooltips (diagnostics)
- [x] Quick-fix UI (code actions) + “Apply fix”
- [x] Go-to definition / find references / rename symbol
- [x] Outline panel (symbols), breadcrumbs, minimap
- [x] Search in file + search in folder
- [x] Multi-tab editor + unsaved indicator + diff view
- [x] File tree + open folder/workspace + recent projects
- [x] Settings UI (theme, format-on-save, proof mode, debounce)
- [x] Keybindings (Ctrl+P, Ctrl+S, Ctrl+Shift+P command palette)

## LSP & tooling

- [x] Full LSP client: completion, hover (server hover), signature help
- [x] Incremental `didChange` (range-based) for large files
- [x] Diagnostics dedupe (server vs proofs) + stable IDs
- [x] Background LSP restart + crash recovery + “reindex” command
- [x] Better root detection (Aura project manifests) + per-project config

## Proofs / verification

- [x] Structured proof obligations view (by function/cell)
- [x] Proof timeline (what changed since last edit)
- [x] “Explain” panel: why a proof failed + counterexample model (when available)
- [x] Trusted-core report export (JSON/HTML) for compliance/CI

## Project & build

- [x] Run/Build buttons (invoke `aura`), output console, error hyperlinks
- [x] Test runner integration (Aura tests + snapshots)
- [x] Package manager UI (`aura-pkg`) + dependency graph

## Interop workflows

- [x] Aura-Bindgen UI integration
	- [x] Select header(s) → generate wrappers → preview diff → write files
	- [x] Surface the trusted-boundary report inside the IDE

## Run & debug (Dev-VM aware)

- [x] “Run (Dev)” uses Dev-VM/JIT path for instant feedback
- [x] Step debugger integrates with Dev-VM execution model (MVP: statement-level)
- [x] Native run hooks via debug protocol (launch/terminate/exit events; no attach/stepping)
- [x] Capability negotiation via `hello` (gate debug features on capabilities)
- [x] Perf memory key helptext (MVP)

## Distribution / productization

- [x] Proper installers (MSI/NSIS) + auto-updates (optional)
- [x] Signed binaries + reproducible builds
- [x] Release pipeline can publish desktop app into website downloads (basic)

## Testing & quality

- [x] Rust unit tests (message framing)
- [x] Integration tests (spawn real `aura-lsp` and assert diagnostics)
- [x] Frontend tests (diagnostics rendering + jump/hover)
- [x] “Golden file” tests (proofs response formatting)
- [x] Debug protocol smoke test (hello + perfReport)
