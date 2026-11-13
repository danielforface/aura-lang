//! Real 65,536-shot CPU-only histogram sampler with no retained traces.
use aura_quantum_sim::factorized::{FactorCircuit,MAX_FACTOR_COUNT_SHOTS};
use aura_quantum_sim::circuit::Instruction;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut p=FactorCircuit::new(1,1);
    p.push(Instruction::Gate(Gate::H(0)));
    p.push(Instruction::Measure{qubit:0,into_bit:0});
    let batch=p.execute_counts(MAX_FACTOR_COUNT_SHOTS,20261108,DEFAULT_MEMORY_BUDGET)?;
    let ones=batch.histogram.get("1").copied().unwrap_or(0);
    let zeros=batch.histogram.get("0").copied().unwrap_or(0);
    if zeros+ones!=65536 || ones.abs_diff(32768)>800 ||
        batch.retained_shot_records!=0 || batch.histogram.len()!=2 ||
        batch.metadata.qpu_hardware_jobs!=0 {
        return Err("65k shot counts contract failed".into());
    }
    println!("Q28_COUNTS_65536_PASS shots=65536 zeros={zeros} ones={ones} \
       retained_records=0 histogram_bytes={} peak_state_bytes={} qpu_jobs=0",
       batch.logical_histogram_reserved_bytes,batch.peak_logical_state_bytes);
    Ok(())
}
