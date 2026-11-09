//! C-023 exact-in-model unconditioned noisy feedback ensemble, no shots.
use aura_quantum_sim::noisy_ensemble::execute_noisy_ensemble;
use aura_quantum_sim::noisy_circuit::{NoisyAction,NoisyCircuit,NoisyInstruction};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut c=NoisyCircuit::new(2,1);
    for op in [
        NoisyInstruction::Gate(Gate::H(0)),
        NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}),
        NoisyInstruction::Noise(NoiseChannel::PhaseDamping{qubit:1,gamma:0.64}),
        NoisyInstruction::Measure{qubit:0,into_bit:0},
        NoisyInstruction::If{bit:0,equals:true,
            action:NoisyAction::Noise(NoiseChannel::Depolarizing{
                qubit:1,probability:0.6,
            }),
        },
        NoisyInstruction::Reset{qubit:0},
    ]{c.push(op)}
    let r=execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET)?;
    println!("Q28_NOISY_ENSEMBLE_PASS branches={} trace={:.12} purity={:.12} p0={:.12} qpu=0",
        r.final_branch_count,r.trace,r.purity,
        r.classical_probabilities.get("0").copied().unwrap_or(0.0));
    Ok(())
}
