//! Deterministic Q28 C-003 metamorphic and numerical edge-case assurance.
//! All claims are scoped to bounded f64 pure-state *simulation*.
use aura_quantum_sim::{
    Complex, Gate, SeededRng, SimError, Statevector, DEFAULT_MEMORY_BUDGET,
};

const EPS: f64 = 5e-13;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= EPS,
        "expected {expected:.17e}, actual {actual:.17e}, difference {:.3e}",
        actual - expected
    );
}

fn complex_close(actual: Complex, expected: Complex) {
    close(actual.re, expected.re);
    close(actual.im, expected.im);
}

fn amplitudes(state: &Statevector) -> Vec<Complex> {
    (0..(1usize << state.qubits()))
        .map(|basis| state.amplitude(basis).unwrap())
        .collect()
}

fn conj(z: Complex) -> Complex {
    Complex::new(z.re, -z.im)
}

fn polar(radius: f64, radians: f64) -> Complex {
    Complex::new(radius * radians.cos(), radius * radians.sin())
}

// Explicit, independent one-wire partial trace over every environment
// basis assignment; the q0=least-significant convention is intentional.
fn reduced_density(state: &Statevector, wire: usize) -> [[Complex; 2]; 2] {
    let n = state.qubits();
    assert!(wire < n);
    let a = amplitudes(state);
    let mut rho = [[Complex::ZERO; 2]; 2];
    for env in 0..(1usize << (n - 1)) {
        let mut base = 0usize;
        let mut env_bit = 0usize;
        for bit in 0..n {
            if bit != wire {
                base |= ((env >> env_bit) & 1) << bit;
                env_bit += 1;
            }
        }
        let pair = [a[base], a[base | (1usize << wire)]];
        for (r, ar) in pair.iter().enumerate() {
            for (c, ac) in pair.iter().enumerate() {
                rho[r][c] = rho[r][c] + *ar * conj(*ac);
            }
        }
    }
    rho
}

fn purity(rho: [[Complex; 2]; 2]) -> f64 {
    rho[0][0].re * rho[0][0].re
        + rho[1][1].re * rho[1][1].re
        + 2.0 * rho[0][1].norm_squared()
}

#[test]
fn rare_complex_phase_preserved_through_postselection() {
    // Squaring 1e-300 to calculate Born probabilities underflows to zero.
    // Conditioning on its *amplitudes* must preserve both relative phases.
    let tiny = 1e-300;
    let mut state = Statevector::from_amplitudes(
        2,
        vec![
            Complex::ONE,
            polar(0.6 * tiny, 0.37),
            Complex::ZERO,
            polar(0.8 * tiny, -1.13),
        ],
        DEFAULT_MEMORY_BUDGET,
    )
    .unwrap();
    close(state.wire_probability(0, true).unwrap(), 0.0);
    state.postselect(0, true).unwrap();
    complex_close(state.amplitude(1).unwrap(), polar(0.6, 0.37));
    complex_close(state.amplitude(3).unwrap(), polar(0.8, -1.13));
    complex_close(state.amplitude(0).unwrap(), Complex::ZERO);
    complex_close(state.amplitude(2).unwrap(), Complex::ZERO);
    close(state.norm_squared().unwrap(), 1.0);
}

#[test]
fn smallest_positive_subnormal_complex_support_can_be_conditioned() {
    let smallest = f64::from_bits(1);
    let mut state = Statevector::from_amplitudes(
        1,
        vec![Complex::ONE, Complex::new(smallest, smallest)],
        DEFAULT_MEMORY_BUDGET,
    )
    .unwrap();
    state.postselect(0, true).unwrap();
    let expected = std::f64::consts::FRAC_1_SQRT_2;
    complex_close(state.amplitude(1).unwrap(), Complex::new(expected, expected));
    close(state.norm_squared().unwrap(), 1.0);
    assert_eq!(state.postselect(0, false), Err(SimError::ImpossibleOutcome));
    complex_close(state.amplitude(1).unwrap(), Complex::new(expected, expected));
}

#[test]
fn commuting_compatible_postselections_are_order_independent_and_repeatable() {
    let prepare = || {
        let mut s = Statevector::zero(2, DEFAULT_MEMORY_BUDGET).unwrap();
        s.apply(Gate::H(0)).unwrap();
        s.apply(Gate::H(1)).unwrap();
        s.apply(Gate::T(0)).unwrap();
        s.apply(Gate::S(1)).unwrap();
        s.apply(Gate::Cnot { control: 0, target: 1 }).unwrap();
        s
    };
    let mut first = prepare();
    let mut second = prepare();
    first.postselect(0, true).unwrap();
    first.postselect(1, false).unwrap();
    second.postselect(1, false).unwrap();
    second.postselect(0, true).unwrap();
    for (a, b) in amplitudes(&first).iter().zip(amplitudes(&second)) {
        complex_close(*a, b);
    }
    let previous = amplitudes(&first);
    first.postselect(0, true).unwrap();
    assert_eq!(amplitudes(&first), previous);
    let mut rng = SeededRng::new(0x5a1e);
    assert!(first.measure(0, &mut rng).unwrap());
    assert!(!first.measure(1, &mut rng).unwrap());
    assert_eq!(amplitudes(&first), previous);
}

