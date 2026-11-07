# Product charter — Aura Quantum

**Strategic mandate:** enter quantum programming and become a serious contender for research, hybrid algorithms and eventually industrial quantum development. This is an ambition and long-horizon target, not an existing advantage or guaranteed leadership.

## North star and differentiation

One Aura program carries explicit classical/quantum resource types, scoped formal obligations, transformation evidence, hardware targets, experiment provenance and developer-facing explanation. Aura's differentiator is the transparent chain **typed source → Quantum IR → checked optimization → target artifact → simulator/QPU → evidence in Sentinel**. All guarantees remain named and bounded; claims require external comparative tests.

## Users and constraints

- Quantum algorithm developers need compositional circuits, measurement semantics, efficient simulation, standard interchange, hardware execution.
- Compiler researchers need a versioned IR, precise dynamic-circuit semantics, checker-aware passes, benchmark corpus and counterexamples.
- Hardware developers need target topology, calibration freshness, scheduling, constraints, pulse-specific profiles and authorization.
- Enterprise developers need trustworthy classical integration, cost controls, reproducible jobs, security, ownership and migration.
- Educators need simple correct explanations without confusing sampling with theorem proving.

## Product profiles (separate release trains)

1. **AQ-CIRCUIT:** typed gate circuits, linear qubits, end measurements, bounded simulator.
2. **AQ-PROOF:** checked resource flow, declared unitary equivalence, translation validation and proof reports.
3. **AQ-INTEROP:** OpenQASM 3 and QIR Base interoperability with independently measured conformance.
4. **AQ-DYNAMIC:** supported mid-circuit measures, reset, classical feed-forward and Adaptive QIR where profile permits.
5. **AQ-HARDWARE:** provider-authenticated submissions with capability, cost and calibration provenance.
6. **AQ-ERROR:** noise and mitigation experimental profiles; no fabricated logical guarantees.
7. **AQ-FTQC:** error-corrected logical-qubit research; independent qualification path.
8. **AQ-PULSE:** highly restricted timing/calibration research with explicit device authorization.

## Identity and non-goals

- Quantum is an **opt-in capability**, never a dependency for Aura Core or Lumina release.
- No general automatic proof of arbitrary quantum programs, no guaranteed quantum advantage, no physics-defying cloning prohibition claim.
- No assertion that Z3 alone establishes all quantum equivalence.
- No claim that entanglement can always be fully characterized statically.
- No cloud-provider lock-in and no execution spend without explicit consent.
- No AI-suggested code accepted without compilation, verification and trust accounting.

## Competitive yardsticks

Use Guppy linear ownership, Q#/QIR interop, Qiskit target compilation, tket routing/optimization, MQT QCEC equivalence and CUDA-Q hybrid execution as explicit baselines. Measure **one end-to-end workflow under equal task constraints** and release reproducible scripts/hardware, not ungrounded superlatives.

## Definition of leadership

Leadership is established only by independently reproduced results in proof integrity, interoperability, usability, real hardware support, source-to-device transparency and benchmark quality. It is not conferred by adding Q tasks to a roadmap.
