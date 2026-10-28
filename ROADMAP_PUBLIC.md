# Aura Public Roadmap

> **2026-10-28 full-platform roadmap extension (proposed, under review):** [Master V2](docs/qualification/MASTER_ROADMAP_V2.md) preserves Q00–Q11 and adds Q12–Q18 for modern Aura-authored Desktop, Web, Android and iOS applications, plus future R9–R15 trains. Current development remains Q03-001 / [Draft PR #57](https://github.com/danielforface/aura-lang/pull/57); Master V1 is unchanged. This is a target, not an implemented/qualified UI release.

> **Long-horizon execution overlay (2026-10-23):** The existing Gates A–G and historical roadmap remain intact. The frozen multi-release acceptance plan, live stage tracker, evidence ledger, and fresh-chat continuity notes are maintained under [docs/qualification/](docs/qualification/README.md). Work follows reviewable incremental releases, not an immediate all-feature 1.0 upgrade.

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

## Future Gate H — Full modern Aura application/UI platform (proposed Master V2)

Aura's **eventual product mandate** is full, modern, accessible and easy-to-author apps in Aura across Windows/macOS/Linux Desktop, Web/browser and Android/iOS, without reducing core verification, memory/type safety, FFI or native semantics. This expansion is independent of the initial core-only stable subset and not implemented simply by editing a roadmap.

- [ ] Q12 typed UI framework: reusable components, state, events, reconciliation, navigation, async and form contracts.
- [ ] Q13 production adaptive layouts, accessible widgets, text/Unicode/RTL/IME, themes, design systems, media and localization.
- [ ] Q14 real installable Windows/macOS/Linux Lumina apps, OS integration and resource lifecycle.
- [ ] Q15 accessible semantic browser/Web frontend, typed JS/WASM interop, routing, SSR/SEO strategy and deploy tooling.
- [ ] Q16 genuine Android and iOS app rendering, native APIs, real-device testing and signed distribution.
- [ ] Q17 easiest practical developer flow: create → live preview/hot reload → debug/test/verify → package/deploy, plus IDE and component packages.
- [ ] Q18 independent per-target security, a11y, differential and performance evidence with competitive peer-app benchmarks and GO/NO-GO.

The original Gates A–G and R0–R8 remain valid and active. New UI trains R9–R15 will be scheduled separately; see [Master V2](docs/qualification/MASTER_ROADMAP_V2.md) for all 102 added requirements.

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
