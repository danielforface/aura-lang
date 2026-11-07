//! C-014: exact-eigenstate no-merge optimization. Verify full complex
//! amplitudes, product topology, honest resource gating and seeded runtime.
//! Product-preserving CX/CZ is NOT a probabilistic near-eigenstate guess.
use aura_quantum_sim::factorized::{FactorState,FactorCircuit,FactorError};
use aura_quantum_sim::circuit::{Circuit,Instruction};
use aura_quantum_sim::{Gate,Statevector,SeededRng,DEFAULT_MEMORY_BUDGET};

fn near(a:f64,b:f64) {
    assert!((a-b).abs()<5e-11,"{a:.16e} != {b:.16e}");
}
fn parity(a:&FactorState,b:&Statevector,n:usize) {
    for i in 0..(1usize<<n) {
        let bits=(0..n).map(|q|i>>q&1!=0).collect::<Vec<_>>();
        let x=a.amplitude_bits(&bits).unwrap();
        let y=b.amplitude(i).unwrap();
        near(x.re,y.re);
        near(x.im,y.im);
    }
}
fn apply_both(a:&mut FactorState,b:&mut Statevector,g:Gate) {
    a.apply(g.clone()).unwrap();
    b.apply(g).unwrap();
}
#[test]
fn no_merge_cx_with_exact_zero_control_on_128_fresh_targets() {
    let mut a=FactorState::zero(128,DEFAULT_MEMORY_BUDGET).unwrap();
    for q in 1..128 {
        a.apply(Gate::Cnot{control:0,target:q}).unwrap();
    }
    assert_eq!(a.factor_count(),128);
    assert_eq!(a.factor_widths(),vec![1;128]);
    assert_eq!(a.logical_state_bytes(),4096);
    assert_eq!(a.peak_logical_bytes(),4096);
    assert_eq!(a.product_preserving_two_wire_gates(),127);
    near(a.basis_probability(&[false;128]).unwrap(),1.0);
}
#[test]
fn no_merge_cx_with_exact_one_control_flips_127_fresh_targets() {
    let mut a=FactorState::zero(128,DEFAULT_MEMORY_BUDGET).unwrap();
    a.apply(Gate::X(0)).unwrap();
    for q in 1..128 {
        a.apply(Gate::Cnot{control:0,target:q}).unwrap();
    }
    assert_eq!(a.factor_count(),128);
    assert_eq!(a.logical_state_bytes(),4096);
    assert_eq!(a.product_preserving_two_wire_gates(),127);
    near(a.basis_probability(&[true;128]).unwrap(),1.0);
}
#[test]
fn no_merge_cz_on_exact_one_of_either_endpoint_preserves_complex_phase() {
    for n in 2..=8 {
        for scenario in 0..4 {
            let mut f=FactorState::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let mut d=Statevector::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let phase=match scenario {
                0=>vec![Gate::X(0),Gate::H(n-1)],
                1=>vec![Gate::H(0),Gate::X(n-1)],
                2=>vec![Gate::X(0),Gate::H(n-1),Gate::T(n-1)],
                _=>vec![Gate::H(0),Gate::X(n-1),Gate::T(0)],
            };
            for g in phase {apply_both(&mut f,&mut d,g);}
            apply_both(&mut f,&mut d,Gate::Cz{control:0,target:n-1});
            assert_eq!(f.factor_count(),n);
            assert_eq!(f.product_preserving_two_wire_gates(),1);
            parity(&f,&d,n);
        }
    }
}
#[test]
fn x_plus_and_x_minus_target_eigenstates_control_phase_without_join() {
    for n in 2..=8 {
        for negative in [false,true] {
            let mut a=FactorState::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let mut d=Statevector::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            apply_both(&mut a,&mut d,Gate::H(0));
            apply_both(&mut a,&mut d,Gate::H(n-1));
            if negative {
                apply_both(&mut a,&mut d,Gate::Z(n-1));
            }
            apply_both(&mut a,&mut d,Gate::Cnot{control:0,target:n-1});
            assert_eq!(a.factor_count(),n);
            assert_eq!(a.product_preserving_two_wire_gates(),1);
            parity(&a,&d,n);
            // Further non-Clifford local gates remain supported.
            apply_both(&mut a,&mut d,Gate::T(0));
            parity(&a,&d,n);
        }
    }
}
#[test]
fn full_factor_x_eigenstate_after_uncompute_does_not_join_remote_control() {
    for negative in [false,true] {
        let mut f=FactorState::zero(3,DEFAULT_MEMORY_BUDGET).unwrap();
        let mut d=Statevector::zero(3,DEFAULT_MEMORY_BUDGET).unwrap();
        let initial=[
            Gate::H(0),
            Gate::Cnot{control:0,target:1},
            Gate::Cnot{control:0,target:1}, // 2-wire factor, physically separable.
            Gate::H(2), // remote superposed control
        ];
        for g in initial {apply_both(&mut f,&mut d,g);}
        if negative {apply_both(&mut f,&mut d,Gate::Z(0));}
        assert_eq!(f.factor_count(),2);
        apply_both(&mut f,&mut d,Gate::Cnot{control:2,target:0});
        assert_eq!(f.factor_count(),2);
        assert_eq!(f.product_preserving_two_wire_gates(),1);
        parity(&f,&d,3);
    }
}
#[test]
fn cz_with_wide_entangled_target_and_known_1_control_does_not_exceed_cap() {
    let mut f=FactorState::zero(11,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut d=Statevector::zero(11,DEFAULT_MEMORY_BUDGET).unwrap();
    apply_both(&mut f,&mut d,Gate::H(0));
    for q in 1..10 {
        apply_both(&mut f,&mut d,Gate::Cnot{control:0,target:q});
    }
    apply_both(&mut f,&mut d,Gate::T(4));
    assert_eq!(f.factor_widths(),vec![10,1]);
    apply_both(&mut f,&mut d,Gate::X(10));
    apply_both(&mut f,&mut d,Gate::Cz{control:10,target:5});
    assert_eq!(f.factor_count(),2);
    assert_eq!(f.factor_widths(),vec![10,1]);
    assert_eq!(f.product_preserving_two_wire_gates(),1);
    parity(&f,&d,11);
    apply_both(&mut f,&mut d,Gate::Cnot{control:10,target:3});
    assert_eq!(f.factor_count(),2);
    assert_eq!(f.product_preserving_two_wire_gates(),2);
    parity(&f,&d,11);
}
#[test]
fn almost_eigenstate_remains_entangling_with_no_tolerance_based_downgrade() {
    let mut f=FactorState::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut d=Statevector::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    for g in [Gate::H(0),Gate::T(0),Gate::H(0)] {
        apply_both(&mut f,&mut d,g);
    }
    assert_eq!(f.factor_count(),2);
    apply_both(&mut f,&mut d,Gate::Cnot{control:0,target:1});
    assert_eq!(f.factor_count(),1);
    assert_eq!(f.product_preserving_two_wire_gates(),0);
    parity(&f,&d,2);
}
#[test]
fn low_memory_exact_control_runs_while_true_entanglement_fails_closed() {
    let mut f=FactorState::zero(2,64).unwrap();
    f.apply(Gate::Cnot{control:0,target:1}).unwrap(); // identity
    f.apply(Gate::X(0)).unwrap();
    f.apply(Gate::Cnot{control:0,target:1}).unwrap(); // X on target
    assert_eq!(f.factor_count(),2);
    assert_eq!(f.product_preserving_two_wire_gates(),2);
    near(f.basis_probability(&[true,true]).unwrap(),1.0);
    let mut g=FactorState::zero(2,64).unwrap();
    g.apply(Gate::H(0)).unwrap();
    let before=g.amplitude_bits(&[false,false]).unwrap();
    assert_eq!(g.apply(Gate::Cnot{control:0,target:1}).unwrap_err(),
        FactorError::BudgetExceeded{required_bytes:128,budget_bytes:64});
    assert_eq!(g.factor_count(),2);
    assert_eq!(g.product_preserving_two_wire_gates(),0);
    assert_eq!(g.amplitude_bits(&[false,false]).unwrap(),before);
}
#[test]
fn phase_gate_fanout_128q_512_instructions_keeps_all_factors_singleton() {
    let mut circuit=FactorCircuit::new(128,0);
    circuit.push(Instruction::Gate(Gate::X(0)));
    for q in 1..128 {
        circuit.push(Instruction::Gate(Gate::H(q)));
        circuit.push(Instruction::Gate(Gate::Cz{control:0,target:q}));
        circuit.push(Instruction::Gate(Gate::Cnot{control:0,target:q}));
        circuit.push(Instruction::Gate(Gate::H(q)));
    }
    for q in 1..=3 {circuit.push(Instruction::Gate(Gate::T(q)));}
    assert_eq!(circuit.instructions().len(),512);
    let batch=circuit.execute_shots(1,1709,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(batch.product_preserving_two_wire_gates,254);
    assert_eq!(batch.projective_wire_splits,0);
    assert_eq!(batch.peak_logical_state_bytes,4096);
    let s=batch.one_shot_state.unwrap();
    assert_eq!(s.factor_count(),128);
    assert_eq!(s.logical_state_bytes(),4096);
    near(s.basis_probability(&[true;128]).unwrap(),1.0);
}
#[test]
fn multi_shot_counts_are_real_per_executed_gate_and_do_not_draw_rng() {
    let mut factor=FactorCircuit::new(3,1);
    let mut reference=Circuit::new(3,1);
    let ops=[
        Instruction::Gate(Gate::X(0)),
        Instruction::Gate(Gate::H(1)),
        Instruction::Gate(Gate::Cz{control:0,target:1}),
        Instruction::Gate(Gate::Cnot{control:0,target:2}),
        Instruction::Measure{qubit:1,into_bit:0},
        Instruction::If{bit:0,equals:true,gate:Gate::Z(0)},
    ];
    for op in ops {factor.push(op.clone());reference.push(op);}
    let a=factor.execute_shots(64,150,DEFAULT_MEMORY_BUDGET).unwrap();
    let b=reference.execute_shots(64,150,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(a.records,b.shots);
    assert_eq!(a.product_preserving_two_wire_gates,128);
    assert_eq!(a.metadata.qpu_hardware_jobs,0);
    let one=factor.execute_shots(1,150,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(one.product_preserving_two_wire_gates,2);
    let dense=reference.execute_once(150,DEFAULT_MEMORY_BUDGET).unwrap();
    parity(one.one_shot_state.as_ref().unwrap(),&dense.final_state,3);
}
#[test]
fn invalid_or_aliased_qubits_remain_invalid_before_shortcut() {
    let mut f=FactorState::zero(4,DEFAULT_MEMORY_BUDGET).unwrap();
    for g in [
        Gate::Cnot{control:0,target:0},
        Gate::Cz{control:1,target:1},
        Gate::Cnot{control:12,target:2},
        Gate::Cz{control:2,target:12}
    ] {
        assert!(f.apply(g).is_err());
    }
    assert_eq!(f.product_preserving_two_wire_gates(),0);
    assert_eq!(f.factor_count(),4);
}
#[test]
fn reference_rng_stream_is_unchanged_by_conditional_optimized_gate_execution() {
    let mut f=FactorState::zero(4,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut d=Statevector::zero(4,DEFAULT_MEMORY_BUDGET).unwrap();
    for g in [
        Gate::H(0),Gate::H(1),Gate::Cnot{control:0,target:1},
        Gate::H(2),Gate::T(2),Gate::H(3),Gate::Cz{control:2,target:3},
    ] {apply_both(&mut f,&mut d,g);}
    let mut a=SeededRng::new(1601);
    let mut b=SeededRng::new(1601);
    assert_eq!(f.measure(0,&mut a).unwrap(),d.measure(0,&mut b).unwrap());
    let control=if f.wire_probability(0,true).unwrap()>0.5 {0} else {1};
    // The projection makes the measured wire exactly Z-classical.
    apply_both(&mut f,&mut d,Gate::Cnot{control:0,target:control});
    assert_eq!(f.measure(3,&mut a).unwrap(),d.measure(3,&mut b).unwrap());
    assert_eq!(a.next_u64(),b.next_u64());
    parity(&f,&d,4);
}
