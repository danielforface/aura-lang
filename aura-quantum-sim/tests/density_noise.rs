//! Q28-007 source-level acceptance: physically motivated CPTP channels.
//! No hardware/calibration or formal positivity certificate is claimed.
use aura_quantum_sim::density::{DensityError, DensityMatrix, NoiseChannel, MAX_DENSITY_QUBITS};
use aura_quantum_sim::{Complex, Gate, Statevector, DEFAULT_MEMORY_BUDGET};

const EPS: f64 = 5e-11;
fn close(got: f64, expected: f64) {
    assert!((got - expected).abs() <= EPS,
        "expected {expected:.17e}, got {got:.17e}");
}
fn close_complex(actual: Complex, expected: Complex) {
    close(actual.re, expected.re);
    close(actual.im, expected.im);
}
fn conjugate(z: Complex) -> Complex {
    Complex::new(z.re, -z.im)
}

#[test]
fn ideal_9_gate_families_match_statevector_outer_products_at_every_step() {
    let mut pure = Statevector::zero(3, DEFAULT_MEMORY_BUDGET).unwrap();
    let mut rho = DensityMatrix::from_statevector(&pure, DEFAULT_MEMORY_BUDGET).unwrap();
    let gates = [
        Gate::H(0), Gate::T(0), Gate::H(1), Gate::Cnot {control:0,target:2},
        Gate::Y(1), Gate::Cz {control:0,target:1},
        Gate::Swap {a:0,b:2}, Gate::Z(2), Gate::S(0), Gate::X(1),
        Gate::Cnot {control:2,target:0}, Gate::H(2),
        Gate::Swap {a:1,b:2}, Gate::Cz {control:1,target:2},
    ];
    for gate in gates {
        pure.apply(gate.clone()).unwrap();
        rho.apply_gate(gate).unwrap();
        for row in 0..8 {
            for col in 0..8 {
                let expected = pure.amplitude(row).unwrap()
                    * conjugate(pure.amplitude(col).unwrap());
                close_complex(rho.element(row,col).unwrap(), expected);
            }
            close(rho.basis_probability(row).unwrap(),
                pure.basis_probability(row).unwrap());
        }
        close(rho.trace(), 1.0);
        close(rho.purity().unwrap(), 1.0);
    }
}

#[test]
fn amplitude_damping_and_phase_flip_preserve_trace_but_reduce_coherence() {
    // Independent closed-form Kraus oracle for |+>, gamma=.36, p=.25:
    // damping => [[.68,.4],[.4,.32]]; Z-flip => [[.68,.2],[.2,.32]].
    let mut s = Statevector::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    s.apply(Gate::H(0)).unwrap();
    let mut rho = DensityMatrix::from_statevector(&s,DEFAULT_MEMORY_BUDGET).unwrap();
    rho.apply_noise(NoiseChannel::AmplitudeDamping{qubit:0,gamma:0.36}).unwrap();
    close_complex(rho.element(0,0).unwrap(),Complex::new(0.68,0.0));
    close_complex(rho.element(0,1).unwrap(),Complex::new(0.4,0.0));
    close_complex(rho.element(1,0).unwrap(),Complex::new(0.4,0.0));
    close_complex(rho.element(1,1).unwrap(),Complex::new(0.32,0.0));
    rho.apply_noise(NoiseChannel::PhaseFlip{qubit:0,probability:0.25}).unwrap();
    close_complex(rho.element(0,1).unwrap(),Complex::new(0.2,0.0));
    close_complex(rho.element(1,0).unwrap(),Complex::new(0.2,0.0));
    close(rho.trace(),1.0);
    close(rho.purity().unwrap(),0.6448);
    assert!(rho.purity().unwrap() < 1.0);
}

#[test]
fn full_damping_of_bell_pair_yields_mixed_other_wire_without_false_entanglement() {
    let mut rho=DensityMatrix::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    rho.apply_gate(Gate::H(0)).unwrap();
    rho.apply_gate(Gate::Cnot{control:0,target:1}).unwrap();
    rho.apply_noise(NoiseChannel::AmplitudeDamping{qubit:1,gamma:1.0}).unwrap();
    close(rho.basis_probability(0).unwrap(),0.5);
    close(rho.basis_probability(1).unwrap(),0.5);
    close(rho.basis_probability(2).unwrap(),0.0);
    close(rho.basis_probability(3).unwrap(),0.0);
    let reduced_q0=rho.reduced_qubit(0).unwrap();
    let reduced_q1=rho.reduced_qubit(1).unwrap();
    close(reduced_q0[0][0].re,0.5);
    close(reduced_q0[1][1].re,0.5);
    close_complex(reduced_q0[0][1], Complex::ZERO);
    close(reduced_q1[0][0].re,1.0);
    close(reduced_q1[1][1].re,0.0);
    close(rho.purity().unwrap(),0.5);
    close(rho.trace(),1.0);
}

#[test]
fn unobserved_phase_flip_is_not_sampled_state_collapse() {
    let mut s=Statevector::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    s.apply(Gate::H(0)).unwrap();
    let mut rho=DensityMatrix::from_statevector(&s,DEFAULT_MEMORY_BUDGET).unwrap();
    rho.apply_noise(NoiseChannel::PhaseFlip{qubit:0,probability:0.5}).unwrap();
    close(rho.basis_probability(0).unwrap(),0.5);
    close(rho.basis_probability(1).unwrap(),0.5);
    close_complex(rho.element(0,1).unwrap(),Complex::ZERO);
    close(rho.purity().unwrap(),0.5);
    let mut fully_flipped=DensityMatrix::from_statevector(&s,DEFAULT_MEMORY_BUDGET).unwrap();
    fully_flipped.apply_noise(NoiseChannel::PhaseFlip{qubit:0,probability:1.0}).unwrap();
    close_complex(fully_flipped.element(0,1).unwrap(),Complex::new(-0.5,0.0));
    close(fully_flipped.purity().unwrap(),1.0);
}

