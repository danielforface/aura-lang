# Aura website content truth

The public website must remain aligned with the repository implementation and public reconciliation layer.

## Canonical public framing

**Aura 2026 Edition — active, pre-stable proof-driven systems programming language and developer platform.**

## Current facts used by the website

- Rust workspace: 22 members.
- Workspace / primary compiler package version: `0.2.0`.
- Language edition concept: `2026`.
- Aura-specific LSP protocol: version `1`.
- Verification: Z3-backed and feature-gated; profiles `fast`, `ci`, `thorough`.
- Development execution: AVM / interpreter path.
- Native-oriented execution: C-oriented backend plus feature-gated evolving LLVM IR path.
- Hybrid selector exists; automatic AVM → LLVM promotion is not implemented and must not be described as a production JIT.
- Package manager, LSP, Sentinel, SDK, Nexus, Lumina, domain plugins, FFI and Android/release tooling are substantive repository surfaces.

## Claims the website may use

- proof-driven systems programming language
- Z3-backed verification
- contracts/invariants integrated into the toolchain
- structured counterexample mapping
- 22-member language-platform workspace
- AVM + C-oriented backend
- evolving feature-gated LLVM IR backend
- Sentinel/LSP proof tooling
- package/FFI/plugin/Android integration work

## Claims the website must not make without new evidence

- formally verified compiler
- stable/frozen Aura v1.0 language
- universal memory-safety / zero-segfault guarantee
- production JIT or automatic AVM → LLVM promotion
- universal `<200ms` verification latency
- `100%` backend agreement unless measured by a repaired fail-closed workflow
- universal production-ready Android support
- binary downloads that are not actually published at the linked path

## Documentation precedence

1. Current compiler implementation.
2. `sdk/docs/reference.md`.
3. Focused current verifier/protocol documents.
4. Historical roadmap/design/completion documents.

## Performance policy

Do not publish illustrative benchmark numbers as evidence. A public performance result should name the commit, machine/software environment, workload/corpus, cache state, solver/toolchain configuration, sample count and percentile methodology.
