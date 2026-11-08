# AURA X-RAY — Engineering Blueprint v0.1
**Status:** PRODUCT BLUEPRINT / not a release or executable certification  
**Product lane:** independent product/xray-windows-v0, based on main@e8f1c35a4a56adbde983dce38bba59a8c4eee105  
**Owner:** X-RAY product lane; language Core A, Lumina B, Quantum C are READ-ONLY dependencies.  
**Date:** 2026-11-07  
**User delivery:** every completed functional release MUST produce a real Windows x64 executable from GitHub Actions, artifact archive, SHA-256, exact-source SHA, CI evidence, tested functions, unsupported functions, and human test checklist. User runs the executable on their Windows computer.

## 0. החלטת מוצר

**AURA X-RAY is a local-first, read-only system observability and incident-analysis instrument** built with Aura as the business-logic language. It records real observations, correlates events, compares baselines, exposes missing coverage, and eventually evaluates possible interventions inside explicit trust boundaries. This is a *tool for existing Windows and later Linux software*, not an Aura compiler demo, synthetic dashboard, or an auto-remediation agent.

The first release is a **Windows x64 executable** and a useful process-observation/incident baseline recorder. It is permitted to be console-first; a native graphical shell is a later separate qualification gate. It must function without Aura already installed on the user's computer.

**Production doctrine:** no fake live telemetry, no AI-produced causality presented as an observation, no automated invasive changes, no driver installation, no privileged service, no admin requirement for the baseline release, no silent network uploads, no fabricated proof/verification status. Never ship a C-only tool with ornamental Aura.

## 1. Users and jobs to be done

1. Windows developer / systems engineer: capture an actual process/resource baseline and compare it with a later sample, identifying conspicuous per-process CPU/memory/I/O changes with precise timestamps and limits.
2. Incident responder: inspect *what was observed*, sampling gaps, access-denied processes, process restarts, and confidence limitations without hiding missing data.
3. Performance engineer: export repeatable, machine-readable local captures and ingest them later in a timeline and a native viewer.
4. Future Linux/SRE user: ingest eBPF and OpenTelemetry with source provenance and per-source clock/error models.
5. Aura language team: exercise real application APIs, catch reproducible source→IR→C→native mismatches, generate narrowly scoped consumer tests for A/B without modifying their code.

**First differentiation thesis:** join accurate low-overhead observations with gap-aware evidence and eventually verified preconditions for intervention. Competition with ETW/WPA, PerfView, ProcMon, OpenTelemetry, Pixie etc. requires independently measured user value, not claims of category leadership.

## 2. Product outcomes / hard stop conditions

- A user can download, extract and run a genuine executable on Windows 10/11 x64. Windows Server support requires a dedicated matrix.
- The executable reports real OS-derived data with explicit field origins and error states; output includes run id, schema version, UTC time, sampling period, OS/tool version, exact build SHA.
- Initial run produces a useful capture without network, admin, driver or registry modifications.
- The Aura-produced C code is REQUIRED input to the EXE link, and at least one runtime analytical operation in the EXE calls a function generated from Aura source. CI checks linkage and effect by testing nontrivial Aura classifications.
- Failure to compile Aura source, to link, or to run Windows smoke tests => CI failure, NO release artifact labeled valid.
- A new release may add capabilities but cannot remove previous guaranteed behavior without a documented compatibility decision.
- No success claims for compiler proof, complete UI, or universal process visibility absent evidence.

## 3. Truth snapshot / code dependency matrix

