# Aura Long-Horizon Qualification Roadmap — Canonical Plan v3 (Quantum Extension)

**Plan ID:** AURA-QUAL-PLAN-003  
**Strategic mandate:** 2026-11-05 — the maintainer explicitly decided Aura WILL enter quantum development.  
**Status:** PROPOSED TECHNICAL PLAN / Draft review — NOT IMPLEMENTED, NOT QUALIFIED, NOT RELEASED.  
**Parent:** Draft planning PR #58, exact immutable V2 snapshot `2d300a7b61316bb722ca6570f18bae6f9374d0ff`.  
**Governance:** original MASTER_ROADMAP_V1.md and MASTER_ROADMAP_V2.md must remain unchanged; this document preserves their complete 190 acceptance task rows verbatim. V2 is still a proposal, and V3 does not silently approve V2 or any implementation.  
**Scope:** additional Q19–Q38, R16–R35 for Quantum, orthogonal to original Q00–Q18 and their release trains.  

> **Future mandate is definite, readiness is not.** Decision to enter quantum is approved; exact task gates/implementation order still require normal maintainers' plan review, independent science/security review and CI evidence. No change to the active work of Agent A (Core Q03 / Draft #57) or Agent B (Lumina IBus / Draft #92). No merger, code feature or QPU access is authorized by this document.

## A. Immutable inherited V2 plan (historical scope preserved in full)

# Aura Long-Horizon Qualification Roadmap — Canonical Plan v2

**Plan ID:** AURA-QUAL-PLAN-002  
**Created:** 2026-10-28  
**Baseline:** `main@e8f1c35a4a56adbde983dce38bba59a8c4eee105`  
**Status:** PROPOSED SUPERSESSION — Draft review required; once accepted V2 freezes; V1 remains unchanged.  
**Owner of scope:** the Aura maintainer. **Execution:** successive, reviewed development sessions and PRs.  
**Purpose:** retain every Q00–Q11 trust/language/systems gate and expand the eventual Aura product to complete modern, accessible, easy-to-build apps for Desktop, Web and Android/iOS, with no weakening of formal verification, native systems behavior or safety.

> **V2 continuity and authority (2026-10-28).** Proposed reviewable scope amendment requested by the maintainer. Existing Q00–Q11 requirements and R0–R8 remain unchanged. The immutable V1 file is preserved. Q12–Q18 and R9–R15 are ADDITIVE future application-platform gates, not a new instruction to abandon ongoing Q03 work. Current implementation continues on qual/Q03-001-native-record-scalar / Draft PR #57; exact current status is in the latest appended PROGRESS and CONTEXT_HANDOFF and live GitHub CI. Core-only qualified releases remain independently possible when scoped truthfully. Old V1 §7 says full UI is not required for an **initial core-only stable subset**, not that complete UI is removed from Aura's long-term product. V2 activation requires reviewed maintainer approval.

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


### V2 ADDITIVE gates Q12–Q18 — complete all-platform application program

### Q12 — Typed declarative UI framework and application semantics
**Dependency:** Q03/Q05/Q06/Q07/Q10. **Outcome:** implemented, reproducibly tested modern applications on declared targets; code existence is not qualification.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q12-001 | P1 | Publish normative typed Aura UI syntax, component semantics, render phases, identity, effects and lifecycle; keep runtime-specific implementation outside core language assurance. |
| AURA-Q12-002 | P1 | Implement typed UI nodes/props/slots and emit actionable errors for unsupported components or properties instead of silent rendering success. |
| AURA-Q12-003 | P1 | Provide reusable composable UI components, prop defaults, local state, lifecycle disposal and stable interface/module boundaries. |
| AURA-Q12-004 | P1 | Provide reactive controlled app state, derived state, subscriptions and deterministic transaction/update ordering. |
| AURA-Q12-005 | P1 | Implement keyed reconciliation, stable component identity, efficient subtree updates, insertion/reorder/delete and state retention tests. |
| AURA-Q12-006 | P1 | Define focus ownership, pointer capture, event propagation/routing, cancellation, disabled state and deterministic callback semantics. |
| AURA-Q12-007 | P1 | Implement async UI tasks, timers, cancellation, concurrency-safe effects, background work and consistent busy/error/empty states without frame blocking. |
| AURA-Q12-008 | P1 | Provide declarative nested navigation/routing, state restoration, validated route params and back/deep-link behavior. |
| AURA-Q12-009 | P1 | Provide typed forms, validation, submit/cancel, error focusing and verifier-aware data/domain constraints. |
| AURA-Q12-010 | P0 | Define formal trust boundary for UI plugins, callbacks, foreign widgets, Web/OS input, network/files/clipboard, and reject fake proofs or unsupported states. |
| AURA-Q12-011 | P1 | Ensure source-to-IR-to-AVM/render/runtime semantics for UI events and state have real differential tests and do not inherit unjustified backend parity. |
| AURA-Q12-012 | P1 | Version Lumina UI SDK/component ABI, surface compatibility/deprecation policy and migration tests. |
| AURA-Q12-013 | P1 | Add error boundaries, safe recovery and source-anchored diagnostics for runtime and layout failures. |
| AURA-Q12-014 | P1 | Demonstrate full multi-screen app in actual Aura source with state, forms, async events and typed models on a declared target. |
| AURA-Q12-015 | P2 | Publish predictable rendering/state cost model and safe custom component/renderer extension points. |

**Exit:** all applicable P0/P1 requirements have source/runtime negative and positive tests, real CI/artifacts and explicit target bounds; otherwise NOT QUALIFIED.

### Q13 — Modern adaptive design system, typography and accessibility
**Dependency:** Q12/Q06/Q07. **Outcome:** implemented, reproducibly tested modern applications on declared targets; code existence is not qualification.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q13-001 | P1 | Implement real responsive Flex and Grid, grow/shrink/wrap, alignment, intrinsic measurement, constraints, min/max sizing and overflow rules. |
| AURA-Q13-002 | P1 | Support resizing, variable DPI/density, breakpoints, orientation changes, safe areas, multiple viewport sizes and no fixed resolution assumption. |
| AURA-Q13-003 | P1 | Implement scrolling, nested scroll input, large virtualized lists/grids/tables, stable item keys and predictable layout performance. |
| AURA-Q13-004 | P1 | Implement Unicode-aware typography, shaping, font loading/fallback, grapheme boundaries, line wrapping, ellipsis, styles and baseline alignment. |
| AURA-Q13-005 | P1 | Implement bidirectional Hebrew/Arabic RTL and international IME composition, selection, accurate cursor movement, clipboard, undo/redo and password-safe input. |
| AURA-Q13-006 | P1 | Provide screen-reader semantics, accessible names/roles/states, keyboard-only focus paths, announcements, high contrast and adjustable text. |
| AURA-Q13-007 | P1 | Audit WCAG 2.2 AA-oriented behavior using automated and real assistive-technology tests on declared platforms; support reduced motion. |
| AURA-Q13-008 | P1 | Ship production widget set: buttons, toggles, radio/checkbox, selection, text fields, date/time, dialogs, menus, combobox, tabs, popovers, tooltips and toasts. |
| AURA-Q13-009 | P1 | Ship production lists, tables, forms, pagination, loading/error/empty states and usable data-intensive app patterns. |
| AURA-Q13-010 | P1 | Provide cohesive design tokens, style cascade/precedence, light/dark/system mode, theme switching and visual consistency. |
| AURA-Q13-011 | P1 | Support vector/SVG, icons, images, clipping, rounded corners, shadows, gradients, alpha compositing and lifecycle-safe cached resources. |
| AURA-Q13-012 | P1 | Support interruptible transitions/animations, gestures, reduced-motion alternatives and measured frame pacing. |
| AURA-Q13-013 | P1 | Provide i18n/l10n, pluralization, translated resources, number/date formats and bidirectional layout adaptation. |
| AURA-Q13-014 | P1 | Test snapshots, geometry, accessibility trees and interaction across DPI, screen sizes, RTL and locales with exact recorded fixtures. |
| AURA-Q13-015 | P2 | Provide charts, advanced editors, custom graphics and rich-media surfaces without bypassing safe resource/focus semantics. |

