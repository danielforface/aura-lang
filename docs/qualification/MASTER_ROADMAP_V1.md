# Aura Long-Horizon Qualification Roadmap — Canonical Plan v1

**Plan ID:** AURA-QUAL-PLAN-001  
**Created:** 2026-10-23  
**Baseline:** `main@e8f1c35a4a56adbde983dce38bba59a8c4eee105`  
**Status:** FROZEN once merged; never edit this file in place.  
**Owner of scope:** the Aura maintainer. **Execution:** successive, reviewed development sessions and PRs.  
**Purpose:** make Aura a trustworthy, usable proof-driven native systems language through many incremental releases, not an all-at-once upgrade.

> **Immutable contract.** This file defines *what* must be accomplished, dependencies and acceptance requirements. Progress, blockers, evidence, versions, implementation choices and conversation handoffs belong exclusively in mutable companion files. Do not check off work here. No unchecked item is evidence of absence, and no checked milestone is evidence without artifact links. If a plan change becomes unavoidable, publish `MASTER_ROADMAP_V2.md` plus a reviewed supersession record; keep v1 available indefinitely. No informal plan rewrite.

## 1. Context, relation to existing roadmap and release philosophy

This plan is an **execution layer**, not a replacement for `ROADMAP_PUBLIC.md`, `ROADMAP.md`, `AURA_1_0_QUALIFICATION_PROGRAM.md`, `AURA_1_0_CAPABILITY_INVENTORY.md`, `PROJECT_STATUS.md`, `PUBLIC_CLAIMS_POLICY.md` or `sdk/docs/reference.md`. Existing Gate A maps chiefly to Q00/Q09; B to Q02/Q03/Q05/Q06; C to Q01/Q04; D to Q02/Q03/Q08; E to Q09; F to Q10; G to Q08/Q11 (experimental platforms). The old inventory was audited at an earlier SHA and cannot automatically prove the latest code.

We will build capability through **multiple independently scoped releases**. Every release must narrow risks, carry tests, disclose limitations, and preserve a working supported subset. A component version (`0.2.0`, `1.0.0`) is not evidence of a language-wide stable release. Release numbering is decided and recorded in the mutable release ledger; this roadmap does not pre-assign exact version numbers or dates.

**Core product hypothesis:** Aura wins through precise, explainable program proofs on native components, auditable trust boundaries and a high-quality edit–prove–run loop. It need not match every Rust, SPARK or Dafny feature for initial stability.

**Definition of supported:** the behavior is specified, tested end-to-end and qualified for a declared target/backend/profile. Parsing a feature, constructing IR, or having a CI file does not by itself constitute qualification.

## 2. Program invariants — apply to every release and session

- **Truthfulness / fail closed:** missing solver, unknown, timeout, cancelled, unsupported and not-run must never be reported as proved; assertions cannot silently disappear when not proved unless their runtime behavior and mode are explicitly defined.
- **No fabricated evidence:** label findings `static-code-confirmed`, `reproduced`, `historical`, `suspected`, `target` or `qualified`. Do not claim tests ran unless a command, environment and actual outcome exist.
- **Single authority:** immutable plan = scope and acceptance; `PROGRESS.md` = current work and release state; `EVIDENCE_LOG.md` = observation and proof; `CONTEXT_HANDOFF.md` = intent, decisions, work context and exact resumption.
- **Handoff on every unit of work:** update `PROGRESS.md` and `CONTEXT_HANDOFF.md` *in the same proposed change* whenever work advances. Explicitly identify unfinished work, blockers and next safe action. A new chat must be able to resume without previous chat history.
- **Stable IDs:** every acceptance item uses its AURA-Qxx-yyy ID permanently; never reuse or renumber. New discoveries are appended to a mutable findings backlog and assigned to the relevant gate; the canonical acceptance requirement remains unchanged.
- **Small reviewable changes:** isolate one failure mode or coherent vertical slice per PR; green CI, regression tests and evidence links precede closure. Never merge autonomously.
- **No accidental broadening:** only a designated supported backend/OS/feature profile may carry stable guarantees. AVM/LLVM/Android/Lumina/AI/JIT/GC remain experimental until separately qualified.
- **Safety and source discipline:** no forced push, no direct write to main, no secrets in files/handoff, no unsupported performance claims or speculative certification.
- **Releases compound:** every new release preserves the previously qualified subset; exceptions require documented regressions, migration and explicit maintainer approval.
- **Current tasks are not release promises:** scope may be executed in smaller batches and in parallel where dependency edges permit, but gates are not bypassed.

