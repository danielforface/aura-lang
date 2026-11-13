//! C-017 independent dense H|psi> numerical oracle.
use aura_quantum_sim::factorized::{
    FactorError,FactorState,PauliTerm,MAX_PAULI_TERMS,
};
use aura_quantum_sim::moments::evaluate_hamiltonian_moments;
use aura_quantum_sim::{Complex,Gate,Statevector,DEFAULT_MEMORY_BUDGET};
fn term(p:&str,c:f64)->PauliTerm {
    PauliTerm{paulis:p.into(),coefficient:c}
}
fn near(a:f64,b:f64) {
    let tolerance=3e-9*b.abs().max(1.0);
    assert!((a-b).abs()<tolerance,"{a:.16e} vs {b:.16e}");
}
fn dense_reference(s:&Statevector,terms:&[PauliTerm])->(f64,f64,f64) {
    // Independent oracle: construct H|psi>, NEVER multiply Pauli words.
    let mut hpsi=vec![Complex::ZERO;1usize<<s.qubits()];
    for term in terms {
        let mut transformed=s.clone();
        for (q,letter) in term.paulis.bytes().enumerate() {
            let gate=match letter {
                b'I'=>continue,b'X'=>Gate::X(q),b'Y'=>Gate::Y(q),
                b'Z'=>Gate::Z(q),_=>panic!("invalid test Pauli"),
            };
            transformed.apply(gate).unwrap();
        }
        for (i,amp) in hpsi.iter_mut().enumerate() {
            *amp=*amp+Complex::new(term.coefficient,0.0)*
                transformed.amplitude(i).unwrap();
        }
    }
    let mut mean=Complex::ZERO;
    let mut second=0.0;
    for (i,v) in hpsi.iter().enumerate() {
        let a=s.amplitude(i).unwrap();
        mean=mean+Complex::new(a.re,-a.im)* *v;
        second+=v.norm_squared();
    }
    near(mean.im,0.0);
    (mean.re,second,(second-mean.re*mean.re).max(0.0))
}
#[test]
fn x_plus_z_anticonmutator_and_exact_x_eigenstate() {
    let f=FactorState::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    let m=evaluate_hamiltonian_moments(
        &f,&[term("X",1.0),term("Z",1.0)]
    ).unwrap();
    near(m.mean,1.0);near(m.second_moment,2.0);near(m.variance,1.0);
    assert_eq!(m.anticommuting_pairs,1);
    let mut plus=f;
    plus.apply(Gate::H(0)).unwrap();
    let m=evaluate_hamiltonian_moments(
        &plus,&[term("X",1.0),term("I",2.0)]
    ).unwrap();
    near(m.mean,3.0);near(m.second_moment,9.0);near(m.variance,0.0);
}
#[test]
fn independent_dense_hpsi_oracle_noncommuting_y_t_and_swap() {
    for n in 2..=8 {
        for variant in 0..8 {
            let mut factor=FactorState::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let mut dense=Statevector::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            for g in [
                Gate::H(0),Gate::H(n-1),Gate::T(0),Gate::S(n-1),
                Gate::Cnot{control:0,target:n-1},
                Gate::Swap{a:0,b:1},Gate::Y(n-1),
            ] {
                factor.apply(g.clone()).unwrap();dense.apply(g).unwrap();
            }
            let letters=['I','X','Y','Z'];
            let mut terms=Vec::new();
            for j in 0..6 {
                terms.push(PauliTerm{
                    paulis:(0..n).map(|q|
                        letters[(q*3+variant+j)%4]).collect(),
                    coefficient:(j as f64-2.5)*0.125,
                });
            }
            let actual=evaluate_hamiltonian_moments(&factor,&terms).unwrap();
            let (mean,second,variance)=dense_reference(&dense,&terms);
            near(actual.mean,mean);
            near(actual.second_moment,second);
            near(actual.variance,variance);
            assert_eq!(actual.commuting_pairs+actual.anticommuting_pairs,15);
        }
    }
}
#[test]
fn real_128_qubit_bell_t_execution_without_dense_global_state() {
    let mut f=FactorState::zero(128,DEFAULT_MEMORY_BUDGET).unwrap();
    for q in (0..128).step_by(2) {
        f.apply(Gate::H(q)).unwrap();
        f.apply(Gate::Cnot{control:q,target:q+1}).unwrap();
        f.apply(Gate::T(q+1)).unwrap();
    }
    let mut xx="I".repeat(128);
    xx.replace_range(0..2,"XX");
    let mut yy="I".repeat(128);
    yy.replace_range(0..2,"YY");
    let result=evaluate_hamiltonian_moments(&f,&[
        term(&xx,0.5),term(&yy,-0.5),term(&"Z".repeat(128),1.0)
    ]).unwrap();
    near(result.mean,1.0+std::f64::consts::FRAC_1_SQRT_2);
    assert!(result.variance>=0.0);
    assert!(result.estimated_work_units<16_777_216);
    assert_eq!(f.factor_count(),64);
    assert!(f.logical_state_bytes()<8192);
}
#[test]
fn rejection_for_bad_input_and_excessive_factor_work() {
    let f=FactorState::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(evaluate_hamiltonian_moments(&f,&[]).unwrap_err(),
        FactorError::InvalidPauliTermCount);
    assert_eq!(evaluate_hamiltonian_moments(&f,&[term("IQ",1.0)])
        .unwrap_err(),FactorError::InvalidPauliSymbol{wire:1,symbol:b'Q'});
    assert_eq!(evaluate_hamiltonian_moments(&f,&[term("I",1.0)])
        .unwrap_err(),FactorError::InvalidBasisWidth{expected:2,got:1});
    assert_eq!(evaluate_hamiltonian_moments(&f,&[term("II",f64::NAN)])
        .unwrap_err(),FactorError::InvalidPauliCoefficient);
    assert_eq!(evaluate_hamiltonian_moments(
        &f,&vec![term("II",1.0);MAX_PAULI_TERMS+1]
    ).unwrap_err(),FactorError::InvalidPauliTermCount);
    let mut joined=FactorState::zero(10,DEFAULT_MEMORY_BUDGET).unwrap();
    joined.apply(Gate::H(0)).unwrap();
    for q in 1..10 {
        joined.apply(Gate::Cnot{control:0,target:q}).unwrap();
    }
    let terms=vec![term(&"I".repeat(10),1.0);64];
    assert_eq!(evaluate_hamiltonian_moments(&joined,&terms).unwrap_err(),
        FactorError::WorkLimit);
}
