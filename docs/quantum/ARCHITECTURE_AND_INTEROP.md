# Aura Quantum architecture and standards interoperability

**Research design, not current code.** Do not create a monolithic quantum-only language fork. Preserve existing Aura classical type system, verifier, C backend, AVM and Sentinel public API until new profile-specific contracts are approved.

## Proposed pipeline

```text
Aura source + quantum feature edition
  → Aura lexer/parser/sema (typed resource/effect checking)
  → Quantum AST / quantum-classical SSA & scoped Quantum IR
  → structural/effect/linearity validator
  → optional exact/bounded proof & equivalence engines
  → transformations: normalization → synthesis → routing → scheduling
  → independent translation validation on supported pass families
  → explicit target profile + calibrated device snapshot
  → QIR Base/Adaptive or OpenQASM 3.x (version-pinned) or native simulator
  → authenticated provider adapter / local simulator
  → typed results, hardware receipts, uncertainty and evidence ledger
  → Sentinel Quantum Studio with stale-evidence fencing
```

## Separate package boundaries (proposed, NOT repositories or crates currently created)

| Package | Owner | Contract |
|---|---|---|
| `aura-quantum-types` | Quantum Type | Qubit linearity, effect signatures, API typing |
| `aura-quantum-ir` | Quantum IR | Versioned validated mixed quantum/classical operations |
| `aura-quantum-semantics` | Semantics | Small exact unitary/channel semantics and witnesses |
| `aura-quantum-verify` | Proof | Specialized checkers, counterexamples and certificate schema |
| `aura-quantum-opt` | Optimizer | Guarded pass management, target-aware rewrites |
| `aura-quantum-sim` | Simulation | Seeded reference engines and statistical outputs |
| `aura-quantum-target` | Hardware | Connectivity, native gates, timings, calibration and cost |
| `aura-quantum-interop` | Standards | OpenQASM/QIR import-export and conformance |
| `aura-quantum-runtime` | Runtime | Typed jobs, cancel, quota, provider bridges |
| `aura-quantum-qec` | QEC | Explicit logical-qubit and decoder research |
| `editors/sentinel-app/quantum` | Sentinel | Visualization, proof provenance and job authorization |

## Integration contracts

- **Quantum IR** schema version, semantic model version, resource-effect graph hash and source spans are frozen for each qualified test.
- **Interop:** OpenQASM 3.1 explicit supported subset; QIR Base only end-measurement/limited classical per spec; dynamic feed-forward requires separately qualified adaptive capability. Backends must report unsupported instructions accurately.
- **Target:** signed/identified snapshot with native instruction set, connectivity, timing, measurement/reset/dynamic capability, calibration TTL, provider version and economic costs.
- **Execution:** no hardware submission from build, check, optimize or simulate. Only explicit authenticated `quantum submit` or equivalent interactive authorization may create billable jobs.
- **Results:** separate `proof`, `simulation`, `hardware`, `estimation`, `mitigation`, with distinct schema version and evidence class.
- **Sentinel:** any edit to source, target, solver, compilation pass or calibration assumptions invalidates dependent green proof visualizations.

## Scheduling dependencies

Q19→Q20/Q21/Q22/Q23→Q24→Q25/Q26. Independent fixtures for Q27/Q28/Q35/Q36/Q37 can start immediately with mocks and semantic stubs. Provider work Q29/Q30 is separately permissioned; Q31/Q32/Q33 require domain-specific calibration and peer review. No full-platform release follows automatically from one green path.

## No unsound shortcuts

- Do not reinterpret a normal `u64` as a safe qubit without an opaque typed handle.
- Do not reuse an LLVM backend compiler-completeness claim for compliant QIR.
- Do not claim an AVM interpreter is a validated quantum simulator without separate fixtures.
- Do not call Z3-only proof of scheduling an equivalence proof.
- Do not claim real QPU access from offline provider mocks.
