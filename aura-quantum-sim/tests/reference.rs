use aura_quantum_sim::{
    Complex, Gate, SeededRng, SimError, Statevector, DEFAULT_MEMORY_BUDGET,
};

const EPS: f64 = 1e-10;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < EPS,
        "expected {expected:.16e}, got {actual:.16e}"
    );
}

fn bell() -> Statevector {
    let mut state = Statevector::zero(2, DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply(Gate::H(0)).unwrap();
    state.apply(Gate::Cnot {
        control: 0,
        target: 1,
    }).unwrap();
    state
}

#[test]
fn double_hadamard_is_identity_on_both_basis_states() {
    for starting_one in [false, true] {
        let mut state = Statevector::zero(1, DEFAULT_MEMORY_BUDGET).unwrap();
        if starting_one {
            state.apply(Gate::X(0)).unwrap();
        }
        state.apply(Gate::H(0)).unwrap();
        state.apply(Gate::H(0)).unwrap();
        close(state.norm_squared().unwrap(), 1.0);
        close(state.basis_probability(0).unwrap(), if starting_one { 0.0 } else { 1.0 });
        close(state.basis_probability(1).unwrap(), if starting_one { 1.0 } else { 0.0 });
    }
}

#[test]
fn bell_golden_amplitudes_and_analytic_probabilities() {
    let state = bell();
    close(state.norm_squared().unwrap(), 1.0);
    let a = std::f64::consts::FRAC_1_SQRT_2;
    close(state.amplitude(0).unwrap().re, a);
    close(state.amplitude(3).unwrap().re, a);
    for i in [1, 2] {
        close(state.amplitude(i).unwrap().norm_squared(), 0.0);
    }
    close(state.basis_probability(0).unwrap(), 0.5);
    close(state.basis_probability(3).unwrap(), 0.5);
    close(state.basis_probability(1).unwrap(), 0.0);
    close(state.basis_probability(2).unwrap(), 0.0);
}

#[test]
fn ghz3_golden_state_and_correlations() {
    let mut state = Statevector::zero(3, DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply(Gate::H(0)).unwrap();
    state.apply(Gate::Cnot { control: 0, target: 1 }).unwrap();
    state.apply(Gate::Cnot { control: 1, target: 2 }).unwrap();
    let a = std::f64::consts::FRAC_1_SQRT_2;
    close(state.amplitude(0).unwrap().re, a);
    close(state.amplitude(7).unwrap().re, a);
    for i in 1..7 {
        close(state.basis_probability(i).unwrap(), 0.0);
    }
    close(state.norm_squared().unwrap(), 1.0);
}

#[test]
fn shots_sample_freshly_without_collapsing_source() {
    let state = bell();
    let shots = state.sample_basis(4096, 0x51_42_33_24).unwrap();
    assert_eq!(shots, state.sample_basis(4096, 0x51_42_33_24).unwrap());
    assert!(shots.iter().all(|&i| i == 0 || i == 3));
    let ones = shots.iter().filter(|&&i| i == 3).count();
    // 4096 Bernoulli(0.5) shots, sigma(count/4096)=0.0078125.
    // 0.05 is 6.4 sigma, not an assertion of precisely 50% shots.
    assert!((ones as f64 / 4096.0 - 0.5).abs() < 0.05);
    close(state.basis_probability(0).unwrap(), 0.5);
    close(state.basis_probability(3).unwrap(), 0.5);
}

#[test]
fn single_measurement_collapses_and_repeat_is_consistent() {
    let mut state = bell();
    let mut rng = SeededRng::new(1234);
    let measured = state.measure(0, &mut rng).unwrap();
    let expected_index = if measured { 3 } else { 0 };
    close(state.basis_probability(expected_index).unwrap(), 1.0);
    close(state.norm_squared().unwrap(), 1.0);
    assert_eq!(state.measure(0, &mut rng).unwrap(), measured);
    close(state.basis_probability(expected_index).unwrap(), 1.0);
}

#[test]
fn gate_oracle_checks_phase_and_basis_permutation() {
    let mut state = Statevector::zero(2, DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply(Gate::X(0)).unwrap(); // |01>, q0 least significant bit
    state.apply(Gate::Swap { a: 0, b: 1 }).unwrap(); // |10>
    close(state.basis_probability(2).unwrap(), 1.0);
    state.apply(Gate::Y(1)).unwrap();
    close(state.amplitude(0).unwrap().im, -1.0); // Y|1> = -i|0>
    state.apply(Gate::Z(0)).unwrap();
    state.apply(Gate::S(0)).unwrap();
    state.apply(Gate::T(0)).unwrap();
    close(state.norm_squared().unwrap(), 1.0);

    let mut phase = Statevector::zero(1, DEFAULT_MEMORY_BUDGET).unwrap();
    phase.apply(Gate::X(0)).unwrap();
    phase.apply(Gate::S(0)).unwrap();
    close(phase.amplitude(1).unwrap().im, 1.0);
    phase.apply(Gate::T(0)).unwrap();
    close(phase.amplitude(1).unwrap().re, -std::f64::consts::FRAC_1_SQRT_2);
    close(phase.amplitude(1).unwrap().im, std::f64::consts::FRAC_1_SQRT_2);
}

#[test]
fn bad_qubit_alias_index_and_unsupported_gate_fail_closed() {
    let mut state = Statevector::zero(2, DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(state.apply(Gate::H(2)), Err(SimError::InvalidQubit { index: 2, qubits: 2 }));
    assert_eq!(state.apply(Gate::Cnot { control: 1, target: 1 }), Err(SimError::DuplicateQubit(1)));
    assert_eq!(state.apply(Gate::Cz { control: 3, target: 0 }), Err(SimError::InvalidQubit { index: 3, qubits: 2 }));
    assert_eq!(state.apply(Gate::Swap { a: 0, b: 0 }), Err(SimError::DuplicateQubit(0)));
    assert_eq!(Gate::from_name("RESET"), Err(SimError::UnsupportedGate("RESET".into())));
    close(state.basis_probability(0).unwrap(), 1.0);
}

#[test]
fn invalid_states_and_impossible_postselection_fail_closed() {
    assert_eq!(
        Statevector::from_amplitudes(1, vec![Complex::new(0.2, 0.0), Complex::new(0.2, 0.0)], DEFAULT_MEMORY_BUDGET)
            .unwrap_err(),
        SimError::NotNormalized,
    );
    assert_eq!(
        Statevector::from_amplitudes(1, vec![Complex::new(f64::NAN, 0.0), Complex::ZERO], DEFAULT_MEMORY_BUDGET)
            .unwrap_err(),
        SimError::NonFiniteAmplitude,
    );
    let mut zero = Statevector::zero(1, DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(zero.postselect(0, true), Err(SimError::ImpossibleOutcome));
    close(zero.basis_probability(0).unwrap(), 1.0);
}

#[test]
fn resource_limits_and_dimension_errors_fail_closed() {
    assert_eq!(Statevector::zero(2, 16).unwrap_err(), SimError::BudgetExceeded);
    assert_eq!(Statevector::zero(usize::BITS as usize, usize::MAX).unwrap_err(), SimError::DimensionOverflow);
    assert_eq!(
        Statevector::from_amplitudes(2, vec![Complex::ONE; 2], DEFAULT_MEMORY_BUDGET).unwrap_err(),
        SimError::DimensionMismatch,
    );
    let state = Statevector::zero(1, 32).unwrap();
    assert_eq!(state.sample_basis(100, 1).unwrap_err(), SimError::BudgetExceeded);
    assert_eq!(state.amplitude(2), Err(SimError::InvalidBasisIndex(2)));
}

#[test]
fn seeded_rng_replay_and_seed_zero_are_defined() {
    let mut a = SeededRng::new(0);
    let mut b = SeededRng::new(0);
    for _ in 0..100 {
        assert_eq!(a.next_u64(), b.next_u64());
        let draw = a.next_f64();
        assert!((0.0..1.0).contains(&draw));
    }
}

#[test]
fn cz_phase_and_control_order_have_distinct_expected_effects() {
    let mut state = Statevector::zero(2, DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply(Gate::X(0)).unwrap();
    state.apply(Gate::X(1)).unwrap();
    state.apply(Gate::Cz { control: 0, target: 1 }).unwrap();
    close(state.amplitude(3).unwrap().re, -1.0);
    state.apply(Gate::Cnot { control: 1, target: 0 }).unwrap();
    close(state.amplitude(2).unwrap().re, -1.0);
    close(state.norm_squared().unwrap(), 1.0);
}