**Exit:** all applicable P0/P1 requirements have source/runtime negative and positive tests, real CI/artifacts and explicit target bounds; otherwise NOT QUALIFIED.

### Q14 — Production native desktop Windows/macOS/Linux
**Dependency:** Q12/Q13/Q05/Q08/Q09. **Outcome:** implemented, reproducibly tested modern applications on declared targets; code existence is not qualification.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q14-001 | P1 | Qualify declared Windows, macOS and Linux OS/CPU/backend combinations with a reproducible feature and install matrix. |
| AURA-Q14-002 | P1 | Select and qualify rendering backend(s), GPU device-loss recovery, low-end/fallback behavior, clean asset loading and renderer equivalence. |
| AURA-Q14-003 | P1 | Support resize, maximize/minimize, fullscreen, multiple windows/monitors, DPI changes and lifecycle/shutdown correctness. |
| AURA-Q14-004 | P1 | Integrate OS file dialogs, clipboard, drag/drop, keyboard shortcuts, accessibility, native menus, notifications and theme settings. |
| AURA-Q14-005 | P1 | Define permissions and safe trust boundaries for native APIs, files/network/processes/plugins and foreign UI callbacks. |
| AURA-Q14-006 | P1 | Implement explicit resource ownership and cleanup for fonts, images, video/audio, GPU textures and long-lived UI caches. |
| AURA-Q14-007 | P1 | Qualify native localization, Unicode input, RTL, screen-reader and focus interactions on real desktops. |
| AURA-Q14-008 | P1 | Build signed/notarized where required installable, uninstallable, updatable packages with reproducible asset bundling and provenance. |
| AURA-Q14-009 | P1 | Test application state persistence/recovery, offline mode, crash recovery, startup and long-session memory/CPU stress. |
| AURA-Q14-010 | P1 | Run a real nontrivial Aura-created desktop app end-to-end from clean environment through install, navigation and UI test. |
| AURA-Q14-011 | P2 | Measure FPS P95/P99, startup, input latency, memory/GPU and idle use with published environments and honest peer comparisons. |
| AURA-Q14-012 | P2 | Provide desktop-specific debugging, profiling and release diagnostics without leaking private user data. |

**Exit:** all applicable P0/P1 requirements have source/runtime negative and positive tests, real CI/artifacts and explicit target bounds; otherwise NOT QUALIFIED.

### Q15 — First-class Web/browser frontend platform
**Dependency:** Q12/Q13/Q06/Q08/Q09. **Outcome:** implemented, reproducibly tested modern applications on declared targets; code existence is not qualification.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q15-001 | P1 | Choose and publish supported Aura Web architecture (semantic DOM/JS/WASM interop or equivalent), explicitly distinguishing Canvas from accessible HTML. |
| AURA-Q15-002 | P1 | Implement browser build/link/runtime bridge with typed ABI, correct Aura semantics, source maps and safe memory/callback boundaries. |
| AURA-Q15-003 | P1 | Map UI controls to semantic DOM with native form/focus/selection/input behaviors and accessible screen-reader state. |
| AURA-Q15-004 | P1 | Implement browser router, nested layouts, history, URL/query params, deep links, 404 and error boundaries. |
| AURA-Q15-005 | P1 | Implement or explicitly scope SSR/SSG/streaming/hydration, and demonstrate correct metadata/SEO without unearned framework parity claims. |
| AURA-Q15-006 | P1 | Provide forms, typed validation and browser-native file upload/download, clipboard, drag-drop and media APIs. |
| AURA-Q15-007 | P1 | Provide Fetch, WebSocket and stream APIs with cancellation, typed serialization, errors, authentication integration and offline handling. |
| AURA-Q15-008 | P0 | Qualify browser security against XSS, CSRF, CSP, CORS, injection, unsafe HTML, session leakage and sensitive credential handling. |
| AURA-Q15-009 | P1 | Implement responsive CSS equivalent/theming with standard typography, accessibility, animations and RTL support. |
| AURA-Q15-010 | P1 | Support browser storage, workers and optional PWA/offline/service-worker lifecycle with controlled updates. |
| AURA-Q15-011 | P1 | Provide typed JavaScript/TypeScript and React/Svelte interop for incremental adoption without weakening Aura's verifier semantics. |
| AURA-Q15-012 | P1 | Provide bundling, tree-shaking, code splitting, source maps, hashed assets and reproducible Web deployment. |
| AURA-Q15-013 | P1 | Provide browser hot reload, developer inspector, accessibility diagnostics and end-to-end test integration. |
| AURA-Q15-014 | P1 | Qualify Chromium/Firefox/WebKit and mobile browser matrix with keyboard, RTL, IME, a11y and actual device tests. |
| AURA-Q15-015 | P1 | Measure Core Web Vitals, load size, time-to-interaction, memory, responsiveness and server-rendering costs on real comparable apps. |
| AURA-Q15-016 | P1 | Ship production-style Aura Web app with forms, routes, API, secure auth integration and deployment smoke. |

**Exit:** all applicable P0/P1 requirements have source/runtime negative and positive tests, real CI/artifacts and explicit target bounds; otherwise NOT QUALIFIED.

### Q16 — Real Android and iOS application development
**Dependency:** Q12/Q13/Q05/Q07/Q08/Q09. **Outcome:** implemented, reproducibly tested modern applications on declared targets; code existence is not qualification.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q16-001 | P1 | Support actual Aura-driven application UI on Android and iOS, not merely packaging an Aura source asset inside a Kotlin/Swift demo. |
| AURA-Q16-002 | P1 | Publish supported Android API/ABI and iOS version/device/CPU toolchain matrix and reproducible setup. |
| AURA-Q16-003 | P1 | Implement adaptive mobile rendering, safe areas, density/orientation, touchscreen/gesture input and OS lifecycle. |
| AURA-Q16-004 | P1 | Handle suspend/background/resume/termination, navigation state, back gestures, deep links and state restoration. |
| AURA-Q16-005 | P1 | Provide reliable soft keyboard/IME, selection, clipboard, RTL and screen-reader mobile accessibility. |
| AURA-Q16-006 | P1 | Implement typed, permission-checked platform bridges for camera, sensors, files, location, microphone, contacts and sharing. |
| AURA-Q16-007 | P1 | Provide background-task limits, notifications, network/offline sync, cancellation and battery-aware scheduling. |
| AURA-Q16-008 | P1 | Provide secure storage, account/session integration and explicit mobile security/privacy permission design. |
| AURA-Q16-009 | P1 | Implement robust media/asset/audio/video support, memory pressure and GPU/resource lifecycle for mobile. |
| AURA-Q16-010 | P1 | Qualify JNI/Kotlin and Swift/Objective-C bindings with ABI, ownership, callback lifetimes and trusted extern reporting. |
| AURA-Q16-011 | P1 | Provide Android APK/AAB and iOS IPA provisioning, signing, install/update, TestFlight/store-ready packaging and real artifacts. |
| AURA-Q16-012 | P1 | Test on emulators/simulators AND real devices with automated UI E2E, permissions, accessibility and lifecycle tests. |
| AURA-Q16-013 | P1 | Measure cold start, memory, battery, frame pacing, crashes and responsiveness against reproducible baselines. |
| AURA-Q16-014 | P1 | Deliver real multi-screen Aura Android+iOS app with forms, navigation, network/offline state and deploy smoke. |

**Exit:** all applicable P0/P1 requirements have source/runtime negative and positive tests, real CI/artifacts and explicit target bounds; otherwise NOT QUALIFIED.

