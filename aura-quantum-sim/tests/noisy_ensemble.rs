//! C-023 true unconditional noisy dynamic ensemble; not seeded shots.
use aura_quantum_sim::noisy_ensemble::{
    execute_noisy_ensemble,EnsembleError,MAX_ENSEMBLE_BRANCHES,
};
use aura_quantum_sim::noisy_circuit::{NoisyAction,NoisyCircuit,NoisyInstruction};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::{Complex,Gate,DEFAULT_MEMORY_BUDGET};
fn near(x:f64,y:f64){assert!((x-y).abs()<1e-9,"{x:.14e} vs {y:.14e}")}
fn at(v:&[Complex],dim:usize,r:usize,c:usize)->Complex{v[r*dim+c]}
#[test]
fn bell_observed_z_dephases_pair_unconditionally_but_keeps_true_born_histogram(){
    let mut c=NoisyCircuit::new(2,1);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    let o=execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(o.final_branch_count,2);
    near(o.classical_probabilities["0"],0.5);
    near(o.classical_probabilities["1"],0.5);
    near(o.purity,0.5);
    near(at(&o.final_density,4,0,3).norm_squared(),0.0);
    near(at(&o.final_density,4,0,0).re,0.5);
    near(at(&o.final_density,4,3,3).re,0.5);
}
#[test]
fn no_measurements_keeps_bell_pure_and_accepts_pauli_noise(){
    let mut c=NoisyCircuit::new(2,0);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    let a=execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET).unwrap();
    near(a.purity,1.0);
    near(at(&a.final_density,4,0,3).re,0.5);
    c.push(NoisyInstruction::Noise(NoiseChannel::Depolarizing{
        qubit:0,probability:0.75,
    }));
    let b=execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET).unwrap();
    near(b.purity,0.25);
    for i in 0..4{near(at(&b.final_density,4,i,i).re,0.25);}
    near(b.classical_probabilities[""],1.0);
}
#[test]
fn adaptive_noise_mixes_only_true_measurement_branch(){
    let mut c=NoisyCircuit::new(2,1);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Gate(Gate::H(1)));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    c.push(NoisyInstruction::If{bit:0,equals:true,action:NoisyAction::Noise(
        NoiseChannel::AmplitudeDamping{qubit:1,gamma:1.0},
    )});
    let r=execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(r.final_branch_count,2);
    near(r.classical_probabilities["0"],0.5);
    near(r.classical_probabilities["1"],0.5);
    // q0=0 branch |+> q1; q0=1 branch |0> q1
    near(at(&r.final_density,4,0,2).re,0.25);
    near(at(&r.final_density,4,1,3).re,0.0);
    near(at(&r.final_density,4,1,1).re,0.5);
    near(at(&r.final_density,4,3,3).re,0.0);
}
#[test]
fn unobserved_reset_is_one_cptp_branch_without_any_classical_result(){
    let mut c=NoisyCircuit::new(2,1);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    c.push(NoisyInstruction::Reset{qubit:0});
    let r=execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(r.final_branch_count,1);
    near(r.purity,0.5);
    near(r.classical_probabilities["?"],1.0);
    near(at(&r.final_density,4,0,0).re,0.5);
    near(at(&r.final_density,4,2,2).re,0.5);
}
#[test]
fn repeated_measurements_overwrite_final_classical_bits(){
    let mut c=NoisyCircuit::new(1,1);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    let r=execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(r.final_branch_count,4);
    near(r.classical_probabilities["0"],0.5);
    near(r.classical_probabilities["1"],0.5);
    near(r.purity,0.5);
}
#[test]
fn full_bounded_six_z_measurements_require_exact_64_histories(){
    let mut c=NoisyCircuit::new(1,1);
    for _ in 0..6{
        c.push(NoisyInstruction::Gate(Gate::H(0)));
        c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    }
    let r=execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(r.final_branch_count,MAX_ENSEMBLE_BRANCHES);
    near(r.classical_probabilities["0"],0.5);
    near(r.classical_probabilities["1"],0.5);
    near(r.probability_mass,1.0);
}
#[test]
fn impossible_excess_measures_and_observed_reset_fail_explicitly(){
    let mut c=NoisyCircuit::new(1,1);
    for _ in 0..7{c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});}
    assert_eq!(execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),EnsembleError::InvalidMeasurements);
    let mut c=NoisyCircuit::new(1,1);
    c.push(NoisyInstruction::ResetObserved{qubit:0});
    assert_eq!(execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),EnsembleError::UnsupportedResetObserved);
}
#[test]
fn unreachable_invalid_classical_noise_is_rejected_by_existing_preflight(){
    let mut c=NoisyCircuit::new(1,1);
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    c.push(NoisyInstruction::If{bit:0,equals:true,action:NoisyAction::Noise(
        NoiseChannel::PauliMixture{
            qubit:0,p_x:0.6,p_y:0.6,p_z:0.0,
        },
    )});
    assert!(format!("{:?}",execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET)
        .unwrap_err()).contains("InvalidProbability"));
}
#[test]
fn insufficient_budget_and_width_classical_bounds_fail_before_simulation(){
    let c=NoisyCircuit::new(4,0);
    assert_eq!(execute_noisy_ensemble(&c,100).unwrap_err(),
        EnsembleError::BudgetExceeded);
    let c=NoisyCircuit::new(5,0);
    assert_eq!(execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        EnsembleError::InvalidQubits);
    let c=NoisyCircuit::new(1,9);
    assert_eq!(execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        EnsembleError::InvalidClassicalBits);
}
#[test]
fn rare_but_nonzero_branch_rejected_instead_of_biased_distribution(){
    let mut c=NoisyCircuit::new(1,1);
    c.push(NoisyInstruction::Gate(Gate::X(0)));
    c.push(NoisyInstruction::Noise(NoiseChannel::AmplitudeDamping{
        qubit:0,gamma:1e-14,
    }));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    assert_eq!(execute_noisy_ensemble(&c,DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),EnsembleError::BranchResolution);
}
