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