#[test]
fn zero_strength_is_identity_and_density_limit_is_explicit() {
    let mut state=DensityMatrix::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply_gate(Gate::H(0)).unwrap();
    let before:Vec<_>=(0..4).flat_map(|r|(0..4).map(move |c|(r,c))).map(
        |(r,c)|state.element(r,c).unwrap()).collect();
    state.apply_noise(NoiseChannel::PhaseFlip{qubit:1,probability:0.0}).unwrap();
    state.apply_noise(NoiseChannel::AmplitudeDamping{qubit:0,gamma:0.0}).unwrap();
    let after:Vec<_>=(0..4).flat_map(|r|(0..4).map(move |c|(r,c))).map(
        |(r,c)|state.element(r,c).unwrap()).collect();
    assert_eq!(before,after);
    assert_eq!(state.logical_bytes(),256);
    assert!(matches!(DensityMatrix::zero(0,DEFAULT_MEMORY_BUDGET),
        Err(DensityError::UnsupportedWidth{qubits:0,..})));
    assert!(matches!(DensityMatrix::zero(MAX_DENSITY_QUBITS+1,DEFAULT_MEMORY_BUDGET),
        Err(DensityError::UnsupportedWidth{..})));
    assert_eq!(DensityMatrix::zero(2,255).unwrap_err(),
        DensityError::BudgetExceeded{required_bytes:256,budget_bytes:255});
}

#[test]
fn invalid_noise_strength_and_aliases_fail_before_any_mutation() {
    let mut state=DensityMatrix::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply_gate(Gate::H(0)).unwrap();
    let expected:Vec<_>=(0..4).flat_map(|r|(0..4).map(move |c|(r,c))).map(
        |(r,c)|state.element(r,c).unwrap()).collect();
    for bad in [
        NoiseChannel::PhaseFlip{qubit:0,probability:-0.01},
        NoiseChannel::PhaseFlip{qubit:0,probability:f64::NAN},
        NoiseChannel::AmplitudeDamping{qubit:1,gamma:1.01},
        NoiseChannel::AmplitudeDamping{qubit:1,gamma:f64::INFINITY},
    ] {
        assert_eq!(state.apply_noise(bad).unwrap_err(),DensityError::InvalidProbability);
    }
    assert_eq!(state.apply_noise(NoiseChannel::PhaseFlip{
        qubit:2,probability:0.5}).unwrap_err(),
        DensityError::InvalidQubit{index:2,qubits:2});
    assert_eq!(state.apply_gate(Gate::Cnot{control:0,target:0}).unwrap_err(),
        DensityError::DuplicateQubit(0));
    assert_eq!(state.apply_gate(Gate::Swap{a:1,b:1}).unwrap_err(),
        DensityError::DuplicateQubit(1));
    let got:Vec<_>=(0..4).flat_map(|r|(0..4).map(move |c|(r,c))).map(
        |(r,c)|state.element(r,c).unwrap()).collect();
    assert_eq!(got,expected);
}

#[test]
fn mixed_channels_on_two_qubits_keep_hermitian_trace_and_purity_bounded() {
    let mut rho=DensityMatrix::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    rho.apply_gate(Gate::H(0)).unwrap();
    rho.apply_gate(Gate::T(0)).unwrap();
    rho.apply_gate(Gate::H(1)).unwrap();
    rho.apply_gate(Gate::Cnot{control:0,target:1}).unwrap();
    for step in 0..100 {
        let p=(step%7) as f64 / 10.0;
        rho.apply_noise(NoiseChannel::PhaseFlip{qubit:step%2,probability:p}).unwrap();
        rho.apply_noise(NoiseChannel::AmplitudeDamping{
            qubit:(step+1)%2,gamma:0.01}).unwrap();
        rho.apply_gate(Gate::H(step%2)).unwrap();
        rho.validate().unwrap();
        close(rho.trace(),1.0);
        assert!((-EPS..=1.0+EPS).contains(&rho.purity().unwrap()));
        for i in 0..4 {for j in 0..4 {
            close_complex(rho.element(i,j).unwrap(),
                conjugate(rho.element(j,i).unwrap()));
        }}
    }
}

#[test]
fn noise_channels_commute_on_independent_qubits() {
    let mut s=Statevector::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    s.apply(Gate::H(0)).unwrap();
    s.apply(Gate::T(0)).unwrap();
    s.apply(Gate::H(1)).unwrap();
    s.apply(Gate::Cnot{control:0,target:1}).unwrap();
    let mut a=DensityMatrix::from_statevector(&s,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut b=a.clone();
    let phase=NoiseChannel::PhaseFlip{qubit:0,probability:0.27};
    let damp=NoiseChannel::AmplitudeDamping{qubit:1,gamma:0.42};
    a.apply_noise(phase).unwrap(); a.apply_noise(damp).unwrap();
    b.apply_noise(damp).unwrap(); b.apply_noise(phase).unwrap();
    for r in 0..4 {for c in 0..4 {
        close_complex(a.element(r,c).unwrap(),b.element(r,c).unwrap());
    }}
}