## 3. Severity and evidence vocabulary

`P0`: incorrect proof/silent semantic miscompilation/memory unsafety claim; cannot qualify as safe/stable. `P1`: required language/toolchain contract or release gate. `P2`: adoption/quality/performance improvement. `P3`: optional extension.  
`NOT_STARTED → IN_PROGRESS → BLOCKED → IN_REVIEW → VERIFIED → RELEASED`; `VERIFIED` requires link to a passing, scoped artifact, not confidence or a source diff. `RELEASED` also requires an actual versioned release artifact.  
Release-quality evidence requires exact commit, target/backend/mode, commands, expected/observed result and links to CI output or permanent tests.

## 4. Execution order and gate acceptance

Each task is an enduring *requirement*, not an implementation prescription. Gates may share preliminary work, but cannot be marked VERIFIED without satisfying their exit requirements.

### Q00 — Baseline, governance and continuity (preparatory gate)
**Dependency:** none. **Outcome:** future chats and contributors can locate objective status, reproduce findings and audit plan changes.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q00-001 | P1 | Establish branch/PR workflow, authority, baseline commit, canonical index and named ownership for decisions. |
| AURA-Q00-002 | P1 | Preserve the legacy Gates A–G and 1.0 qualification documents; cross-map old items to new IDs without replacing history. |
| AURA-Q00-003 | P1 | Keep separate immutable plan, mutable progress/release ledger, append-only evidence and always-current cross-chat handoff. |
| AURA-Q00-004 | P1 | Enforce in CI that this canonical plan cannot be edited in place and substantive PRs update progress and handoff together. |
| AURA-Q00-005 | P1 | Re-audit current code vs outdated inventory with precise source paths, confidence and reproduction status. |
| AURA-Q00-006 | P1 | Publish a minimal reproducible compiler+verify+native smoke procedure on clean checkout with required prerequisites. |
| AURA-Q00-007 | P1 | Record version namespaces, chosen initial tier/platform, supported profiles and experimental exclusions; no unsupported claims. |
| AURA-Q00-008 | P2 | Define bug intake, triage, security reporting and decision log / explicit maintainer approvals. |

**Exit:** governance guard passes; branch+draft PR exist; baseline and evidence entries linked; current next step explicit in handoff.

### Q01 — Fail-closed verifier and trustworthy CLI semantics
**Dependency:** Q00 tracking. **Outcome:** no false success or erased obligations.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q01-001 | P0 | `aura verify` without a working Z3 verifier returns an explicit non-success state and nonzero exit. |
| AURA-Q01-002 | P0 | `verify --report` cannot emit `ok:true` when verification did not run; use complete typed status taxonomy. |
| AURA-Q01-003 | P0 | Define and enforce when `requires/ensures/assert/assume` are proved, runtime-checked, rejected or explicitly trusted in each build profile. |
| AURA-Q01-004 | P0 | Eliminate assertions/contracts silently discarded on native build paths without a defined assurance mode. |
| AURA-Q01-005 | P0 | Negative CLI/solver-fixture tests for not-run, unknown, timeout, failed, unavailable, unsupported, cancelled and internal-error. |
| AURA-Q01-006 | P1 | Typed exit codes and machine-readable proof report schema with versioning and reproducible diagnostics. |
| AURA-Q01-007 | P1 | Trace every trust-producing assume/extern/axiom to a source position and include it in an auditable assurance manifest. |
| AURA-Q01-008 | P1 | CI must call the real command users run and must fail on a deliberately unprovable assertion. |