| Aura component | Present in repo | X-RAY use now | Future integration / important limit |
|---|---|---|---|
| aura-lex, aura-parse, aura-ast | main code | Compile .aura programs | Syntax grows only behind qualified releases |
| aura-core | main code; Core A Draft #57 evolves composites | Type check and lower basic functions | Pin exact compiler SHA; no reliance on unmerged records, Phi or unsupported ABI |
| aura-ir | main code | Lower scalar analytical functions | Extend consumers after A contracts / differential proof |
| aura-backend-c | main default portable emitter | **Primary executable path: Aura → module.c → Win32 EXE** | Composite/FFI contract is not assumed ready; explicit external host on v0 |
| aura-backend-llvm | evolving and feature-gated | OUT of first release | Candidate after qualification; not interchangeable proof |
| aura-interpret (AVM) | present | Optional development/differential oracle | AVM performance or UI support does not establish native parity |
| aura-verify + Z3 | feature-gated, present | Contract checking on bounded Aura pure functions if actually supported | Must fail closed; never claim backend itself proven |
| aura-rt, aura-rt-native, aura-stdlib, sdk | present | C emitter/runtime dependencies; link only what is needed | No hidden mock tensors, OS placeholders or unbounded alloc |
| aura-bridge, bindgen | bootstrap bridge present | Document FFI; *handwritten minimal C adapter with stable C ABI* initially | Move to generated bindings only on verified support |
| aura-pkg | present | Repo-pinned dependencies / lockfile, not remote runtime | No registry dependency for first offline EXE |
| aura-lsp, Sentinel IDE | present | Authoring/diagnostics for Aura source | Future evidence inspector and developer loop; not first customer GUI |
| aura-nexus | present | Future plugin boundary | Versioned adapters, explicit capabilities and permissions |
| aura-plugin-lumina | present, AVM/Raylib UI evolving; B Draft #106 | Not assumed native EXE GUI capable | Future X-RAY UI after native-event, text, accessibility, headless and package qualification |
| aura-plugin-ai, aura-plugin-iot | experimental optional | OUT of v0 | Later optional inference only with observation/proposal separation |
| Aura Quantum / Agent C Draft #108 | standalone simulator on unmerged branch | OUT of first product and dependency graph | Optional future quantum experiment tracing only if justified by users, no artificial coupling |
| release tooling, .github/actions | main code | Deterministic metadata and CI artifact strategy | Independent X-RAY workflow; integrator-owned global tooling untouched |
| Android and Web surface | partial/evolving | OUT of first release | Future remote view only after user demand and separate security review |

Status sources: README.md, ARCHITECTURE.md, PROJECT_STATUS.md, TOOLCHAIN.md, sdk/docs/reference.md, docs/lumina-ui.md (main); plan/aura-quantum-full-stack-v3/AGENTS.md, delivery policy, cross-agent protocol, V3; active A #57, B #106, C #108. 'Present' does not imply qualified or stable.

## 4. Architectural decomposition

    Win32 process/time/memory/IO observation adapter [C, thin, read-only]
       → versioned raw observation records (quality flags; source API; timestamps)
       → Aura analysis cells (classification, deltas, thresholds, invariants)
       → bounded event store / JSON Lines capture [host IO boundary, explicit]
       → baseline comparison / incident candidate engine [Aura, expanding]
       → CLI output now / Lumina timeline later
       → evidence manifest and external user-run feedback

Other future adapters feed the same normalized *observation contract*: Windows ETW optional/privilege-aware; Linux eBPF + tracepoints; OpenTelemetry; offline import; eventually process/network service maps. Do not try to reuse Linux eBPF itself as a Windows telemetry source.

**Important architecture note:** in the current Aura CLI the C backend build emits module.c, while the convenient C run path only links emitted C plus stdlib and does not fully implement arbitrary extra C bridge/link inputs. X-RAY therefore **explicitly runs Aura build to emit module.c and links module.c with the audited Win32 host shim via clang on Windows CI**. This is a legitimate use of Aura's current language/compiler; the thin C host is disclosed as trusted platform integration. An app using Aura only for a decorative constant fails the exit gate.

## 5. Windows adapter: capability contract

Phase 1 can use Toolhelp32 snapshots (CreateToolhelp32Snapshot/Process32FirstW/Process32NextW), OpenProcess with limited query rights, GetProcessTimes, GetProcessMemoryInfo, GetProcessIoCounters, GetSystemTimes and GetSystemTimePreciseAsFileTime where supported. Data is observed rather than inferred. Windows ETW is a *separate phase*: kernel/session privileges, lost-event counters and privacy budgets must be declared. ETW real-time controller permissions are not universal, so never request elevation silently.

Process identity is (PID, process-create-time) when obtainable, not PID alone. Access denied is a first-class observation status. CPU percent uses differences over a declared sample interval and processor-normalization policy; first sample is a baseline, not instant CPU utilization. I/O counters are cumulative and require delta. No false zero for unreadable processes. Sample clocks must include UTC and monotonic reference where available.

## 6. Data contract (forward-compatible)

Schema id: aura.xray.capture/1. Every record: record_type, schema_version, producer SHA and version, OS name/build, capture id, sample id, UTC timestamp, monotonic ticks, source adapter and API, observed fields, field-level status, elapsed sample ns, and loss/permission/parse warnings. Emit JSONL UTF-8 with controlled escaping, no multiline or silently truncated records; cap record length and process count; bounded on-disk retention.

