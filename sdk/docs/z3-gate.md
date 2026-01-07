# Z3 Gate (Sentinel ↔ aura-lsp) Protocol

Z3 Gate is the IDE contract used by **Aura Sentinel** to run proofs non-blocking and render proof UX (proven glow, counterexamples) as first-class UI.

## Goals
- Non-blocking proofs (no UI freeze, cancellable)
- Streaming UI cues (start/phase/done/error)
- Stable payload format for proof/counterexample details via `Diagnostic.data`

## JSON-RPC Methods (custom LSP)

### `aura/proofsStreamStart`
Starts a proof run for a document.

Request params:
```json
{ "uri": "file:///..." }
```

Response:
```json
{ "id": 123 }
```

### `aura/proofsStreamCancel`
Cancels a running proof stream.

Request params:
```json
{ "id": 123 }
```

Response: `{}`

## Notifications

### `aura/proofsStream`
Emitted by `aura-lsp` to report proof progress and results.

Payload:
```json
{
  "id": 123,
  "uri": "file:///...",
  "state": "start|phase|done|error|cancelled",
  "phase": "parse|sema|normalize|z3",
  "diagnostics": [ /* LSP diagnostics */ ],
  "error": "string"
}
```

Notes:
- `phase` is present only when `state == "phase"`.
- `diagnostics` is present only when `state == "done"`.

## Proof Details / Counterexamples
Proof notes and verification errors attach machine-parsable data:
- Proof notes (`source == "aura-nexus"`) include `data` fields like `plugin`, `kind`, `mask`, `range`, `smt`, and `related`.
- Verification errors include `data.model` when a Z3 model/counterexample is available.
- Verification errors may include a structured counterexample payload:
  - `data.counterexample.slice`: minimal binding slice for the failure (array of `{name,value,relevant}`)
  - `data.meta.bindings`: best-effort extracted model bindings (array of `{name,value,relevant}`)
  - `data.meta.relevantBindings`: subset of bindings that appear relevant to the failing assertion

### Counterexample Mapping (v1)

When available, verification errors also include a best-effort mapping from model bindings back into the source:

- `data.counterexample.schema == "aura.counterexample.v1"`
- `data.counterexample.mapped`:
  - `schema`: string
  - `bindings`: array of:
    - `name`: string
    - `value`: string
    - `value_kind` (optional): string (e.g. `bool|int|string|bitvec`)
    - `relevant`: boolean
    - `best_range` (optional): LSP `Range` where this binding name appears in the source
  - `injections`: array of:
    - `range`: LSP `Range` anchoring the injection
    - `text`: string to render as ghost-text in the editor
    - `name` (optional): binding name

Sentinel renders this via the Proofs panel “Data / Counterexample” section.
