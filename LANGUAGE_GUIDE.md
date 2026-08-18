# Aura Language Guide — Current Implemented Surface

This guide is intentionally conservative. It summarizes the language surface supported by the repository's compact SDK reference and verifier documentation. It does not treat every syntax fragment in historical design docs as stable language syntax.

For conflicts, use this precedence:

1. current compiler implementation,
2. `sdk/docs/reference.md`,
3. focused current protocol/verifier docs,
4. historical roadmap/completion/design documents.

## Source structure

Aura uses significant indentation. The lexer emits indentation-aware structural tokens.

```aura
cell main():
    val x: u32 = 1
    if x == 1:
        log("one")
```

## Modules and imports

```aura
import std::io
import aura::tensor
```

SDK installs may inject default std modules based on imports.

## Bindings

Immutable binding:

```aura
val answer: u32 = 42
```

Mutable binding:

```aura
val mut index: u32 = 0
index = index + 1
```

Type annotations may be omitted where supported by the compiler's inference path, but explicit types are preferable in verification-oriented examples.

## Cells

Functions are declared with `cell`:

```aura
cell add(a: u32, b: u32) -> u32:
    a + b
```

### External cells

```aura
extern cell native_read(fd: u32) -> u32
```

Trusted external declaration:

```aura
trusted extern cell audited_clock() -> u32
```

Trust is semantic metadata, not proof that the external implementation is correct.

## Current built-in type surface

The compact reference currently names:

- `u32`
- `bool`
- `String`
- `Unit`
- `Tensor<Elem, [d0, d1, ...]>`
- `Model`
- `Style`

This is a current snapshot, not a promise that Aura will never add or change built-ins before a stable edition/release.

## Refinements

Range refinement syntax is currently documented for `u32`:

```aura
val percent: u32[0..100] = 75
```

Refinements are especially useful when they shorten proof obligations at API boundaries.

## Contracts

### Preconditions

```aura
cell divide_like(x: u32, d: u32) -> u32:
    requires d > 0
    x
```

### Postconditions

```aura
cell inc(x: u32) -> u32:
    requires x < 100
    ensures result == x + 1
    x + 1
```

## Assertions and assumptions

```aura
assert x <= limit
```

An assertion is a proof obligation.

```aura
assume platform_contract
```

An assumption intentionally adds trust. It should be visible and rare.

## Loops

```aura
val mut i: u32 = 0
while i < n invariant i <= n:
    i = i + 1
```

Verifier documentation also supports `decreases` hints for termination reasoning.

## Quantifiers

The verifier guide documents:

```text
forall(...): <bool-expr>
exists(...): <bool-expr>
```

Quantifiers can increase solver cost, so interactive code should prefer simpler invariants when possible.

## Control flow

Current reference includes:

- `if`
- `match`
- `while`

The repository contains pattern-lowering/compiler work beyond a trivial conditional parser, but precise pattern grammar should be taken from current parser behavior rather than old roadmap prose.

## Flow operators

The current reference distinguishes:

```aura
left -> right
```

for synchronous flow and:

```aura
left ~> right
```

for asynchronous flow.

## Async capture safety

The current reference states that async lambdas may not capture mutable outer bindings.

This is a concrete current safety rule and should be distinguished from broader planned concurrency guarantees.

## Resource moves

For resource-like types such as Tensor, Model and Style, the current reference documents move behavior when binding/assigning from an identifier.

Conceptually:

```text
Owned resource
    └─ move ─→ Moved resource
```

Using a moved value should be rejected by the relevant semantic/safety layer where supported.

The repository also contains broader ownership enforcement and linear-type infrastructure. Stable language documentation should converge those implementations with the compact reference before a 1.0 guarantee is made.

## Unsafe and FFI

Calling an untrusted extern requires an explicit unsafe block in the current model:

```aura
unsafe:
    native_read(fd)
```

A `trusted extern cell` can be called without `unsafe:` because the programmer/project has moved that external implementation into the trusted boundary.

## UI syntax

Aura's current reference includes `layout:` and `render:` statements. Lumina builds on this surface.

```aura
cell main():
    layout:
        VStack(alignment: "center") {
            render: Text(text: "Aura")
        }
```

Lumina is evolving independently enough that UI syntax should not be treated as evidence that every core-language semantic is stable.

## Verification-friendly style

Good Aura code makes obligations local and obvious:

```aura
cell push_index(i: u32[0..1023], len: u32[1..1024]) -> u32:
    requires i < len
    ensures result <= len

    val next: u32 = i + 1
    assert next <= len
    next
```

Prefer:

- small contracts at boundaries,
- simple loop invariants,
- explicit unsafe boundaries,
- narrow assumptions,
- stable assertions promoted to pre/postconditions,
- solver-friendly refinements over broad opaque predicates.

## What is not a stable language promise yet

Do not infer from the repository that Aura currently guarantees:

- every Rust-like ownership rule,
- a complete borrow checker equivalent to Rust,
- a frozen generic/type-trait model,
- a stable ABI,
- a stable package ecosystem API,
- a frozen UI DSL,
- universal no-segfault behavior,
- stable source compatibility across all future pre-1.0 revisions.

Those are release-contract questions, not just implementation questions.
