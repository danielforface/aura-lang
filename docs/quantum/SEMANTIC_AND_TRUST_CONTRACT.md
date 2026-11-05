# Quantum semantic, linearity and trust contract (future normative target)

**Status:** proposed, non-implemented scientific contract. All exact proof and compatibility obligations require separate independent review before marking qualified.

## Resource semantics

- A `Qubit` is an opaque linear **handle to a resource**, NOT a freely copyable in-memory quantum state. Copy/clone, aliases in multi-qubit gates, silent drop and escape are rejected in safe profiles.
- Borrow/use of a gate must not duplicate the resource. Values returned by measurement are classical and copyable; the quantum resource is retained or consumed according to operation signature.
- Destructive measurement consumes ownership; nondestructive measurement may preserve handle while changing state; reset requires defined post-state. **Repeated measurement is not universally forbidden.**
- Composite types containing a linear field inherit linearity. Explicit ancilla release is checked by profile-specific preconditions. Region exit cannot leak live qubits.
- No-cloning theorem constrains arbitrary unknown states; ownership prevents representational duplication, not every conceivable physical cloning exploit nor physics validity of opaque hardware.

## Mathematical domains

- Pure circuits: finite-dimensional complex Hilbert spaces and completely specified unitaries.
- Equivalence can be **up to global phase** for closed, identical, pure-state unitary interfaces when appropriate; e.g. `U = exp(i θ) V`. This is not generally sufficient for exposed phase control, instruments, noise, measured distributions, ancilla effects or dynamic control.
- Measurements: probabilistic instruments, measurement record ordering and post-measurement states; dynamic classical feed-forward must be reflected in both execution and proof model.
- Noisy circuits: density matrices / CPTP maps and explicitly stated noise channels. Approximate equivalence requires a named metric and certified bound, not just sample overlap.
- Symbolic parameters: disclose exact algebraic subset, assumptions, domain restrictions, numerical tolerance, unsupported nonlinear cases.

## Proof authority

| Status | Exact meaning |
|---|---|
| `PROVEN_EXACT` | Independent checker established the specified equality for the declared fully supported semantic model. |
| `PROVEN_UP_TO_GLOBAL_PHASE` | Established unitary equality modulo global phase under an explicitly allowed profile. |
| `APPROXIMATE_BOUND` | A separately checkable, explicit metric bound with all numerical/physical assumptions. |
| `FALSIFIED_BY_WITNESS` | Concrete counterexample supported by reference semantics for the declared task. |
| `SIMULATED` | Observed in a simulator with seed, engine, shots and precision; not formal proof. |
| `OBSERVED_ON_HARDWARE` | Observed device results under job receipt, shots and calibration ID; not formal proof. |
| `UNKNOWN / TIMEOUT / UNSUPPORTED / NOT_RUN / CANCELLED / ERROR` | Not proven; no green proof indicator or successful verify status. |

## Trusted computing base

Quantum syntax/typing, compiler normalization, Quantum IR validation, external equivalence engine, interpreter, numeric libraries, backend bridge, hardware API, calibration, noise model, device firmware and observation are separate trust surfaces. Every certificate records which were assumed and which checked. Changes invalidate applicable proof caches. A Z3 success on target topology or integer resources is NOT a quantum-semantic theorem.

## Concrete negative controls

1. `cx(q,q)` and duplicate borrowed aliases must fail type checking.
2. Destructive measure followed by use fails; nondestructive measure followed by legal operations succeeds with updated semantics.
3. A malicious Rz angle mutation must refute a supported equivalence check.
4. A hardware swap that changes output-wire assignment without recorded permutation must fail.
5. A measurement moved across a non-commuting gate without proof must fail.
6. Simulator seed agreement without universal proof must never produce `PROVEN_EXACT`.
7. Unsupported dynamic circuit under QIR Base must fail, not erase effects.
8. A provider timeout, stale calibration or insufficient budget must not produce successful QPU receipt.

## Release boundary

Proof correctness claims require positive and adversarial tests on exact artifacts plus external review. Q01–Q11 and Q18 trust protections always apply. Qualified releases can be scoped to one explicitly supported profile and hardware target.
