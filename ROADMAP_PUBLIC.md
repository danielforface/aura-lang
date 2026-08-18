# Aura Public Roadmap

This is the concise public roadmap. It does not replace the repository's detailed historical `ROADMAP.md`; it provides a stable, readable view of what matters before Aura can honestly call the whole language platform “stable 1.0.”

## North star

Aura should become a daily-driver systems language where:

- contracts are natural to write,
- proof failures are understandable without SMT expertise,
- edit/prove latency is low enough for interactive use,
- safety rules are normative rather than aspirational,
- execution backends agree on observable semantics,
- package provenance and trust boundaries are inspectable,
- releases are reproducible and versioned coherently.

## Gate A — Repository truth convergence

- [ ] reconcile workspace/compiler/component version namespaces
- [ ] define one repository-wide release version policy
- [ ] reconcile package-manager docs with executable commands
- [ ] designate canonical VS Code integration path
- [ ] move/label historical completion docs so they cannot be mistaken for current normative docs
- [ ] repair differential CI reporting so pass/fail is data-derived
- [ ] publish CI badges only after measured workflows are trustworthy

## Gate B — Language contract

- [ ] publish a versioned normative 2026 Edition specification
- [ ] freeze syntax/semantic compatibility rules for the first stable edition
- [ ] reconcile ownership/linear/capability implementation with normative safety text
- [ ] define exact behavior of `unsafe` and trusted externs
- [ ] define standard-library stability tiers
- [ ] define diagnostics compatibility policy beyond protocol codes

## Gate C — Verification contract

- [ ] define proof result taxonomy: proved / failed / unknown / timeout / cancelled
- [ ] publish trusted-computing-base model
- [ ] establish cache invalidation soundness tests
- [ ] make proof summaries reproducible across supported modes
- [ ] publish benchmark harness and corpus for latency SLOs
- [ ] report `<200ms` only as measured percentile data with environment metadata

## Gate D — Backend trust

- [ ] select stable-release backends
- [ ] make differential suite executable from a clean checkout
- [ ] fail CI on backend discrepancies
- [ ] remove success-hardcoding / swallowed backend failures
- [ ] capture regression fixtures on semantic divergence
- [ ] define C vs LLVM vs AVM parity expectations
- [ ] either implement hybrid AVM→LLVM promotion or rename/document hybrid behavior to avoid JIT implication

## Gate E — Package and supply chain

- [ ] unify `aura pkg` and standalone `aura-pkg` command contract
- [ ] freeze lockfile schema version
- [ ] publish signing/trust-key model
- [ ] publish registry protocol or local-registry contract
- [ ] generate trusted-boundary package reports from actual metadata
- [ ] make release artifacts reproducible in CI
- [ ] publish checksums/attestations for official artifacts

## Gate F — IDE / developer experience

- [ ] stabilize Aura protocol beyond v1 as needed
- [ ] end-to-end proof streaming tests
- [ ] end-to-end counterexample v2 tests
- [ ] Sentinel installer/release flow
- [ ] canonical VSIX build/release flow
- [ ] debugger capability matrix by platform/backend
- [ ] recovery/stress tests for long-running LSP/Sentinel sessions

## Gate G — Android/platform qualification

- [ ] publish supported ABI matrix
- [ ] clean-checkout Android toolchain bootstrap
- [ ] sample Aura app build/install/run test
- [ ] Lumina basic UI device test
- [ ] input/media smoke tests
- [ ] release APK signing story
- [ ] clearly separate runtime cross-compile from complete app support

## Stable 1.0 definition

Do not define Aura 1.0 by feature count.

Define it by **contracts**:

1. language semantics are versioned,
2. supported backends are identified,
3. verifier/trust semantics are documented,
4. CI can detect disagreement instead of asserting agreement,
5. package/release artifacts are reproducible,
6. core tooling has a documented compatibility policy,
7. public performance claims are benchmark-backed,
8. version numbers agree across the product story.

Feature work can continue after 1.0. Trust convergence cannot be postponed indefinitely.
