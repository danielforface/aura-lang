# Stability and compatibility guarantees (1.0)

This repo treats Aura **editions** and **feature gates** as the mechanism for making compatibility promises.

## What is stable in 1.0

Within a given **edition** (currently: `2026`), the following are intended to be stable:

- **Syntax**: previously-valid code keeps parsing.
- **Static semantics**: type checking and name resolution for valid code remains compatible, except for bug fixes.
- **Runtime semantics**: behavior is preserved, except where the prior behavior violated the UB boundary policy (see `docs/ub-boundaries.md`).

Bug fixes may change:

- Error messages / diagnostics wording (diagnostic *codes* should stay stable).
- Rejection/acceptance of programs that were previously mis-parsed or mis-checked.

## How breaking changes happen

Breaking changes must be introduced using **one** of:

- A new **edition** (e.g. `2028`), selected via `--edition` or `aura.toml`.
- A new **feature gate** (unstable), selected via `--feature <name>` or `aura.toml`.

If a change would break existing code in the current edition and is not a bug fix, it must not land without an edition bump or a feature gate.

## Current limitations

- Only edition `2026` is considered supported by the CLI/manifest today.
- Some features are intentionally gated (example: `callsite-generics`).
