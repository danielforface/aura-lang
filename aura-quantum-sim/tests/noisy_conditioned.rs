//! C-024 real noise+measurement final-register postselection (not shots).
use aura_quantum_sim::noisy_ensemble::{
    execute_noisy_conditioned_ensemble,execute_noisy_ensemble,
    EnsembleError,NoisyCondition,
};
use aura_quantum_sim::noisy_circuit::{NoisyAction,NoisyCircuit,NoisyInstruction};
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::{Gate,Complex,DEFAULT_MEMORY_BUDGET};
fn c(bit:usize,equals:bool)->NoisyCondition{NoisyCondition{bit,equals}}
fn close(x:f64,y:f64){assert!((x-y).abs()<1e-9,"{x:.14e} vs {y:.14e}")}
fn rho(a:&[Complex],dim:usize,row:usize,col:usize)->Complex{a[row*dim+col]}
#[test]
fn bell_measurement_born_postselection_preserves_selected_pure_state(){
    let mut q=NoisyCircuit::new(2,1);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    let avg=execute_noisy_ensemble(&q,DEFAULT_MEMORY_BUDGET).unwrap();
    close(avg.purity,0.5);
    for (flag,basis) in [(false,0),(true,3)]{
        let r=execute_noisy_conditioned_ensemble(
            &q,&[c(0,flag)],DEFAULT_MEMORY_BUDGET).unwrap();
        close(r.condition_probability,0.5);
        close(r.purity,1.0);
        close(rho(&r.final_density,4,basis,basis).re,1.0);
        assert_eq!(r.selected_branch_count,1);
        assert_eq!(r.conditions,[c(0,flag)]);
    }
}
#[test]
fn noise_before_measurement_is_conditioned_not_assumed_pure(){
    let mut q=NoisyCircuit::new(2,1);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    q.push(NoisyInstruction::Noise(NoiseChannel::AmplitudeDamping{
        qubit:1,gamma:0.5,
    }));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    let yes=execute_noisy_conditioned_ensemble(
        &q,&[c(0,true)],DEFAULT_MEMORY_BUDGET).unwrap();
    close(yes.condition_probability,0.5);
    close(yes.purity,0.5);
    close(rho(&yes.final_density,4,1,1).re,0.5);
    close(rho(&yes.final_density,4,3,3).re,0.5);
    let no=execute_noisy_conditioned_ensemble(
        &q,&[c(0,false)],DEFAULT_MEMORY_BUDGET).unwrap();
    close(no.purity,1.0);
    close(rho(&no.final_density,4,0,0).re,1.0);
}
#[test]
fn adaptive_noisy_feedforward_and_hidden_reset_keep_correct_selected_rho(){
    let mut q=NoisyCircuit::new(2,1);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::H(1)));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    q.push(NoisyInstruction::If{
        bit:0,equals:true,
        action:NoisyAction::Noise(NoiseChannel::AmplitudeDamping{
            qubit:1,gamma:1.0,
        }),
    });
    q.push(NoisyInstruction::Reset{qubit:0});
    let yes=execute_noisy_conditioned_ensemble(
        &q,&[c(0,true)],DEFAULT_MEMORY_BUDGET).unwrap();
    close(yes.condition_probability,0.5);
    close(yes.purity,1.0);
    close(rho(&yes.final_density,4,0,0).re,1.0);
    let no=execute_noisy_conditioned_ensemble(
        &q,&[c(0,false)],DEFAULT_MEMORY_BUDGET).unwrap();
    close(no.purity,1.0);
    close(rho(&no.final_density,4,0,2).re,0.5);
}
#[test]
fn nonuniform_born_hth_with_postmeasurement_depolarizer(){
    let mut q=NoisyCircuit::new(1,1);
    for g in [Gate::H(0),Gate::T(0),Gate::H(0)]{
        q.push(NoisyInstruction::Gate(g));
    }
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    q.push(NoisyInstruction::Noise(NoiseChannel::Depolarizing{
        qubit:0,probability:0.75,
    }));
    let p0=(1.0+std::f64::consts::FRAC_1_SQRT_2)/2.0;
    for (flag,p) in [(false,p0),(true,1.0-p0)]{
        let x=execute_noisy_conditioned_ensemble(
            &q,&[c(0,flag)],DEFAULT_MEMORY_BUDGET).unwrap();
        close(x.condition_probability,p);
        close(x.purity,0.5);
        close(rho(&x.final_density,2,0,0).re,0.5);
        close(rho(&x.final_density,2,1,1).re,0.5);
    }
}
#[test]
fn partial_final_register_condition_leaves_other_bit_mixed(){
    let mut q=NoisyCircuit::new(2,2);
    for wire in 0..2{
        q.push(NoisyInstruction::Gate(Gate::H(wire)));
        q.push(NoisyInstruction::Measure{qubit:wire,into_bit:wire});
    }
    let r=execute_noisy_conditioned_ensemble(
        &q,&[c(0,true)],DEFAULT_MEMORY_BUDGET).unwrap();
    close(r.condition_probability,0.5);
    assert_eq!(r.selected_branch_count,2);
    close(r.purity,0.5);
    close(rho(&r.final_density,4,1,1).re,0.5);
    close(rho(&r.final_density,4,3,3).re,0.5);
    let both=execute_noisy_conditioned_ensemble(
        &q,&[c(0,true),c(1,false)],DEFAULT_MEMORY_BUDGET).unwrap();
    close(both.condition_probability,0.25);
    close(both.purity,1.0);
    close(rho(&both.final_density,4,1,1).re,1.0);
}
#[test]
fn overwritten_final_bit_not_first_instrument_result_is_selected(){
    let mut q=NoisyCircuit::new(1,1);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    q.push(NoisyInstruction::Gate(Gate::X(0)));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    let r=execute_noisy_conditioned_ensemble(
        &q,&[c(0,true)],DEFAULT_MEMORY_BUDGET).unwrap();
    close(r.condition_probability,0.5);
    close(rho(&r.final_density,2,1,1).re,1.0);
    assert_eq!(r.selected_branch_count,1);
    assert_eq!(r.measurement_count,2);
}
#[test]
fn full_sixty_four_branch_profile_postselects_exactly_half(){
    let mut q=NoisyCircuit::new(1,1);
    for _ in 0..6{
        q.push(NoisyInstruction::Gate(Gate::H(0)));
        q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    }
    let r=execute_noisy_conditioned_ensemble(
        &q,&[c(0,true)],DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(r.selected_branch_count,32);
    close(r.condition_probability,0.5);
    close(r.purity,1.0);
    close(rho(&r.final_density,2,1,1).re,1.0);
}
#[test]
fn malformed_duplicate_reversed_missing_reset_only_and_out_of_range_bits_fail(){
    let mut q=NoisyCircuit::new(1,2);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    for list in [
        vec![],vec![c(0,false),c(0,true)],
        vec![c(1,true)],vec![c(2,true)]
    ]{
        assert_eq!(execute_noisy_conditioned_ensemble(
            &q,&list,DEFAULT_MEMORY_BUDGET).unwrap_err(),
            EnsembleError::InvalidCondition);
    }
    let mut q=NoisyCircuit::new(1,2);
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:1});
    assert_eq!(execute_noisy_conditioned_ensemble(
        &q,&[c(1,false),c(0,false)],DEFAULT_MEMORY_BUDGET).unwrap_err(),
        EnsembleError::InvalidCondition);
    let mut q=NoisyCircuit::new(1,1);
    q.push(NoisyInstruction::Reset{qubit:0});
    assert_eq!(execute_noisy_conditioned_ensemble(
        &q,&[c(0,true)],DEFAULT_MEMORY_BUDGET).unwrap_err(),
        EnsembleError::InvalidCondition);
}
#[test]
fn impossible_event_and_rare_branch_fail_closed(){
    let mut q=NoisyCircuit::new(1,1);
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    assert_eq!(execute_noisy_conditioned_ensemble(
        &q,&[c(0,true)],DEFAULT_MEMORY_BUDGET).unwrap_err(),
        EnsembleError::ImpossibleCondition);
    let mut q=NoisyCircuit::new(1,1);
    q.push(NoisyInstruction::Gate(Gate::X(0)));
    q.push(NoisyInstruction::Noise(NoiseChannel::AmplitudeDamping{
        qubit:0,gamma:1e-14,
    }));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    assert_eq!(execute_noisy_conditioned_ensemble(
        &q,&[c(0,true)],DEFAULT_MEMORY_BUDGET).unwrap_err(),
        EnsembleError::BranchResolution);
}
#[test]
fn conditional_memory_charge_is_additional_and_checked_before_work(){
    let mut q=NoisyCircuit::new(1,1);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    let base=execute_noisy_ensemble(&q,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(execute_noisy_conditioned_ensemble(
        &q,&[c(0,true)],base.reserved_peak_logical_bytes).unwrap_err(),
        EnsembleError::BudgetExceeded);
}
#[test]
fn unreachable_invalid_kraus_in_if_rejected_by_common_preflight(){
    let mut q=NoisyCircuit::new(1,1);
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    q.push(NoisyInstruction::If{
        bit:0,equals:true,
        action:NoisyAction::Noise(NoiseChannel::PauliMixture{
            qubit:0,p_x:0.6,p_y:0.6,p_z:0.0,
        }),
    });
    let x=execute_noisy_conditioned_ensemble(
        &q,&[c(0,false)],DEFAULT_MEMORY_BUDGET).unwrap_err();
    assert!(format!("{x:?}").contains("InvalidProbability"));
}
