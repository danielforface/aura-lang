//! Real dynamic mixed-state application: noise + conditional corrections,
//! seeded observed instruments and nonselective reset from public Rust API.
//! cargo run --manifest-path aura-quantum-sim/Cargo.toml --example noisy_feedback
use aura_quantum_sim::noisy_circuit::{
    NoisyAction, NoisyCircuit, NoisyInstruction, NOISY_DYNAMIC_PROFILE,
};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::{Gate, DEFAULT_MEMORY_BUDGET};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut circuit = NoisyCircuit::new(2, 2);
    circuit.push(NoisyInstruction::Gate(Gate::H(0)));
    circuit.push(NoisyInstruction::Gate(Gate::Cnot { control: 0, target: 1 }));
    circuit.push(NoisyInstruction::Noise(NoiseChannel::AmplitudeDamping {
        qubit: 1, gamma: 0.2,
    }));
    circuit.push(NoisyInstruction::Measure { qubit: 0, into_bit: 0 });
    circuit.push(NoisyInstruction::If {
        bit: 0,
        equals: true,
        action: NoisyAction::Gate(Gate::X(1)),
    });
    circuit.push(NoisyInstruction::Reset { qubit: 0 });
    circuit.push(NoisyInstruction::Measure { qubit: 1, into_bit: 1 });

    // Ideal Kraus model: residual |1> on q1 after feed-forward is gamma/2.
    let single = circuit.execute_once(0xc008, DEFAULT_MEMORY_BUDGET)?;
    if single.metadata.profile != NOISY_DYNAMIC_PROFILE || single.shot.trace.len()!=3 {
        return Err("invalid dynamic mixed-state result/trace".into());
    }
    let shots = 1024_usize;
    let batch = circuit.execute_shots(shots, 0xc008, DEFAULT_MEMORY_BUDGET)?;
    let errors = batch.records.iter()
        .filter(|shot| shot.bits[1] == Some(true))
        .count();
    let expected = 0.2 / 2.0;
    let measured = errors as f64 / shots as f64;
    // Six-sigma bound for Bernoulli(p=.1) at n=1024, widened by 1e-9.
    let sigma = (expected * (1.0 - expected) / shots as f64).sqrt();
    if (measured-expected).abs() > 6.0*sigma + 1e-9 {
        return Err(format!(
            "unexpected residual excitation: measured={measured}, expected={expected}"
        ).into());
    }
    println!(
        "Q28_DYNAMIC_NOISY_PASS profile={} shots={} \
        actual_excited={} expected_excited_fraction={expected:.3} \
        observed_fraction={measured:.5} seed={} \
        evidence_class={} qpu_hardware_jobs={}",
        batch.metadata.profile, shots, errors, batch.metadata.seed,
        batch.metadata.evidence_class, batch.metadata.qpu_hardware_jobs
    );
    Ok(())
}
