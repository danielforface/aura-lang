# Quantum qualification matrix — required evidence and failures

Every test captures **source commit SHA, profile/version, toolchain, supported hardware/backend, environment, random seeds, target snapshot, proof checker, artifact hashes, expected result, actual result and trust exceptions**. A passing CI build is not itself semantics proof.

| Qualification lane | Mandatory positive control | Mandatory negative control | Evidence |
|---|---|---|---|
| Linear resource | Allocate, gate, return/measure supported qubits | Clone, double-use `cx(q,q)`, leaked or consumed handle | Typed diagnostics, source spans, tests |
| Quantum semantics | H/H identity, Bell circuit, unitary composition | Invalid phase interpretation or swapped non-commuting gates | Exact reference tensors/certificates |
| QIR Base | End-measure circuit with valid QIR flags | Unqualified mid-measure/control path rejected | LLVM/QIR conformance fixture |
| OpenQASM | Supported import→export round trip | Invalid/unsupported gate/control aborts explicitly | Third-party validator/round trip |
| Optimizer | Legal gate cancellation preserved | Non-commuting reorder, measurement move must fail | Per-pass proof or explicit UNKNOWN |
| Target | Routing on available coupler | Missing directed edge, stale calibration | Target snapshot and source mapping |
| Simulator | Small exact distribution and seeded sampling | Wrong Born probabilities, invalid normalization | Seed, shots, tolerance, oracle |
| Equivalence | H·H=I, identity global phase | Rz changed angle, output map corruption | Checker method and certificate |
| Hybrid jobs | Cost-approved idempotent submit | Budget overrun, timeout, duplicate retry | Mock/provider trace and consent |
| Noise | Known synthetic channel statistics | Noise-bound violation or overconfident CI | Confidence interval, model provenance |
| QEC | Small repetition/Steane syndrome oracle | Wrong syndrome or unjustified threshold | Decoder oracle and failure bounds |
| Pulse | Target-approved safe simulated schedule | Port collision, invalid duration, stale calibration | Dry-run, provider restriction |
| Sentinel | Correct stale-proof invalidation | After edit, stale proof still green | E2E source/target change |
| Security | Redacted token and signed job receipt | Token printed, unauthorized spend, malicious artifact | Negative security CI |

## Scientific benchmark and review policy

- At minimum: mutually independent reference implementation, control fixtures, methods, version and platform disclosure.
- Do not conflate circuit equality, distribution similarity, sample statistics, approximate numerical bounds or hardware reliability.
- Report median, P95 and tail latency for real supported workloads; RAM, gate count, depth, two-qubit count, T-count/T-depth and cost-per-job where applicable.
- Compare with explicit versions of Qiskit, tket, MQT QCEC, Guppy, Q#, CUDA-Q and OpenQASM/QIR interoperable readers. Publish scripts and representative circuits.
- Cloud hardware is flaky and paid; run offline mocks for normal CI and permit controlled scheduled hardware E2E only with authorization and budget.
- External scientific review is required for any broad proof-soundness or fault-tolerance claim.

## Gates and states

All new tasks are `NOT_STARTED`. An agent can mark `IN_PROGRESS` only with an owned issue/branch and source-specific acceptance definition. `VERIFIED` requires finished external review of the scoped criterion, not just code, tests or a draft PR. Q38 gives profile-specific GO/NO-GO. Release without an independent trust review is forbidden.
