//! Executable 96-qubit GHZ tableau + adaptive Clifford feedback.
use aura_quantum_sim::stabilizer::{
    StabilizerCircuit,StabilizerInstruction,StabilizerTableau,
};
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};

fn main()->Result<(),Box<dyn std::error::Error>> {
    let n=96;
    let mut state=StabilizerTableau::zero(n,DEFAULT_MEMORY_BUDGET)?;
    state.apply(Gate::H(0))?;
    for q in 1..n {
        state.apply(Gate::Cnot{control:q-1,target:q})?;
    }
    let p0=state.basis_probability(&vec![false;n])?;
    let p1=state.basis_probability(&vec![true;n])?;
    if (p0-0.5).abs()>1e-12||(p1-0.5).abs()>1e-12||
        state.logical_bytes()>64*1024 {
        return Err("96-qubit stabilizer GHZ exact parity/memory check failed".into());
    }
    let mut circuit=StabilizerCircuit::new(2,2);
    circuit.push(StabilizerInstruction::Gate(Gate::H(0)));
    circuit.push(StabilizerInstruction::Gate(Gate::Cnot{control:0,target:1}));
    circuit.push(StabilizerInstruction::Measure{qubit:0,into_bit:0});
    circuit.push(StabilizerInstruction::If{
        bit:0,equals:true,gate:Gate::X(1),
    });
    circuit.push(StabilizerInstruction::Reset{qubit:0});
    circuit.push(StabilizerInstruction::Measure{qubit:1,into_bit:1});
    let batch=circuit.execute_shots(1024,0xc009,DEFAULT_MEMORY_BUDGET)?;
    let ones=batch.records.iter().filter(|s|s.bits[0]==Some(true)).count();
    if batch.records.iter().any(|s|s.bits[1]!=Some(false))||
        !(400..=624).contains(&ones) {
        return Err("seeded adaptive Clifford Bell acceptance failed".into());
    }
    println!(
        "Q28_STABILIZER_DELIVERY_PASS qubits={} memory_bytes={} GHZ_p0={} \
         GHZ_p1={} bell_shots={} bell_ones={} profile={} \
         evidence_class={} qpu_hardware_jobs={}",
        n,state.logical_bytes(),p0,p1,batch.records.len(),ones,
        batch.metadata.profile,batch.metadata.evidence_class,
        batch.metadata.qpu_hardware_jobs
    );
    Ok(())
}
