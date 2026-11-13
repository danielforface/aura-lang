//! Real 3-qubit channel tomography after Bell measurement/feedback/reset.
use aura_quantum_sim::channel_tomography::execute_channel_tomography;
use aura_quantum_sim::circuit::{Circuit,Instruction};
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
    let result=execute_channel_tomography(
        &c,&[[0,1],[0,2]],DEFAULT_MEMORY_BUDGET)?;
    println!("Q28_CHANNEL_TOMOGRAPHY_PASS wires={} pairs={} histories={} mass={:.12} measured_purity={:.12} feedback_purity={:.12} qpu_jobs=0",
        result.wires.len(),result.pairs.len(),result.branch_count,
        result.probability_mass,result.wires[0].purity,
        result.wires[1].purity);
    Ok(())
}
