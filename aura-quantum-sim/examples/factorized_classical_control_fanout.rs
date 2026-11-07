//! C-014 real 128-wire non-entangling controlled fanout with phases.
//! Each cross-factor CX/CZ is algebraically proven product-preserving.
use aura_quantum_sim::circuit::Instruction;
use aura_quantum_sim::factorized::FactorCircuit;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut circuit=FactorCircuit::new(128,0);
    circuit.push(Instruction::Gate(Gate::X(0)));
    for q in 1..128 {
        circuit.push(Instruction::Gate(Gate::H(q)));
        circuit.push(Instruction::Gate(Gate::Cz{control:0,target:q}));
        circuit.push(Instruction::Gate(Gate::Cnot{control:0,target:q}));
        circuit.push(Instruction::Gate(Gate::H(q)));
    }
    for q in 1..=3 {circuit.push(Instruction::Gate(Gate::T(q)));}
    if circuit.instructions().len()!=512 {return Err("instruction accounting failure".into());}
    let shots=circuit.execute_shots(1,20261107,DEFAULT_MEMORY_BUDGET)?;
    let state=shots.one_shot_state.as_ref().ok_or("missing state")?;
    let p=state.basis_probability(&[true;128])?;
    if shots.product_preserving_two_wire_gates!=254 ||
       state.factor_count()!=128 || state.logical_state_bytes()!=4096 ||
       state.peak_logical_bytes()!=4096 || shots.projective_wire_splits!=0 ||
       (p-1.0).abs()>1e-10 || shots.metadata.qpu_hardware_jobs!=0 {
       return Err("controlled fanout product state invariant broken".into());
    }
    println!(
        "Q28_PRODUCT_CONTROL_128_PASS qubits=128 instructions=512 \
         non_entangling_cross_gates={} factors={} state_bytes={} \
         peak_logical_bytes={} basis_probability={p:.16e} qpu_hardware_jobs=0",
        shots.product_preserving_two_wire_gates,state.factor_count(),
        state.logical_state_bytes(),shots.peak_logical_state_bytes,
    );
    Ok(())
}