Records: CAPTURE_START, SAMPLE_START, PROCESS_OBSERVATION, SAMPLE_END, GAP, CAPTURE_END, ERROR. Future records: ETW_EVENT, OTEL_SPAN, CAUSAL_HYPOTHESIS, PROOF_RESULT, INTERVENTION_PLAN, INTERVENTION_EXECUTION. Every claim must carry evidence class OBSERVED / COMPUTED / INFERRED / PROVED_BOUNDED / UNKNOWN and source references. PROVED_BOUNDED only when true validator evidence is attached. Prevent stale claims after source/model change.

For the very first slice a smaller *explicitly versioned subset* of the above schema is acceptable; announce missing fields/privilege restrictions, do not disguise subset as full contract.

## 7. Core Aura algorithm responsibilities

- Classify raw metrics using bounded typed scalar inputs; pure and deterministic.
- Calculate deltas only when sample identities/timestamps are valid, no counter reset.
- Compare baseline vs new sample and identify candidate regressions with evidence pointers.
- Future: conservative dependency graph, span correlation, change-point detection, exact source-level traces, uncertainty budgets, replay matching and counterfactual tests.
- Define independent C/Windows caller test vectors for every public Aura function; detect literal stubs or ignored results.
- Never silently substitute C implementation for unimplemented Aura business logic. Temporary collection/serialization in C is an FFI boundary and must migrate as Aura capabilities stabilize, with explicit ownership.

## 8. Experience: CLI-first then Lumina

**V0:** AuraXRay.exe --help, --self-test, --capture --seconds N --out FILE, --version. Console output describes what was captured and excluded; no GUI claim. User only runs executable after review, CI may run it automatically on Windows runner for smoke.

**V1 native viewer:** Lumina app shell and timeline when the Windows native path can render text, Unicode, focus, keyboard, scrolling and graph hit tests reliably. UI consumes stable capture schema through typed adapter, doesn't own capture logic. Accessibility, RTL, large captures and failure states are release gates.

**V2 high-value features:** compare capture sessions; live backlog/latency breakdown; explain diagnostic with evidence and unknowns; session export; signed hash manifest.

**V3 cautious intervention:** read-only analysis -> proposed action -> precondition checks -> isolated rehearsal -> approval UI -> typed audit record; no write to production in default mode. No claim that an SMT proof alone establishes safe kernel/OS effects.

## 9. Product slice / release train

| Gate | Shippable outcome | Required Aura role | Windows test |
|---|---|---|---|
| XR-00 BLUEPRINT | This document, source/ownership scope, CI artifact contract | Code-backed mapping | No EXE claimed |
| XR-01 BOOTSTRAP | Windows x64 AuraXRay.exe captures real process observations and a deterministic Aura classification; --self-test | Aura→C output physically linked | CLI and capture JSON parsed on windows runner |
| XR-02 EVIDENCE | Repeat samples, process identity, per-field status and loss counters, bounded capture files | Aura delta/validation logic | Known process lifecycle, negative permissions, counters |
| XR-03 COMPARE | Two captures with named regression candidates and explanation references | Aura comparison and thresholds | Controlled CPU/IO stress fixtures with outcome bounds |
| XR-04 VIEWER | Installed native GUI explores captures; no fake animation | Lumina API after native qualification | Windows UI automation/screenshot/golden + manual user |
| XR-05 TRACE | ETW and/or provider event correlation for supported Windows versions | Aura typed correlation / checked inputs | Windows ETW fixture + real trace with permissions |
| XR-06 SYSTEM MAP | Linked processes/services/requests when instrumented | Aura graph inference with uncertainty | Pairwise observation and missing-edge tests |
| XR-07 REPLAY | Import/replay a recorded incident with explicit clock and coverage | Aura deterministic engine and versioned event store | Byte-stable replay golden, not universal time travel |
| XR-08 INTERVENTION | Auditable dry-run remediation preconditions, no automatic changes | Aura Verify + trust manifest | Failing/unknown proofs block execution |
| XR-09 LINUX | eBPF + OTel independent adapter under same schema | Aura portable analysis engine | Native Linux tests and kernel/version support matrix |
| XR-10 ADVANCED | Optional AI summaries and specialty profiling driven by real demand | Post-checked suggestions, no fabricated certainty | Independent benchmark and harm/quality review |

