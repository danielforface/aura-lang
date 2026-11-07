//! AURA-Q28-008: exact tensor *factorization* (no truncation), f64 dense
//! arithmetic inside components; real dynamic and negative acceptance.
use aura_quantum_sim::factorized::{
    FactorCircuit,FactorError,FactorState,MAX_FACTOR_QUBITS,MAX_FACTOR_SHOTS,
    FACTORIZED_PROFILE,
};
use aura_quantum_sim::circuit::{Circuit,Instruction};
use aura_quantum_sim::{Gate,Statevector,SeededRng,DEFAULT_MEMORY_BUDGET};

fn near(actual:f64,want:f64) {
    assert!((actual-want).abs()<4e-11,
        "{actual:.16e} != {want:.16e}");
}
fn next(s:&mut u64)->u64 {
    *s=s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *s
}
fn random_gate(n:usize,s:&mut u64)->Gate {
    let kind=(next(s)%if n>1 {9} else {6}) as usize;
    let a=next(s) as usize % n;
    let b=(a+1+next(s) as usize %(n-1).max(1))%n;
    match kind {
        0=>Gate::H(a),1=>Gate::X(a),2=>Gate::Y(a),
        3=>Gate::Z(a),4=>Gate::S(a),5=>Gate::T(a),
        6=>Gate::Cnot{control:a,target:b},
        7=>Gate::Cz{control:a,target:b},
        _=>Gate::Swap{a,b},
    }
}
#[test]
fn factor_128_qubit_64_independent_bell_pairs_with_t_need_only_4096_live_state_bytes(){
    let mut state=FactorState::zero(128,DEFAULT_MEMORY_BUDGET).unwrap();
    for q in (0..128).step_by(2) {
        state.apply(Gate::H(q)).unwrap();
        state.apply(Gate::Cnot{control:q,target:q+1}).unwrap();
        state.apply(Gate::T(q+1)).unwrap();
    }
    assert_eq!(state.qubits(),128);
    assert_eq!(state.factor_count(),64);
    assert_eq!(state.factor_widths(),vec![2;64]);
    assert_eq!(state.logical_state_bytes(),4096);
    assert!(state.peak_logical_bytes()<=2*4096);
    let all_zero=vec![false;128];
    let all_one=vec![true;128];
    let expected=2f64.powi(-64);
    near(state.basis_probability(&all_zero).unwrap(),expected);
    near(state.basis_probability(&all_one).unwrap(),expected);
    near(state.wire_probability(127,true).unwrap(),0.5);
}