### Q17 — Developer-first DX, tooling and UI ecosystem
**Dependency:** Q09/Q10/Q12-Q16. **Outcome:** implemented, reproducibly tested modern applications on declared targets; code existence is not qualification.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q17-001 | P1 | Create simple Aura app templates and cohesive project manifests for Desktop, Web, Android and iOS; avoid unnecessary boilerplate. |
| AURA-Q17-002 | P1 | Provide single coherent check/test/verify/run/build/package/deploy workflow per supported target with meaningful diagnostics. |
| AURA-Q17-003 | P1 | Deliver reliable UI hot reload/fast refresh, explicit state retention/reset rules, accurate errors and reproducible dev builds. |
| AURA-Q17-004 | P1 | Provide UI component/prop completion, navigation, type and accessibility diagnostics in VS Code/LSP and Sentinel. |
| AURA-Q17-005 | P1 | Deliver interactive source-mapped inspector for component tree, layout bounds, events, focus, state and performance. |
| AURA-Q17-006 | P1 | Provide responsive/device/locale/RTL UI preview that is clearly distinguished from actual platform/runtime proof. |
| AURA-Q17-007 | P1 | Provide UI unit, event, component, snapshot, keyboard/accessibility and real browser/mobile/desktop end-to-end test harnesses. |
| AURA-Q17-008 | P1 | Provide debug event traces, async state transitions, error recovery, memory and rendering profiling linked to Aura source. |
| AURA-Q17-009 | P1 | Publish standard components, design systems, common app patterns and fully executable maintained Cookbook examples. |
| AURA-Q17-010 | P1 | Provide trustworthy versioned UI component packages, dependency compatibility, publisher/provenance and migration strategy. |
| AURA-Q17-011 | P1 | Support design assets, icon/font bundles, localization and secure release packaging without external manual hacks. |
| AURA-Q17-012 | P1 | Prove portable sharing of Aura domain/business models across Desktop/Web/Mobile without changing verified behavior. |
| AURA-Q17-013 | P1 | Measure newcomer time-to-first-screen and iteration time with independent user testing; publish the actual environments and targets. |
| AURA-Q17-014 | P2 | Support optional visual design token/Figma/AI assistance as auditable aids, not required runtime or unsupported safety claims. |
| AURA-Q17-015 | P2 | Publish reproducible competitor workflow comparisons and documented migration/interoperability examples. |

**Exit:** all applicable P0/P1 requirements have source/runtime negative and positive tests, real CI/artifacts and explicit target bounds; otherwise NOT QUALIFIED.

### Q18 — Cross-platform app qualification, safety and competitiveness
**Dependency:** Q01-Q17 relevant gates. **Outcome:** implemented, reproducibly tested modern applications on declared targets; code existence is not qualification.

| ID | Severity | Non-negotiable acceptance requirement |
|---|---|---|
| AURA-Q18-001 | P1 | Define distinct release/qualification tiers for Aura Core, Lumina Desktop, Web, Android and iOS; never imply all-platform stability from a core-only release. |
| AURA-Q18-002 | P1 | Maintain target/browser/device/OS/backend/ABI/locale/a11y feature matrix, precise limitations and source-to-package provenance. |
| AURA-Q18-003 | P0 | Enforce existing Q00-Q11 proof, memory, numeric, type, FFI and security invariants through all UI callback/runtime/backend/platform boundaries. |
| AURA-Q18-004 | P1 | Run deterministic portable-source AVM/Native/Web/Mobile state-and-event differential tests with explicit trusted-extern assumptions. |
| AURA-Q18-005 | P1 | Threat-model user input, remote assets, plugins, clipboard/files/network, mobile permissions, storage, deployment and update channels. |
| AURA-Q18-006 | P1 | Maintain auditable UI verification scope and trusted computing base; do not advertise proven widgets/OS effects absent formal evidence. |
| AURA-Q18-007 | P1 | Set and publish measurable UI budgets for interaction, frame time, startup, RAM, download/install size, battery and resource leaks. |
| AURA-Q18-008 | P1 | Compare representative applications to modern React/Svelte/Flutter/Compose/Slint stacks using identical requirements, published scripts and hardware. |
| AURA-Q18-009 | P1 | Audit RTL/i18n, screen readers, keyboard, focus, reduced motion, contrast and responsive behavior on every qualified target. |
| AURA-Q18-010 | P1 | Test clean checkout to scaffold, build, run, verify, package, install and deploy on each supported platform with actual artifacts. |
| AURA-Q18-011 | P1 | Ship signed reproducible app tools and artifacts with release, update, rollback, dependency and incident response policy. |
| AURA-Q18-012 | P1 | Maintain two realistic reference apps (data-intensive dashboard; accessibility-heavy workflow) with shared Aura logic and E2E regression. |
| AURA-Q18-013 | P1 | Require separate independently reviewed per-platform GO/NO-GO and a distinct all-platform production readiness decision. |
| AURA-Q18-014 | P1 | Demonstrate external developer onboarding and independently reproducible usability/stability/performance results. |
| AURA-Q18-015 | P1 | Keep Q03 and earlier active work uninterrupted; a new UI plan does not authorize unsafe shortcuts, merges or unqualified releases. |

**Exit:** all applicable P0/P1 requirements have source/runtime negative and positive tests, real CI/artifacts and explicit target bounds; otherwise NOT QUALIFIED.

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
| R9 — UI foundation | Q03/Q05-Q07, Q12 | typed components, state, events and application semantics | source/runtime tests and scoped CI |
| R10 — Accessible design platform | Q12-Q13 | responsive layout, widgets, text/RTL, accessibility | layout/locale/a11y E2E evidence |
| R11 — Desktop production | Q12-Q14 | Windows/macOS/Linux apps | signed actual installers and device/UI CI |
| R12 — Web production | Q12-Q13/Q15 | semantic Web frontend, typed interop, deploy | browser matrix, Web E2E and deploy CI |
| R13 — Mobile production | Q12-Q13/Q16 | Android and iOS apps | actual device tests and install/signing CI |
| R14 — Developer-first application ecosystem | Q12-Q17 | first-class IDE, hot reload, templates, SDK and package DX | external dev create→ship test |
| R15 — Full-platform readiness | relevant Q00-Q18 | independently scoped cross-platform releases | per-platform independent GO/NO-GO |

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


## 8. V2 product mandate and operational continuity

- **Ultimate scope:** write full modern desktop (Windows/macOS/Linux), browser/Web and Android/iOS applications in Aura, with competitive visual quality, native-feeling interaction, reusable UI, accessibility, localization and exceptionally simple developer experience. Implementation may use reviewed adapters, but must not misrepresent bridge-based packaging as native Aura UI support.
- **Never degrade Aura core:** do not sacrifice type/memory safety, solver honesty, contracts, proof trust, concurrency correctness, ABI, systems performance or existing Q00–Q11 gates to achieve frontend functionality.
- **Remain on current active work:** Q03-001 / Draft PR #57 implementation continues on its real branch/CI. V2 is a separate docs-only proposal; review does not authorize any implementation task switch, merge or rebase. Fresh chats inspect the latest handoff, progress and GitHub HEAD before resuming.
- **Independent honest release tiers:** a stable Aura Core subset does not imply Lumina Desktop, Web, Android or iOS stability. Each target earns qualification separately; “complete all-platform” requires relevant Q12–Q18 evidence as well as core gates.
- **Historical preservation:** V1 bytes remain untouched forever. V2 is a proposed additive reviewed successor, frozen after approval; later scope changes require V3. Latest PROGRESS/CONTEXT_HANDOFF and current PR/CI are the live state, not dated top snapshots.
- **Competition is tested, not advertised:** compare representative end-to-end apps with contemporary React, Svelte, Flutter, Compose and Slint using reproductions, ergonomics, a11y, performance and real release artifacts; unsupported claims remain prohibited.


---

## B. V3 ADDITIVE Quantum Program — Q19–Q38

**Intent:** Build a leading interoperable, proof-aware Quantum software stack in Aura spanning quantum/classical language semantics, typed linear qubits, circuit IR, verified passes, simulators, real QPU backend adapters, error correction and a scientific Sentinel Quantum Studio. This is a strategic PRODUCT goal, not a claim of market leadership, fault tolerance, proven quantum advantage, physical safety, universal equivalence, or currently working QPU integration.

**Separation of guarantees:** Static linear ownership prevents invalid aliasing/usage in specified programs; it does not prove No-Cloning physics generally. Measurement can be destructive or non-destructive; repeated measurement is not categorically forbidden. Static entanglement analysis must distinguish definite entanglement, MAY_ENTANGLE and UNKNOWN. Classical Z3 checks are useful for discrete constraints; general quantum equivalence requires quantum-specific models/checkers. Finite testing/shot counts cannot establish universal formal equivalence. Target calibration-dependent noise estimates are not proof of hardware execution quality. Every claim is scoped to specified semantic/profile/target versions.

