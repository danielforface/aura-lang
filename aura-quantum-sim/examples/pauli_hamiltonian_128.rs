//! Real VQE-style analytic Pauli Hamiltonian energy on a 128-wire
//! untruncated product of 64 independently entangled Bell+T pairs.
use aura_quantum_sim::factorized::{FactorCircuit,PauliTerm};
use aura_quantum_sim::circuit::Instruction;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn word(symbols:&[(usize,u8)])->String{
    let mut word=vec![b'I';128];
    for &(q,c) in symbols{word[q]=c;}
    String::from_utf8(word).expect("ASCII Pauli word")
}
fn main()->Result<(),Box<dyn std::error::Error>>{
    let mut circuit=FactorCircuit::new(128,0);
    for q in (0..128).step_by(2) {
        circuit.push(Instruction::Gate(Gate::H(q)));
        circuit.push(Instruction::Gate(Gate::Cnot{control:q,target:q+1}));
        circuit.push(Instruction::Gate(Gate::T(q+1)));
    }
    let terms=vec![
        PauliTerm{paulis:word(&[(0,b'Z'),(1,b'Z')]),coefficient:1.0},
        PauliTerm{paulis:word(&[(0,b'X'),(1,b'X')]),coefficient:0.5},
        PauliTerm{paulis:word(&[(0,b'Y'),(1,b'Y')]),coefficient:-0.5},
        PauliTerm{paulis:"Z".repeat(128),coefficient:0.25},
        PauliTerm{paulis:"I".repeat(128),coefficient:0.0},
    ];
    let result=circuit.execute_observables(&terms,20261108,DEFAULT_MEMORY_BUDGET)?;
    let want=1.25+std::f64::consts::FRAC_1_SQRT_2;
    if circuit.instructions().len()!=192||result.estimates.len()!=5||
        (result.weighted_sum-want).abs()>1e-10||
        result.peak_logical_state_bytes>=8192||
        result.metadata.qpu_hardware_jobs!=0 {
        return Err("Pauli expectation 128q energy regression".into());
    }
    println!("Q28_PAULI_HAMILTONIAN_128_PASS qubits=128 bell_pairs=64 T_gates=64 \
        terms=5 weighted_energy={:.16e} peak_state_bytes={} \
        qpu_hardware_jobs=0",result.weighted_sum,
        result.peak_logical_state_bytes);
    Ok(())
}
