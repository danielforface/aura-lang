//! C-016 exact analytic Pauli observables on genuinely executed factor
//! products, compared with independent dense Pauli gate compositions.
use aura_quantum_sim::factorized::{
    FactorCircuit,FactorError,FactorState,PauliTerm,MAX_PAULI_TERMS,
};
use aura_quantum_sim::circuit::{Circuit,Instruction};
use aura_quantum_sim::{Complex,Gate,Statevector,DEFAULT_MEMORY_BUDGET};
fn near(actual:f64,expected:f64){
    assert!((actual-expected).abs()<4e-10,
        "observable phase/basis mismatch {actual:.16e} vs {expected:.16e}");
}
fn dense_expectation(state:&Statevector,word:&str)->f64{
    assert_eq!(state.qubits(),word.len());
    let mut transformed=state.clone();
    for (wire,symbol) in word.bytes().enumerate() {
        let gate=match symbol {
            b'I'=>continue,
            b'X'=>Gate::X(wire),
            b'Y'=>Gate::Y(wire),
            b'Z'=>Gate::Z(wire),
            _=>panic!("test oracle unsupported symbol"),
        };
        transformed.apply(gate).unwrap();
    }
    let mut result=Complex::ZERO;
    for i in 0..(1usize<<state.qubits()) {
        let original=state.amplitude(i).unwrap();
        let new=transformed.amplitude(i).unwrap();
        result=result+Complex::new(original.re,-original.im)*new;
    }
    near(result.im,0.0);
    result.re
}
fn paulis(n:usize,marked:&[(usize,char)])->String {
    let mut word=vec![b'I';n];
    for &(wire,p) in marked {word[wire]=p as u8;}
    String::from_utf8(word).unwrap()
}
fn term(word:String,coefficient:f64)->PauliTerm {
    PauliTerm{paulis:word,coefficient}
}
#[test]
fn computational_basis_plus_y_and_t_phases_have_correct_pauli_sign() {
    let mut f=FactorState::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    near(f.expectation_pauli("I").unwrap(),1.0);
    near(f.expectation_pauli("Z").unwrap(),1.0);
    near(f.expectation_pauli("X").unwrap(),0.0);
    near(f.expectation_pauli("Y").unwrap(),0.0);
    f.apply(Gate::H(0)).unwrap();
    near(f.expectation_pauli("X").unwrap(),1.0);
    near(f.expectation_pauli("Y").unwrap(),0.0);
    f.apply(Gate::S(0)).unwrap();
    near(f.expectation_pauli("Y").unwrap(),1.0);
    near(f.expectation_pauli("X").unwrap(),0.0);
    f.apply(Gate::Z(0)).unwrap();
    near(f.expectation_pauli("Y").unwrap(),-1.0);
    f.apply(Gate::S(0)).unwrap();
    // S Z S = identity: diag(1,i) diag(1,-1) diag(1,i)
    // = diag(1,1), so H|0> returns to |+>.
    near(f.expectation_pauli("Y").unwrap(),0.0);
    near(f.expectation_pauli("X").unwrap(),1.0);
}
#[test]
fn bell_yy_is_negative_and_nonclifford_t_rotates_coherence() {
    let mut f=FactorState::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut d=Statevector::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    for g in [Gate::H(0),Gate::Cnot{control:0,target:1}] {
        f.apply(g.clone()).unwrap(); d.apply(g).unwrap();
    }
    for (word,want) in [("XX",1.0),("YY",-1.0),("ZZ",1.0),
                        ("ZI",0.0),("IZ",0.0),("II",1.0)] {
        near(f.expectation_pauli(word).unwrap(),want);
        near(f.expectation_pauli(word).unwrap(),dense_expectation(&d,word));
    }
    f.apply(Gate::T(1)).unwrap();d.apply(Gate::T(1)).unwrap();
    for (word,want) in [("XX",std::f64::consts::FRAC_1_SQRT_2),
                        ("YY",-std::f64::consts::FRAC_1_SQRT_2),
                        ("ZZ",1.0),("ZI",0.0)] {
        near(f.expectation_pauli(word).unwrap(),want);
        near(f.expectation_pauli(word).unwrap(),dense_expectation(&d,word));
    }
}
#[test]
fn cross_factor_swap_and_all_four_paulis_match_dense_2_to_8_wires() {
    for n in 2..=8 {
        for case in 0..8 {
            let mut f=FactorState::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let mut d=Statevector::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let gates=[
                Gate::H(0),Gate::H(n-1),Gate::T(0),
                Gate::Cnot{control:0,target:n-1},
                Gate::S(n-1),Gate::Swap{a:0,b:1},
                Gate::T(n-1),Gate::Y(0),
            ];
            for g in gates {
                f.apply(g.clone()).unwrap();d.apply(g).unwrap();
            }
            let letters=['I','X','Y','Z'];
            for offset in 0..6 {
                let word=(0..n).map(|q|letters[(q*3+case+offset)%4])
                    .collect::<String>();
                near(f.expectation_pauli(&word).unwrap(),
                     dense_expectation(&d,&word));
            }
            near(f.expectation_pauli(&"I".repeat(n)).unwrap(),1.0);
        }
    }
}
#[test]
fn ten_wire_entangled_factor_and_outside_wire_support_sparse_queries() {
    let mut f=FactorState::zero(11,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut dense=Statevector::zero(11,DEFAULT_MEMORY_BUDGET).unwrap();
    f.apply(Gate::H(0)).unwrap();dense.apply(Gate::H(0)).unwrap();
    for q in 1..10 {
        let g=Gate::Cnot{control:0,target:q};
        f.apply(g.clone()).unwrap();dense.apply(g).unwrap();
    }
    f.apply(Gate::T(4)).unwrap();dense.apply(Gate::T(4)).unwrap();
    assert_eq!(f.factor_widths(),vec![10,1]);
    for t in [paulis(11,&[(0,'Z'),(9,'Z')]),
              paulis(11,&[(0,'X'),(9,'Y')]),
              paulis(11,&[(0,'Y'),(4,'X')]),
              paulis(11,&[(10,'Z')]),
              "Z".repeat(11), "I".repeat(11)] {
        near(f.expectation_pauli(&t).unwrap(),dense_expectation(&dense,&t));
    }
}
#[test]
fn quantum_128_nonclifford_bell_product_has_exact_analytic_hamiltonian_energy() {
    let mut p=FactorCircuit::new(128,0);
    for q in (0..128).step_by(2) {
        p.push(Instruction::Gate(Gate::H(q)));
        p.push(Instruction::Gate(Gate::Cnot{control:q,target:q+1}));
        p.push(Instruction::Gate(Gate::T(q+1)));
    }
    let words=vec![
        term(paulis(128,&[(0,'Z'),(1,'Z')]),1.0),
        term(paulis(128,&[(0,'X'),(1,'X')]),0.5),
        term(paulis(128,&[(0,'Y'),(1,'Y')]),-0.5),
        term("Z".repeat(128),0.25),
        term("I".repeat(128),0.0),
    ];
    let r=p.execute_observables(&words,20261108,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(p.instructions().len(),192);
    assert_eq!(r.metadata.shots,1);
    assert_eq!(r.estimates.len(),5);
    assert!(r.shot.trace.is_empty());
    near(r.estimates[0].expectation,1.0);
    near(r.estimates[1].expectation,std::f64::consts::FRAC_1_SQRT_2);
    near(r.estimates[2].expectation,-std::f64::consts::FRAC_1_SQRT_2);
    near(r.estimates[3].expectation,1.0);
    near(r.estimates[4].expectation,1.0);
    near(r.weighted_sum,1.25+std::f64::consts::FRAC_1_SQRT_2);
    assert_eq!(r.metadata.qpu_hardware_jobs,0);
    assert!(r.peak_logical_state_bytes<8192);
}
#[test]
fn one_seeded_dynamic_measurement_and_reset_yields_conditioned_expectations(){
    let mut f=FactorCircuit::new(3,1);
    let mut d=Circuit::new(3,1);
    for op in [
        Instruction::Gate(Gate::H(0)),
        Instruction::Gate(Gate::Cnot{control:0,target:1}),
        Instruction::Measure{qubit:0,into_bit:0},
        Instruction::If{bit:0,equals:true,gate:Gate::X(1)},
        Instruction::Gate(Gate::H(2)),
        Instruction::Reset{qubit:2},
        Instruction::Gate(Gate::T(1)),
    ] {f.push(op.clone());d.push(op);}
    let terms=vec![
        term("ZZI".into(),0.7),term("YII".into(),-1.2),
        term("IIZ".into(),0.3),term("III".into(),1.0),
    ];
    for seed in [0,1,17,20261108,u64::MAX]{
        let a=f.execute_observables(&terms,seed,DEFAULT_MEMORY_BUDGET).unwrap();
        let b=d.execute_once(seed,DEFAULT_MEMORY_BUDGET).unwrap();
        assert_eq!(a.shot,b.shot);
        assert_eq!(a.shot.trace.len(),2);
        let mut total=0.0;
        for (t,e) in terms.iter().zip(&a.estimates) {
            let v=dense_expectation(&b.final_state,&t.paulis);
            near(e.expectation,v);
            near(e.contribution,t.coefficient*v);
            total+=t.coefficient*v;
        }
        near(a.weighted_sum,total);
    }
}
#[test]
fn invalid_pauli_symbols_and_width_reject_without_changing_state(){
    let mut f=FactorState::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    f.apply(Gate::H(0)).unwrap();
    let x=f.amplitude_bits(&[false,false]).unwrap();
    assert_eq!(f.expectation_pauli("I").unwrap_err(),
        FactorError::InvalidBasisWidth{expected:2,got:1});
    assert_eq!(f.expectation_pauli("IQ").unwrap_err(),
        FactorError::InvalidPauliSymbol{wire:1,symbol:b'Q'});
    assert!(matches!(f.expectation_pauli("λZ"),
        Err(FactorError::InvalidBasisWidth{..})|
        Err(FactorError::InvalidPauliSymbol{..})));
    assert_eq!(f.amplitude_bits(&[false,false]).unwrap(),x);
}
#[test]
fn term_bounds_coefficients_and_small_memory_fail_closed(){
    let f=FactorCircuit::new(2,0);
    assert_eq!(f.execute_observables(&[],0,DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),FactorError::InvalidPauliTermCount);
    let over=vec![term("II".into(),1.0);MAX_PAULI_TERMS+1];
    assert_eq!(f.execute_observables(&over,0,DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),FactorError::InvalidPauliTermCount);
    for c in [f64::NAN,f64::INFINITY,-f64::INFINITY,10001.0]{
        let input=[term("II".into(),c)];
        assert_eq!(f.execute_observables(&input,0,DEFAULT_MEMORY_BUDGET)
            .unwrap_err(),FactorError::InvalidPauliCoefficient);
    }
    let t=[term("II".into(),1.0)];
    assert!(matches!(f.execute_observables(&t,0,128),
        Err(FactorError::BudgetExceeded{..})));
}
#[test]
fn maximum_sixty_four_terms_are_still_finite_and_bounded(){
    let f=FactorCircuit::new(1,0);
    let list=vec![term("Z".into(),10_000.0);64];
    let batch=f.execute_observables(&list,5,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(batch.estimates.len(),64);
    near(batch.weighted_sum,640_000.0);
    assert_eq!(batch.metadata.qpu_hardware_jobs,0);
}
#[test]
fn unread_classical_bit_and_unreachable_invalid_gate_fail_before_expectation(){
    let mut f=FactorCircuit::new(2,1);
    f.push(Instruction::If{bit:0,equals:true,gate:Gate::X(0)});
    let terms=[term("II".into(),1.0)];
    assert_eq!(f.execute_observables(&terms,5,DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),FactorError::ReadBeforeWrite{bit:0,instruction:0});
    let mut invalid=FactorCircuit::new(2,1);
    invalid.push(Instruction::Measure{qubit:0,into_bit:0});
    invalid.push(Instruction::If{bit:0,equals:true,
        gate:Gate::Cnot{control:1,target:1}});
    assert_eq!(invalid.execute_observables(&terms,5,DEFAULT_MEMORY_BUDGET)
        .unwrap_err(),FactorError::DuplicateQubit(1));
}
