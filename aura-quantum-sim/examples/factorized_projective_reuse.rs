//! C-013 actual CPU app: recycle one measured qubit through 127 Bell
//! interactions across 128 global wires, no rank truncation or QPU.
use aura_quantum_sim::circuit::{Instruction,TraceKind};
use aura_quantum_sim::factorized::FactorCircuit;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};

fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut circuit=FactorCircuit::new(128,1);
    for q in 1..128 {
        circuit.push(Instruction::Gate(Gate::H(0)));
        circuit.push(Instruction::Gate(Gate::Cnot{control:0,target:q}));
        circuit.push(Instruction::Measure{qubit:0,into_bit:0});
        circuit.push(Instruction::If{bit:0,equals:true,gate:Gate::X(0)});
    }
    for q in 1..=4 {circuit.push(Instruction::Gate(Gate::T(q)));}
    let result=circuit.execute_shots(1,20261107,DEFAULT_MEMORY_BUDGET)?;
    let state=result.one_shot_state.as_ref()
        .ok_or("missing requested single-shot final state")?;
    let mut expected=vec![false;128];
    for (index,event) in result.records[0].trace.iter().enumerate() {
        match event.kind {
            TraceKind::Measure{into_bit:0,value}=>{expected[index+1]=value;},
            _=>return Err("unexpected reset/missing projective event".into()),
        }
    }
    let fidelity=state.basis_probability(&expected)?;
    if circuit.instructions().len()!=512 ||
        result.records[0].trace.len()!=127 ||
        state.factor_count()!=128 ||
        result.projective_wire_splits!=127 ||
        result.projective_split_deferred!=0 ||
        state.logical_state_bytes()!=4096 ||
        (fidelity-1.0).abs()>1e-10 ||
        result.metadata.qpu_hardware_jobs!=0 {
        return Err("projective wire recycling 128q contract failed".into());
    }
    println!("Q28_PROJECTIVE_RECYCLE_128_PASS wires=128 reuses=127 instructions=512 \
        factors={} state_bytes={} peak_logical_bytes={} \
        measured_wire_splits={} deferred={} basis_probability={:.16e} \
        numerical_precision=f64-complex tensor_truncation=none qpu_hardware_jobs=0",
        state.factor_count(),state.logical_state_bytes(),
        result.peak_logical_state_bytes,
        result.projective_wire_splits,result.projective_split_deferred,
        fidelity);
    Ok(())
}
