//! C-026 real noisy Hamiltonian energy and variance, no shot estimator.
use aura_quantum_sim::noisy_hamiltonian::analyze_noisy_hamiltonian;
use aura_quantum_sim::noisy_circuit::{NoisyCircuit,NoisyInstruction};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::factorized::PauliTerm;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut q=NoisyCircuit::new(2,0);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    q.push(NoisyInstruction::Noise(NoiseChannel::AmplitudeDamping{
        qubit:1,gamma:0.2,
    }));
    let terms=[
        PauliTerm{paulis:"XX".into(),coefficient:0.7},
        PauliTerm{paulis:"ZZ".into(),coefficient:-0.3},
        PauliTerm{paulis:"YY".into(),coefficient:0.15},
    ];
    let m=analyze_noisy_hamiltonian(&q,&terms,None,DEFAULT_MEMORY_BUDGET)?;
    println!(
        "Q28_C026_NOISY_HAMILTONIAN_PASS mean={:.12} second={:.12} variance={:.12} terms={} qpu_jobs=0",
        m.weighted_sum,m.weighted_second_moment,
        m.weighted_variance,m.terms.len()
    );
    Ok(())
}
