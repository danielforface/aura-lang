# Aura v1.0 Reliability Spec

Status: Draft (v1.0.0 Reliability)

This document defines what “Reliability” means for Aura v1.0: the guarantees we ship, the measurable targets we hold ourselves to, and the boundaries we make explicit.

מילת מפתח: **ודאות בלי לוותר על ביצועים** (Certainty without Sacrifice).

---

## 1) Reliability, defined

In Aura, Reliability is the combination of:

1. **Semantic reliability** — the same source has the same meaning across machines, builds, and releases (within the declared Edition + feature gates).
2. **Verification reliability** — the verifier provides stable, explainable results; when it cannot, it says *why* and *what is trusted*.
3. **Toolchain reliability** — builds are reproducible; artifacts are attributable; the user can reason about the compiler/verifier boundary.
4. **UX reliability** — interactive workflows stay interactive: proofs stream quickly, failures are explainable, and partial states are visible.

Non-negotiable principle:
- **Trust boundaries must be explicit**. If a result depends on a trusted core component, we surface that dependency.

---

## 2) Scope and non-goals

### In scope
- Verification streaming, caching, and incremental behavior.
- Trusted Core boundary definition and reporting.
- Deterministic builds and artifact manifests.
- Backend alignment (Dev-VM vs native) via differential testing.
- Region + linear ownership safety model (Option B) as the v1.0 target.
- Explainability: counterexamples, traces, and unsat-core driven explanations.

### Out of scope (v1.0)
- Full formal proof of the compiler/verifier implementation.
- Whole-program optimization equivalence proofs.
- A verified OS/runtime.

---

## 3) Vocabulary

- **Trusted Core**: the minimum set of components whose correctness we must assume for a given guarantee.
- **Trust Gate**: a CI-enforced policy that blocks release if reliability invariants are violated.
- **Proof Streaming**: incremental verification that reports partial progress/results without blocking the IDE.
- **Explain Engine**: the system that converts verifier outcomes into actionable explanations (counterexample + trace + source span mapping).
- **Option B**: Aura’s region-based memory model paired with linear ownership states.

Related references:
- `docs/trusted-core.md`
- `docs/ub-boundaries.md`
- `docs/stability-and-compat.md`
- `docs/effects-ownership-model.md`

---

## 4) Reliability SLOs (interactive + CI)

These are targets, not marketing.

### Interactive (Sentinel / LSP)
- Proof feedback latency: **p95 < 200ms** for incremental re-check on small edits in typical modules.
- Cancellation: verifier operations must be cancellable and yield quickly (no UI “hangs”).
- Partial results: the user must see phase-level progress (Parse → Sema → Verify) with partial diagnostics.

UI language guideline:
- Use Hebrew for UI labels where helpful (למשל: "הסבר", "ליבה מהימנה"), keep technical terms in English.

### CI
- Deterministic results: verifier outcomes are stable across CI runs given the same inputs + pinned toolchain.
- Differential testing: Dev-VM and native backends must agree on observable program behavior for the supported subset.

---

## 5) Trust Boundary and the Trusted Core Report

### Goal
Every build can answer: *What did we trust to claim this result?*

### Requirements
- The toolchain produces a **Trusted Core Report** for verification artifacts.
- The report is machine-readable (JSON) and can be rendered in Sentinel.
- The report includes:
  - Toolchain versions (compiler, verifier, Z3/solver versions)
  - Enabled Edition + feature gates
  - Backend selection and configuration
  - Hashes for relevant artifacts/config files
  - A clear list of trusted components and why they are trusted

### CI Gate
- Changes to the trusted core surface are treated as high-risk.
- CI can allowlist intentional changes, but it must be explicit and reviewed.

---

## 6) Proof Streaming and Incrementality

### Z3 Gate (incremental)
Reliability requirements:
- Incremental re-check does not re-verify the world on small edits.
- Streaming does not report “success” until the relevant obligations are discharged.
- If streaming is partial, the UI must show that it is partial ("מתבצע…").

### Stable proof keys (Merkle caching)
- A proof obligation must have a stable identifier derived from:
  - normalized source identity (module path + symbol name)
  - semantic signature (types + effects + constraints)
  - relevant environment (Edition + feature gates)
- Cache invalidation must be explainable: we should be able to show *what changed* and *why it caused re-proof*.

---

## 7) Memory safety model (Option B) and reliability invariants

v1.0 target: Region + Linear ownership states.

Key invariants (informal):
- A linear value is used exactly once unless it is explicitly transitioned to a shareable/borrowed form.
- Regions define lifetimes; borrows cannot outlive their region.
- Effects and ownership transitions are explicit and type-checked.

Reliability requirement:
- The verifier’s diagnostics must explain ownership failures in source terms ("מי מחזיק? מי שאל? מתי נצרך?") rather than solver jargon.

---

## 8) Concurrency reliability

Goal: prevent the most expensive class of bugs (data races, deadlocks) by construction.

v1.0 requirements:
- A concurrency model that statically maps shared state to protection (ownership, locks, channels, or effects).
- Diagnostics that name the contested resource and the protection contract.

---

## 9) Backend alignment: Differential Testing Trust Gate

Aura is allowed to have multiple execution backends, but not multiple meanings.

Requirements:
- Define a supported subset for differential testing (determinism-friendly, no UB, pinned runtime config).
- Run programs on both backends and compare:
  - exit status
  - stdout/stderr
  - structured outputs (when available)
- Divergence must be:
  - minimized (bugs fixed)
  - or explicitly scoped (known-different behavior with documented rationale)

---

## 10) Failure modes and diagnostics

Reliability also means **failure is structured**.

Requirements:
- Every verifier failure path returns:
  - a human-readable summary
  - a stable diagnostic code
  - structured payload (for IDE rendering)
- Common categories:
  - unsatisfied obligation (counterexample)
  - solver timeout / resource exhaustion
  - unsupported feature / gated feature
  - internal error (must produce a minimal repro bundle when possible)

---

## 11) Release criteria for “v1.0 Reliability”

Minimum bar:
- Trusted Core Report is generated and surfaced.
- Proof streaming meets the interactive SLO on representative projects.
- Cache is stable and explainable (no “mystery re-proofs”).
- Differential backend gate runs in CI and blocks regressions.
- Deterministic artifact manifests exist for release builds.

---

## 12) Open questions

- What is the minimal stable proof key that balances precision and cache hit rate?
- How do we best surface solver timeouts without training users to ignore them?
- Which subset is v1.0 “differentially tested” by default?