**Gate contract:** each row requires an independent issue, named owner, source-first positive/negative reproduction where applicable, exact files/paths, assertion of trust boundaries, tests and CI on exact commit, reversible Draft PR, updated handoff and evidence. ALL new gates start NOT_STARTED until actual evidence is reviewed. Cross-cutting work obeys Q01–Q08/09/10/11, Q12–Q18 and signed release gating. No merge or QPU spend without approval.

### Q19 — Quantum charter, scientific governance, assurance tiers

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q19-001 | P0 | Define Aura Quantum as a versioned opt-in capability, not a prerequisite for Core, Lumina, or Aura 1.0 stability. |
| AURA-Q19-002 | P0 | Create reference scientific glossary: qubit, logical/physical qubit, channel, instrument, measurement, reset, entanglement, noise, fidelity, and equivalence. |
| AURA-Q19-003 | P0 | Specify claims taxonomy: mathematical proof, bounded verification, simulation, statistical experimental estimate, and hardware observation. |
| AURA-Q19-004 | P0 | Specify quantum feature maturity tiers: PROPOSED, EXPERIMENTAL, QUALIFIED_BY_PROFILE, and RELEASED_BY_TARGET. |
| AURA-Q19-005 | P0 | Define fail-closed quantum CLI status schema including UNSUPPORTED, UNKNOWN, TIMEOUT, CANCELLED and ERROR. |
| AURA-Q19-006 | P0 | Define research safety, provider credential, experiment authorization, and consent boundaries. |
| AURA-Q19-007 | P1 | Publish complete architecture decision records for quantum language embedding versus standalone DSL. |
| AURA-Q19-008 | P1 | Define versioned quantum compilation profiles, initial circuit-only subset, and explicit dynamic-circuit upgrade path. |
| AURA-Q19-009 | P1 | Map all quantum features onto existing Aura Q00-Q18 obligations without weakening them. |
| AURA-Q19-010 | P1 | Create external reviewer roles for quantum semantics, hardware assumptions, security, and reproducibility. |
| AURA-Q19-011 | P1 | Create machine-readable target-feature and proof-assumption registry with schema migration rules. |
| AURA-Q19-012 | P1 | Publish auditable quantum scope-mutation process preserving all previous roadmap task IDs. |
| AURA-Q19-013 | P2 | Perform transparent market and research gap comparison versus Guppy, Q#, Qiskit, CUDA-Q and tket using runnable examples. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q20 — Linear quantum ownership, resource and effect types

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q20-001 | P0 | Introduce opaque QuantumQubit handles with exactly-once ownership and no implicit Copy, Clone, serialization or memory introspection. |
| AURA-Q20-002 | P0 | Reject duplicate operand aliasing in multi-qubit gates including same qubit via compound aliases. |
| AURA-Q20-003 | P0 | Reject use-after-destructive-measure, use-after-discard, double release and implicit qubit leaks. |
| AURA-Q20-004 | P0 | Distinguish non-destructive measurement, destructive measurement, reset and disposal in type/effect signatures. |
| AURA-Q20-005 | P0 | Specify quantum allocation, return, move and borrow invariants across branches, loops, calls and exceptions. |
| AURA-Q20-006 | P0 | Prevent qubit references from escaping region/lifetime boundaries or crossing untrusted FFI silently. |
| AURA-Q20-007 | P0 | Propagate linearity through arrays, records, generics, tuples, closures and container destructors. |
| AURA-Q20-008 | P0 | Define a typed quantum-effect system for unitary, measurement, reset, conditional, calibration and host-control operations. |
| AURA-Q20-009 | P1 | Distinguish immutable classical measurement records from non-copyable quantum resources. |
| AURA-Q20-010 | P1 | Provide safe typed ancilla management, release preconditions and explicit dirty/clean ancilla profiles. |
| AURA-Q20-011 | P1 | Implement branch-join linear ownership checking and adversarial source-to-IR negative fixtures. |
| AURA-Q20-012 | P1 | Define zero-runtime-overhead quantum handle ownership ABI where feasible and prove preservation at lowering boundaries. |
| AURA-Q20-013 | P1 | Make lexical and borrow diagnostics source-mapped with ownership trace and nearest fix guidance. |
| AURA-Q20-014 | P1 | Fuzz quantum aliasing and linear resource transitions including nested user Cells. |
| AURA-Q20-015 | P2 | Independently review no-cloning claims to ensure static alias prevention is not misrepresented as complete physical theorem verification. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q21 — Quantum syntax, typed circuits, hybrid language design

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q21-001 | P0 | Define normative versioned Aura quantum syntax for qubit declarations, gates, circuits, measurement and reset. |
| AURA-Q21-002 | P0 | Define typed quantum/classical function boundaries, return protocols and effect requirements. |
| AURA-Q21-003 | P0 | Implement parameterized gates and unit-aware angle, duration and frequency types with explicit conversions. |
| AURA-Q21-004 | P0 | Define gate modifiers control, adjoint and power with supported-domain validation and recursion constraints. |
| AURA-Q21-005 | P0 | Specify measurement-conditioned classical control and target capability restrictions. |
| AURA-Q21-006 | P0 | Reject nonunitary operations inside adjoint/uncompute contexts unless explicitly supported by semantic rules. |
| AURA-Q21-007 | P1 | Support generic quantum registers with fixed/dynamic size bounds and scoped lifetime ownership. |
| AURA-Q21-008 | P1 | Define composable circuit functions, controlled subroutines, modules, visibility and verified library boundaries. |
| AURA-Q21-009 | P1 | Provide structured unitary blocks and explicit measurement/reset effect boundaries. |
| AURA-Q21-010 | P1 | Define deterministic classical control evaluation independent of sampling and QPU nondeterminism. |
| AURA-Q21-011 | P1 | Test parser/checker acceptance and rejection with generated source programs and normative spec tables. |
| AURA-Q21-012 | P1 | Support logical qubit and physical qubit handle distinctions without exposing fabricated hardware guarantees. |
| AURA-Q21-013 | P1 | Specify controlled dynamic circuits, bounded loops and feed-forward capabilities with explicit target profiles. |
| AURA-Q21-014 | P2 | Research advanced higher-order circuit abstractions, deferring unqualified metaprogramming or self-modifying code. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q22 — Quantum SSA IR and transformation invariants

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q22-001 | P0 | Create versioned Quantum IR with explicit qubit lifetime, gates, measure/reset, classical SSA, control and effects. |
| AURA-Q22-002 | P0 | Provide canonical quantum operation identity, qubit operand ordering, source spans and stable provenance IDs. |
| AURA-Q22-003 | P0 | Define quantum/classical mixed control-flow joins without duplicating live quantum resources. |
| AURA-Q22-004 | P0 | Represent dynamic measurements as classical result values with explicit non-deterministic semantics. |
| AURA-Q22-005 | P0 | Define precise Quantum IR validator rejecting malformed operands, unsupported gates and fabricated success. |
| AURA-Q22-006 | P0 | Separate physical placement from logical program semantics and retain layout permutation history. |
| AURA-Q22-007 | P0 | Record compilation pass preconditions, effect footprints and preserved invariants per pass. |
| AURA-Q22-008 | P1 | Support region-aware quantum resource liveness and ancilla allocation transformations. |
| AURA-Q22-009 | P1 | Implement canonical serialization, deterministic hashing and round-trip Quantum IR reader/writer. |
| AURA-Q22-010 | P1 | Enforce per-instruction source and contract attribution for verification evidence. |
| AURA-Q22-011 | P1 | Provide reference interpreter for small supported Quantum IR circuits and differential fixtures. |
| AURA-Q22-012 | P1 | Prevent unsound C/LLVM/native lowering of opaque quantum resources to arbitrary integers. |
| AURA-Q22-013 | P1 | Version profile and semantic compatibility checks for cross-module Quantum IR linking. |
| AURA-Q22-014 | P2 | Evaluate MLIR-based dialect interoperability with explicit licenses and stability restrictions. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q23 — Formal quantum semantics and verification foundations

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q23-001 | P0 | Define pure-state Hilbert-space denotational semantics for supported finite circuits. |
| AURA-Q23-002 | P0 | Define unitary and global-phase equivalence independently of exact matrix byte equality. |
| AURA-Q23-003 | P0 | Specify measurement probabilities and classical-output distributions including post-measurement state updates. |
| AURA-Q23-004 | P0 | Define mixed-state density matrices and completely positive trace-preserving channel semantics for noisy profiles. |
| AURA-Q23-005 | P0 | Specify composition, measurement instruments, reset, classical feed-forward and partial trace formally. |
| AURA-Q23-006 | P0 | Distinguish proof obligations from modeling assumptions and trusted external oracles. |
| AURA-Q23-007 | P0 | Do not treat classical Z3 QF_BV/QF_LIA success as proof of arbitrary quantum circuit equivalence. |
| AURA-Q23-008 | P1 | Provide small-step operational semantics and cross-check with denotational golden fixtures. |
| AURA-Q23-009 | P1 | Specify error tolerance, floating-point/interval enclosures and analytical versus numerical claims. |
| AURA-Q23-010 | P1 | Model and bound symbolic parameter domains and disclose unsupported transcendental proofs. |
| AURA-Q23-011 | P1 | Implement finite-state, Clifford/stabilizer and small exact algebra proof profiles. |
| AURA-Q23-012 | P1 | Specify qubit garbage/ancilla semantics, output observables and environment equivalence. |
| AURA-Q23-013 | P1 | Create trusted-computing-base and external-solver provenance manifest for every quantum proof. |
| AURA-Q23-014 | P1 | Audit universal quantification over inputs versus sample-based falsification; prohibit their conflation. |
| AURA-Q23-015 | P2 | Research quantum Hoare logic and compositional predicates as separate reviewed RFCs. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q24 — Equivalence engines and translation validation

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q24-001 | P0 | Create quantum equivalence result taxonomy PROVEN_EXACT, PROVEN_UP_TO_GLOBAL_PHASE, DISPROVEN, APPROXIMATE_BOUND, UNKNOWN and UNSUPPORTED. |
| AURA-Q24-002 | P0 | Check normalized gate decomposition on small exact matrix fixtures before trusting optimized circuits. |
| AURA-Q24-003 | P0 | Validate layout permutations, swapped wires, ancilla initialization and garbage outputs against observational semantics. |
| AURA-Q24-004 | P0 | Distinguish unitary equivalence from equivalence of channels and classical output distributions. |
| AURA-Q24-005 | P0 | Separate simulation-based counterexamples from complete mathematical equivalence proofs. |
| AURA-Q24-006 | P0 | Reject timeouts, unsupported parameterization and numeric tolerance violations as proof success. |
| AURA-Q24-007 | P1 | Integrate or independently implement decision-diagram-based exact equivalence for constrained circuit families. |
| AURA-Q24-008 | P1 | Investigate ZX-calculus equivalence with explicit support and termination boundaries. |
| AURA-Q24-009 | P1 | Provide independent oracle differential checks against MQT QCEC for overlapping supported cases. |
| AURA-Q24-010 | P1 | Validate each selected quantum compiler transformation by translation validation under explicit resource budgets. |
| AURA-Q24-011 | P1 | Implement unitary vs measured-output proof variants including mid-circuit measurements where supported. |
| AURA-Q24-012 | P1 | Produce machine-readable certificate, pass chain, checker/version hashes and source-linked mismatches. |
| AURA-Q24-013 | P1 | Check proof-cache invalidation on source, gate basis, compiler flags, target calibration and checker version. |
| AURA-Q24-014 | P1 | Implement reproducible RED fixtures demonstrating unsound gate rewrites and honest UNKNOWN outcomes. |
| AURA-Q24-015 | P2 | Research scalable compositional proofs for larger circuits without assuming polynomial complexity. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q25 — Quantum optimizer and synthesis pipeline

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q25-001 | P0 | Implement deterministic supported gate normalization with source and semantics preserved. |
| AURA-Q25-002 | P0 | Define reversible cancellation and identity simplification with proof or bounded independently checked rewrite rules. |
| AURA-Q25-003 | P0 | Introduce pass manager that records input/output Quantum IR and explicit effect signatures. |
| AURA-Q25-004 | P0 | Prevent optimizer from moving gates illegally across measurement, reset, barrier and classical feed-forward. |
| AURA-Q25-005 | P1 | Add Clifford and phase polynomial optimizations with verified preconditions. |
| AURA-Q25-006 | P1 | Implement gate synthesis and unitary decomposition with precision and cost budgets. |
| AURA-Q25-007 | P1 | Enable hardware-native gate rebasing with independent equivalence checks. |
| AURA-Q25-008 | P1 | Support depth, two-qubit count, T-count, T-depth and scheduled latency objectives separately. |
| AURA-Q25-009 | P1 | Introduce multi-objective optimizer with Pareto output and auditable chosen cost model. |
| AURA-Q25-010 | P1 | Handle bounded synthesis timeout deterministically without pretending global optimality. |
| AURA-Q25-011 | P1 | Provide seed-controlled stochastic heuristic passes with replayable optimization traces. |
| AURA-Q25-012 | P1 | Build adversarial pass fuzzers preserving quantum effects and symbolic parameter semantics. |
| AURA-Q25-013 | P1 | Benchmark optimizer against declared Qiskit/tket baselines with the same input circuits. |
| AURA-Q25-014 | P2 | Investigate AI-guided compilation only behind checker-approved postconditions. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q26 — Target models, routing, scheduling and hardware constraints

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q26-001 | P0 | Version hardware Target schema: native gates, topology, directionality, basis, timing, reset and feed-forward constraints. |
| AURA-Q26-002 | P0 | Reject scheduling or routing that targets unavailable couplers, gates or unsupported control conditions. |
| AURA-Q26-003 | P0 | Represent calibration timestamp, TTL and device-state snapshot in build artifacts. |
| AURA-Q26-004 | P0 | Track logical-to-physical and physical-to-logical mappings for every transformation. |
| AURA-Q26-005 | P0 | Keep target-aware optimization quality estimates separate from proven functional equivalence. |
| AURA-Q26-006 | P1 | Implement connected topology-constrained SWAP insertion with verified resulting layout. |
| AURA-Q26-007 | P1 | Support native gate direction reversal where legal and check decomposition semantics. |
| AURA-Q26-008 | P1 | Add crosstalk-aware scheduling and idle/decoherence estimates as modeled heuristics. |
| AURA-Q26-009 | P1 | Represent measurement latency, classical feedback latency and device instruction granularity. |
| AURA-Q26-010 | P1 | Produce target compliance validation and precise unsupported-feature diagnostics before submission. |
| AURA-Q26-011 | P1 | Support ion-trap, superconducting and neutral-atom target adapters via capability profiles, not fabricated universality. |
| AURA-Q26-012 | P1 | Version snapshot fixtures for device drift and regression reproducibility. |
| AURA-Q26-013 | P1 | Compare router/scheduler performance with independently timed standard device profiles. |
| AURA-Q26-014 | P2 | Investigate dynamic re-routing and mid-circuit state-aware scheduling with proof-preserving boundaries. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q27 — OpenQASM, QIR and interoperable representations

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q27-001 | P0 | Implement a version-pinned supported-subset OpenQASM 3.1 parser/importer with structural round-trip validation. |
| AURA-Q27-002 | P0 | Implement emitted OpenQASM 3.x syntax with gate/circuit/measurement profile feature detection. |
| AURA-Q27-003 | P0 | Implement QIR Base Profile emission with LLVM/QIR profile metadata and required output recording contract. |
| AURA-Q27-004 | P0 | Reject unsupported mid-circuit feed-forward under Base Profile rather than silently rewriting it. |
| AURA-Q27-005 | P0 | Preserve qubit indexing, result ordering, classical data mapping and quantum layout provenance. |
| AURA-Q27-006 | P0 | Define import trust model for foreign externs, opaque gates and untrusted serialized artifacts. |
| AURA-Q27-007 | P1 | Implement QIR Adaptive Profile support only after explicit version/profile agreement and independent fixtures. |
| AURA-Q27-008 | P1 | Add OpenQASM 3 dynamic classical-control subset and map unsupported constructs accurately. |
| AURA-Q27-009 | P1 | Implement QIR/OpenQASM independent parser/validator differential tests with third-party toolchains. |
| AURA-Q27-010 | P1 | Provide Python SDK bridge for Qiskit and tket interchange with supported version matrix. |
| AURA-Q27-011 | P1 | Publish conformance corpus with positive, invalid, version mismatch and fallback cases. |
| AURA-Q27-012 | P1 | Version Quantum IR converters to prevent implicit semantic drift between toolchain upgrades. |
| AURA-Q27-013 | P1 | Provide source-mapped diagnostics for imported artifacts and round-trip losses. |
| AURA-Q27-014 | P2 | Investigate pulse and calibration dialect interoperability separately from high-level gate semantics. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q28 — Simulation backends and numerical reference models

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q28-001 | P0 | Implement deterministic seeded statevector simulator for bounded supported qubit counts. |
| AURA-Q28-002 | P0 | Implement reference measurement sampling with correct Born-rule probabilities and seeded reproducibility. |
| AURA-Q28-003 | P0 | Test Bell, GHZ, teleportation and interference golden distributions with statistical uncertainty. |
| AURA-Q28-004 | P0 | Reject impossible qubit counts and allocation failures with explicit resource-budget diagnostics. |
| AURA-Q28-005 | P0 | Specify simulator numerical precision, normalization, complex arithmetic and tolerance policy. |
| AURA-Q28-006 | P1 | Implement stabilizer simulator for Clifford circuits with cross-check against statevector reference. |
| AURA-Q28-007 | P1 | Add density-matrix reference simulator for bounded noisy channels. |
| AURA-Q28-008 | P1 | Research tensor-network simulation with explicit approximation/truncation limits. |
| AURA-Q28-009 | P1 | Add GPU acceleration as optional profile after CPU oracle parity; no fake speedup claims. |
| AURA-Q28-010 | P1 | Implement exact small symbolic simulator for phase and measurement verification fixtures. |
| AURA-Q28-011 | P1 | Validate non-Clifford and mid-circuit reset/control simulation under supported subset. |
| AURA-Q28-012 | P1 | Emit seed, engine version, device profile, shots and numeric tolerance with every result. |
| AURA-Q28-013 | P1 | Benchmark time/memory scaling and disclose exponential worst-case complexity. |
| AURA-Q28-014 | P2 | Investigate distributed simulation behind exact reference and resource-usage acceptance gates. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q29 — Hybrid quantum-classical execution and job orchestration

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q29-001 | P0 | Define classical/quantum boundary ABI with typed shot counts, parameters, results and cancellation. |
| AURA-Q29-002 | P0 | Represent asynchronous queue, submit, accepted, running, completed, failed, expired and cancelled job states. |
| AURA-Q29-003 | P0 | Guarantee idempotent request identifiers and explicit at-most-once or at-least-once semantics by provider. |
| AURA-Q29-004 | P0 | Prevent automatic QPU spending or submission without explicit user authorization. |
| AURA-Q29-005 | P0 | Keep sampling nondeterminism separate from reproducible classical control semantics. |
| AURA-Q29-006 | P1 | Add parameter sweeps, variational optimization and batched circuit execution with quotas. |
| AURA-Q29-007 | P1 | Support mid-circuit classical feed-forward only for targets declaring the capability. |
| AURA-Q29-008 | P1 | Define reproducible local fallback when offline without falsely claiming hardware execution. |
| AURA-Q29-009 | P1 | Specify result stream ordering, retries, partial failures and cancellation cleanup. |
| AURA-Q29-010 | P1 | Add durable local job receipt and provenance ledger with redacted credentials. |
| AURA-Q29-011 | P1 | Support shot-wise confidence intervals and measurement readout processing as non-proof statistics. |
| AURA-Q29-012 | P1 | Expose resource budgeting: queued jobs, shots, spend cap, timeout, memory and concurrency. |
| AURA-Q29-013 | P1 | Write fault-injection integration tests for timeouts, duplicate messages and network partition. |
| AURA-Q29-014 | P2 | Investigate quantum/classical feedback co-scheduling based on device latency constraints. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q30 — QPU adapters, cloud providers and real-device execution

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q30-001 | P0 | Define provider-neutral backend capability and authentication adapter contract with explicit trust assumptions. |
| AURA-Q30-002 | P0 | Require opt-in authenticated QPU submission with provider/job identity and budget caps. |
| AURA-Q30-003 | P0 | Store secrets outside source, IR, proof report and logs; redact access tokens. |
| AURA-Q30-004 | P0 | Validate target capability freshness and submission compatibility before cost-incurring actions. |
| AURA-Q30-005 | P1 | Implement first provider simulator adapter using stable documented API and deterministic mock fixtures. |
| AURA-Q30-006 | P1 | Implement a first real-hardware adapter with provider-owned integration and sandboxed test credentials. |
| AURA-Q30-007 | P1 | Add second independent provider adapter to test vendor-neutral semantics. |
| AURA-Q30-008 | P1 | Handle billing metadata, queue delays, calibration changes, retryability and execution receipts. |
| AURA-Q30-009 | P1 | Provide provider-specific capability matrix and honest unsupported-feature mapping. |
| AURA-Q30-010 | P1 | Capture immutable submitted QIR/QASM payload hashes and reported device calibration IDs. |
| AURA-Q30-011 | P1 | Verify statistical hardware results against small reference circuits without presenting them as proofs. |
| AURA-Q30-012 | P1 | Implement privacy and data retention configuration per provider and jurisdiction. |
| AURA-Q30-013 | P2 | Validate reproducibility of provider adapters on real hardware with independently documented test windows. |
| AURA-Q30-014 | P2 | Investigate on-premise QPU control as a separately permissioned research profile. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q31 — Noise modeling, characterization and error mitigation

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q31-001 | P0 | Define modeled noise channels and calibration assumptions independently from circuit functional equivalence. |
| AURA-Q31-002 | P0 | Document shot noise, readout error, gate error and drift with separate metrics and uncertainty bounds. |
| AURA-Q31-003 | P0 | Never advertise error mitigation as guaranteed error correction or proof of result correctness. |
| AURA-Q31-004 | P1 | Implement depolarizing, amplitude damping, phase damping and readout assignment models. |
| AURA-Q31-005 | P1 | Add calibration ingestion with provenance, version and expiration handling. |
| AURA-Q31-006 | P1 | Implement measurement error mitigation with matrix conditioning and honest numerical diagnostics. |
| AURA-Q31-007 | P1 | Implement zero-noise extrapolation research profile with confidence intervals and failure modes. |
| AURA-Q31-008 | P1 | Implement randomized compiling and twirling experimental profiles with paired control fixtures. |
| AURA-Q31-009 | P1 | Provide sensitivity analysis across noise assumptions and device variations. |
| AURA-Q31-010 | P1 | Build synthetic-noise test datasets where ground truth is known. |
| AURA-Q31-011 | P1 | Publish fidelity, TVD, KL and estimator-bias metrics with precise interpretation and limitations. |
| AURA-Q31-012 | P1 | Compare mitigation versus unmitigated results under fixed shot budgets. |
| AURA-Q31-013 | P1 | Provide Sentinel model-vs-hardware divergence view with explicit uncertainty. |
| AURA-Q31-014 | P2 | Investigate pulse-level noise-aware characterization beyond gate-level assumptions. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q32 — Quantum error correction and fault-tolerant computation

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q32-001 | P0 | Separate physical-qubit experiments from encoded logical-qubit/fault-tolerant guarantees in API and claims. |
| AURA-Q32-002 | P0 | Specify stabilizer codes, syndrome measurement and decoder contracts with mathematically explicit assumptions. |
| AURA-Q32-003 | P0 | Prohibit claimed fault tolerance without quantified physical error model and logical failure evidence. |
| AURA-Q32-004 | P1 | Implement reference repetition, Steane and surface-code syndrome examples in simulators. |
| AURA-Q32-005 | P1 | Define logical gates, magic-state resources and ancilla factories as typed resources. |
| AURA-Q32-006 | P1 | Implement syndrome extraction and canonical syndrome record structures with temporal ordering. |
| AURA-Q32-007 | P1 | Integrate pluggable decoders with exact small-code oracles and cross-checks. |
| AURA-Q32-008 | P1 | Build logical error-rate estimates under calibrated synthetic-noise sweeps. |
| AURA-Q32-009 | P1 | Specify leakage, erasure and correlated noise assumptions, plus unsupported profiles. |
| AURA-Q32-010 | P1 | Add logical-resource estimates for qubit count, T factories, code distance and runtime. |
| AURA-Q32-011 | P1 | Provide QEC experiment manifests, reproducible seeds and confidence bounds. |
| AURA-Q32-012 | P1 | Develop independent threshold/scaling benchmark harness; no unsupported extrapolation. |
| AURA-Q32-013 | P2 | Investigate lattice surgery and code deformation with reviewed specialist RFCs. |
| AURA-Q32-014 | P2 | Plan hardware-backed QEC only with audited device capabilities and experiments. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q33 — Pulse control, calibration and time-critical systems

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q33-001 | P0 | Keep pulse-level control experimental and distinct from ordinary circuit equivalence claims. |
| AURA-Q33-002 | P0 | Define trusted timing, amplitude, frequency, frame, port and waveform units with bounds. |
| AURA-Q33-003 | P0 | Reject unsupported calibration statements, pulse hardware resources or unsafe schedule overlaps. |
| AURA-Q33-004 | P0 | Version calibration recipes and require explicit hardware/provider authorization for execution. |
| AURA-Q33-005 | P1 | Support OpenPulse-compatible restricted IR/import/export with independent parser conformance. |
| AURA-Q33-006 | P1 | Define frame-phase and waveform semantics for bounded supported targets. |
| AURA-Q33-007 | P1 | Build pulse timeline and collision detector with target-specific constraint fixtures. |
| AURA-Q33-008 | P1 | Provide simulation-only calibration experiments with dry-run reports and zero hardware commands. |
| AURA-Q33-009 | P1 | Implement target-specific timing resolution and phase accumulation regression corpus. |
| AURA-Q33-010 | P1 | Record calibration drift, validity window and authoritative snapshot identity. |
| AURA-Q33-011 | P1 | Expose pulse risk and estimated device load in Sentinel with non-proof labels. |
| AURA-Q33-012 | P2 | Support real hardware pulse submission only after dedicated provider sign-off and safety review. |
| AURA-Q33-013 | P2 | Investigate closed-loop calibration with explicit experiment budget and emergency stops. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q34 — Quantum standard library, algorithms and education

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q34-001 | P0 | Define versioned quantum stdlib boundaries for gates, operations, algorithms, measurements and numerical estimates. |
| AURA-Q34-002 | P0 | Ensure examples never bypass linear ownership, trusted-extern or proof profile requirements. |
| AURA-Q34-003 | P1 | Implement standard gate library with controlled/adjoint variants and numerical identity checks. |
| AURA-Q34-004 | P1 | Provide Bell/GHZ, teleportation, superdense coding and Deutsch-Jozsa reference examples. |
| AURA-Q34-005 | P1 | Implement QFT, phase estimation and amplitude amplification with exact small-case oracles. |
| AURA-Q34-006 | P1 | Add Grover and variational example profiles with bounded resource estimates. |
| AURA-Q34-007 | P1 | Provide Hamiltonian simulation and Pauli operator abstractions with declared approximation error. |
| AURA-Q34-008 | P1 | Implement classical optimization interop and differentiation for parameterized quantum circuits. |
| AURA-Q34-009 | P1 | Develop measurement aggregation, histogram and uncertainty-analysis APIs. |
| AURA-Q34-010 | P1 | Add resource estimation and compiler report APIs designed for future error-corrected targets. |
| AURA-Q34-011 | P1 | Create maintained executable cookbook with expected simulator/hardware-difference notes. |
| AURA-Q34-012 | P1 | Add structured migration examples from Qiskit/Q#/Guppy with explicit semantics limitations. |
| AURA-Q34-013 | P2 | Research chemistry, finance and optimization vertical packages without unsupported quantum advantage claims. |
| AURA-Q34-014 | P2 | Publish education modules covering no-cloning, measurement and common quantum safety myths. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q35 — Sentinel Quantum Studio, visualization and explanation

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q35-001 | P0 | Show exact quantum proof status, trust assumptions, target profile and stale-result invalidation. |
| AURA-Q35-002 | P0 | Distinguish mathematically established entanglement from conservative MAY_ENTANGLE analysis. |
| AURA-Q35-003 | P0 | Never label uncertain entanglement, collapse or hardware noise as statically proven. |
| AURA-Q35-004 | P1 | Implement source-linked circuit DAG, gate timeline, register lifetimes and measurement outcome views. |
| AURA-Q35-005 | P1 | Display resource ownership violations as move/borrow/measure/reset traces. |
| AURA-Q35-006 | P1 | Visualize logical-to-physical qubit mapping, SWAP insertion and target topology. |
| AURA-Q35-007 | P1 | Provide before/after optimizer diff with proof certificate or UNKNOWN label per pass. |
| AURA-Q35-008 | P1 | Display statevector/Bloch representation only for applicable bounded pure-state simulations. |
| AURA-Q35-009 | P1 | Display density-matrix and uncertainty views for explicitly modeled noisy states. |
| AURA-Q35-010 | P1 | Add entanglement witnesses for supported small-state or stabilizer profiles. |
| AURA-Q35-011 | P1 | Render simulator-vs-hardware statistical divergence, shots, seed and calibration provenance. |
| AURA-Q35-012 | P1 | Provide job queue cost/permission details before authenticated QPU submissions. |
| AURA-Q35-013 | P1 | Support accessible, keyboard-operable circuits, high contrast, RTL and screen-reader summaries. |
| AURA-Q35-014 | P1 | Make all panels source-mapped and block stale proof UI after edits or backend profile changes. |
| AURA-Q35-015 | P2 | Create visual quantum algorithm debugger with supported breakpoint and replay semantics. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q36 — Quantum developer experience, SDK and package ecosystem

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q36-001 | P0 | Expose a coherent quantum command family for check, verify, simulate, compile, estimate and submit with fail-closed statuses. |
| AURA-Q36-002 | P0 | Separate simulate, prove, measure and hardware-submit outputs and exit codes. |
| AURA-Q36-003 | P0 | Require explicit quantum profile in project manifest and lockfile; no implicit escalation to hardware. |
| AURA-Q36-004 | P1 | Publish quantum project scaffolds and multi-module templates with full clean-checkout smoke tests. |
| AURA-Q36-005 | P1 | Implement LSP diagnostics, autocomplete, hover, source navigation and qubit lifetime visual hints. |
| AURA-Q36-006 | P1 | Expose Quantum IR source maps and certificate inspection in CLI and Sentinel. |
| AURA-Q36-007 | P1 | Version quantum packages, gate sets, providers, backend profiles and manifests independently. |
| AURA-Q36-008 | P1 | Deliver CI action templates for no-hardware tests with reproducible seed and resource caps. |
| AURA-Q36-009 | P1 | Publish signed/notarized SDK tools as applicable with manifest/SBOM/provenance. |
| AURA-Q36-010 | P1 | Define Python/Jupyter integration and safe process isolation with typed payload serialization. |
| AURA-Q36-011 | P1 | Provide sample app and docs for local simulator then cloud provider transition. |
| AURA-Q36-012 | P1 | Create quantum diagnostics catalogue with stable error identifiers and remediation docs. |
| AURA-Q36-013 | P1 | Benchmark new developer time-to-first-safe-circuit with independent testers. |
| AURA-Q36-014 | P2 | Explore AI-assisted circuit suggestions only when post-checked and clearly labeled. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q37 — Quantum security, privacy, performance and assurance CI

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q37-001 | P0 | Threat-model code, gate parameters, providers, tokens, job results, calibration artifacts and shared proof certificates. |
| AURA-Q37-002 | P0 | Implement sandboxed third-party quantum artifact parsing with size, recursion and execution limits. |
| AURA-Q37-003 | P0 | Prohibit unapproved spend, credential leakage, unbounded resource allocation and silent job retries. |
| AURA-Q37-004 | P0 | Protect proof cache and provider receipts with content hashes, signing/provenance and anti-replay checks. |
| AURA-Q37-005 | P0 | Require concrete supported-profile negative controls for every quantum proof/safety claim. |
| AURA-Q37-006 | P1 | Establish independent CI for parser/checker, simulator, verifier, equivalence and hardware adapters. |
| AURA-Q37-007 | P1 | Implement deterministic cross-engine fuzz and metamorphic test generation with seed recording. |
| AURA-Q37-008 | P1 | Run numerical stability tests across supported platforms, compilers and architecture targets. |
| AURA-Q37-009 | P1 | Build hardware mock chaos tests for network faults, quotas, cancellation and stale calibration. |
| AURA-Q37-010 | P1 | Publish representative latency, depth, fidelity, simulator RAM and build-cost profiles with P50/P95/P99. |
| AURA-Q37-011 | P1 | Publish repeatable comparison methodology versus Qiskit, tket, Guppy, Q# and CUDA-Q. |
| AURA-Q37-012 | P1 | Define distributed trace privacy and redaction policy for Sentinel diagnostics. |
| AURA-Q37-013 | P1 | Maintain versioned exception list for unsupported features; no fabricated pass rates. |
| AURA-Q37-014 | P1 | Require independent adversarial review of quantum ownership and equivalence claim boundaries. |
| AURA-Q37-015 | P2 | Evaluate supply-chain reproducibility and dependency/license audit against quantum tool integrations. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

