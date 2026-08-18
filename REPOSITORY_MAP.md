# Aura Repository Map

Aura is a large monorepo. This map separates product code, current reference material, historical reports, generated distribution copies, examples, and website assets.

## Canonical product code

### Primary compiler/tooling workspace

| Path | Purpose |
|---|---|
| `aura/` | primary CLI and orchestration |
| `aura-lex/` | lexer |
| `aura-parse/` | parser |
| `aura-ast/` | AST |
| `aura-core/` | semantic core, lowering, diagnostics, safety/explanation modules |
| `aura-ir/` | intermediate representation |
| `aura-verify/` | proof/solver/counterexample layer |
| `aura-interpret/` | AVM / development runtime |
| `aura-backend-c/` | C-oriented backend |
| `aura-backend-llvm/` | LLVM IR backend |
| `aura-rt/` | runtime |
| `aura-rt-native/` | native runtime support |
| `aura-stdlib/` | standard library |

### Ecosystem/tooling crates

| Path | Purpose |
|---|---|
| `aura-pkg/` | package manager |
| `aura-sdk/` | SDK crate |
| `aura-lsp/` | language server |
| `aura-bridge/` | FFI bridge |
| `aura-nexus/` | plugin/integration layer |
| `aura-plugin-lumina/` | Lumina UI plugin |
| `aura-plugin-ai/` | AI plugin |
| `aura-plugin-iot/` | IoT plugin |
| `aura-ai-opt/` | AI-oriented optimization tooling |

## Editor products

```text
editors/sentinel-app/
editors/aura-vscode/
editors/vscode/
```

`sentinel-app` is the dedicated Aura desktop IDE application.

The two VS Code-oriented directories should eventually be reconciled/documented as canonical vs legacy if both remain.

## Current reference documentation

Prefer these for current behavior:

```text
sdk/docs/reference.md
sdk/docs/verifier-guide.md
sdk/docs/z3-gate.md
sdk/docs/lsp-stability.md
docs/debug-protocol.md
docs/effects-ownership-model.md
docs/lumina-ui.md
docs/lumina-media.md
docs/release-channels.md
```

And the public wrapper files:

```text
PROJECT_STATUS.md
ARCHITECTURE.md
LANGUAGE_GUIDE.md
VERIFICATION_MODEL.md
TOOLCHAIN.md
ECOSYSTEM.md
ANDROID_SUPPORT.md
PUBLIC_CLAIMS_POLICY.md
ROADMAP_PUBLIC.md
```

## Detailed/historical reports

The repository contains many milestone and completion reports, including phase reports, January status material, roadmap snapshots and root-level APK completion documents.

These are valuable development history but should not be the first documents a new user sees.

Their role should be:

```text
historical evidence / engineering log
```

rather than:

```text
current normative product contract
```

## Generated / replicated documentation

The repository also contains distribution-oriented trees such as `dist-release/` and `dist-complete/` with replicated documentation.

Search results may therefore return multiple copies of the same report.

When investigating current truth, prefer the canonical source path outside generated distribution trees.

## Examples

`examples/` contains runnable/illustrative vertical slices.

Use examples to understand integrations, but verify the current compiler command/reference before treating example syntax as a stable guarantee.

## SDK

`sdk/` contains developer-facing docs, standard-library material and Android/platform content.

The SDK is packaged by release tooling rather than being only a documentation folder.

## Tools

`tools/` includes:

```text
compat/
trusted-core/
release/
z3/
run_examples.py
native bridge C/H files
```

These are important evidence of a productized toolchain.

## CI

`.github/workflows/` contains multiple workflows with different maturity levels.

The simple `ci.yml` is the cleanest baseline. Differential workflows require reconciliation before they should be used as public proof badges.

## Website

`website/` contains the project website source. The repository metadata points to:

```text
https://aura.geniuses.team/
```

The site should consume `PROJECT_STATUS.md` / `PUBLIC_CLAIMS_POLICY.md` semantics to avoid drift.

## Root-directory hygiene recommendation

Do not delete product files merely to make the repository look clean. Instead, over time:

1. move historical completion reports into `docs/history/`,
2. move APK-specific reports under a platform documentation subtree,
3. keep generated distribution copies out of primary search/index paths where possible,
4. keep current public wrapper docs at root,
5. maintain `docs/README.md` as the documentation index.

This wrapper intentionally does **not** perform those moves because they could break existing links/history and the request is to wrap the current product, not restructure its implementation.