**Exit:** fail-closed regression suite and real release-mode verification prove that unverified work cannot be mislabeled as verified.

### Q02 — Scalar correctness, ranges and typed lowering
**Dependency:** Q01 assurance mode and Q00 corpus. **Outcome:** exact numeric semantics across proof and execution.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q02-001 | P0 | Specify exact-width integer and float semantics, cast rules, overflow, divide-by-zero, shifts, NaN and signed comparisons. |
| AURA-Q02-002 | P0 | Remove hard-coded `U32` typing in C binary/unary emission; preserve operand/result types and all widths. |
| AURA-Q02-003 | P0 | Make refinement/range handling sound for signed, 64/128-bit and floating operations; eliminate silent `u32` truncation. |
| AURA-Q02-004 | P0 | Align SMT arithmetic/BitVector encoding with actual native integer behavior and proof obligations. |
| AURA-Q02-005 | P0 | Reject unresolved/unknown types before code generation, rather than silently replacing with `U32`. |
| AURA-Q02-006 | P1 | Add execution and negative-tests for every supported numeric type, boundary, optimization mode and verifier mode. |
| AURA-Q02-007 | P1 | Gate release on AVM/native differential agreement for specified scalar behavior and overflow cases. |

**Exit:** scalar conformance corpus passes across declared supported targets; no untyped scalar lowering remains on those paths.

### Q03 — Composites, control flow and IR soundness
**Dependency:** Q02 typed IR. **Outcome:** source programs mean the same thing in the supported backend.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q03-001 | P0 | Implement actual C emission for construct/get/set record instructions and preserve field layout and types. |
| AURA-Q03-002 | P0 | Implement enum tagged payload construction, tag reads, projections and pattern matching in supported backend. |
| AURA-Q03-003 | P0 | Preserve/lower semantic information for tuples, applied generics, references, constrained ranges and tensor layouts or fail explicitly. |
| AURA-Q03-004 | P1 | Define and test pattern exhaustiveness, guarded/nested patterns, unreachable arms and payload binding. |
| AURA-Q03-005 | P0 | Validate SSA dominance, Phi correctness, effect ordering and every optimization pass with metamorphic/differential tests. |
| AURA-Q03-006 | P1 | Differentially execute deterministic AVM/C fixtures with output, exit status, memory effects and failure comparison. |
| AURA-Q03-007 | P1 | Ensure unsupported IR instructions fail compilation rather than becoming no-op success. |

**Exit:** representative real composite programs compile/run correctly; O0/O1/O2 and AVM/native agree on defined observations.

### Q04 — Formal verification soundness and trust accounting
**Dependency:** Q01-Q03 semantic foundation. **Outcome:** proof applies to the execution model with clearly bounded assumptions.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q04-001 | P0 | Publish normative meaning of proved contracts, postconditions, loops, termination, overflow and quantified specifications. |
| AURA-Q04-002 | P0 | Define trustworthy modeling of external calls, purity, mutable state, FFI contracts and unknown results. |
| AURA-Q04-003 | P0 | Define source→IR→backend preservation obligations and specify remaining trusted computing base. |
| AURA-Q04-004 | P0 | Invalidate proof cache for all relevant source, dependency, environment, solver, flag, compiler and trust-contract changes. |
| AURA-Q04-005 | P1 | Add translation-validation/differential fixtures for optimized and unoptimized code and proof caching. |
| AURA-Q04-006 | P1 | Ship auditable machine-readable TCB/assumption summaries and source-anchored counterexamples. |
| AURA-Q04-007 | P1 | Cross-check solver failures/unknown/model traces in CI with carefully chosen soundness regressions. |
| AURA-Q04-008 | P2 | Measure verification latency P50/P95/P99 on versioned corpus and never market targets as measurements. |

