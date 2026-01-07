# LSP Stability Contract

This document describes the stability guarantees for `aura-lsp`.

## Protocol version

The server exposes an Aura-specific protocol version via:

- `initialize` result `capabilities.experimental.auraProtocolVersion`

Current value: `1`.

Clients should use this value to gate usage of Aura-specific extensions.

## Compatibility promises

Within the same `auraProtocolVersion`:

- Aura-specific `experimental` fields are backward compatible.
- Diagnostic `code` values are stable.
- `Diagnostic.data.stable_code` (when present) is stable.

If we need to make a breaking change to Aura-specific extensions, we will bump `auraProtocolVersion`.
