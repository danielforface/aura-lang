# Aura Language (2026 Edition) — Prototype Compiler

This workspace contains a minimal, end-to-end prototype of the Aura compiler pipeline:

- `aura-ast`: Span-carrying AST nodes.
- `aura-lex`: Indentation-aware lexer (produces `INDENT/DEDENT/NEWLINE`) using `logos`.
- `aura-parse`: Recursive-descent parser for the current Aura grammar.
- `aura-core`: First semantic pass (type aliases + basic range proof for literals).
- `aura`: CLI that parses + checks a `.aura` file.

## Run

```bash
cargo run -p aura -- main.aura
```

If the program type-checks, it prints `ok`. Otherwise you’ll get `miette` diagnostics.

## Native (Phase 3) skeleton

Native compilation is behind Cargo features because LLVM/Z3 require system installs.

- LLVM build (requires LLVM + Z3 installed and linkable):
	- `cargo run -p aura --features z3,llvm -- build main.aura --backend llvm`
	- Output: `build/main/module.ll`

## Verification (Z3)

When built with `--features z3`, Aura can verify additional safety/correctness annotations:

- Contracts:
	- `requires <bool-expr>`
	- `ensures <bool-expr>`
- Assertions:
	- `assert <bool-expr>`
	- `assume <bool-expr>`
- Loops:
	- `while <cond> invariant <bool-expr> decreases <int-expr> { ... }`
- Quantifiers (guardrailed):
	- `forall(x: T, y: U): <bool-expr>`
	- `exists(x: T): <bool-expr>`

Solver configuration is exposed via an SMT profile:

```bash
cargo run -p aura --features z3,llvm -- build main.aura --backend llvm --smt-profile ci
cargo run -p aura --features z3,llvm -- run   main.aura --mode llvm   --smt-profile fast
```

Profiles: `fast`, `ci`, `thorough` (quantifiers are only accepted in `thorough`).