### Q38 — Independent qualification, research partnerships and release

| Stable task ID | Priority | Testable acceptance obligation |
|---|---|---|
| AURA-Q38-001 | P0 | Publish a quantum-specific GO/NO-GO profile gate distinct from Core, UI and platform release decisions. |
| AURA-Q38-002 | P0 | Require no unaccounted P0 soundness, ownership, provider-spend or secret-handling defects before public release. |
| AURA-Q38-003 | P0 | Require independently reproducible quantum equivalence fixtures before any formal verification claim. |
| AURA-Q38-004 | P0 | Keep experimental profiles clearly distinguished from qualified gate-level, dynamic and pulse execution. |
| AURA-Q38-005 | P1 | Require audited OpenQASM/QIR conformance with published interop corpus and versions. |
| AURA-Q38-006 | P1 | Require an independently maintained nontrivial Aura Quantum reference application. |
| AURA-Q38-007 | P1 | Qualify at least one provider-independent simulator and two distinct real-device/provider configurations before broad hardware claims. |
| AURA-Q38-008 | P1 | Publish public target/profile/device support matrix, limitations and uptime expectations. |
| AURA-Q38-009 | P1 | Qualify quantum SDK installation and reproducible examples on clean Windows/macOS/Linux CI. |
| AURA-Q38-010 | P1 | Publish independent comparative benchmarks with fixed inputs, published scripts and cost disclosure. |
| AURA-Q38-011 | P1 | Request external review by quantum compilation, programming-language and formal-methods specialists. |
| AURA-Q38-012 | P1 | Document incident response, security reporting, deprecation and migration for quantum ABI and SDK. |
| AURA-Q38-013 | P1 | Run community reproducibility event or open challenge on deliberately unsound transpilation fixtures. |
| AURA-Q38-014 | P1 | Require explicit maintainer-reviewed Quantum GA decision and signed release artifact provenance. |
| AURA-Q38-015 | P2 | Sustain transparent releases, external adoption feedback and research interoperability governance. |

