# Recommended GitHub Repository Settings

These settings are not applied by the overlay script. They are owner/admin actions in GitHub.

## About description

Recommended:

> **Proof-driven systems programming language with Z3-backed verification, AVM/native-oriented backends, LSP proof streaming, Sentinel IDE, FFI and package tooling.**

This is more accurate than describing the compiler itself as formally verified or presenting an unqualified sub-200ms guarantee.

## Website

```text
https://aura.geniuses.team/
```

## Topics

Recommended focused set:

```text
programming-language
compiler
systems-programming
formal-methods
program-verification
z3
rust
llvm
lsp
static-analysis
refinement-types
ffi
package-manager
tauri
android
```

## Features

Recommended:

```text
Issues       ON
Projects     optional
Wiki         OFF
Discussions  optional after community demand
```

Documentation already lives in version control, so Wiki adds another drift surface.

## Main branch

The audited `main` branch was not protected.

Recommended ruleset:

- restrict deletion,
- block force pushes,
- require pull request before merge,
- require conversation resolution,
- allow repository owner to recover/administer rules appropriately.

### Status checks / Required Branch Protection

Following the CI architecture refresh (PR #5), all 7 workflows are validated and passing.

Because some workflows are path-filtered (e.g. `Backend Matrix`, `Android`, `LSP and Debugger Compatibility`), they run automatically when their relevant subsystems are modified, but are skipped on docs-only changes.

**Recommended Required Status Checks for `main` (always-triggering checks):**

1. `Core CI / Rust workspace` (runs on all PRs)
2. `Website Static Export / Build and verify static site` (runs on all PRs)
3. `Dependency Review / Block newly introduced high-risk dependencies` (runs on all PRs)

When pull requests touch compiler backends, Android runtime, or LSP/editor tooling, their respective specialized workflows (`Backend Matrix`, `Android`, `LSP and Debugger Compatibility`, `Release Tooling Validation`) must pass prior to merge.

## Merge strategy

Recommended:

- squash merge ON for focused feature/fix PRs,
- merge commits optional for large coordinated branches,
- automatically delete merged branches ON.

## Security

Enable:

- private vulnerability reporting,
- Dependabot alerts if appropriate for the repository,
- secret scanning where available.

## Social preview

Use a clean technical image emphasizing:

```text
AURA
Proof-driven systems programming
Source → Proof → IR → Runtime
```

Avoid putting unqualified performance numbers or “formally verified compiler” in the preview.

## Releases

When repository-wide releases start, publish:

- SDK artifact,
- checksums,
- attestation metadata,
- Sentinel installer when qualified,
- VSIX when canonicalized,
- release notes,
- exact supported backend/platform matrix.

Use tags that represent the **whole release contract**, not a component's internal version.
