//! C-026 real mixed-state Pauli Hamiltonian moments, NOT sampled shots.
use aura_quantum_sim::noisy_hamiltonian::{
    analyze_noisy_hamiltonian,NoisyHamiltonianError,
};
use aura_quantum_sim::noisy_circuit::{NoisyAction,NoisyCircuit,NoisyInstruction};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::factorized::PauliTerm;
use aura_quantum_sim::noisy_ensemble::NoisyCondition;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn t(word:&str,c:f64)->PauliTerm{
    PauliTerm{paulis:word.into(),coefficient:c}
}
fn near(a:f64,b:f64){
    assert!((a-b).abs()<2e-9,"actual {a:.16e} expected {b:.16e}");
}
fn zero(n:usize,c:usize)->NoisyCircuit{NoisyCircuit::new(n,c)}
#[test]
fn identity_energy_deterministic_variance_zero(){
    let q=zero(1,0);
    let m=analyze_noisy_hamiltonian(
        &q,&[t("I",3.5)],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.weighted_sum,3.5);
    near(m.weighted_second_moment,12.25);
    near(m.weighted_variance,0.0);
    near(m.terms[0].expectation,1.0);
    assert_eq!(m.condition_probability,None);
}
#[test]
fn noncommuting_x_plus_z_has_quantum_cross_terms_cancel_in_h_square(){
    let q=zero(1,0);
    let m=analyze_noisy_hamiltonian(
        &q,&[t("X",1.0),t("Z",1.0)],
        None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.weighted_sum,1.0);
    near(m.weighted_second_moment,2.0);
    near(m.weighted_variance,1.0);
    near(m.terms[0].expectation,0.0);
    near(m.terms[1].expectation,1.0);
}
#[test]
fn complex_y_phase_convention_correct(){
    let mut q=zero(1,0);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::S(0)));
    let m=analyze_noisy_hamiltonian(
        &q,&[t("Y",1.0)],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.weighted_sum,1.0);
    near(m.weighted_second_moment,1.0);
    near(m.weighted_variance,0.0);
}
#[test]
fn bell_xx_and_zz_commute_and_pure_energy_two(){
    let mut q=zero(2,0);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    let m=analyze_noisy_hamiltonian(
        &q,&[t("XX",1.0),t("ZZ",1.0)],
        None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.weighted_sum,2.0);
    near(m.weighted_second_moment,4.0);
    near(m.weighted_variance,0.0);
    let yy=analyze_noisy_hamiltonian(
        &q,&[t("YY",1.0)],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(yy.weighted_sum,-1.0);
    q.push(NoisyInstruction::Noise(NoiseChannel::PhaseDamping{
        qubit:1,gamma:1.0,
    }));
    let changed=analyze_noisy_hamiltonian(
        &q,&[t("XX",1.0)],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(changed.weighted_sum,0.0);
    near(changed.weighted_second_moment,1.0);
    near(changed.weighted_variance,1.0);
}
#[test]
fn real_amplitude_damping_mixed_qubit_energy_and_variance(){
    let mut q=zero(1,0);
    q.push(NoisyInstruction::Gate(Gate::X(0)));
    q.push(NoisyInstruction::Noise(NoiseChannel::AmplitudeDamping{
        qubit:0,gamma:0.25,
    }));
    let m=analyze_noisy_hamiltonian(
        &q,&[t("Z",1.0)],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.weighted_sum,-0.5);
    near(m.weighted_second_moment,1.0);
    near(m.weighted_variance,0.75);
}
#[test]
fn noisy_selected_event_has_conditional_energy_not_average(){
    let mut q=zero(1,1);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    let all=analyze_noisy_hamiltonian(
        &q,&[t("Z",1.0)],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(all.weighted_sum,0.0);
    near(all.weighted_variance,1.0);
    for (value,mean) in [(false,1.0),(true,-1.0)]{
        let selected=[NoisyCondition{bit:0,equals:value}];
        let m=analyze_noisy_hamiltonian(
            &q,&[t("Z",1.0)],Some(&selected),DEFAULT_MEMORY_BUDGET).unwrap();
        near(m.condition_probability.unwrap(),0.5);
        near(m.weighted_sum,mean);
        near(m.weighted_variance,0.0);
        assert_eq!(m.selected_branch_count,Some(1));
        assert_eq!(m.conditions,Some(selected.to_vec()));
    }
}
#[test]
fn classical_feedback_changes_unconditional_hamiltonian(){
    let mut q=zero(1,1);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    q.push(NoisyInstruction::If{
        bit:0,equals:true,action:NoisyAction::Gate(Gate::X(0)),
    });
    let m=analyze_noisy_hamiltonian(
        &q,&[t("Z",2.0)],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.weighted_sum,2.0);
    near(m.weighted_second_moment,4.0);
    near(m.weighted_variance,0.0);
}
#[test]
fn full_noise_depolarizer_zeroes_x_but_retains_identity_constant(){
    let mut q=zero(1,0);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Noise(NoiseChannel::Depolarizing{
        qubit:0,probability:0.75,
    }));
    let m=analyze_noisy_hamiltonian(
        &q,&[t("X",0.6),t("I",0.2)],
        None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.weighted_sum,0.2);
    near(m.weighted_second_moment,0.4);
    near(m.weighted_variance,0.36);
}
#[test]
fn duplicate_terms_cancel_exact_hamiltonian_not_individual_measurements(){
    let mut q=zero(1,0);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    let m=analyze_noisy_hamiltonian(
        &q,&[t("X",1.0),t("X",-1.0)],
        None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.weighted_sum,0.0);
    near(m.weighted_second_moment,0.0);
    near(m.weighted_variance,0.0);
    near(m.terms[0].expectation,1.0);
    near(m.terms[1].expectation,1.0);
}
#[test]
fn sixty_four_bounded_terms_supported_and_resource_charged(){
    let q=zero(2,0);
    let terms=(0..64).map(|_|t("II",1.0)).collect::<Vec<_>>();
    let m=analyze_noisy_hamiltonian(
        &q,&terms,None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.weighted_sum,64.0);
    near(m.weighted_second_moment,4096.0);
    near(m.weighted_variance,0.0);
    assert!(m.charged_work_units>0);
    assert!(m.reserved_peak_logical_bytes>16384);
}
#[test]
fn invalid_pauli_count_shape_symbol_coefficient_and_budget_fail_closed(){
    let q=zero(1,0);
    for terms in [
        vec![],vec![t("I",f64::INFINITY)],
        vec![t("Z",f64::NAN)],vec![t("Z",10001.0)],
        vec![t("ZX",1.0)],vec![t("A",1.0)],
        vec![t("כ",1.0)],
        (0..65).map(|_|t("Z",1.0)).collect(),
    ]{
        assert!(analyze_noisy_hamiltonian(
            &q,&terms,None,DEFAULT_MEMORY_BUDGET).is_err());
    }
    assert_eq!(analyze_noisy_hamiltonian(
        &q,&[t("I",1.0)],None,1).unwrap_err(),
        NoisyHamiltonianError::BudgetExceeded);
}
#[test]
fn invalid_untaken_if_pauli_noise_and_impossible_condition_still_rejected(){
    let mut q=zero(1,1);
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    q.push(NoisyInstruction::If{
        bit:0,equals:true,
        action:NoisyAction::Noise(NoiseChannel::PauliMixture{
            qubit:0,p_x:0.6,p_y:0.6,p_z:0.0,
        }),
    });
    assert!(format!("{:?}",analyze_noisy_hamiltonian(
        &q,&[t("Z",1.0)],None,DEFAULT_MEMORY_BUDGET).unwrap_err())
        .contains("InvalidProbability"));
    let mut q=zero(1,1);
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    let selected=[NoisyCondition{bit:0,equals:true}];
    assert!(format!("{:?}",analyze_noisy_hamiltonian(
        &q,&[t("Z",1.0)],Some(&selected),DEFAULT_MEMORY_BUDGET).unwrap_err())
        .contains("ImpossibleCondition"));
}