**Exit:** all P0/P1 acceptance obligations evidenced on the declared Quantum profile; negative controls, exact source-to-artifact provenance and independent gate review. P2 research is explicitly scoped or tracked as deferred.

## C. R16–R35 — Quantum release trains (not dates or release promises)

| Train | Gate | Deliverable | Exit artifact |
|---|---|---|---|
| R16 — Quantum charter, scientific governance, assurance tiers | Q19 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q19 |
| R17 — Linear quantum ownership, resource and effect types | Q20 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q20 |
| R18 — Quantum syntax, typed circuits, hybrid language design | Q21 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q21 |
| R19 — Quantum SSA IR and transformation invariants | Q22 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q22 |
| R20 — Formal quantum semantics and verification foundations | Q23 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q23 |
| R21 — Equivalence engines and translation validation | Q24 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q24 |
| R22 — Quantum optimizer and synthesis pipeline | Q25 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q25 |
| R23 — Target models, routing, scheduling and hardware constraints | Q26 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q26 |
| R24 — OpenQASM, QIR and interoperable representations | Q27 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q27 |
| R25 — Simulation backends and numerical reference models | Q28 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q28 |
| R26 — Hybrid quantum-classical execution and job orchestration | Q29 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q29 |
| R27 — QPU adapters, cloud providers and real-device execution | Q30 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q30 |
| R28 — Noise modeling, characterization and error mitigation | Q31 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q31 |
| R29 — Quantum error correction and fault-tolerant computation | Q32 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q32 |
| R30 — Pulse control, calibration and time-critical systems | Q33 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q33 |
| R31 — Quantum standard library, algorithms and education | Q34 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q34 |
| R32 — Sentinel Quantum Studio, visualization and explanation | Q35 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q35 |
| R33 — Quantum developer experience, SDK and package ecosystem | Q36 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q36 |
| R34 — Quantum security, privacy, performance and assurance CI | Q37 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q37 |
| R35 — Independent qualification, research partnerships and release | Q38 | Scoped implementation and trust contract | Independently reproducible acceptance dossier for Q38 |

