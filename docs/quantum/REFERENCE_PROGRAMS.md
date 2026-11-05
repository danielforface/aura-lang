# Reference quantum programs and test oracles

All examples below are **future acceptance cases**, not executable Aura programs at the time of planning. The final syntax is subject to Q21 approval. Use independent mathematical oracles, versioned output schemas and exact limits.

| Fixture | Expected scientific property | Fault injection | Gate |
|---|---|---|---|
| H then H | Identity up to allowed global phase | Remove H or replace by X | Q23/Q24 |
| Bell preparation | Distribution 00/11 with ideal probability 1/2 each, and entangled pre-measurement state | Swap CNOT operand order / omit H | Q28/Q35 |
| GHZ-3 | Pre-measurement 3-qubit entanglement; sample 000/111 ideally | Eliminate one entangling gate | Q28/Q35 |
| Teleportation | Final reduced output matches input with classical correction, not no-cloning | Remove feed-forward correction | Q21/Q23/Q34 |
| Controlled adjoint | Compiled inverse restores supported unitary | Insert nonunitary reset in adjoint block | Q21/Q24 |
| No cloning | `cx(q,q)` compile-time rejected; unknown quantum handle cannot be copied | Alias through record/closure | Q20 |
| Measurement | Destructive use-after-measure rejected; non-destructive measurement may preserve handle | Reuse consumed handle | Q20/Q23 |
| Compilation swap | Logical output permutation preserved through routing | Drop logical-wire mapping metadata | Q24/Q26 |
| OpenQASM round trip | Parse/emit supported subset with same semantics | Unsupported dynamic control rejected | Q27 |
| QIR Base export | End-of-circuit measurement accepted | Mid-circuit feed-forward rejected on Base | Q27 |
| Noise calibration | Density-matrix channel match within explicit tolerance | Bad Kraus map / stale calibration | Q28/Q31 |
| QEC syndrome | Small code decoder matches independent reference | Wrong syndrome bit flip | Q32 |
| Provider fault | No cost without authorization; retry is bounded and idempotent | Timeout/duplicate request/expired calibration | Q29/Q30/Q37 |
| Sentinel stale proof | Edit or target change removes previously green proof | Deliberate old report replay | Q35/Q37 |

## Reproducibility requirements

Every fixture defines source hash, quantum IR hash, gate order, target and simulator version, expected result/profile, random seed, shots, numeric precision, tester name, pass/fail/unknown states and exact semantic assumptions. Use positive controls and adversarial negative tests in CI. Dynamic hardware tests are opt-in, cost capped and NEVER prerequisites for ordinary local builds.
