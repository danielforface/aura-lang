//! C-017 runnable 128-global-wire analytic Hamiltonian energy variance.
use aura_quantum_sim::factorized::{FactorState,PauliTerm};
use aura_quantum_sim::moments::evaluate_hamiltonian_moments;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut state=FactorState::zero(128,DEFAULT_MEMORY_BUDGET)?;
    for q in (0..128).step_by(2) {
        state.apply(Gate::H(q))?;
        state.apply(Gate::Cnot{control:q,target:q+1})?;
        state.apply(Gate::T(q+1))?;
    }
    let mut xx="I".repeat(128);
    xx.replace_range(0..2,"XX");
    let mut yy="I".repeat(128);
    yy.replace_range(0..2,"YY");
    let terms=[
        PauliTerm{paulis:xx,coefficient:0.5},
        PauliTerm{paulis:yy,coefficient:-0.5},
        PauliTerm{paulis:"Z".repeat(128),coefficient:1.0},
    ];
    let stats=evaluate_hamiltonian_moments(&state,&terms)?;
    println!("qubits={} factors={} mean={:.14} second={:.14} variance={:.14} logical_bytes={} qpu_jobs=0",
        state.qubits(),state.factor_count(),stats.mean,stats.second_moment,
        stats.variance,state.logical_state_bytes());
    Ok(())
}
