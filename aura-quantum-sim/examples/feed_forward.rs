//! Execute a genuinely dynamic Bell circuit through the public Rust API.
//!
//! Run:
//! cargo run --manifest-path aura-quantum-sim/Cargo.toml --example feed_forward
//!
//! Pure CPU simulation only: no quantum hardware, no Aura compiler/QIR bridge.
use aura_quantum_sim::circuit::{
    Circuit, Instruction, TraceKind, CIRCUIT_PROFILE, CIRCUIT_SCHEMA_VERSION,
};
use aura_quantum_sim::{Gate, DEFAULT_MEMORY_BUDGET};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut circuit = Circuit::new(2, 1);
    circuit.push(Instruction::Gate(Gate::H(0)));
    circuit.push(Instruction::Gate(Gate::Cnot { control: 0, target: 1 }));
    circuit.push(Instruction::Measure { qubit: 0, into_bit: 0 });
    circuit.push(Instruction::If {
        bit: 0,
        equals: true,
        gate: Gate::X(1),
    });
    circuit.push(Instruction::Reset { qubit: 0 });

    let seed = 0x5151_2626_u64;
    let single = circuit.execute_once(seed, DEFAULT_MEMORY_BUDGET)?;
    let measured = single.shot.bits[0].ok_or("missing measurement result")?;
    if single.shot.trace.len() != 2 {
        return Err("missing ordered measurement/reset events".into());
    }
    if !matches!(single.shot.trace[0].kind, TraceKind::Measure { .. })
        || !matches!(single.shot.trace[1].kind, TraceKind::Reset { .. })
    {
        return Err("wrong event order".into());
    }
    let final_probability_zero = single.final_state.basis_probability(0)?;
    if (final_probability_zero - 1.0).abs() > 1e-10 {
        return Err("feed-forward or reset failed".into());
    }

    let batch = circuit.execute_shots(512, seed, DEFAULT_MEMORY_BUDGET)?;
    let measured_true = batch
        .shots
        .iter()
        .filter(|shot| shot.bits[0] == Some(true))
        .count();

    println!(
        "SIMULATED_ONLY schema={} profile={} engine={} seed={} shots={} measured_true={} measured_false={} single_measurement={} final_p00={:.12}",
        CIRCUIT_SCHEMA_VERSION,
        CIRCUIT_PROFILE,
        batch.metadata.engine_version,
        seed,
        batch.metadata.shots,
        measured_true,
        batch.metadata.shots - measured_true,
        measured,
        final_probability_zero
    );
    Ok(())
}
