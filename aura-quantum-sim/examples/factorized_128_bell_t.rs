//! Real app: 128 qubits as 64 non-Clifford-independent Bell factors.
//! Full dense 128-qubit statevector would need 16 * 2^128 bytes.
use aura_quantum_sim::factorized::{FactorState,MAX_FACTOR_QUBITS};
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};

fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut state=FactorState::zero(128,DEFAULT_MEMORY_BUDGET)?;
    for a in (0..128).step_by(2) {
        state.apply(Gate::H(a))?;
        state.apply(Gate::Cnot{control:a,target:a+1})?;
        state.apply(Gate::T(a+1))?;
    }
    let expected=2f64.powi(-64);
    let zero=state.basis_probability(&[false;128])?;
    let one=state.basis_probability(&[true;128])?;
    if state.factor_count()!=64 ||
        state.logical_state_bytes()!=4096 ||
        (zero-expected).abs()>1e-25 ||
        (one-expected).abs()>1e-25 {
        return Err("factor-product Bell/T invariant broken".into());
    }
    println!(
        "Q28_FACTOR_128_PASS qubits=128 factors=64 local_state_bytes={} \
         max_factor_width={} max_merged_component={} \
         tensor_truncation=none basis_P_all_zero={zero:.17e} qpu_hardware_jobs=0",
        state.logical_state_bytes(),MAX_FACTOR_QUBITS,
        state.factor_widths().into_iter().max().unwrap_or(0),
    );
    Ok(())
}
