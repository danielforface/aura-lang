//! C-002 independent small-system oracle: dense N×N matrix action on 3 qubits.
//! This test uses a separate "matrix entry then dense dot product" algorithm
//! instead of Statevector's mutation/swap implementation. It is a numerical
//! cross-check, not a formal proof or a third-party quantum reference engine.
use aura_quantum_sim::{Complex, Gate, Statevector, DEFAULT_MEMORY_BUDGET};

const EPS: f64 = 2e-11;

fn close(got: f64, expected: f64) {
    assert!(
        (got - expected).abs() < EPS,
        "expected {expected:.17e}, got {got:.17e}"
    );
}

fn matrix_entry_single(gate: &Gate, row: usize, col: usize) -> Complex {
    use Gate::*;
    let a = std::f64::consts::FRAC_1_SQRT_2;
    match gate {
        H(_) => match (row, col) {
            (0, 0) | (0, 1) | (1, 0) => Complex::new(a, 0.0),
            (1, 1) => Complex::new(-a, 0.0),
            _ => unreachable!(),
        },
        X(_) => if row != col { Complex::ONE } else { Complex::ZERO },
        Y(_) => match (row, col) {
            (0, 1) => Complex::NEG_I,
            (1, 0) => Complex::I,
            _ => Complex::ZERO,
        },
        Z(_) => if row != col { Complex::ZERO } else if row == 1 {
            Complex::new(-1.0, 0.0)
        } else {
            Complex::ONE
        },
        S(_) => if row != col { Complex::ZERO } else if row == 1 {
            Complex::I
        } else {
            Complex::ONE
        },
        T(_) => if row != col { Complex::ZERO } else if row == 1 {
            Complex::new(a, a)
        } else {
            Complex::ONE
        },
        _ => unreachable!("not a single-wire gate"),
    }
}

fn matrix_entry(gate: &Gate, row: usize, col: usize) -> Complex {
    let one = Complex::ONE;
    let zero = Complex::ZERO;
    match gate {
        Gate::H(q) | Gate::X(q) | Gate::Y(q) | Gate::Z(q) |
        Gate::S(q) | Gate::T(q) => {
            let mask = 1usize << q;
            if row & !mask != col & !mask {
                return zero;
            }
            matrix_entry_single(gate, ((row & mask) != 0) as usize, ((col & mask) != 0) as usize)
        }
        Gate::Cnot { control, target } => {
            let moved = col ^ (if (col >> control) & 1 == 1 { 1 << target } else { 0 });
            if row == moved { one } else { zero }
        }
        Gate::Cz { control, target } => {
            if row != col {
                zero
            } else if ((col >> control) & 1 == 1) && ((col >> target) & 1 == 1) {
                Complex::new(-1.0, 0.0)
            } else {
                one
            }
        }
        Gate::Swap { a, b } => {
            let abit = (col >> a) & 1;
            let bbit = (col >> b) & 1;
            let swapped = if abit == bbit { col } else { col ^ (1 << a) ^ (1 << b) };
            if row == swapped { one } else { zero }
        }
    }
}

fn dense_oracle_apply(input: &[Complex], gate: &Gate) -> Vec<Complex> {
    (0..input.len()).map(|row| {
        (0..input.len()).fold(Complex::ZERO, |sum, col| {
            sum + matrix_entry(gate, row, col) * input[col]
        })
    }).collect()
}

#[test]
fn nine_gates_match_dense_three_qubit_complex_oracle_after_entanglement() {
    let raw: Vec<_> = (0..8).map(|i| {
        Complex::new((i + 1) as f64 * 0.03125, ((i * 3) % 7) as f64 * -0.0375)
    }).collect();
    let magnitude = raw.iter().map(|z| z.norm_squared()).sum::<f64>().sqrt();
    let initial: Vec<_> = raw.iter().map(|z| {
        Complex::new(z.re / magnitude, z.im / magnitude)
    }).collect();
    let mut state = Statevector::from_amplitudes(
        3, initial.clone(), DEFAULT_MEMORY_BUDGET
    ).unwrap();
    let mut dense = initial;
    let gates = [
        Gate::H(0), Gate::T(2), Gate::Cnot { control: 1, target: 0 },
        Gate::Y(1), Gate::S(2), Gate::Swap { a: 0, b: 2 },
        Gate::H(1), Gate::Cz { control: 0, target: 1 },
        Gate::X(2), Gate::Z(1), Gate::T(0),
        Gate::Cnot { control: 2, target: 0 },
        Gate::Swap { a: 1, b: 2 }, Gate::H(2),
        Gate::Cz { control: 2, target: 1 },
    ];
    for gate in gates {
        dense = dense_oracle_apply(&dense, &gate);
        state.apply(gate).unwrap();
        for (basis, expected) in dense.iter().enumerate() {
            let actual = state.amplitude(basis).unwrap();
            close(actual.re, expected.re);
            close(actual.im, expected.im);
        }
        close(state.norm_squared().unwrap(), 1.0);
    }
}

