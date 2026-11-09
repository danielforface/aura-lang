//! C-022 actual dynamic noisy circuit: Pauli mixture + dephasing + feedback.
use aura_quantum_sim::noisy_circuit::{NoisyCircuit,NoisyAction,NoisyInstruction};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut c=NoisyCircuit::new(2,1);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Gate(Gate::H(1)));
    c.push(NoisyInstruction::Noise(NoiseChannel::PhaseDamping{
        qubit:1,gamma:0.64,
    }));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    c.push(NoisyInstruction::If{bit:0,equals:true,
        action:NoisyAction::Noise(NoiseChannel::PauliMixture{
            qubit:1,p_x:0.2,p_y:0.1,p_z:0.3,
        })
    });
    c.push(NoisyInstruction::Noise(NoiseChannel::Depolarizing{
        qubit:1,probability:0.36,
    }));
    let r=c.execute_once(12345,DEFAULT_MEMORY_BUDGET)?;
    println!("Q28_C022_NOISY_CPTP_PASS qubits=2 classical={:?} trace={:.12} purity={:.12} qpu_jobs=0",
        r.shot.bits,r.final_density.trace(),r.final_density.purity()?);
    Ok(())
}
