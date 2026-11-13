//! C-018 real unconditioned ideal-channel moment acceptance.
use aura_quantum_sim::channel_moments::{
    execute_channel_moments,ChannelError,MAX_CHANNEL_INSTRUMENTS,
};
use aura_quantum_sim::circuit::{Circuit,Instruction};
use aura_quantum_sim::factorized::PauliTerm;
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn term(p:&str,c:f64)->PauliTerm{
    PauliTerm{paulis:p.into(),coefficient:c}
}
fn near(a:f64,b:f64){assert!((a-b).abs()<1e-9,"{a:.16e} vs {b:.16e}");}
#[test]
fn coherent_hadamard_without_instrument_is_quantum_variance(){
    let mut c=Circuit::new(1,0);
    c.push(Instruction::Gate(Gate::H(0)));
    let a=execute_channel_moments(&c,&[term("Z",1.0)],DEFAULT_MEMORY_BUDGET).unwrap();
    near(a.mean,0.0);near(a.variance,1.0);
    near(a.within_branch_variance,1.0);near(a.between_branch_variance,0.0);
    assert_eq!(a.branch_count,1);assert_eq!(a.instrument_count,0);
}
#[test]
fn ideal_measurement_moves_quantum_variance_to_between_branches(){
    let mut c=Circuit::new(1,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    let a=execute_channel_moments(&c,&[term("Z",1.0)],DEFAULT_MEMORY_BUDGET).unwrap();
    near(a.mean,0.0);near(a.second_moment,1.0);near(a.variance,1.0);
    near(a.within_branch_variance,0.0);near(a.between_branch_variance,1.0);
    near(*a.classical_probabilities.get("0").unwrap(),0.5);
    near(*a.classical_probabilities.get("1").unwrap(),0.5);
    assert_eq!(a.branch_count,2);
}
#[test]
fn reset_has_two_hidden_branches_but_rejoins_density_at_zero(){
    let mut c=Circuit::new(1,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Reset{qubit:0});
    let a=execute_channel_moments(&c,&[term("Z",1.0)],DEFAULT_MEMORY_BUDGET).unwrap();
    near(a.mean,1.0);near(a.variance,0.0);
    near(a.within_branch_variance,0.0);near(a.between_branch_variance,0.0);
    assert_eq!(a.branch_count,2);
    near(*a.classical_probabilities.get("?").unwrap(),1.0);
}
#[test]
fn bell_measure_conditional_x_and_unobserved_reset_keep_probability(){
    let mut c=Circuit::new(3,1);
    for op in [
        Instruction::Gate(Gate::H(0)),
        Instruction::Gate(Gate::Cnot{control:0,target:1}),
        Instruction::Measure{qubit:0,into_bit:0},
        Instruction::If{bit:0,equals:true,gate:Gate::X(1)},
        Instruction::Gate(Gate::H(2)),
        Instruction::Reset{qubit:2},
    ]{c.push(op);}
    let a=execute_channel_moments(&c,&[term("ZII",1.0)],DEFAULT_MEMORY_BUDGET).unwrap();
    near(a.mean,0.0);near(a.variance,1.0);
    near(a.within_branch_variance,0.0);near(a.between_branch_variance,1.0);
    near(a.probability_mass,1.0);
    assert_eq!(a.branch_count,4);assert_eq!(a.instrument_count,2);
    near(*a.classical_probabilities.get("0").unwrap(),0.5);
    near(*a.classical_probabilities.get("1").unwrap(),0.5);
}
#[test]
fn noncommuting_x_plus_z_energy_matches_direct_operator_identity(){
    let c=Circuit::new(1,0);
    let a=execute_channel_moments(&c,&[
        term("X",1.0),term("Z",1.0)
    ],DEFAULT_MEMORY_BUDGET).unwrap();
    near(a.mean,1.0);near(a.second_moment,2.0);
    near(a.variance,1.0);
}
#[test]
fn repeated_measurements_have_exact_correlations_not_independent_draws(){
    let mut c=Circuit::new(1,2);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    c.push(Instruction::Measure{qubit:0,into_bit:1});
    let a=execute_channel_moments(&c,&[term("Z",1.0)],DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(a.branch_count,2);
    near(*a.classical_probabilities.get("00").unwrap(),0.5);
    near(*a.classical_probabilities.get("11").unwrap(),0.5);
    assert!(!a.classical_probabilities.contains_key("01"));
    assert!(!a.classical_probabilities.contains_key("10"));
}
#[test]
fn long_circuit_and_unreachable_gates_are_fail_closed(){
    let mut c=Circuit::new(2,1);
    c.push(Instruction::If{bit:0,equals:true,gate:Gate::X(0)});
    assert_eq!(execute_channel_moments(&c,&[term("II",1.0)],DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),ChannelError::ReadBeforeWrite{instruction:0,bit:0});
    let mut b=Circuit::new(2,1);
    b.push(Instruction::Measure{qubit:0,into_bit:0});
    b.push(Instruction::If{bit:0,equals:true,gate:Gate::Cnot{control:1,target:1}});
    assert!(matches!(
        execute_channel_moments(&b,&[term("II",1.0)],DEFAULT_MEMORY_BUDGET),
        Err(ChannelError::Simulator(_))
    ));
    let mut many=Circuit::new(1,1);
    for _ in 0..(MAX_CHANNEL_INSTRUMENTS+1){
        many.push(Instruction::Reset{qubit:0});
    }
    assert_eq!(execute_channel_moments(&many,&[term("Z",1.0)],DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),ChannelError::InvalidInstruments);
}
#[test]
fn wrong_pauli_nonfinite_coefficient_width_budget_and_empty_terms_rejected(){
    let c=Circuit::new(1,0);
    assert_eq!(execute_channel_moments(&c,&[],DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),ChannelError::InvalidTerms);
    assert!(matches!(execute_channel_moments(&c,&[term("Q",1.0)],DEFAULT_MEMORY_BUDGET),
        Err(ChannelError::InvalidPauli{..})));
    assert_eq!(execute_channel_moments(&c,&[term("X",f64::NAN)],DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),ChannelError::InvalidCoefficient);
    assert_eq!(execute_channel_moments(&c,&[term("Z",1.0)],1)
        .unwrap_err(),ChannelError::BudgetExceeded);
    let too_wide=Circuit::new(9,0);
    assert_eq!(execute_channel_moments(&too_wide,&[term(&"I".repeat(9),1.0)],DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),ChannelError::InvalidWidth);
}

#[test]
fn nonuniform_t_interference_has_true_born_weighted_channel_energy(){
    // H T H |0>: P(0)=(1+1/sqrt(2))/2, P(1)=(1-1/sqrt(2))/2.
    // A false "average each branch equally" produces mean=0, not 1/sqrt(2).
    let mut c=Circuit::new(1,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::T(0)));
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    let m=execute_channel_moments(&c,&[term("Z",1.0)],DEFAULT_MEMORY_BUDGET).unwrap();
    near(m.mean,std::f64::consts::FRAC_1_SQRT_2);
    near(m.second_moment,1.0);
    near(m.variance,0.5);
    near(m.within_branch_variance,0.0);
    near(m.between_branch_variance,0.5);
    let p0=(1.0+std::f64::consts::FRAC_1_SQRT_2)/2.0;
    near(*m.classical_probabilities.get("0").unwrap(),p0);
    near(*m.classical_probabilities.get("1").unwrap(),1.0-p0);
}
#[test]
fn y_phase_coherence_disappears_only_after_z_measurement(){
    let mut c=Circuit::new(1,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::S(0)));
    let coherent=execute_channel_moments(
        &c,&[term("Y",1.0)],DEFAULT_MEMORY_BUDGET,
    ).unwrap();
    near(coherent.mean,1.0);
    near(coherent.variance,0.0);
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    let dephased=execute_channel_moments(
        &c,&[term("Y",1.0)],DEFAULT_MEMORY_BUDGET,
    ).unwrap();
    near(dephased.mean,0.0);
    near(dephased.variance,1.0);
    near(dephased.within_branch_variance,1.0);
    near(dephased.between_branch_variance,0.0);
}
#[test]
fn complete_256_branch_profile_has_correct_uniform_register_distribution(){
    let mut c=Circuit::new(8,8);
    for q in 0..8{
        c.push(Instruction::Gate(Gate::H(q)));
        c.push(Instruction::Measure{qubit:q,into_bit:q});
    }
    let m=execute_channel_moments(
        &c,&[term("ZIIIIIII",1.0)],DEFAULT_MEMORY_BUDGET,
    ).unwrap();
    assert_eq!(m.branch_count,256);
    assert_eq!(m.instrument_count,8);
    assert_eq!(m.classical_probabilities.len(),256);
    near(m.probability_mass,1.0);
    near(m.mean,0.0);
    near(m.variance,1.0);
    near(m.within_branch_variance,0.0);
    near(m.between_branch_variance,1.0);
    near(*m.classical_probabilities.get("00000000").unwrap(),1.0/256.0);
    near(*m.classical_probabilities.get("11111111").unwrap(),1.0/256.0);
}
#[test]
fn worst_case_instruction_term_branch_work_is_fail_closed_before_execution(){
    let mut c=Circuit::new(8,0);
    for q in 0..8{c.push(Instruction::Reset{qubit:q});}
    for _ in 0..504{c.push(Instruction::Gate(Gate::H(0)));}
    let terms=vec![term("IIIIIIII",1.0);64];
    assert_eq!(execute_channel_moments(
        &c,&terms,DEFAULT_MEMORY_BUDGET
    ).unwrap_err(),ChannelError::WorkLimit);
}