**Exit:** no unsupported equivalence between solver model and emitted behavior is assumed silently; trust report accompanies verified build.

### Q05 — Memory safety, ownership and concurrency contract
**Dependency:** Q01, Q03 and Q04 contracts. **Outcome:** enforceable safety envelope for supported subset.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q05-001 | P0 | Publish precise region, move, consume, borrow, aliasing, mutable access, cleanup and lifetime rules. |
| AURA-Q05-002 | P0 | Cover control-flow joins, loops, region escapes, compound values and function boundaries with negative tests. |
| AURA-Q05-003 | P0 | Reject use-after-free, double-free, dangling references, use-after-move and unsafe aliasing on supported paths. |
| AURA-Q05-004 | P1 | Define safe/unsafe/extern boundary semantics and explicit trusted-extern obligations. |
| AURA-Q05-005 | P0 | Define concurrency scheduling, memory-order/race guarantees, task lifetimes, cancellation and error propagation. |
| AURA-Q05-006 | P1 | Correct or explicitly demote current synchronous `~>` prototype behavior and non-general GC prototype. |
| AURA-Q05-007 | P1 | Run target-relevant sanitizers and race/ownership adversarial integration tests against real native runtime. |

**Exit:** bounded safety guarantee with negative tests and no blanket universal memory/race-safety claim.

### Q06 — Language specification and composability
**Dependency:** Q02, Q03, Q05 core meaning. **Outcome:** a versioned reference and language constructs adequate for real libraries.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q06-001 | P1 | Publish normative, versioned grammar, evaluation order, name binding, scopes, imports, visibility, diagnostics and edition policy. |
| AURA-Q06-002 | P1 | Make module resolution, dependency graph, cycles, public API boundaries and separate compilation reliable. |
| AURA-Q06-003 | P1 | Implement coherent generic substitution/inference and retain actual type arguments through AST/IR to backend. |
| AURA-Q06-004 | P1 | Provide a documented bounded trait/interface mechanism with implementation coherence and verifier-aware contracts. |
| AURA-Q06-005 | P1 | Qualify `Option`, `Result`, `?` and deterministic error/panic semantics. |
| AURA-Q06-006 | P1 | Qualify strings, slices, arrays, tuples and iteration necessary for a practical core. |
| AURA-Q06-007 | P2 | Compile docs/examples as tests and verify diagnostics remain actionable across supported features. |
| AURA-Q06-008 | P3 | Defer reflection, general macros, JIT and unqualified syntax extensions without compromising stable core. |

**Exit:** spec conformance tests cover all declared stable constructs, including deliberate rejection of unsupported forms.

### Q07 — Standard library and native runtime
**Dependency:** Q05-Q06. **Outcome:** useful native programs without mock-only APIs.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q07-001 | P1 | Implement and qualify core, errors, formatting, owned strings and basic iterator protocols. |
| AURA-Q07-002 | P1 | Implement bounds-safe arrays/slices/vector/maps/sets or publish a narrower supported collection set. |
| AURA-Q07-003 | P1 | Implement real files, streams, paths, buffered IO, environment and time APIs with typed errors. |
| AURA-Q07-004 | P1 | Define allocator failure, object ownership, destruction, shutdown and no-silent-error behavior. |
| AURA-Q07-005 | P1 | Replace or segregate tensor/AI/IO mocks (fixed global handle pools, zero-return on invalid indices, fake inference). |
| AURA-Q07-006 | P2 | Provide network/TLS/serialization modules through audited packages or measured standard APIs, not placeholders. |
| AURA-Q07-007 | P1 | Make library public surface and runtime ABI versioned and qualified on declared targets. |

**Exit:** a nontrivial external Aura project uses stable library APIs and passes end-to-end native tests.