No arbitrary completion date or performance claim is inferred. Promote a gate only when its observable result, failing negative controls, and actual exact-SHA CI exist. A green unit test alone isn't a product release.

## 10. Release CI design and executable contract

Use a dedicated workflow in .github/workflows/xray-windows.yml. Triggers should be narrowly scoped to product/xray-windows-v0 push and manual dispatch, with explicit restricted permissions and concurrency. Runner windows-2022 x64 initially; toolchain Rust stable, checked Cargo.lock, Visual Studio/Clang (version printed), no downloads from untrusted sources. Pin GitHub Actions versions to reviewed releases in follow-up hardening; permission contents: read, no broad token.

Strict pipeline:
1. Checkout exact SHA and print SHA + rustc/cargo/clang versions.
2. Build Aura CLI from repository source with locked dependencies and no optional experimental integrations (cargo build -p aura --locked --no-default-features).
3. Compile product Aura code with the repository Aura executable (--backend c); capture generated module.c and runtime header. Treat any omitted/unsupported instruction as failure.
4. Compile and link the generated module.c + Win32 read-only adapter + Aura stdlib C using Clang/MSVC Windows SDK to one PE32+ x86-64 AuraXRay.exe. Link errors are hard failures, no fallback to host-only tool.
5. Run executable on the Windows runner: --self-test (positive+negative Aura vectors), --version, --capture with bounded duration and path; parse capture output with PowerShell/Python and validate schema and nonzero observations. Validate export and exit codes. No admin on runner.
6. Check architecture (PE x64), imported libraries, artifact contents, SHA-256. Use file hash over binary, source commit, source tree and workflow; create BUILD-MANIFEST.json with explicit feature scope, unverified claims and tests.
7. Upload ZIP using actions/upload-artifact@v4 with fixed retention (30 days initially). Build result is NOT complete if EXE is missing, hash missing, tests fail or artifact upload fails.
8. Optional release sign/notarize only with dedicated secrets and audited publishing protocol; unsigned early builds are documented.
9. User receives artifact URL/ZIP or direct EXE file when retrievable, SHA256, run ID, exact SHA, behavior, Windows compatibility, security/permission note, and feedback checklist. No raw binary is ever claimed to be produced until observed.

**Important operational constraint:** GitHub artifact download may be an authenticated archive. The assistant must actually retrieve/attach the artifact where access permits; otherwise it must state precisely that the EXE has been built but supply the verified Actions artifact link, not invent a local download.

## 11. Reproducibility / pinning

- X-RAY is an independent product lane from main. Never merge, rebase, close or rewrite A #57, B #106, C #108 or integration #95/Issue #109.
- Every release records Aura compiler exact SHA, backend, stdlib ABI, OS, runner, Clang version, target triple, product schema, artifact SHA256 and build commands.
- Releases XR-01 onward require acceptance against a pinned main compiler SHA; advanced capability previews may have separately named pre-release profiles pointing to specific approved source/PR commits, never silently inherit open PR claims.
- User feedback is a real external test result, not automatically an AI/CI success. It becomes regression acceptance with reproduction steps.

## 12. Security, privacy, operational budgets

- Default local-only, no sockets/telemetry. No persistence without explicit --out; never upload process names, paths or memory.
- Read-only Win32 handles, least privilege and safe handle cleanup; no process memory reads or arbitrary injected code.
- Explicit consent before attaching to elevated processes, enabling ETW sessions, or capturing process names/paths. Sensitive fields redacted by default.
- Bound duration, interval, event count, bytes and memory; backpressure produces counted gaps instead of silent loss.
- Crash-safe output staging and atomic finalization for completed captures; partial captures visibly marked partial.
- Untrusted trace import: validate schema/length/UTF-8/CRC/signature where applicable and sandbox parsing.
- Exception hygiene: report unknown/timeout/denied distinctly; no auto-retries that hide missing data.
- Build integrity: no third-party binary downloaded without provenance; no unreviewed runtime network dependencies.

## 13. Measurable nonfunctional targets (TARGET ONLY)

Not current measurements:
- First CLI help/version in <1s on representative Windows device after warm cache.
- Baseline idle CPU <1% on reference hardware at default interval; memory <100MB; full sample gap accounting.
- Default capture 30s, 1Hz with configurable 1–10Hz profile after measured performance; no elevation.
- CI full product smoke on supported runner; no fixed arbitrary cross-machine performance pass/fail without dedicated benchmark isolation.
- Full process coverage is NOT guaranteed; OS access restrictions are reported.
- Later causal false-positive / false-negative rates measured with controlled incidents, not inferred from synthetic demos.

