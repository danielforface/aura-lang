# Aura Ecosystem

Aura is more than a compiler pipeline. Its repository already contains an ecosystem layer around packages, SDK distribution, IDE integration, UI/application plugins, FFI, domain plugins and release tooling.

## Package manager

`aura-pkg` contains dedicated modules for:

- cache,
- CLI,
- commands,
- configuration,
- metadata,
- registry interaction,
- resolution,
- lockfiles,
- security,
- signing.

The crate itself is versioned `1.0.0`, but that is a **component version**, not evidence that the entire Aura language is a stable v1.0 release.

### Standalone command surface

Code-backed standalone commands:

```text
init
add
remove
list
publish
verify
```

### Primary CLI package surface

The primary `aura` executable currently contains integrated package commands including:

```text
add
publish
deprecate
```

The two surfaces should eventually converge into one documented command contract.

### Package integrity

The package subsystem includes dependencies and source modules for:

- SHA-256 integrity checks,
- semantic-version resolution,
- artifact download/cache,
- lockfile state,
- optional Ed25519 signing.

These are strong building blocks for supply-chain-aware packaging.

### Documentation drift

`docs/book/package-management-guide.md` currently shows a wider command vocabulary, including examples such as update/search/audit/login/stats/info. Not all of those commands were found in the current executable surface during this audit.

Therefore:

> treat `aura-pkg --help` / `aura pkg --help` and code as authoritative until the guide is reconciled.

## SDK

Aura has both an `aura-sdk` crate and a top-level `sdk/` tree.

SDK docs include:

- getting started,
- language reference,
- verifier guide,
- Z3 Gate protocol,
- LSP stability contract,
- cookbook/book material,
- welcome example.

The primary release script stages SDK content into a portable distribution tree.

## Standard library

`aura-stdlib` is a workspace member and verifier-related modules contain region/collection verification work.

Before a stable release, stdlib API stability should receive an explicit compatibility policy separate from compiler implementation status.

## Aura Sentinel

`editors/sentinel-app` is the dedicated desktop IDE shell.

Technology:

- Tauri 2,
- CodeMirror 6,
- TypeScript,
- Vite,
- Vitest.

Sentinel is the natural place for Aura-specific UX such as:

- streamed proof state,
- counterexample trees,
- source injections,
- debugger integration,
- proof/performance telemetry.

## VS Code

The repository contains multiple VS Code/editor integration trees. A future cleanup should designate one canonical extension package and archive/migrate legacy duplicates so users do not have to infer which is current.

## LSP

`aura-lsp` is the shared protocol/server foundation and should remain usable independently of Sentinel.

Documented Aura protocol version: `1`.

## Nexus and plugins

Workspace plugin-related crates:

```text
aura-nexus
aura-plugin-lumina
aura-plugin-ai
aura-plugin-iot
```

The CLI resolves plugin manifests and verification/build paths can receive Nexus plugin information.

This creates a route for extending the language platform without placing every domain feature into the core grammar/compiler.

## Lumina

Lumina is the largest visible application-layer plugin.

Current repository evidence includes:

- a substantial `aura-plugin-lumina` source implementation,
- layout/render syntax integration,
- Raylib-backed feature support,
- UI docs and cookbook,
- interactive input work,
- `TextInput`,
- `Box`,
- grid layout,
- image fit modes,
- audio controls,
- application examples.

Lumina should be marketed as an **evolving Aura UI/application layer**, not as a frozen cross-platform GUI standard.

## AI and IoT

The workspace contains optional `aura-plugin-ai` and `aura-plugin-iot` components. The main `z3` feature enables verification-related plugin features for these domains.

Public claims should be tied to concrete examples/modules rather than implying complete domain frameworks.

## AI optimization / bridges

The workspace also includes:

- `aura-ai-opt`,
- ONNX Runtime bridge C/H files under `tools/`,
- Raylib bridge files,
- `aura-bridge` for FFI/native linkage.

These are useful evidence of platform experimentation and native interoperability.

## Examples as product surface

Examples are part of the ecosystem because they reveal which vertical slices have actually been assembled.

Current examples include:

- verification,
- move/resource safety,
- concurrent queue,
- TCP echo server,
- IoT safety,
- vision safety,
- Lumina grid/image/audio UI.

## Release tooling

`tools/release/release.py` supports deterministic SDK ZIP packaging and provenance-oriented metadata.

Relevant behaviors include:

- stable archive timestamps/permissions,
- deterministic entry ordering,
- SHA-256 digest generation,
- `aura.attestation.v1` JSON emission,
- optional Windows signing,
- Sentinel LSP sidecar staging.

This is a strong basis for future nightly/beta/stable artifacts.

## Website

The repository contains a `website/` application and GitHub metadata points to:

```text
https://aura.geniuses.team/
```

The website should consume the same status/claims policy as the repository so that “stable,” “verified,” and performance wording do not drift independently.

## Ecosystem priorities before stable 1.0

1. unify package-manager command documentation,
2. select one canonical VS Code extension path,
3. publish a component/repository version policy,
4. publish signed/reproducible release artifacts,
5. define stdlib compatibility guarantees,
6. turn CI evidence into measured, non-hardcoded release proof,
7. publish a stable language specification/edition boundary.
