//! A real postselected Bell + feedforward + hidden reset application.
use aura_quantum_sim::channel_tomography::{
    execute_conditioned_tomography,ClassicalCondition,
};
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
    let r=execute_conditioned_tomography(
        &c,&[[0,1]],&[ClassicalCondition{bit:0,equals:true}],
        DEFAULT_MEMORY_BUDGET)?;
    println!(
        "Q28_CONDITIONED_TOMOGRAPHY_PASS selected_probability={:.12} selected_histories={} q0_z={:.12} q1_z={:.12} pair_purity={:.12} qpu_jobs=0",
        r.condition_probability,r.selected_branch_count,r.wires[0].bloch[2],
        r.wires[1].bloch[2],r.pairs[0].purity);
    Ok(())
}
