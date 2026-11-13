//! C-021 conditioned final-classical-register tomography, true product.
use aura_quantum_sim::channel_tomography::{
    execute_conditioned_tomography,execute_channel_tomography,
    ClassicalCondition,
};
use aura_quantum_sim::channel_moments::ChannelError;
use aura_quantum_sim::circuit::{Circuit,Instruction};
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn cond(bit:usize,equals:bool)->ClassicalCondition{ClassicalCondition{bit,equals}}
fn approx(x:f64,y:f64){assert!((x-y).abs()<1e-9,"{x} vs {y}");}
#[test]
fn bell_selected_0_or_1_yields_pure_correlated_two_wire_state(){
    let mut c=Circuit::new(2,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::Cnot{control:0,target:1}));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    let avg=execute_channel_tomography(&c,&[[0,1]],DEFAULT_MEMORY_BUDGET).unwrap();
    approx(avg.pairs[0].purity,0.5);
    for (outcome,index,z) in [(false,0,1.0),(true,3,-1.0)]{
        let d=execute_conditioned_tomography(
            &c,&[[0,1]],&[cond(0,outcome)],DEFAULT_MEMORY_BUDGET).unwrap();
        approx(d.condition_probability,0.5);
        assert_eq!(d.selected_branch_count,1);
        approx(d.pairs[0].purity,1.0);
        approx(d.pairs[0].density[index][index].re,1.0);
        approx(d.wires[0].bloch[2],z);
        approx(d.wires[1].bloch[2],z);
        approx(d.wires[0].branch_mixing_gap,0.0);
        assert_eq!(d.conditions,[cond(0,outcome)]);
    }
}
#[test]
fn nonuniform_born_postselection_reports_actual_event_probability(){
    let mut c=Circuit::new(1,1);
    for g in [Gate::H(0),Gate::T(0),Gate::H(0)]{
        c.push(Instruction::Gate(g));
    }
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    let p0=(1.0+std::f64::consts::FRAC_1_SQRT_2)/2.0;
    for (value,prob,z) in [(false,p0,1.0),(true,1.0-p0,-1.0)]{
        let d=execute_conditioned_tomography(&c,&[],&[cond(0,value)],
            DEFAULT_MEMORY_BUDGET).unwrap();
        approx(d.condition_probability,prob);
        approx(d.wires[0].bloch[2],z);
        approx(d.wires[0].purity,1.0);
        approx(d.wires[0].mean_branch_purity,1.0);
        approx(d.wires[0].branch_mixing_gap,0.0);
    }
}
#[test]
fn partial_condition_leaves_hidden_histories_mixed_without_exposing_them(){
    let mut c=Circuit::new(2,2);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::H(1)));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    c.push(Instruction::Measure{qubit:1,into_bit:1});
    let d=execute_conditioned_tomography(
        &c,&[[0,1]],&[cond(0,true)],DEFAULT_MEMORY_BUDGET).unwrap();
    approx(d.condition_probability,0.5);
    assert_eq!(d.selected_branch_count,2);
    approx(d.wires[0].bloch[2],-1.0);
    approx(d.wires[0].purity,1.0);
    approx(d.wires[1].purity,0.5);
    approx(d.wires[1].branch_mixing_gap,0.5);
    approx(d.pairs[0].purity,0.5);
}
#[test]
fn joint_register_condition_and_overwritten_bit_use_final_values_only(){
    let mut c=Circuit::new(2,2);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::Cnot{control:0,target:1}));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    c.push(Instruction::Measure{qubit:1,into_bit:1});
    c.push(Instruction::Gate(Gate::X(0)));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    let d=execute_conditioned_tomography(
        &c,&[[0,1]],&[cond(0,true),cond(1,false)],
        DEFAULT_MEMORY_BUDGET).unwrap();
    approx(d.condition_probability,0.5);
    assert_eq!(d.selected_branch_count,1);
    approx(d.wires[0].bloch[2],-1.0);
    approx(d.wires[1].bloch[2],1.0);
    approx(d.pairs[0].purity,1.0);
    assert_eq!(d.instrument_count,3);
}
#[test]
fn hidden_reset_never_authorizes_postselection_on_unwritten_bit(){
    let mut c=Circuit::new(1,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Reset{qubit:0});
    assert_eq!(
        execute_conditioned_tomography(&c,&[],&[cond(0,false)],DEFAULT_MEMORY_BUDGET)
            .unwrap_err(),ChannelError::InvalidCondition);
}
#[test]
fn deterministic_impossible_condition_refused(){
    let mut c=Circuit::new(1,1);
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    assert_eq!(
        execute_conditioned_tomography(&c,&[],&[cond(0,true)],DEFAULT_MEMORY_BUDGET)
            .unwrap_err(),ChannelError::ImpossibleCondition);
}
#[test]
fn duplicate_reversed_unwritten_and_out_of_range_conditions_rejected(){
    let mut c=Circuit::new(2,3);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    c.push(Instruction::Measure{qubit:1,into_bit:1});
    for list in [
        vec![],vec![cond(0,false),cond(0,true)],
        vec![cond(1,false),cond(0,false)],
        vec![cond(2,false)],vec![cond(3,false)]
    ]{
        assert_eq!(execute_conditioned_tomography(&c,&[],&list,DEFAULT_MEMORY_BUDGET)
            .unwrap_err(),ChannelError::InvalidCondition);
    }
}
#[test]
fn bounded_256_branch_case_full_and_partial_postselection(){
    let mut c=Circuit::new(8,8);
    for i in 0..8{
        c.push(Instruction::Gate(Gate::H(i)));
        c.push(Instruction::Measure{qubit:i,into_bit:i});
    }
    let all=execute_conditioned_tomography(
        &c,&[[0,7]],&[cond(0,true)],DEFAULT_MEMORY_BUDGET).unwrap();
    approx(all.condition_probability,0.5);
    assert_eq!(all.selected_branch_count,128);
    approx(all.wires[0].purity,1.0);
    approx(all.wires[7].purity,0.5);
    let conditions=(0..8).map(|i|cond(i,true)).collect::<Vec<_>>();
    let one=execute_conditioned_tomography(
        &c,&[[0,7]],&conditions,DEFAULT_MEMORY_BUDGET).unwrap();
    approx(one.condition_probability,1.0/256.0);
    assert_eq!(one.selected_branch_count,1);
    approx(one.pairs[0].purity,1.0);
}
#[test]
fn budget_and_pair_issues_fail_closed(){
    let mut c=Circuit::new(2,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    assert_eq!(execute_conditioned_tomography(
        &c,&[],&[cond(0,false)],1).unwrap_err(),ChannelError::BudgetExceeded);
    assert_eq!(execute_conditioned_tomography(
        &c,&[[1,0]],&[cond(0,false)],DEFAULT_MEMORY_BUDGET).unwrap_err(),
        ChannelError::InvalidTomographyPairs);
}
