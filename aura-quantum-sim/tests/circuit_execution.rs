//! End-to-end public Rust API tests for Q28-011/012 dynamic circuit execution.
//! These are ideal statevector simulations, never hardware measurements.
use aura_quantum_sim::circuit::{
    Circuit, CircuitError, Instruction, TraceKind, CIRCUIT_PROFILE, CIRCUIT_SCHEMA_VERSION,
};
use aura_quantum_sim::{Complex, Gate, SimError, DEFAULT_MEMORY_BUDGET, ENGINE_VERSION, NORM_TOLERANCE};

const EPS: f64 = 2e-11;

fn close(value: f64, expected: f64) {
    assert!(
        (value - expected).abs() <= EPS,
        "got {value:.17e}, expected {expected:.17e}"
    );
}

fn bell_feed_forward() -> Circuit {
    let mut circuit = Circuit::new(2, 2);
    circuit.push(Instruction::Gate(Gate::H(0)));
    circuit.push(Instruction::Gate(Gate::Cnot { control: 0, target: 1 }));
    circuit.push(Instruction::Measure {
        qubit: 0,
        into_bit: 0,
    });
    circuit.push(Instruction::If {
        bit: 0,
        equals: true,
        gate: Gate::X(1),
    });
    circuit.push(Instruction::Reset { qubit: 0 });
    circuit
}

#[test]
fn bell_measurement_conditional_correction_and_reset_returns_clean_state() {
    let circuit = bell_feed_forward();
    let mut observed = [false; 2];
    for seed in 0..128_u64 {
        let output = circuit.execute_once(seed, DEFAULT_MEMORY_BUDGET).unwrap();
        let measured = output.shot.bits[0].unwrap();
        observed[measured as usize] = true;
        assert_eq!(output.shot.bits[1], None);
        close(output.final_state.basis_probability(0).unwrap(), 1.0);
        for basis in 1..4 {
            close(output.final_state.basis_probability(basis).unwrap(), 0.0);
        }
        assert_eq!(output.shot.trace.len(), 2);
        assert_eq!(output.shot.trace[0].instruction, 2);
        assert_eq!(output.shot.trace[0].qubit, 0);
        assert_eq!(
            output.shot.trace[0].kind,
            TraceKind::Measure { into_bit: 0, value: measured }
        );
        assert_eq!(output.shot.trace[1].instruction, 4);
        assert_eq!(output.shot.trace[1].qubit, 0);
        assert_eq!(
            output.shot.trace[1].kind,
            TraceKind::Reset { previous_value: measured }
        );
        assert_eq!(output.metadata.seed, seed);
        assert_eq!(output.metadata.shots, 1);
        assert_eq!(output.metadata.schema_version, CIRCUIT_SCHEMA_VERSION);
        assert_eq!(output.metadata.engine_version, ENGINE_VERSION);
        assert_eq!(output.metadata.profile, CIRCUIT_PROFILE);
        assert_eq!(output.metadata.norm_tolerance, NORM_TOLERANCE);
    }
    assert!(observed.into_iter().all(|x| x));
}

#[test]
fn batch_shots_are_fresh_seeded_deterministic_and_have_born_marginals() {
    let circuit = bell_feed_forward();
    let a = circuit
        .execute_shots(4096, 0x52c1_0044, DEFAULT_MEMORY_BUDGET)
        .unwrap();
    let b = circuit
        .execute_shots(4096, 0x52c1_0044, DEFAULT_MEMORY_BUDGET)
        .unwrap();
    assert_eq!(a, b, "identical seed must replay full classical trace");
    assert_eq!(a.metadata.shots, 4096);
    let true_count = a
        .shots
        .iter()
        .filter(|shot| shot.bits[0] == Some(true))
        .count();
    // Bernoulli(1/2), sigma at n=4096 is 32 counts. ±192 is 6 sigma.
    assert!(
        (1856..=2240).contains(&true_count),
        "Bell Z samples violate a declared 6-sigma envelope: {true_count}/4096"
    );
    for shot in &a.shots {
        assert_eq!(shot.trace.len(), 2);
        assert_eq!(shot.bits[1], None);
    }
    let c = circuit
        .execute_shots(4096, 0x52c1_0045, DEFAULT_MEMORY_BUDGET)
        .unwrap();
    assert_ne!(a.shots, c.shots);
}

