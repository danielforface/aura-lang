//! C-025 real mixed-state NPT entanglement / classical correlation distinction.
use aura_quantum_sim::noisy_entanglement::analyze_noisy_pair;
use aura_quantum_sim::noisy_circuit::{NoisyCircuit,NoisyInstruction};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut q=NoisyCircuit::new(2,0);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    let bell=analyze_noisy_pair(&q,[0,1],None,DEFAULT_MEMORY_BUDGET)?;
    q.push(NoisyInstruction::Noise(NoiseChannel::PhaseDamping{
        qubit:0,gamma:1.0,
    }));
    let dephased=analyze_noisy_pair(&q,[0,1],None,DEFAULT_MEMORY_BUDGET)?;
    println!(
        "Q28_C025_ENTANGLEMENT_PASS bell_negativity={:.12} dephased_negativity={:.12} classical_mi_bits={:.12} qpu=0",
        bell.negativity,dephased.negativity,dephased.mutual_information_bits
    );
    Ok(())
}
