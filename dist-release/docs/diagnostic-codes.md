# Aura diagnostic codes (spec 0.2)

Aura diagnostics exposed via LSP use **stable, versioned codes**.

Format:

- `AUR-<spec>-<id>`
- Example: `AUR-0.2-0001`

## Code list

- `AUR-0.2-0001` — Parse error
  - Source: `aura-parse` errors surfaced through `aura-lsp`
- `AUR-0.2-0002` — Semantic/type error
  - Source: `aura-core` semantic checks surfaced through `aura-lsp`
- `AUR-0.2-0003` — Verification error
  - Source: `aura-verify` / Z3 Gate surfaced through `aura-lsp`
- `AUR-0.2-9000` — Internal/tooling error
  - Source: `aura-lsp` (or unexpected diagnostic wrapper)

## Notes

- Proof-note diagnostics keep their existing `nexus:<plugin>;...` codes; those are part of the Nexus protocol stream and are not in the `AUR-*` namespace.
- The stable code is intended for filtering, UI grouping, and regression tests. The human-facing message may change.