Quantum trains have dependencies; this ordered list is a **qualification registry**, not a claim of strict execution order. Independent subcomponents may be developed concurrently after contract freeze. Core and Q12–Q18 releases remain separately possible. No train completion is asserted.

## D. Workstream sequencing and non-negotiable dependencies

1. Freeze Q19–Q23 language/resource/IR/semantic profile before claiming proof-safe general compiler transformations.
2. Begin Q27 interop fixtures, Q28 reference simulator, Q35 Sentinel mock UI, Q36 SDK mock tooling and Q37 negative-test harness IN PARALLEL using versioned contract stubs; qualify integration later.
3. Use Q24 equivalence as an explicit translation-validation boundary for Q25 synthesis and Q26 target routing. Do not certify unsupported dynamic circuits using unitary-only checkers.
4. Hardware submission (Q29/Q30) requires permissioned authenticated execution, cost caps, capability provenance and no fake hardware receipts. Q31 noise, Q32 QEC and Q33 pulse need separate assumptions and specialist review.
5. Q38 gives independent GO/NO-GO separately for Quantum Core, Simulator, OpenQASM, QIR, Dynamic Circuit, Providers, QEC research and Pulse research. Commercial release is not implicit.

## E. References, charter and agent coordination

- [Quantum program index](../quantum/README.md)
- [Quantum charter and positioning](../quantum/PRODUCT_CHARTER.md)
- [Quantum semantics, proof and trust](../quantum/SEMANTIC_AND_TRUST_CONTRACT.md)
- [Architecture and interoperability](../quantum/ARCHITECTURE_AND_INTEROP.md)
- [Qualification test matrix](../quantum/QUALIFICATION_MATRIX.md)
- [Agent and branch ownership](../quantum/AGENT_WORKSTREAMS.md)
- [Standards and competing baselines](../quantum/SCIENTIFIC_REFERENCES.md)
- [Reference programs and acceptance demonstrations](../quantum/REFERENCE_PROGRAMS.md)

**Historical preservation test:** exactly 190 inherited V2 task rows, plus exactly 284 new Q19–Q38 task rows, 474 total unique task rows; full original V1/V2 content is embedded above without modification and remains separately versioned in the repository.
