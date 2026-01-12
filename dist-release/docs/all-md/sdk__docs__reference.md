# Aura Reference (Normative, Snapshot)

This reference describes the language as implemented in this repository.

If the implementation and this document disagree, treat the implementation as the source of truth for now.

## 1. Lexing

- Indentation is significant.
- The lexer emits `INDENT`, `DEDENT`, and `NEWLINE` tokens.

## 2. Modules and imports

- Imports are statements:
  - `import std::io`
  - `import aura::tensor`

The SDK may auto-inject `sdk/std/<module>.aura` based on imports.

## 3. Declarations

- Value binding:
  - `val <name> : <Type>? (= <expr>)`
  - `val mut <name> : <Type>? (= <expr>)`
- Cell (function) definition:
  - `cell <name>(<params>...) -> <Type>? : <block>`
- Extern cell:
  - `extern cell <name>(<params>...) -> <Type>`
  - `trusted extern cell <name>(<params>...) -> <Type>`

## 4. Statements

- Assignment: `<ident> = <expr>`
- Control flow: `if`, `match`, `while`
- Proof annotations: `requires`, `ensures`, `assert`, `assume`
- UI blocks: `layout: <block>`, `render: <expr-or-block>`
- Unsafe block: `unsafe: <block>`

## 5. Expressions

- Literals: integers, strings
- Identifiers
- Unary/binary operators over `u32` and `bool` where supported
- Calls: `f(x)`; member/calls: `mod::fn(x)`
- Flow operator:
  - Sync: `left -> right`
  - Async: `left ~> right`

## 6. Type system (current)

Implemented built-in types include:

- `u32`, `bool`, `String`, `Unit`
- `Tensor<Elem, [d0, d1, ...]>` (shape optional)
- `Model`
- `Style`

Refinements:

- Range constraints on `u32` are supported in the form `u32[lo..hi]`.

## 7. Safety rules (current MVP)

### 7.1 Mutation aliasing (resource move)

For resource-like types (Tensor/Model/Style):

- Binding from an identifier moves the value.
- Assignment from an identifier moves the value.

### 7.2 Concurrency (async capture)

- Async lambdas may not capture mutable outer bindings.

### 7.3 FFI boundaries

- Calling an untrusted extern cell requires being inside `unsafe:`.
- `trusted extern cell` may be called without an `unsafe:` block.
