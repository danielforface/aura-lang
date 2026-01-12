# Aura Sentinel (Standalone App)

This is a **standalone desktop IDE shell** for Aura, built with **Tauri**.
It is meant to live alongside the VS Code extension (VSIX) and reuse the same `aura-lsp` proof-driven APIs.

## What it does (MVP)

- Open a `.aura` file
- Real editor surface (CodeMirror) with line numbers and Aura syntax highlighting
- Live diagnostics while typing via LSP `textDocument/publishDiagnostics`
- Live proof refresh (debounced) via custom `aura/proofs`
- Inline underlines for diagnostic ranges + click a diagnostic to jump
- Manual **Proofs** refresh button

Under the hood:

- The backend starts `aura-lsp` and speaks JSON-RPC over stdio.

## Bundled LSP sidecar

When packaging the desktop app, `aura-lsp` is bundled as a sidecar binary via Tauri `bundle.externalBin`.

- Build-time expected path: `src-tauri/bin/aura-lsp-<target-triple>(.exe)`
- On Windows, `libz3.dll` and runtime DLLs are also staged into `src-tauri/bin/` so the sidecar runs on clean machines.
- `tools/release/release.py --sentinel-app` automatically stages these files before running `tauri:build`.
- The frontend sends `textDocument/didOpen` + `textDocument/didChange` (full text).
- The frontend listens for `textDocument/publishDiagnostics` notifications.
- The Proofs button (and debounced typing refresh) calls the custom `aura/proofs` request.

## Dev run

```powershell
cd editors\sentinel-app
npm install
npm run tauri:dev
```

## Tests

```powershell
cd editors\sentinel-app\src-tauri
cargo test
```

## Notes

- The backend starts `aura-lsp` as a subprocess and speaks JSON-RPC over stdio.
- This is intentionally minimal UI (no Monaco yet). It’s the foundation for the full product.
