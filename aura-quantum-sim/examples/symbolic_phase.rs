//! Real Q28-010 exact Clifford+T phase and measurement oracle consumer.
//! cargo run --manifest-path aura-quantum-sim/Cargo.toml --example symbolic_phase
use aura_quantum_sim::symbolic::{
    ExactProbability,Root2,SymbolicCircuit,SymbolicInstruction,SymbolicState,
};
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};

fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut state=SymbolicState::zero(1,DEFAULT_MEMORY_BUDGET)?;
    for gate in [Gate::H(0),Gate::T(0),Gate::H(0)] {
        state.apply(gate)?;
    }
    let bright=state.basis_weight(0)?;
    let dark=state.basis_weight(1)?;
    if bright!=ExactProbability{
        numerator:Root2::new(2,1),denominator_power_of_two:2
    } || dark!=ExactProbability{
        numerator:Root2::new(2,-1),denominator_power_of_two:2
    } {
        return Err("exact HTH interference identity failed".into());
    }
    let mut circuit=SymbolicCircuit::new(1,1);
    circuit.push(SymbolicInstruction::Gate(Gate::H(0)));
    circuit.push(SymbolicInstruction::Gate(Gate::T(0)));
    circuit.push(SymbolicInstruction::Gate(Gate::H(0)));
    circuit.push(SymbolicInstruction::Measure{
        qubit:0,into_bit:0,outcome:true
    });
    circuit.push(SymbolicInstruction::If{
        bit:0,equals:true,gate:Gate::X(0)
    });
    let run=circuit.execute(DEFAULT_MEMORY_BUDGET)?;
    if run.trace.len()!=1||run.trace[0].after_mass!=dark||
       run.final_state.total_mass()!=Ok(dark)||
       run.classical_bits!=vec![Some(true)] {
        return Err("exact conditional symbolic postselection failed".into());
    }
    println!(
       "Q28_SYMBOLIC_PHASE_PASS profile={} exact_P0=({}+{}sqrt2)/2^{} \
        exact_P1=({}+{}sqrt2)/2^{} selected_branch_mass=({}+{}sqrt2)/2^{} \
        evidence_class={} qpu_hardware_jobs={}",
       run.metadata.profile,
       bright.numerator.rational,bright.numerator.radical,bright.denominator_power_of_two,
       dark.numerator.rational,dark.numerator.radical,dark.denominator_power_of_two,
       dark.numerator.rational,dark.numerator.radical,dark.denominator_power_of_two,
       run.metadata.evidence_class,run.metadata.qpu_hardware_jobs
    );
    Ok(())
}
