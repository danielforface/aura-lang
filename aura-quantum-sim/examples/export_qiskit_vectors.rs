//! Emit deterministic, self-describing CSV vectors for *external* Qiskit parity.
//! Gate sequence is encoded as semicolon-separated lower-case instructions,
//! with q0 the least significant wire, matching Qiskit's little-endian qubits.
//! This is not an interchange standard or a general QASM emitter.
use aura_quantum_sim::{Gate, Statevector, DEFAULT_MEMORY_BUDGET};

fn draw(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    (*state).wrapping_mul(0x2545_f491_4f6c_dd1d)
}

fn encoded(gate: &Gate) -> String {
    match gate {
        Gate::H(a) => format!("h:{a}"),
        Gate::X(a) => format!("x:{a}"),
        Gate::Y(a) => format!("y:{a}"),
        Gate::Z(a) => format!("z:{a}"),
        Gate::S(a) => format!("s:{a}"),
        Gate::T(a) => format!("t:{a}"),
        Gate::Cnot { control, target } => format!("cx:{control}:{target}"),
        Gate::Cz { control, target } => format!("cz:{control}:{target}"),
        Gate::Swap { a, b } => format!("swap:{a}:{b}"),
    }
}

fn gate(state: &mut u64, width: usize, family: usize) -> Gate {
    let q = (draw(state) as usize) % width;
    let other = if width > 1 {
        (q + 1 + (draw(state) as usize % (width - 1))) % width
    } else {
        q
    };
    match family {
        0 => Gate::H(q),
        1 => Gate::X(q),
        2 => Gate::Y(q),
        3 => Gate::Z(q),
        4 => Gate::S(q),
        5 => Gate::T(q),
        6 => Gate::Cnot { control: q, target: other },
        7 => Gate::Cz { control: q, target: other },
        8 => Gate::Swap { a: q, b: other },
        _ => unreachable!("invalid fixture family"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const CASES: usize = 8;
    const DEPTH: usize = 48;
    println!("case_id,qubits,gates,basis,re,im");
    for width in 1usize..=4 {
        for example in 0..CASES {
            let mut seed = 0x51_c0_a4_b1_e5_60_20_26_u64
                ^ ((width as u64) << 40)
                ^ ((example as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
            let mut simulator = Statevector::zero(width, DEFAULT_MEMORY_BUDGET)?;
            let mut sequence = Vec::new();
            // Make early states genuinely superposed and phase rich.
            for q in 0..width {
                for initial_gate in [Gate::H(q), Gate::T(q)] {
                    simulator.apply(initial_gate.clone())?;
                    sequence.push(encoded(&initial_gate));
                }
            }
            for depth in 0..DEPTH {
                let family = if width == 1 { depth % 6 } else { depth % 9 };
                let operation = gate(&mut seed, width, family);
                simulator.apply(operation.clone())?;
                sequence.push(encoded(&operation));
            }
            let gates = sequence.join(";");
            let case_id = format!("q{width}_case{example:02}");
            for basis in 0..(1usize << width) {
                let z = simulator.amplitude(basis)?;
                println!(
                    "{case_id},{width},{gates},{basis},{:.17e},{:.17e}",
                    z.re, z.im
                );
            }
        }
    }
    Ok(())
}
