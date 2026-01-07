# Editions and feature gates

Aura uses two mechanisms to control language evolution:

- **Editions**: coarse-grained compatibility versions.
- **Feature gates**: opt-in unstable features.

## Configuration

You can set these in either the CLI or `aura.toml`.

### CLI

- `--edition <YEAR>` (example: `--edition 2026`)
- `--feature <NAME>` (repeatable)

### `aura.toml`

Under `[project]`:

- `edition = "2026"`
- `features = ["callsite-generics"]`

CLI flags take precedence and are merged with manifest features.

## Behavior

- If a feature is not enabled, any syntax/behavior behind the gate should produce a clear parse error mentioning the feature name.
- Feature names are treated case-insensitively.

## Example gate

- `callsite-generics`: enables call-site generic arguments like `foo<T>(x)`.