#[test]
fn invalid_operations_preserve_amplitudes_and_invalid_measurement_preserves_rng() {
    let mut s = Statevector::zero(3, DEFAULT_MEMORY_BUDGET).unwrap();
    s.apply(Gate::H(0)).unwrap();
    s.apply(Gate::T(0)).unwrap();
    s.apply(Gate::Cnot { control: 0, target: 2 }).unwrap();
    let before = amplitudes(&s);
    assert_eq!(
        s.apply(Gate::Cnot { control: 0, target: 0 }),
        Err(SimError::DuplicateQubit(0))
    );
    assert_eq!(s.postselect(3, false), Err(SimError::InvalidQubit { index: 3, qubits: 3 }));
    let mut rng = SeededRng::new(0xbead);
    let mut reference_rng = rng.clone();
    assert_eq!(
        s.measure(9, &mut rng),
        Err(SimError::InvalidQubit { index: 9, qubits: 3 })
    );
    assert_eq!(rng.next_u64(), reference_rng.next_u64());
    assert_eq!(amplitudes(&s), before);
}

#[test]
fn multiplying_every_amplitude_by_a_global_phase_preserves_born_statistics() {
    let mut original = Statevector::zero(3, DEFAULT_MEMORY_BUDGET).unwrap();
    for gate in [
        Gate::H(0), Gate::T(0), Gate::H(1),
        Gate::Cnot { control: 1, target: 2 }, Gate::S(2),
        Gate::H(2), Gate::Y(0),
    ] {
        original.apply(gate).unwrap();
    }
    let factor = polar(1.0, 0.71);
    let phased = Statevector::from_amplitudes(
        3,
        amplitudes(&original).into_iter().map(|a| a * factor).collect(),
        DEFAULT_MEMORY_BUDGET,
    ).unwrap();
    for basis in 0..8 {
        close(original.basis_probability(basis).unwrap(), phased.basis_probability(basis).unwrap());
        complex_close(phased.amplitude(basis).unwrap(), original.amplitude(basis).unwrap() * factor);
    }
    for q in 0..3 {
        close(original.wire_probability(q, true).unwrap(), phased.wire_probability(q, true).unwrap());
    }
    assert_eq!(original.sample_basis(4096, 77).unwrap(), phased.sample_basis(4096, 77).unwrap());
}

#[test]
fn splitmix64_version_one_seed_zero_golden_sequence_is_pinned() {
    let mut rng = SeededRng::new(0);
    let golden = [
        0xe220_a839_7b1d_cdaf,
        0x6e78_9e6a_a1b9_65f4,
        0x06c4_5d18_8009_454f,
        0xf88b_b8a8_724c_81ec,
        0x1b39_896a_51a8_749b,
    ];
    for expected in golden {
        assert_eq!(rng.next_u64(), expected);
    }
}

#[test]
fn bell_and_ghz_reduced_density_show_entanglement_while_product_is_pure() {
    let mut product = Statevector::zero(3, DEFAULT_MEMORY_BUDGET).unwrap();
    product.apply(Gate::H(0)).unwrap();
    product.apply(Gate::T(0)).unwrap();
    for wire in 0..3 {
        close(purity(reduced_density(&product, wire)), 1.0);
    }
    let mut bell = Statevector::zero(2, DEFAULT_MEMORY_BUDGET).unwrap();
    bell.apply(Gate::H(0)).unwrap();
    bell.apply(Gate::Cnot { control: 0, target: 1 }).unwrap();
    for wire in 0..2 {
        let rho = reduced_density(&bell, wire);
        close(rho[0][0].re, 0.5);
        close(rho[1][1].re, 0.5);
        complex_close(rho[0][1], Complex::ZERO);
        complex_close(rho[1][0], Complex::ZERO);
        close(purity(rho), 0.5);
    }
    let mut ghz = Statevector::zero(3, DEFAULT_MEMORY_BUDGET).unwrap();
    ghz.apply(Gate::H(0)).unwrap();
    ghz.apply(Gate::Cnot { control: 0, target: 1 }).unwrap();
    ghz.apply(Gate::Cnot { control: 1, target: 2 }).unwrap();
    for wire in 0..3 {
        close(purity(reduced_density(&ghz, wire)), 0.5);
    }
    // Mutation-sensitive negative control: omitting GHZ's last entangler
    // leaves q2 separable, whose reduced density must have purity 1.
    let mut broken = Statevector::zero(3, DEFAULT_MEMORY_BUDGET).unwrap();
    broken.apply(Gate::H(0)).unwrap();
    broken.apply(Gate::Cnot { control: 0, target: 1 }).unwrap();
    close(purity(reduced_density(&broken, 2)), 1.0);
}

#[test]
fn bell_pauli_xx_yy_zz_correlators_have_distinct_physical_signs() {
    let mut state = Statevector::zero(2, DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply(Gate::H(0)).unwrap();
    state.apply(Gate::Cnot { control: 0, target: 1 }).unwrap();
    let a = amplitudes(&state);
    let xx: f64 = a.iter().enumerate()
        .map(|(basis, amp)| (conj(*amp) * a[basis ^ 3]).re)
        .sum();
    let yy: f64 = a.iter().enumerate()
        .map(|(basis, amp)| {
            let same = (basis & 1 == 0) == (basis & 2 == 0);
            let sign = if same { -1.0 } else { 1.0 };
            sign * (conj(*amp) * a[basis ^ 3]).re
        }).sum();
    let zz: f64 = a.iter().enumerate()
        .map(|(basis, amp)| {
            let same = (basis & 1 == 0) == (basis & 2 == 0);
            if same { amp.norm_squared() } else { -amp.norm_squared() }
        }).sum();
    close(xx, 1.0);
    close(yy, -1.0);
    close(zz, 1.0);
    // Algebraic combination for textbook CHSH settings, not sampled rotated
    // measurement experiments or a Bell test on quantum hardware.
    close(std::f64::consts::SQRT_2 * (xx + zz), 2.0 * std::f64::consts::SQRT_2);
}