#[test]
fn randomized_entanglement_and_nonclifford_phase_match_dense_amplitudes(){
    let mut examined=0usize;
    for n in 1..=8 {
        for case in 0..12 {
            let mut seed=0xc0080000u64+n as u64*1003+case;
            let mut factored=FactorState::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let mut dense=Statevector::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            for _ in 0..28 {
                let gate=random_gate(n,&mut seed);
                factored.apply(gate.clone()).unwrap();
                dense.apply(gate).unwrap();
            }
            for basis in 0..(1usize<<n) {
                let bits=(0..n).map(|q|basis>>q&1!=0).collect::<Vec<_>>();
                let a=factored.amplitude_bits(&bits).unwrap();
                let b=dense.amplitude(basis).unwrap();
                near(a.re,b.re);
                near(a.im,b.im);
                near(factored.basis_probability(&bits).unwrap(),
                     dense.basis_probability(basis).unwrap());
                examined+=1;
            }
        }
    }
    assert_eq!(examined,12*((1usize<<9)-2));
}
#[test]
fn remote_swap_of_entangled_pair_wires_is_pure_relabeling_not_merge(){
    let mut s=FactorState::zero(4,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut dense=Statevector::zero(4,DEFAULT_MEMORY_BUDGET).unwrap();
    let ops=[
        Gate::H(0),Gate::Cnot{control:0,target:1},
        Gate::H(2),Gate::Cnot{control:2,target:3},
        Gate::T(1),Gate::Swap{a:1,b:2},
    ];
    for g in ops {s.apply(g.clone()).unwrap();dense.apply(g).unwrap();}
    assert_eq!(s.factor_count(),2);
    assert_eq!(s.factor_widths(),vec![2,2]);
    let ws=s.factor_wires();
    assert!(ws.iter().any(|w|w.contains(&0)&&w.contains(&2)));
    assert!(ws.iter().any(|w|w.contains(&1)&&w.contains(&3)));
    for i in 0..16 {
        let bits=(0..4).map(|q|i>>q&1!=0).collect::<Vec<_>>();
        let x=s.amplitude_bits(&bits).unwrap();
        let d=dense.amplitude(i).unwrap();
        near(x.re,d.re);near(x.im,d.im);
    }
}
#[test]
fn same_seed_dynamic_measure_reset_and_classical_feedforward_match_reference(){
    let mut factor=FactorCircuit::new(4,2);
    let mut reference=Circuit::new(4,2);
    let circuit=[
        Instruction::Gate(Gate::H(0)),
        Instruction::Gate(Gate::T(1)),
        Instruction::Gate(Gate::Cnot{control:0,target:1}),
        Instruction::Gate(Gate::H(2)),
        Instruction::Gate(Gate::Cnot{control:2,target:3}),
        Instruction::Measure{qubit:0,into_bit:0},
        Instruction::If{bit:0,equals:true,gate:Gate::X(1)},
        Instruction::Reset{qubit:2},
        Instruction::Measure{qubit:3,into_bit:1},
    ];
    for op in circuit {
        factor.push(op.clone());
        reference.push(op);
    }
    let a=factor.execute_shots(64,20261107,DEFAULT_MEMORY_BUDGET).unwrap();
    let b=reference.execute_shots(64,20261107,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(a.records,b.shots);
    assert!(a.one_shot_state.is_none());
    assert_eq!(a.metadata.profile,FACTORIZED_PROFILE);
    assert_eq!(a.metadata.truncation,"none;factor-width-hard-limit");
    assert_eq!(a.metadata.qpu_hardware_jobs,0);
    let one=factor.execute_shots(1,20261107,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(one.records[0],b.shots[0]);
    assert!(one.one_shot_state.is_some());
}
#[test]
fn reference_rng_consumes_one_draw_for_every_measured_or_reset_event(){
    let mut state=FactorState::zero(3,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut oracle=Statevector::zero(3,DEFAULT_MEMORY_BUDGET).unwrap();
    for gate in [Gate::H(0),Gate::T(1),Gate::Cnot{control:0,target:1}] {
        state.apply(gate.clone()).unwrap();oracle.apply(gate).unwrap();
    }
    let mut rng1=SeededRng::new(98765);
    let mut rng2=SeededRng::new(98765);
    for q in [0,2,1] {
        assert_eq!(state.measure(q,&mut rng1).unwrap(),
                   oracle.measure(q,&mut rng2).unwrap());
    }
    assert_eq!(rng1.next_u64(),rng2.next_u64());
}
#[test]
fn eleven_qubit_join_fails_atomically_without_truncating_entanglement(){
    let mut state=FactorState::zero(11,DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply(Gate::H(0)).unwrap();
    for q in 0..9 {
        state.apply(Gate::Cnot{control:q,target:q+1}).unwrap();
    }
    assert_eq!(state.factor_widths(),vec![10,1]);
    let sample=(0..11).map(|q|q%2==0).collect::<Vec<_>>();
    let before=state.amplitude_bits(&sample).unwrap();
    let err=state.apply(Gate::Cnot{control:9,target:10}).unwrap_err();
    assert_eq!(err,FactorError::FactorWidthExceeded{
        required:11,max:MAX_FACTOR_QUBITS,
    });
    assert_eq!(state.factor_widths(),vec![10,1]);
    assert_eq!(state.amplitude_bits(&sample).unwrap(),before);
}
#[test]
fn temporary_join_allocation_has_explicit_budget_and_no_half_applied_gate(){
    let mut state=FactorState::zero(2,96).unwrap();
    state.apply(Gate::H(0)).unwrap();
    let before=state.amplitude_bits(&[false,false]).unwrap();
    let err=state.apply(Gate::Cnot{control:0,target:1}).unwrap_err();
    assert_eq!(err,FactorError::BudgetExceeded{
        required_bytes:128,budget_bytes:96,
    });
    assert_eq!(state.factor_count(),2);
    assert_eq!(state.amplitude_bits(&[false,false]).unwrap(),before);
}
#[test]
fn invalid_aliased_and_out_of_bounds_gates_are_not_applied(){
    let mut state=FactorState::zero(3,DEFAULT_MEMORY_BUDGET).unwrap();
    for gate in [
        Gate::Cnot{control:1,target:1},
        Gate::Cz{control:2,target:2},
        Gate::Swap{a:0,b:0},
    ] {
        assert!(matches!(state.apply(gate),Err(FactorError::DuplicateQubit(_))));
    }
    assert_eq!(state.apply(Gate::T(99)).unwrap_err(),
        FactorError::InvalidQubit{qubit:99,qubits:3});
    assert_eq!(state.factor_count(),3);
    assert!(matches!(state.amplitude_bits(&[false]),
        Err(FactorError::InvalidBasisWidth{expected:3,got:1})));
}
#[test]
fn all_shots_preflight_rejects_classical_use_before_write_and_unreachable_gates(){
    let mut bad=FactorCircuit::new(2,1);
    bad.push(Instruction::If{bit:0,equals:false,gate:Gate::X(0)});
    assert_eq!(bad.execute_shots(1,0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        FactorError::ReadBeforeWrite{bit:0,instruction:0});
    let mut bad2=FactorCircuit::new(2,1);
    bad2.push(Instruction::Measure{qubit:0,into_bit:0});
    bad2.push(Instruction::If{bit:0,equals:true,gate:Gate::Swap{a:1,b:1}});
    assert!(matches!(
        bad2.execute_shots(1,0,DEFAULT_MEMORY_BUDGET),
        Err(FactorError::DuplicateQubit(1))
    ));
}
#[test]
fn work_shots_width_and_factor_initial_memory_are_bounded(){
    assert!(matches!(FactorState::zero(0,DEFAULT_MEMORY_BUDGET),
        Err(FactorError::UnsupportedWidth{..})));
    assert!(matches!(FactorState::zero(129,DEFAULT_MEMORY_BUDGET),
        Err(FactorError::UnsupportedWidth{..})));
    assert_eq!(FactorState::zero(128,4095).unwrap_err(),
        FactorError::BudgetExceeded{required_bytes:4096,budget_bytes:4095});
    let mut p=FactorCircuit::new(2,0);
    assert_eq!(p.execute_shots(0,0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        FactorError::ZeroShots);
    assert_eq!(p.execute_shots(MAX_FACTOR_SHOTS+1,0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        FactorError::TooManyShots);
    for _ in 0..256 {
        p.push(Instruction::Gate(Gate::X(0)));
    }
    assert_eq!(p.execute_shots(1024,0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        FactorError::WorkLimit);
}
