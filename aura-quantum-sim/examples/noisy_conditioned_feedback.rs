//! C-024 real noisy channel with final measured-bit postselection.
use aura_quantum_sim::noisy_ensemble::{
    execute_noisy_conditioned_ensemble,NoisyCondition,
};
use aura_quantum_sim::noisy_circuit::{
    NoisyAction,NoisyCircuit,NoisyInstruction,
};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut q=NoisyCircuit::new(2,1);
    for op in [
        NoisyInstruction::Gate(Gate::H(0)),
        NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}),
        NoisyInstruction::Noise(NoiseChannel::AmplitudeDamping{
            qubit:1,gamma:0.4,
        }),
        NoisyInstruction::Measure{qubit:0,into_bit:0},
        NoisyInstruction::If{bit:0,equals:true,
            action:NoisyAction::Noise(NoiseChannel::Depolarizing{
                qubit:1,probability:0.6,
            }),
        },
    ]{q.push(op);}
    let r=execute_noisy_conditioned_ensemble(
        &q,&[NoisyCondition{bit:0,equals:true}],
        DEFAULT_MEMORY_BUDGET)?;
    println!(
        "Q28_NOISY_CONDITIONED_PASS event_probability={:.12} selected_histories={} purity={:.12} trace={:.12} qpu=0",
        r.condition_probability,r.selected_branch_count,r.purity,r.trace);
    Ok(())
}
