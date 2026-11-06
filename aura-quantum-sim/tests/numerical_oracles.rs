//! C-002: scientific fixtures with independent analytic expectations.
//! The teleportation circuit's feed-forward is explicit test-harness logic:
//! NOT a claim of a dynamic Aura language/Quantum IR implementation.
use aura_quantum_sim::{
    Complex, Gate, SeededRng, SimError, Statevector, DEFAULT_MEMORY_BUDGET,
};

const EPS: f64 = 3e-11;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= EPS,
        "expected {expected:.17e}, got {actual:.17e} (difference {:.3e})",
        actual - expected
    );
}

fn ket_complex() -> [Complex; 2] {
    let alpha = 0.37_f64.sqrt();
    let magnitude = 0.63_f64.sqrt();
    let phase = 0.43_f64;
    [
        Complex::new(alpha, 0.0),
        Complex::new(magnitude * phase.cos(), magnitude * phase.sin()),
    ]
}

/// Independent overlap in a fixed two-dimensional subspace. Global phase is
/// deliberately ignored, while relative phase cannot be discarded.
fn fidelity(reference: &[Complex; 2], observed: &[Complex; 2]) -> f64 {
    let overlap = Complex::new(
        reference[0].re, -reference[0].im
    ) * observed[0]
        + Complex::new(reference[1].re, -reference[1].im) * observed[1];
    overlap.norm_squared()
}

#[test]
fn rare_but_representable_amplitude_can_be_postselected_without_norm_corruption() {
    // The probability 1e-320 is a subnormal f64. The amplitude itself is
    // representable and nonzero. Squaring then renormalizing via 1/sqrt(p)
    // loses enough precision to violate 1e-10 norm tolerance in C-001.
    let tiny = 1e-160;
    let mut state = Statevector::from_amplitudes(
        1,
        vec![Complex::ONE, Complex::new(tiny, 0.0)],
        DEFAULT_MEMORY_BUDGET,
    )
    .unwrap();
    state.postselect(0, true).unwrap();
    close(state.basis_probability(1).unwrap(), 1.0);
    close(state.basis_probability(0).unwrap(), 0.0);
    close(state.norm_squared().unwrap(), 1.0);
}

#[test]
fn underflowing_probability_does_not_mislabel_a_nonzero_amplitude_impossible() {
    // 1e-300 squared underflows to zero even though amplitude != 0.
    let mut state = Statevector::from_amplitudes(
        1,
        vec![Complex::ONE, Complex::new(1e-300, 0.0)],
        DEFAULT_MEMORY_BUDGET,
    )
    .unwrap();
    state.postselect(0, true).unwrap();
    close(state.basis_probability(1).unwrap(), 1.0);
    close(state.norm_squared().unwrap(), 1.0);
}

#[test]
fn physically_impossible_postselection_is_rejected_without_mutation() {
    let mut state = Statevector::zero(2, DEFAULT_MEMORY_BUDGET).unwrap();
    let before: Vec<_> = (0..4).map(|i| state.amplitude(i).unwrap()).collect();
    assert_eq!(state.postselect(1, true), Err(SimError::ImpossibleOutcome));
    for (i, original) in before.iter().enumerate() {
        assert_eq!(state.amplitude(i).unwrap(), *original);
    }
}

#[test]
fn hadamard_z_hadamard_reconstitutes_x_without_measuring_mid_circuit() {
    for initial_one in [false, true] {
        let mut s = Statevector::zero(1, DEFAULT_MEMORY_BUDGET).unwrap();
        if initial_one {
            s.apply(Gate::X(0)).unwrap();
        }
        s.apply(Gate::H(0)).unwrap();
        close(s.basis_probability(0).unwrap(), 0.5);
        close(s.basis_probability(1).unwrap(), 0.5);
        s.apply(Gate::Z(0)).unwrap();
        // Phase-only change does not alter the intermediate measurement distribution.
        close(s.basis_probability(0).unwrap(), 0.5);
        close(s.basis_probability(1).unwrap(), 0.5);
        s.apply(Gate::H(0)).unwrap();
        close(s.basis_probability(initial_one as usize).unwrap(), 0.0);
        close(s.basis_probability((!initial_one) as usize).unwrap(), 1.0);
    }
}

#[test]
fn non_clifford_t_interference_matches_closed_form_probabilities() {
    let mut state = Statevector::zero(1, DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply(Gate::H(0)).unwrap();
    state.apply(Gate::T(0)).unwrap();
    state.apply(Gate::H(0)).unwrap();
    let p0 = (2.0 + 2.0_f64.sqrt()) / 4.0;
    let p1 = (2.0 - 2.0_f64.sqrt()) / 4.0;
    close(state.basis_probability(0).unwrap(), p0);
    close(state.basis_probability(1).unwrap(), p1);
    close(state.norm_squared().unwrap(), 1.0);
}

#[test]
fn teleportation_manual_classical_corrections_recover_complex_phase() {
    let input = ket_complex();
    let mut covered = [false; 4];
    let mut negative_control_seen = false;

    for seed in 0..128 {
        // Input at q0, Bell-pair ancillas q1 and q2.
        let mut state = Statevector::from_amplitudes(
            3,
            vec![
                input[0], input[1], Complex::ZERO, Complex::ZERO,
                Complex::ZERO, Complex::ZERO, Complex::ZERO, Complex::ZERO,
            ],
            DEFAULT_MEMORY_BUDGET,
        ).unwrap();
        state.apply(Gate::H(1)).unwrap();
        state.apply(Gate::Cnot { control: 1, target: 2 }).unwrap();
        // Sender Bell-basis operations.
        state.apply(Gate::Cnot { control: 0, target: 1 }).unwrap();
        state.apply(Gate::H(0)).unwrap();
        let mut rng = SeededRng::new(seed);
        let m0 = state.measure(0, &mut rng).unwrap();
        let m1 = state.measure(1, &mut rng).unwrap();
        let prefix = (m0 as usize) | ((m1 as usize) << 1);
        covered[prefix] = true;
        let uncorrected = [
            state.amplitude(prefix).unwrap(),
            state.amplitude(prefix | 4).unwrap(),
        ];
        if fidelity(&input, &uncorrected) < 0.95 {
            negative_control_seen = true;
        }
        // Classical feed-forward is performed by the test itself, not by a
        // supported dynamic circuit or quantum IR instruction.
        if m1 {
            state.apply(Gate::X(2)).unwrap();
        }
        if m0 {
            state.apply(Gate::Z(2)).unwrap();
        }
        let output = [
            state.amplitude(prefix).unwrap(),
            state.amplitude(prefix | 4).unwrap(),
        ];
        close(fidelity(&input, &output), 1.0);
        close(state.norm_squared().unwrap(), 1.0);
    }

    assert!(covered.iter().all(|seen| *seen), "all four Bell outcomes must appear");
    assert!(
        negative_control_seen,
        "without a classical correction some measured branches must fail"
    );
}

#[test]
fn repeated_unitary_cancellations_do_not_accumulate_large_norm_drift() {
    let mut state = Statevector::zero(1, DEFAULT_MEMORY_BUDGET).unwrap();
    for _ in 0..10_000 {
        state.apply(Gate::H(0)).unwrap();
        state.apply(Gate::H(0)).unwrap();
    }
    close(state.norm_squared().unwrap(), 1.0);
    close(state.basis_probability(0).unwrap(), 1.0);
    close(state.basis_probability(1).unwrap(), 0.0);
}