#[test]
fn reset_is_an_ideal_z_measurement_followed_by_correction() {
    let mut circuit = Circuit::new(1, 2);
    circuit.push(Instruction::Gate(Gate::X(0)));
    circuit.push(Instruction::Measure { qubit: 0, into_bit: 0 });
    circuit.push(Instruction::Reset { qubit: 0 });
    circuit.push(Instruction::Measure { qubit: 0, into_bit: 1 });
    let outcome = circuit.execute_once(0, DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(outcome.shot.bits, vec![Some(true), Some(false)]);
    assert_eq!(outcome.shot.trace.len(), 3);
    assert_eq!(
        outcome.shot.trace[1].kind,
        TraceKind::Reset { previous_value: true }
    );
    close(outcome.final_state.basis_probability(0).unwrap(), 1.0);
    close(outcome.final_state.basis_probability(1).unwrap(), 0.0);
}

#[test]
fn false_classical_branch_executes_only_after_its_measurement() {
    let mut c = Circuit::new(1, 1);
    c.push(Instruction::Measure { qubit: 0, into_bit: 0 });
    c.push(Instruction::If {
        bit: 0,
        equals: false,
        gate: Gate::X(0),
    });
    let result = c.execute_once(42, DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(result.shot.bits, vec![Some(false)]);
    close(result.final_state.basis_probability(1).unwrap(), 1.0);
}

#[test]
fn genuine_nonclifford_teleportation_works_through_public_dynamic_api() {
    let mut c = Circuit::new(3, 2);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::T(0)));
    c.push(Instruction::Gate(Gate::H(1)));
    c.push(Instruction::Gate(Gate::Cnot { control: 1, target: 2 }));
    c.push(Instruction::Gate(Gate::Cnot { control: 0, target: 1 }));
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Measure { qubit: 0, into_bit: 0 });
    c.push(Instruction::Measure { qubit: 1, into_bit: 1 });
    c.push(Instruction::If {
        bit: 1,
        equals: true,
        gate: Gate::X(2),
    });
    c.push(Instruction::If {
        bit: 0,
        equals: true,
        gate: Gate::Z(2),
    });
    c.push(Instruction::Reset { qubit: 0 });
    c.push(Instruction::Reset { qubit: 1 });

    let alpha = Complex::new(std::f64::consts::FRAC_1_SQRT_2, 0.0);
    let beta = Complex::new(0.5, 0.5);
    let mut seen = [false; 4];

    for seed in 0..128 {
        let output = c.execute_once(seed, DEFAULT_MEMORY_BUDGET).unwrap();
        let m0 = output.shot.bits[0].unwrap();
        let m1 = output.shot.bits[1].unwrap();
        seen[(m0 as usize) | ((m1 as usize) << 1)] = true;
        let z0 = output.final_state.amplitude(0).unwrap();
        let z1 = output.final_state.amplitude(4).unwrap();
        let overlap = Complex::new(alpha.re, -alpha.im) * z0
            + Complex::new(beta.re, -beta.im) * z1;
        close(overlap.norm_squared(), 1.0);
        close(output.final_state.norm_squared().unwrap(), 1.0);
        for idx in [1, 2, 3, 5, 6, 7] {
            close(output.final_state.basis_probability(idx).unwrap(), 0.0);
        }
        assert_eq!(output.shot.trace.len(), 4);
        assert!(matches!(output.shot.trace[2].kind, TraceKind::Reset { .. }));
        assert!(matches!(output.shot.trace[3].kind, TraceKind::Reset { .. }));
    }
    assert!(seen.into_iter().all(|x| x), "all four Bell branches must be covered");
}

#[test]
fn uninitialized_classical_control_fails_preflight() {
    let mut c = Circuit::new(1, 1);
    c.push(Instruction::If { bit: 0, equals: false, gate: Gate::X(0) });
    c.push(Instruction::Measure { qubit: 0, into_bit: 0 });
    assert_eq!(
        c.execute_once(0, DEFAULT_MEMORY_BUDGET).unwrap_err(),
        CircuitError::ReadBeforeWrite { bit: 0, instruction: 0 }
    );
}

#[test]
fn invalid_unused_conditional_gate_fails_even_when_condition_is_false() {
    let mut c = Circuit::new(2, 1);
    c.push(Instruction::Measure { qubit: 0, into_bit: 0 });
    c.push(Instruction::If {
        bit: 0,
        equals: true,
        gate: Gate::Cnot { control: 0, target: 0 },
    });
    assert_eq!(
        c.execute_once(0, DEFAULT_MEMORY_BUDGET).unwrap_err(),
        CircuitError::Simulator(SimError::DuplicateQubit(0))
    );
}

#[test]
fn invalid_measurement_target_and_reset_qubit_fail_closed() {
    let mut bad_bit = Circuit::new(1, 1);
    bad_bit.push(Instruction::Measure { qubit: 0, into_bit: 1 });
    assert_eq!(
        bad_bit.execute_once(1, DEFAULT_MEMORY_BUDGET).unwrap_err(),
        CircuitError::InvalidClassicalBit { bit: 1, bits: 1 }
    );
    let mut bad_reset = Circuit::new(2, 0);
    bad_reset.push(Instruction::Reset { qubit: 2 });
    assert_eq!(
        bad_reset.execute_once(1, DEFAULT_MEMORY_BUDGET).unwrap_err(),
        CircuitError::Simulator(SimError::InvalidQubit { index: 2, qubits: 2 })
    );
}

#[test]
fn exact_budget_and_integer_overflow_are_checked_before_executing() {
    let circuit = bell_feed_forward();
    assert!(matches!(
        circuit.execute_shots(100, 1, 100).unwrap_err(),
        CircuitError::BudgetExceeded { .. }
    ));
    assert_eq!(
        circuit.execute_shots(0, 1, DEFAULT_MEMORY_BUDGET).unwrap_err(),
        CircuitError::ZeroShots
    );
    let empty = Circuit::new(0, 0);
    assert_eq!(
        empty.execute_once(0, DEFAULT_MEMORY_BUDGET).unwrap_err(),
        CircuitError::EmptyQuantumRegister
    );
    let huge = Circuit::new(1, usize::MAX);
    assert_eq!(
        huge.execute_once(0, usize::MAX).unwrap_err(),
        CircuitError::ArithmeticOverflow
    );
    let mut invalid_gate = Circuit::new(2, 0);
    invalid_gate.push(Instruction::Gate(Gate::Swap { a: 1, b: 1 }));
    assert_eq!(
        invalid_gate.execute_once(0, DEFAULT_MEMORY_BUDGET).unwrap_err(),
        CircuitError::Simulator(SimError::DuplicateQubit(1))
    );
}