## 14. Verification test pyramid

**Layer A**: Pure Aura scoring/classification vectors exercised through generated C ABI; negative arithmetic and boundary tests; minimum functional non-triviality.
**Layer B**: Windows adapter unit: snapshot count >0, invalid process denied rather than zero, real timestamp, handle leak checks.
**Layer C**: End-to-end Windows EXE: real capture JSONL parse, schema, errors, no network, bounded duration and exit status.
**Layer D**: Differential / verifier for Aura subset only where features pass actual Z3 gate; fail closed and retain trust boundary.
**Layer E**: Manual user-run: clean extraction, SmartScreen unsigned warning documented, --version, 10s capture, attach sanitized result and screenshot if GUI later.
**Layer F**: Dedicated bench with scripts and competing baselines when relevant; no unsupported leadership statements.

## 15. Manual Windows acceptance handoff

Deliver at each completed executable gate:
- Name + version, exact GitHub commit, branch, PR and CI run.
- SHA256 of downloadable exe (and archive), x64 target and required Windows version.
- One command to run it, expected file locations, feature demo using real machine data.
- Expected permission behavior; no administrator privileges or network in XR-01.
- Known limitations / privacy and security disclosures.
- Ask for user's OBSERVED result only after artifact exists; don't claim it was run on user's PC.
- If user reports error, prioritize that reproduction and correction before unrelated expansion.

## 16. Cross-agent integration policy