### Q08 — Backend, FFI, targets and performance
**Dependency:** Q02-Q05 and Q07. **Outcome:** supported native target matrix, without implying other targets are ready.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q08-001 | P1 | Designate C backend Tier 1 only after passing complete conformance, with explicit platform/ABI/target matrix. |
| AURA-Q08-002 | P1 | Define layout, alignment, calling conventions, callbacks, linking, pointers, nullability and error/ownership FFI contracts. |
| AURA-Q08-003 | P0 | Replace regex-only C-header binding inference and unknown-type-to-`u32` fallback with fail-closed typed parsing/bindings. |
| AURA-Q08-004 | P1 | Test C native compilation on clean environments and exercise reference third-party C libraries. |
| AURA-Q08-005 | P1 | Ensure build-cache keys track compiler, flags, target, headers, linked libraries, dependencies and runtime versions. |
| AURA-Q08-006 | P1 | Set explicit experimental status for LLVM, hybrid/JIT, WASM and Android unless independently qualified. |
| AURA-Q08-007 | P2 | Publish reproducible runtime, binary-size, build-time and memory benchmarks vs documented baselines. |

**Exit:** one native backend/platform combination can be built, verified, installed and independently reproduced without semantic surprises.

### Q09 — Package manager, reproducible supply chain and releases
**Dependency:** Q01 and Q07-Q08 stable subset. **Outcome:** users install the tested bits and can trust their provenance.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q09-001 | P1 | Unify `aura` and `aura-pkg` commands, manifests, generated templates, paths, dependency semantics and documentation. |
| AURA-Q09-002 | P1 | Make `aura-pkg init` emit valid Aura syntax and prove create→build→verify→run in CI. |
| AURA-Q09-003 | P0 | Make package `verify` actually validate required hashes/signatures and reject missing/invalid metadata. |
| AURA-Q09-004 | P1 | Version lockfile/registry protocols, validate HTTP status/content and defend archive extraction from traversal/bombs. |
| AURA-Q09-005 | P1 | Define immutable package versions, publisher keys, yank/revocation/incident policy and registry failure mode. |
| AURA-Q09-006 | P1 | Build versioned, signed/checksummed OS artifacts with SBOM and provenance; test clean install/uninstall. |
| AURA-Q09-007 | P1 | Document compiler/language/edition/ABI/SDK/LSP compatibility and migration policy; enforce SemVer boundaries. |
| AURA-Q09-008 | P1 | Make release CI exercise the real generated artifact, not only release-script syntax. |

**Exit:** at least one genuine tagged release includes a reproducible installed toolchain and verified artifacts.

### Q10 — Developer tooling, diagnostics and real-user workflow
**Dependency:** Q01 report schema and Q06 language contract. **Outcome:** practical edit–prove–run without an insider's help.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q10-001 | P1 | Separate behavioral `aura test` from formal proof checks; no-tests behavior explicitly defined for strict CI. |
| AURA-Q10-002 | P1 | Qualify one canonical VS Code LSP/VSIX path with version and install compatibility. |
| AURA-Q10-003 | P1 | Integrate proof result/assumption/counterexample source spans and ensure stale editor proofs cannot look current. |
| AURA-Q10-004 | P2 | Test cancellation, incremental edits, multi-file diagnostics, completion, rename, navigation and long sessions. |
| AURA-Q10-005 | P2 | Document debugger capabilities per backend, recovery from solver failure and Sentinel's distinct research role. |
| AURA-Q10-006 | P2 | Publish start-to-first-proof onboarding and measure usability with external contributors. |

**Exit:** a developer unfamiliar with SMT can troubleshoot a verified example in a supported editor from clean install.

