//! C-004 empirical long-depth numeric drift fixtures.
//! These are finite f64 experiments, NOT certified worst-case error bounds.
use aura_quantum_sim::{Complex, Gate, Statevector, DEFAULT_MEMORY_BUDGET};

fn source_state(n: usize) -> Vec<Complex> {
    let len = 1usize << n;
    let raw: Vec<Complex> = (0..len)
        .map(|i| {
            let x = ((i * 17 + 3) % 29) as f64 / 29.0 - 0.5;
            let y = ((i * 13 + 9) % 31) as f64 / 31.0 - 0.5;
            Complex::new(x, y)
        })
        .collect();
    let norm = raw.iter().map(|z| z.norm_squared()).sum::<f64>().sqrt();
    raw.into_iter()
        .map(|z| Complex::new(z.re / norm, z.im / norm))
        .collect()
}

fn inverse(gate: &Gate) -> Vec<Gate> {
    match gate {
        Gate::S(q) => vec![Gate::S(*q); 3],
        Gate::T(q) => vec![Gate::T(*q); 7],
        _ => vec![gate.clone()],
    }
}

fn trace_max_abs_error(state: &Statevector, reference: &[Complex]) -> f64 {
    reference
        .iter()
        .enumerate()
        .map(|(i, correct)| {
            let actual = state.amplitude(i).unwrap();
            (actual.re - correct.re)
                .abs()
                .max((actual.im - correct.im).abs())
        })
        .fold(0.0, f64::max)
}

#[test]
fn long_depth_inverse_composition_preserves_general_complex_states() {
    // The inverse order is mathematically exact for every supported gate.
    // Unlike H² on |0>, this exercises nontrivial phase-rich, entangled states.
    const ROUNDS: usize = 256;
    const LIMIT: f64 = 2e-11;
    for n in 1usize..=4 {
        let input = source_state(n);
        let mut state =
            Statevector::from_amplitudes(n, input.clone(), DEFAULT_MEMORY_BUDGET).unwrap();
        let mut forward = Vec::new();
        for q in 0..n {
            forward.extend([
                Gate::H(q),
                Gate::T(q),
                Gate::Y(q),
                Gate::S(q),
                Gate::X(q),
                Gate::Z(q),
            ]);
        }
        if n > 1 {
            forward.extend([
                Gate::Cnot { control: 0, target: n - 1 },
                Gate::Cz { control: n - 1, target: 0 },
                Gate::Swap { a: 0, b: n - 1 },
            ]);
        }
        let inverse_seq: Vec<Gate> = forward.iter().rev().flat_map(inverse).collect();
        let mut worst_difference: f64 = 0.0;
        let mut worst_norm_difference: f64 = 0.0;
        for _ in 0..ROUNDS {
            for gate in &forward {
                state.apply(gate.clone()).unwrap();
            }
            for gate in &inverse_seq {
                state.apply(gate.clone()).unwrap();
            }
            worst_difference =
                worst_difference.max(trace_max_abs_error(&state, &input));
            worst_norm_difference =
                worst_norm_difference.max((state.norm_squared().unwrap() - 1.0).abs());
        }
        eprintln!(
            "C004_PRECISION width={} rounds={} forward_gates={} inverse_gates={} max_component_error={:.17e} max_norm_error={:.17e}",
            n, ROUNDS, forward.len(), inverse_seq.len(), worst_difference, worst_norm_difference
        );
        assert!(
            worst_difference <= LIMIT && worst_norm_difference <= LIMIT,
            "long-depth numerical drift n={n}: component={worst_difference:.5e} norm={worst_norm_difference:.5e}"
        );
    }
}

#[test]
fn nonclifford_t_phase_roundtrip_after_32768_gates_has_bounded_empirical_drift() {
    let mut state = Statevector::zero(1, DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply(Gate::X(0)).unwrap();
    for _ in 0..4096 {
        for _ in 0..8 {
            state.apply(Gate::T(0)).unwrap();
        }
    }
    let final_amp = state.amplitude(1).unwrap();
    let max_error = (final_amp.re - 1.0).abs().max(final_amp.im.abs());
    let norm_error = (state.norm_squared().unwrap() - 1.0).abs();
    eprintln!(
        "C004_PRECISION T_roundtrip=4096 gates=32768 max_component_error={:.17e} norm_error={:.17e}",
        max_error, norm_error
    );
    assert!(max_error <= 2e-11);
    assert!(norm_error <= 2e-11);
}