This lane owns only apps/xray/** and the dedicated isolated workflow .github/workflows/xray-windows.yml (workflow path is shared operational D5: ask integrator before merge). No edits to root Cargo.toml/Cargo.lock, compiler, Lumina, Quantum, central ledgers or frozen plans. Named dependency: compiler main@exact SHA. To adopt A/B capabilities from Draft branches, request producer/consumer interface contract, exact SHA, positive and negative integration tests, integrator-approved order. No unilateral merges.

X-RAY can surface legitimate missing compiler features as reproducible minimized programs in GitHub Issues; Agent A owns compiler fixes. B owns native UI capabilities. C quantum remains isolated; add quantum integration only if a real user workflow demands it. The product can ship an independent read-only subset without waiting for all of V3.

## 17. Anti-toy / anti-fake acceptance

Reject a release if any:
- Windows EXE actually written only in C/Rust while Aura is not linked or has no semantic effect.
- Synthetic data labeled LIVE.
- CI passes without running an executable on Windows.
- Missing EXE, corrupt PE, stale SHA or untraceable compiler revision.
- GUI drawn as screenshot rather than runtime actual observations.
- Causality proof claimed from temporal correlation alone.
- Permission-denied field silently converted into zero.
- Incomplete solver proof labeled green, or incident hypothesis labeled verified.
- Production mutation enabled by default.
- Regression in earlier released workflow without explicit compatibility path.

## 18. Readiness decisions, risks and mitigation

**R1 Critical — C emitter limited type and FFI shape.** Build a narrow pure-Aura scalar kernel and a tested ABI; host C adapter reads Win32 and invokes generated functions. Any backend mismatch is a blocked release, not a workaround that deletes Aura.
**R2 High — current Lumina native cross-platform gaps.** Console EXE ships useful data first; GUI requires B qualified native input/rendering/capture interop; don't promise GUI in XR-01.
**R3 High — user machine differs from GitHub runner.** Windows 10/11 x64 matrix and manual user execution are mandatory to broaden claims; no unsupported AV or signing promise.
**R4 High — ETW permissions/loss.** Isolate ETW phase with opt-in and explicit lost counters; Win32 snapshots first.
**R5 High — causal inference.** Keep evidence classes separate and mark unknown; introduce controlled regression corpus and independent ground truth.
**R6 High — cross-agent changes.** Isolated source path, pinned main, independent workflow and no changes to shared compilation toolchain absent contract.
**R7 Medium — output privacy.** Process names and paths can be sensitive; redact or allow opt-in, never upload.
**R8 Medium — artifact retrieval.** CI upload is not the same as direct client attachment; confirm file bytes locally before offering a sandbox link.

## 19. Immediate execution queue

1. Publish this blueprint in the isolated product branch; record exact base SHA. This is XR-00, not a binary.
2. Implement XR-01 real native process snapshot and pure Aura metrics classifier; test source compile, generated C and ABI locally where toolchain available.
3. Add dedicated push Windows workflow using pinned toolchain and exact acceptance, artifact upload. Run via branch push, not via modifying main.
4. Inspect exact job and evidence; repair genuine compile/runtime defects in X-RAY lane. If core-language defect, report to Core A with repro; do not change A's owned source.
5. Retrieve real ZIP/EXE from successful CI and deliver hash, executable and run instructions. Only then label XR-01 executable ready for user.
6. Iterate in coherent, tested product releases; each release ends with EXE delivery and explicit user feedback.

## 20. External reference standards

- Microsoft ETW model and access restrictions: https://learn.microsoft.com/en-us/windows/win32/etw/about-event-tracing and https://learn.microsoft.com/en-us/windows/win32/etw/controlling-event-tracing-sessions
- Microsoft Tool Help snapshot API: https://learn.microsoft.com/en-us/windows/win32/toolhelp/tool-help-functions
- GitHub artifact protocol: https://docs.github.com/en/actions/tutorials/store-and-share-data
- Repo internal: README.md, TOOLCHAIN.md, ARCHITECTURE.md, sdk/docs/reference.md, docs/lumina-ui.md, docs/qualification/MASTER_ROADMAP_V3.md (planning branch), docs/qualification/DELIVERY_OPERATING_POLICY_V1.md, docs/qualification/CROSS_AGENT_COORDINATION_V1.md.

**Blueprint authority rule:** A functional product result requires code+real Windows runner+downloadable binary; this document alone does not satisfy XR-01.


## 21. XR-02 delivery contract / process-history extension — 2026-11-07

**BEFORE:** XR-01/01.1 produced real Windows process snapshots and simple Aura memory/IO buckets, but not inter-sample rates, process activity ranking or a useful local viewer. Real user Windows acceptance of XR-01.1 returned 10 samples, 2,140 process observation records, 2,130 memory reads, 0 access-denied, 0 reported errors, elapsed_ms=10,016 and open_failed=10. Windows build and limited-language ABI were proven; other language features are NOT silently assumed.

**AFTER target:** XR-02 should emit /2 capture records with identity-guarded process history and CPU millicores, read/write KiB/s, memory deltas, nullable activity score; and an interactive offline HTML explorer created in the EXE's own capture process. Scores are heuristic and explicitly do not prove root cause.

**Ownership:** D0 product files apps/xray; D1 read-only consumption of compiled Aura uint32 ABI (six functions compiled from analysis.aura, fail closed at Windows link); D5 dedicated scoped workflow. Platform OS calls, 64-bit counter validation and bounded history remain in portable C/Win32 adapter. No changes to Core A #57, Lumina B #106, Quantum C #108, frozen roadmap or Cargo root. No merge until maintainer review.

**Truth and error semantics:** V2 schema distinct from V1; process identity must include Windows creation time; first sample = baseline; only successive valid monotonic windows may produce deltas; counter resets, missing field, PID reuse, invalid clock or overflow produce null rather than falsely positive metrics. CPU 1000 millicores = one fully used logical CPU. IO API counters can include non-disk I/O. Activity score is transparent and unit-checked but not a malware/leak classifier. Local report contains no network links or external assets. Process names are opt-in.

**Performance and privacy bounds:** no more than 4096 process histories and 128 report leaderboard entries, maximum 30-second collection; report rendered as HTML/CSS without a browser/network server and with HTML escaping. Tests must cover all unknown/negative cases and avoid embedding private user captures in the repository.

**XR-02 qualification:** exact-HEAD Windows CI compiles real Aura product source through current C backend, generates six uint32 C functions, links with Windows collector and portable engine; requires semantic vectors and deterministic PID-reuse/reset/unknown fixtures, actual live 3-second capture with derived vectors and independent rate/score oracle, generated HTML with redacted names and no external dependencies, negative no-overwrite and invalid-duration tests, x64 PE hash/manifest, and uploaded artifact. No success claim absent CI plus artifact. External tester runs on their hardware afterwards. Next practical slice after that: cross-session comparison and replay/import with independent parser proofing, not a fake causal diagnosis.


## 22. XR-03 — Evidence Workbench / Replay / Compare (2026-11-07)

**Source-of-truth user hardware audit:** Valid user XR-02 capture received with 2,217 JSONL records (2,195 process observations, 10 sample-start, 10 sample-end, 1 capture-start/end), 1,966 scored temporal vectors, 10.016s elapsed, 10 open failures all Windows 87 (PID0), no access-denied or collector errors. 219 baseline processes, 10 identity-unavailable observations, 0 reset/reuse/capacity. The source JSONL is **private user data and MUST NEVER be pushed**. Independent score/memory-delta audit 0 discrepancies; report had 128 leaderboard rows from a 220-PID capture.

**Product-level discovered problem:** XR-02 summary table combines independent peak CPU/IO/memory values that were observed at different times. Its peak-activity timeline clamps very small and even zero values to visible bars. It has no true saved-capture replay, no cross-capture comparison, no per-event inspector and no persisted evidence navigation. These are **presentation and investigative-semantics defects**, not bugs in the Aura score formula. Public claim must not imply causal diagnosis based on isolated score peaks. At least two memory-growth spikes during user capture subsequently contracted, showing why memory leak claims would be unjustified.

**XR-03 vertical slice:** Add a strict bounded local importer of schema/2, replay mode, aggregate cross-run compare (no cross-session PID matching assumption), interactive multi-axis timeline, per-event same-sample evidence tables, process-identity matrix, focused event detail and quality/provenance desk. Generated HTML must be local-only, no fetched assets, default redaction; validate all input, reject tampered scores, preserve null/zero distinction. Avoid native Lumina GUI claim until Agent B's Windows UI contracts are independently qualified.

**Real Aura ownership:** add three impact cells in analysis.aura (CPU, I/O, positive working-set growth). Compilation must generate nine real uint32 signatures, and offline importer must execute all new cells on actual scored vectors, not silently reimplement them. Win32 adapter handles file/OS crossing; portable C parser/rendering owns untrusted JSON parsing, bounded storage, schema verification, HTML+JS serialization and compare. This is honest co-designed architecture (Aura domain + C adapters), not cosmetic Aura branding.

**Acceptance gates:** Windows build from exact branch SHA -> generated Aura C ABI -> clang native link -> 24 scalar self-test vectors + identity/reset fixture -> actual 3-second process capture and report -> strict replay -> compare separately named fixtures -> deliberately tampered activity score rejects -> malformed JSON rejects with no output -> privacy/HTML structure/no network -> PE x64/sha256/manifest -> downloadable zipped EXE. No production claim until artifact is physically verified. User then runs replay against prior personal XR-02 file on their Windows PC, tests new 10-second capture+report, and confirms independent UI interactions.

**Ownership and integration:** D0 apps/xray and D5 isolated workflow only; D1 Aura ABI consumer pinned to product head. Draft PR #113 remains Draft until reviewed; NO automatic merge, no touching Core A, Lumina B, Quantum C, root Cargo, central qualification ledger. Document future limitation: this is not ETW-based full host tracing, live Lumina GUI, malware detection, formal causal inference, or automatic remediation.

## 23. XR-04 — Raw source counters and cross-version evidence trust (2026-11-07)

**External source audit:** User-supplied local Windows XR-03 JSONL contained 2,123 process observations, 1,901 scored vectors, ten 1s samples and 10,015 ms elapsed. The matching Live and Replay and Compare HTML files were rendered in headless Chromium; observed correct source provenance, chart switching, focused real process event, identity matrix, active dataset selection, 0 script exceptions and 0 external requests. User files remain private, were NOT copied to GitHub. All score formulas and sequential memory-delta checks passed.

**Defect:** previous capture source exposed rounded CPU and I/O counters, so the replay importer could not independently reconstruct exact rates, despite rechecking scores. Score-consistent but source-inaccurate rate changes could be accepted. This is a measurable evidence-integrity limitation, not a hypothetical production crash.

**XR-04 contract:** additive (no schema/2 break) raw 64-bit CPU 100ns and read/write byte counters, parent PID and instantaneous thread count. Version 0.4 importer validates each scored vector against matched PID+creation identity, previous raw state, source monotonic clock and published integer arithmetic; refuses altered raw/derived mismatches. Older /2 captures replay but are visibly tagged legacy score-only. New evidence view exposes parent PID as hint only, never proven ancestry; retained baseline creation identity.

**Ownership:** D0 apps/xray, D5 dedicated workflow only, D1 read-only consumer of nine Aura domain cells. No compiler, Lumina, Quantum or central governance edits. Draft PR #113 remains unmerged. No source SHA qualifies until Windows CI native capture+replay+compare, raw rate tamper rejection, browser report smoke, PE x64 and exact binary manifest pass. User hardware acceptance separate. No cryptographic authenticity, ETW or root-cause claim.

## 24. XR-05 — Aura-powered casebook, process temporal evidence (2026-11-07)

**Actual external XR-04 acceptance:** private uploaded source JSONL + live/legacy/compare HTML inspected locally; 2,175 lines / 2,153 processes / 1,928 scored vectors / ten samples / 10,000ms / 0 independent exact-source counter discrepancies / 0 lost source IDs in live HTML. 1,928 score recalculations and raw rate derivations match source; 2,153 topology snapshots, names redacted by default. Prior XR-03 legacy replay correctly displays 0 exact-source verifications, without pretending rounded counters are exact. Incomplete PID0 always marked unknown; 10 open failures, no reported collector errors. Current release SHA 121a710a.

**New user-observable delivery:** XR-05 Casebook offers source-linked process behavioral episodes, not just isolated peak score. Thresholds (working-set pulse >=32768 KiB, >=50% consecutive retraction, >=700 millicores for >=3 contiguous samples, >=16384 KiB/s read/write transfers) are compiled from 5 new Aura cells; process matching, chronology and bounded evidence snapshots are handled by standalone native C adapter, and HTML is read-only offline. Negative fixtures exclude disconnected samples, different process creation ID and near-threshold values. Source verification confidence preserved across old and new captures.

**Grounded three cases on actual user's capture:** PID 7140 S2 +91932 KiB working set, S3 -106348 KiB same identity; PID 7140 S5–10 sustained 984–1190 millicores; PID 13408 S7 17243 KiB/s write. Interpretation bounded: not malware, not a proven leak or resolved leak, not causation, not physical disk throughput, not proof of actual lineage. Private files are not committed to repo. Data schema remains /2; product version 0.5.0-xr05.

**Acceptance targets:** compile 14 genuine Aura cells and fail on missing generated functions, link Win32 x64 app and portable casebook engine, native self-test with 4 expected synthetic cases + negative boundary fixtures, capture actual Windows processes, reproduce score/monotonic/raw checks, replay/compare, reject falsified rate, verify standalone HTML casebook JS syntax, artifact PE x64/manifest/SHA. UI must show literal cases with original sample trail, preserved privacy and correct legacy fidelity. User-machine XR05 smoke is pending until user runs EXE. No merge outside product Draft PR #113.


## 25. XR-06 — Aura compiled temporal state, recurring evidence and machine-readable API

**Delivery goal**: replace XR-05's C-centered episode decisions with actual
Aura-compiled recurrence, bounded retraction state, episode phase transitions,
coincident dimensions and evidence reliability grades; export casebook/1 from
the same strict, privacy-safe Capture/2 reader for other analysis programs.

**BEFORE**: 14 Aura business cells chiefly normalize/threshold, while C owns
memory net-retraction and CPU streak decisions. HTML casebook is the only
external episode API. Missing future samples are not distinguished visibly
from a truly exhausted recovery window.

**AFTER candidate**: 20 Aura business cells, including six real stateful fold
and provenance functions called per process sample (not one-off decorations).
C performs transport, sorting, resource checks, bounds and stream grouping;
Aura controls recurrences, memory state, phase closure, simultaneous pressure
dimension count, and classification fidelity. Clear four-state memory outcome:
OPEN, RETRACTED, WINDOW_EXPIRED, INTERRUPTED. Fourth case type measures
coincident CPU/memory/I/O *observations*, not cause. New
\`--explain CAPTURE.jsonl --out CASES.json\` emits validated private-free JSON
with source SHA and evidence grade; files must be new, complete input required.

**Independent local preflight**: read user-provided actual 177 KiB XR-05
workbench and recompute its 2,153 event rows via a separate Python reference:
three expected XR-06 model cases (7140 S2–S3 memory, 7140 S5–S10 CPU,
13408 S7 transfer), no compound false positives. Real Aura native function
and compile tests executed via exact-SHA Windows CI, not imputed from local
reference. New negative fixtures: sample gaps, PID reuse, near-threshold
signals, partial/unknown record, output overwrite and rate-falsification.
Windows CI outcome must be attached to the exact final source revision.
No automatic Merge, no shared compiler or other-agent path mutation.

**Guarantee boundary**: not whole-program Aura; no third-party dependencies
in the offline UI; no native Lumina UI, ETW, malware/root-cause label,
systemwide physical RAM or device I/O inference. Exact provenance is from
source observations, not from an invented runtime proof.
