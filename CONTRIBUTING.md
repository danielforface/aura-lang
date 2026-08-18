# Contributing to Aura

Aura is a programming-language monorepo spanning compiler engineering, formal verification, runtime systems, package management, LSP/IDE work, UI/plugins and platform tooling.

Changes that look local can alter a language contract. Please classify the change before implementation.

## Change classes

### Language syntax / grammar

Touchpoints can include:

```text
aura-lex
aura-parse
aura-ast
sdk/docs/reference.md
examples / parser tests
```

A syntax PR should state:

- grammar change,
- edition/feature-gate behavior,
- parse error behavior,
- compatibility impact.

### Type / safety semantics

Likely touchpoints:

```text
aura-core
aura-verify
aura-ir
sdk/docs/reference.md
docs/effects-ownership-model.md
```

State:

- accepted/rejected programs before/after,
- trusted-boundary impact,
- verifier impact,
- runtime/backend assumptions.

### Verification

State:

- generated obligation change,
- solver profile impact,
- timeout/unknown behavior,
- counterexample schema impact,
- cache invalidation implications.

Never convert timeout/unknown into success.

### IR/backend

State:

- Aura IR changes,
- C/LLVM/AVM behavior,
- ABI/runtime impact,
- differential/compatibility evidence.

### LSP/protocol

If Aura-specific protocol fields change, preserve the compatibility contract or bump the Aura protocol version.

### Package manager

State:

- manifest schema,
- lockfile schema,
- registry behavior,
- resolver behavior,
- signing/trust changes,
- CLI compatibility.

### Sentinel/Lumina/platform

Keep application-layer changes from silently redefining core-language semantics.

## Before opening a PR

At minimum for Rust workspace changes:

```bash
cargo fmt --check
cargo test
```

When applicable:

```bash
bash tools/compat/run.sh
bash tools/trusted-core/check.sh
```

With Z3 installed for verifier changes:

```bash
cargo test -p aura-verify --features z3
```

LSP:

```bash
cargo test -p aura-lsp
```

Sentinel:

```bash
cd editors/sentinel-app
npm install
npm test
npm run build
```

## PR evidence

A good PR includes:

1. problem statement,
2. semantic contract affected,
3. implementation summary,
4. tests added/changed,
5. before/after example,
6. compatibility notes,
7. trust/safety impact,
8. performance evidence if making a performance claim,
9. documentation updates.

## Performance claims

Do not write “X% faster” or “<200ms” without recording:

- commit,
- machine/environment,
- corpus/workload,
- cold/warm state,
- sample count,
- measurement method.

## Verification claims

Use precise words:

- `proved` only when the solver/toolchain produced a proof result under the documented model,
- `tested` for tests,
- `validated` for structural validation,
- `trusted` for assumptions/boundaries,
- `target` for objectives.

These words are not interchangeable.

## Large design changes

For a major change to syntax, types, ownership, effects, verification semantics, IR or ABI, open a design issue first using the language-design template.

## Security

Do not file exploitable security issues publicly. Follow [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contribution is provided under the repository's MIT License unless explicitly agreed otherwise.
