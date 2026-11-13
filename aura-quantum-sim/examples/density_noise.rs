//! Q28-007 runnable noisy simulation: real density matrix API, no QPU calls.
//! cargo run --manifest-path aura-quantum-sim/Cargo.toml --example density_noise
use aura_quantum_sim::density::{DensityMatrix, NoiseChannel, DENSITY_PROFILE};
use aura_quantum_sim::{Gate, DEFAULT_MEMORY_BUDGET};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A Hadamard-prepared |+> loses coherence under a non-unitary channel.
    let mut rho=DensityMatrix::zero(1,DEFAULT_MEMORY_BUDGET)?;
    rho.apply_gate(Gate::H(0))?;
    let original_purity=rho.purity()?;
    rho.apply_noise(NoiseChannel::AmplitudeDamping{qubit:0,gamma:0.36})?;
    rho.apply_noise(NoiseChannel::PhaseFlip{qubit:0,probability:0.25})?;
    let p0=rho.basis_probability(0)?;
    let p1=rho.basis_probability(1)?;
    let coherence=rho.element(0,1)?;
    let mixed_purity=rho.purity()?;
    if (p0-0.68).abs()>1e-10 ||
       (p1-0.32).abs()>1e-10 ||
       (coherence.re-0.20).abs()>1e-10 ||
       (mixed_purity-0.6448).abs()>1e-10 ||
       original_purity<=mixed_purity {
        return Err("Kraus analytical oracle disagrees with density reference".into());
    }
    println!(
        "Q28_DENSITY_MODEL_PASS profile={DENSITY_PROFILE} qubits=1 \
         gamma=0.36 phase_flip=0.25 p0={p0:.8} p1={p1:.8} \
         coherence_re={:.8} purity={mixed_purity:.8} trace={:.8} \
         evidence_class=simulation qpu_hardware_jobs=0",
        coherence.re, rho.trace()
    );
    Ok(())
}