### Q11 — External qualification, adoption and stable-language decision
**Dependency:** release-capable Q01-Q10 subset; no forced deadline. **Outcome:** credible public language-level stability or an honest narrower release.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q11-001 | P1 | Publish explicit Go/No-Go matrix and independently reproducible qualification bundle for stable subset. |
| AURA-Q11-002 | P1 | Audit release security/trust and remediate all unresolved relevant P0 defects before any stable claim. |
| AURA-Q11-003 | P1 | Validate compatibility and upgrade path with at least one maintained external nontrivial example project. |
| AURA-Q11-004 | P2 | Invite independent review of numeric semantics, verifier soundness, ownership and native backend. |
| AURA-Q11-005 | P2 | Document comparison with Rust/Dafny/SPARK using reproducible data and defined use cases, not marketing assertions. |
| AURA-Q11-006 | P1 | Publish language-wide stable release **only if** all required gates are evidenced, otherwise publish smaller honest channel/version. |
| AURA-Q11-007 | P2 | Sustain release/maintenance cadence, regression policy, issue response and active external adoption. |

**Exit:** signed stable release with meaningful external use, or an explicit NO-GO decision and continued prerelease iteration.

## 5. Release trains (sequencing guidance, not promised dates or version numbers)

| Train | Prerequisites | User-visible scope | Release gate |
|---|---|---|---|
| R0 — Program foundation | Q00 | roadmap governance, audit baseline, handoff and evidence | all Q00 artifacts reviewed |
| R1 — Trust emergency | Q01 | fail-closed verifier/CLI/contract profiles | adversarial no-solver/false-proof regression suite |
| R2 — Scalar fidelity | Q02 | typed numbers, refinements, overflow parity | numeric end-to-end conformance |
| R3 — Composite/native fidelity | Q03 | records/enums/pattern/IR/optimizations | AVM↔C differential CI |
| R4 — Proof & safety envelope | Q04-Q05 | trust accounting, verified memory/ownership boundary | explicit TCB and safety counterexamples |
| R5 — Productive stable subset | Q06-Q07 | frozen core, modules, errors, collections/IO | external example builds/runs |
| R6 — Native production pilot | Q08 | C Tier 1, FFI, sanitizer and performance qualification | reproducible native qualification |
| R7 — Distribution & usability | Q09-Q10 | packages, installers, tests, LSP | user install→prove→run smoke |
| R8 — Stable decision | Q11 | public qualification and external adoption | reviewed GO/NO-GO |

Each train may produce multiple prereleases; no automatic version bump is mandated. Never treat a train as complete merely because its associated crates compile. The mutable release ledger records real tags only.

## 6. Entry/exit discipline for every implementation slice

1. Read the handoff, current progress, baseline findings, open PRs and latest branch head before starting.
2. Choose a *small* stable task ID and note assumptions, dependency and failure mode.
3. Capture a failing/negative regression or other executable baseline when feasible.
4. Implement only the smallest coherent fix, including diagnostic and trust behavior.
5. Run actual relevant tests and report commands and outcomes exactly; unresolved CI is NOT VERIFIED.
6. Add stable evidence links (source diff, tests, CI, build artifact), adjust evidence confidence, update progress.
7. Update the handoff in the same change with decisions, blockers, next step and connection/branch context.
8. Open/update a draft PR, request explicit maintainer review; only after approval merge/release.
9. On failure, document the gap and keep the scope/release claim narrow; never mark a blocker complete to advance.

## 7. Non-goals and formal plan amendments

Not required for initial stable subset: production JIT, automatic AVM→LLVM promotion, broad GC, independent IDE as a prerequisite, generalized reflection/macros, production AI inference, complete UI/Android stack, every language ecosystem feature. They remain permitted experiments with honest flags. Any permanent change to goals or gate definitions requires a **new immutable plan version**, linked supersession rationale and maintainer approval, rather than editing this document.

The user explicitly requires that a *fresh chat with no prior conversation* can carry on directly. The authoritative starting point is `docs/qualification/CONTEXT_HANDOFF.md`; never rely on assumed conversational memory or carry-over tool authentication. See `OPERATING_PROTOCOL.md`.
