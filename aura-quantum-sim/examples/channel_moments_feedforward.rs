//! Real dynamic Bell+Reset unconditioned ideal-channel moments.
use aura_quantum_sim::channel_moments::execute_channel_moments;
use aura_quantum_sim::circuit::{Circuit,Instruction};
use aura_quantum_sim::factorized::PauliTerm;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut c=Circuit::new(3,1);
    for op in [
        Instruction::Gate(Gate::H(0)),
        Instruction::Gate(Gate::Cnot{control:0,target:1}),
        Instruction::Measure{qubit:0,into_bit:0},
        Instruction::If{bit:0,equals:true,gate:Gate::X(1)},
        Instruction::Gate(Gate::H(2)),
        Instruction::Reset{qubit:2},
    ]{c.push(op);}
    let m=execute_channel_moments(
        &c,&[PauliTerm{paulis:"ZII".into(),coefficient:1.0}],
        DEFAULT_MEMORY_BUDGET,
    )?;
    println!("Q28_CHANNEL_ENSEMBLE_PASS branches={} instruments={} mass={:.12} mean={:.12} variance={:.12} within={:.12} between={:.12} qpu_jobs=0",
        m.branch_count,m.instrument_count,m.probability_mass,m.mean,
        m.variance,m.within_branch_variance,m.between_branch_variance);
    Ok(())
}
