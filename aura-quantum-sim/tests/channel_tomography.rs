//! C-020 true local reduced density and branch-mixture behavior.
use aura_quantum_sim::channel_tomography::execute_channel_tomography;
use aura_quantum_sim::channel_moments::ChannelError;
use aura_quantum_sim::circuit::{Circuit,Instruction};
use aura_quantum_sim::{Gate,DEFAULT_MEMORY_BUDGET};
fn near(x:f64,y:f64){assert!((x-y).abs()<1e-9,"{x} vs {y}");}
#[test]
fn single_hadamard_keeps_coherence_pure_bloch_x(){
    let mut c=Circuit::new(1,0);
    c.push(Instruction::Gate(Gate::H(0)));
    let d=execute_channel_tomography(&c,&[],DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(d.wires.len(),1);
    near(d.wires[0].bloch[0],1.0);
    near(d.wires[0].bloch[1],0.0);
    near(d.wires[0].bloch[2],0.0);
    near(d.wires[0].purity,1.0);
    near(d.wires[0].branch_mixing_gap,0.0);
}
#[test]
fn y_plus_state_bloch_sign_matches_pauli_y(){
    let mut c=Circuit::new(1,0);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::S(0)));
    let d=execute_channel_tomography(&c,&[],DEFAULT_MEMORY_BUDGET).unwrap();
    near(d.wires[0].bloch[1],1.0);
    near(d.wires[0].density[0][1].im,-0.5);
    near(d.wires[0].purity,1.0);
}
#[test]
fn unmeasured_bell_is_pure_joint_but_mixed_locally_without_branch_gap(){
    let mut c=Circuit::new(2,0);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::Cnot{control:0,target:1}));
    let d=execute_channel_tomography(&c,&[[0,1]],DEFAULT_MEMORY_BUDGET).unwrap();
    for w in &d.wires{
        near(w.purity,0.5);
        near(w.mean_branch_purity,0.5);
        near(w.branch_mixing_gap,0.0);
    }
    near(d.pairs[0].purity,1.0);
    near(d.pairs[0].density[0][3].re,0.5);
    near(d.pairs[0].density[3][0].re,0.5);
    near(d.pairs[0].branch_mixing_gap,0.0);
    assert_eq!(d.branch_count,1);
}
#[test]
fn bell_measurement_dephases_joint_density_and_adds_instrument_mixture(){
    let mut c=Circuit::new(2,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::Cnot{control:0,target:1}));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    let d=execute_channel_tomography(&c,&[[0,1]],DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(d.branch_count,2);
    for w in &d.wires{
        near(w.purity,0.5);
        near(w.mean_branch_purity,1.0);
        near(w.branch_mixing_gap,0.5);
    }
    near(d.pairs[0].purity,0.5);
    near(d.pairs[0].mean_branch_purity,1.0);
    near(d.pairs[0].branch_mixing_gap,0.5);
    near(d.pairs[0].density[0][3].re,0.0);
    near(d.pairs[0].density[0][0].re,0.5);
    near(d.pairs[0].density[3][3].re,0.5);
}
#[test]
fn bell_measurement_conditional_x_removes_partner_classical_mixture(){
    let mut c=Circuit::new(2,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Gate(Gate::Cnot{control:0,target:1}));
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    c.push(Instruction::If{bit:0,equals:true,gate:Gate::X(1)});
    let d=execute_channel_tomography(&c,&[[0,1]],DEFAULT_MEMORY_BUDGET).unwrap();
    near(d.wires[0].purity,0.5);
    near(d.wires[1].purity,1.0);
    near(d.wires[1].bloch[2],1.0);
    near(d.pairs[0].purity,0.5);
}
#[test]
fn reset_hidden_branch_recovers_pure_zero_without_classical_record(){
    let mut c=Circuit::new(1,1);
    c.push(Instruction::Gate(Gate::H(0)));
    c.push(Instruction::Reset{qubit:0});
    let d=execute_channel_tomography(&c,&[],DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(d.branch_count,2);
    near(d.wires[0].purity,1.0);
    near(d.wires[0].bloch[2],1.0);
    near(*d.classical_probabilities.get("?").unwrap(),1.0);
}
#[test]
fn nonuniform_hth_outcomes_produce_correct_z_marginal(){
    let mut c=Circuit::new(1,1);
    for g in [Gate::H(0),Gate::T(0),Gate::H(0)]{
        c.push(Instruction::Gate(g));
    }
    c.push(Instruction::Measure{qubit:0,into_bit:0});
    let d=execute_channel_tomography(&c,&[],DEFAULT_MEMORY_BUDGET).unwrap();
    near(d.wires[0].bloch[2],std::f64::consts::FRAC_1_SQRT_2);
    near(d.wires[0].purity,0.75);
    near(d.wires[0].mean_branch_purity,1.0);
    near(d.wires[0].branch_mixing_gap,0.25);
}
#[test]
fn swap_and_pair_order_are_explicit_q0_lsb(){
    let mut c=Circuit::new(3,0);
    c.push(Instruction::Gate(Gate::X(0)));
    c.push(Instruction::Gate(Gate::Swap{a:0,b:2}));
    let d=execute_channel_tomography(&c,&[[0,2]],DEFAULT_MEMORY_BUDGET).unwrap();
    near(d.wires[0].bloch[2],1.0);
    near(d.wires[2].bloch[2],-1.0);
    near(d.pairs[0].density[2][2].re,1.0); // [0,2] pair |q2 q0> = 10
}
#[test]
fn maximum_256_real_branch_profile_and_8_marginals(){
    let mut c=Circuit::new(8,8);
    for i in 0..8{
        c.push(Instruction::Gate(Gate::H(i)));
        c.push(Instruction::Measure{qubit:i,into_bit:i});
    }
    let d=execute_channel_tomography(&c,&[[0,7]],DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(d.branch_count,256);
    near(d.probability_mass,1.0);
    assert_eq!(d.classical_probabilities.len(),256);
    near(d.pairs[0].purity,0.25);
    near(d.pairs[0].mean_branch_purity,1.0);
    near(d.pairs[0].branch_mixing_gap,0.75);
    for w in &d.wires{near(w.purity,0.5);near(w.branch_mixing_gap,0.5);}
}
#[test]
fn duplicate_reversed_invalid_and_too_many_pairs_reject_atomically(){
    let c=Circuit::new(8,0);
    for pairs in [
        vec![[0,1],[0,1]],vec![[1,0]],vec![[0,0]],vec![[0,8]],
        vec![[0,1];17],
    ]{
        assert_eq!(execute_channel_tomography(&c,&pairs,DEFAULT_MEMORY_BUDGET)
            .unwrap_err(),ChannelError::InvalidTomographyPairs);
    }
    assert_eq!(execute_channel_tomography(&c,&[],1).unwrap_err(),
        ChannelError::BudgetExceeded);
}