#[test]
fn teleportation_bell_measurement_register_is_uniform_analytically_and_by_shots() {
    // Nontrivial complex input; Bell register q0,q1 should have probabilities
    // 1/4 each regardless of input state, even before corrective feed-forward.
    let alpha = 0.37_f64.sqrt();
    let beta_mag = 0.63_f64.sqrt();
    let beta_phase = 0.43_f64;
    let mut state = Statevector::from_amplitudes(
        3,
        vec![
            Complex::new(alpha, 0.0),
            Complex::new(beta_mag * beta_phase.cos(), beta_mag * beta_phase.sin()),
            Complex::ZERO, Complex::ZERO, Complex::ZERO, Complex::ZERO,
            Complex::ZERO, Complex::ZERO,
        ],
        DEFAULT_MEMORY_BUDGET,
    ).unwrap();
    state.apply(Gate::H(1)).unwrap();
    state.apply(Gate::Cnot { control: 1, target: 2 }).unwrap();
    state.apply(Gate::Cnot { control: 0, target: 1 }).unwrap();
    state.apply(Gate::H(0)).unwrap();
    let mut analytic = [0.0; 4];
    for basis in 0..8 {
        analytic[basis & 3] += state.basis_probability(basis).unwrap();
    }
    for actual in analytic {
        close(actual, 0.25);
    }
    let mut counts = [0usize; 4];
    for basis in state.sample_basis(4096, 0x7654_3210).unwrap() {
        counts[basis & 3] += 1;
    }
    for count in counts {
        let observed = count as f64 / 4096.0;
        // Single-cell sigma = sqrt(0.25*0.75/4096) ~= 0.00677.
        // Tolerance 0.035 is >5 sigma per cell; not an exact 25% claim.
        assert!((observed - 0.25).abs() < 0.035, "observed {observed}");
    }
}


/// Independent test-only Xorshift64* generator, deliberately distinct from
/// the simulator's versioned SplitMix64 sampling stream.
fn test_next(rng: &mut u64) -> u64 {
    *rng ^= *rng >> 12;
    *rng ^= *rng << 25;
    *rng ^= *rng >> 27;
    (*rng).wrapping_mul(0x2545_f491_4f6c_dd1d)
}

fn test_unit_interval(rng: &mut u64) -> f64 {
    ((test_next(rng) >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
}

fn test_gate(rng: &mut u64, n: usize, gate_family: usize) -> Gate {
    let a = test_next(rng) as usize % n;
    let b = if n > 1 {
        (a + 1 + test_next(rng) as usize % (n - 1)) % n
    } else {
        a
    };
    match gate_family {
        0 => Gate::H(a),
        1 => Gate::X(a),
        2 => Gate::Y(a),
        3 => Gate::Z(a),
        4 => Gate::S(a),
        5 => Gate::T(a),
        6 => Gate::Cnot { control: a, target: b },
        7 => Gate::Cz { control: a, target: b },
        8 => Gate::Swap { a, b },
        _ => unreachable!("fixture gate family must be within 0..9"),
    }
}

#[test]
fn seeded_multiqubit_circuit_corpus_agrees_with_independent_dense_oracle() {
    const CASES_PER_WIDTH: u64 = 32;
    const DEPTH: usize = 63;
    const LIMIT: f64 = 2e-11;
    let mut covered = [false; 9];
    let mut gates_checked = 0usize;

    for n in 1usize..=4 {
        let length = 1usize << n;
        for case in 0..CASES_PER_WIDTH {
            let mut rng = 0xd15f_344d_823b_84c7_u64
                ^ ((n as u64) << 48)
                ^ case.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let mut input: Vec<Complex> = (0..length)
                .map(|_| Complex::new(test_unit_interval(&mut rng) - 0.5, test_unit_interval(&mut rng) - 0.5))
                .collect();
            let norm = input.iter().map(|z| z.norm_squared()).sum::<f64>().sqrt();
            for z in &mut input {
                z.re /= norm;
                z.im /= norm;
            }
            let mut simulator = Statevector::from_amplitudes(
                n, input.clone(), DEFAULT_MEMORY_BUDGET
            ).unwrap();
            let mut dense = input;
            for depth in 0..DEPTH {
                let family = if n == 1 { depth % 6 } else { depth % 9 };
                covered[family] = true;
                let gate = test_gate(&mut rng, n, family);
                dense = dense_oracle_apply(&dense, &gate);
                simulator.apply(gate.clone()).unwrap();
                gates_checked += 1;
                for (basis, expected) in dense.iter().enumerate() {
                    let actual = simulator.amplitude(basis).unwrap();
                    assert!(
                        (actual.re - expected.re).abs() <= LIMIT
                        && (actual.im - expected.im).abs() <= LIMIT,
                        "n={n}, case={case}, depth={depth}, gate={gate:?}, basis={basis}: got {actual:?}, expected {expected:?}"
                    );
                }
                let numerical_norm = simulator.norm_squared().unwrap();
                assert!(
                    (numerical_norm - 1.0).abs() <= LIMIT,
                    "norm drift n={n}, case={case}, depth={depth}: {numerical_norm}"
                );
            }
        }
    }
    assert!(covered.into_iter().all(|gate| gate));
    assert_eq!(gates_checked, 4 * CASES_PER_WIDTH as usize * DEPTH);
}
