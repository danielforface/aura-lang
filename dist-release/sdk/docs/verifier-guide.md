# Verifier Guide (Z3 Gate Patterns)

This guide explains how to *use* Aura’s verification features effectively.

It complements:

- `sdk/docs/z3-gate.md` (IDE/LSP streaming protocol)

## 1. Contracts

Use contracts to document and prove intent at boundaries.

- `requires <bool-expr>`: precondition
- `ensures <bool-expr>`: postcondition

Example:

```aura
cell inc(x: u32) -> u32:
    requires x < 100
    ensures result == x + 1
    x + 1
```

## 2. Assertions and assumptions

- `assert <bool-expr>` is a proof obligation.
- `assume <bool-expr>` is a proof hint (use sparingly; it can hide bugs).

## 3. Loops

Loops should include an invariant when correctness matters.

- `while <cond> invariant <inv>:`
- Add `decreases <expr>` to help termination.

Guidelines:

- Make invariants strong enough to prove postconditions, but simple.
- Prefer bounds like `i <= n` rather than complex arithmetic.

## 4. Quantifiers

Quantifiers are powerful but can be expensive.

- `forall(...) : <bool-expr>`
- `exists(...) : <bool-expr>`

If proofs become slow:

- Reduce quantifier usage.
- Use the `fast`/`ci` SMT profiles for day-to-day work.

## 5. Counterexamples

When an `assert` fails, the verifier can return a counterexample model.

In the IDE:

- Sentinel renders a minimal binding slice when available.
- Prefer writing assertions that point to a specific property (“x stays in range”) rather than broad ones.

## 6. Practical workflow

- Start with assertions close to the bug.
- Add loop invariants once the shape of the proof is clear.
- Promote stable assertions to contracts (`requires`/`ensures`).
