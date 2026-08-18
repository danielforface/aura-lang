# Aura Toolchain

This file documents the **code-backed command surface** observed in the primary CLI and related workspace tools.

## Primary CLI

Run:

```bash
cargo run -p aura -- --help
```

The `aura` crate is the main orchestration entry point.

## Global concepts

### Edition

The CLI accepts a global edition override:

```bash
aura --edition 2026 ...
```

### Feature gates

Unstable language features can be enabled through repeatable feature arguments and/or manifest configuration.

## Build profiles

```text
dev
release
verify
```

Profiles select defaults for optimization and SMT behavior.

## Execution modes

```text
avm       Aura VM / interpreter development mode
llvm      LLVM-oriented native path
hybrid    AVM for run; native-oriented build behavior
```

### Hybrid caveat

Current source explicitly says automatic promotion from AVM execution to LLVM inside the AVM is **not yet implemented**.

Do not describe current hybrid mode as a JIT.

## Build

```bash
aura build [path]
```

Current options include:

- build profile,
- execution mode,
- backend selection,
- bridge headers,
- extra library paths,
- extra libraries,
- optimization selection,
- SMT profile.

CLI backend labels include:

```text
c       C23-oriented transpilation/emission path
llvm    LLVM IR path
wasm    WASI-oriented path through external toolchain/clang
```

The backend maturity levels are not identical.

## Run

```bash
aura run [path]
```

Supports:

- profile,
- mode,
- backend,
- bridge/link inputs,
- optimization,
- SMT profile,
- `--hot` source monitoring/rebuild behavior.

## Verify

```bash
aura verify [path]
```

With the Z3 feature:

```bash
cargo run -p aura --features z3 -- verify main.aura --smt-profile ci
```

The command can emit machine-readable trusted-core report JSON through `--report`.

For workspaces, report output must be a directory so individual target reports can be written.

## Test

```bash
aura test .
```

The primary CLI discovers Aura tests under `tests/**/*.aura` in resolved workspace roots and verifies them.

## Lint

```bash
aura lint main.aura
```

Current linting includes format/parse/semantic-oriented checks rather than claiming an exhaustive Clippy-equivalent policy engine.

## Format

```bash
aura fmt main.aura --check
aura fmt main.aura --write
```

## Initialize

```bash
aura init my-project
```

The main CLI contains project initialization support.

## Bindgen

```bash
aura bindgen --header demo.h --out build/bindgen
```

Optional best-effort refinement mapping:

```bash
aura bindgen \
  --header demo.h \
  --out build/bindgen \
  --refine-types
```

Current bindgen is explicitly described as a bootstrap implementation, so generated bindings should be reviewed at trust boundaries.

## Package surface in the primary CLI

The integrated `aura pkg` surface currently includes commands such as:

```text
add
publish
deprecate
```

with support for registry paths/URLs, version selection, signatures/trusted keys and local publishing behaviors in the relevant paths.

## Standalone `aura-pkg`

The separate package-manager crate exposes a broader standalone CLI:

```text
init
add
remove
list
publish
verify
```

This command set does not exactly match every example in `docs/book/package-management-guide.md`.

Treat the executable `--help` as authoritative.

## Language server

```bash
cargo run -p aura-lsp
```

The LSP is its own substantial tool with Aura-specific protocol extensions.

## Sentinel

```bash
cd editors/sentinel-app
npm install
npm run tauri:dev
```

Build:

```bash
npm run tauri:build
```

The app uses Tauri 2, CodeMirror, Vite and TypeScript.

## SDK packaging

```bash
python tools/release/release.py --help
```

The release tool can stage an SDK and create deterministic artifacts.

## Workspace CI baseline

The straightforward CI workflow currently runs:

```bash
cargo test
bash tools/compat/run.sh
bash tools/trusted-core/check.sh
```

This is the cleanest CI surface to advertise once a passing run is linked.

## Recommended local verification before a PR

Core-only change:

```bash
cargo fmt --check
cargo test
bash tools/compat/run.sh
bash tools/trusted-core/check.sh
```

Verifier change with Z3 available:

```bash
cargo test -p aura-verify --features z3
cargo run -p aura --features z3 -- verify <fixture> --smt-profile ci
```

LSP change:

```bash
cargo test -p aura-lsp
```

Sentinel change:

```bash
cd editors/sentinel-app
npm install
npm test
npm run build
```

Android changes should use the Android-specific setup/build path or the repository workflow.

## Backend terminology

Use precise descriptions:

- **AVM** = development VM/interpreter.
- **C backend** = default portable/native-oriented emission path.
- **LLVM IR backend** = implemented feature-gated backend, evolving.
- **WASM/WASI path** = toolchain-mediated build route, not a dedicated workspace backend crate.
- **Hybrid** = current mixed mode selector, not automatic JIT promotion.
