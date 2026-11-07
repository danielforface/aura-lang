//! C-013: observed Z projection *guarantees* release of the measured wire.
//! No Schmidt truncation; compare full complex phases with the dense reference.
use aura_quantum_sim::circuit::{Circuit,Instruction,TraceKind};
use aura_quantum_sim::factorized::{FactorCircuit,FactorState};
use aura_quantum_sim::{Gate,SeededRng,Statevector,DEFAULT_MEMORY_BUDGET};

fn near(a:f64,b:f64) {
    assert!((a-b).abs()<4e-11,"{a:.16e} != {b:.16e}");
}
fn equal_amplitudes(a:&FactorState,b:&Statevector,n:usize) {
    for i in 0..(1usize<<n) {
        let bits=(0..n).map(|q|((i>>q)&1)!=0).collect::<Vec<_>>();
        let x=a.amplitude_bits(&bits).unwrap();
        let y=b.amplitude(i).unwrap();
        near(x.re,y.re);
        near(x.im,y.im);
    }
}
fn streaming_bus(n:usize,with_t:bool)->FactorCircuit {
    let mut program=FactorCircuit::new(n,1);
    for q in 1..n {
        program.push(Instruction::Gate(Gate::H(0)));
        program.push(Instruction::Gate(Gate::Cnot{control:0,target:q}));
        program.push(Instruction::Measure{qubit:0,into_bit:0});
        program.push(Instruction::If{bit:0,equals:true,gate:Gate::X(0)});
    }
    if with_t {
        for q in 1..=4.min(n-1) {
            program.push(Instruction::Gate(Gate::T(q)));
        }
    }
    program
}
#[test]
fn reuse_127_times_on_128_wires_with_nonclifford_t_and_no_entanglement_truncation() {
    let program=streaming_bus(128,true);
    assert_eq!(program.instructions().len(),512);
    let result=program.execute_shots(1,20261107,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(result.records.len(),1);
    assert_eq!(result.records[0].trace.len(),127);
    assert_eq!(result.projective_wire_splits,127);
    assert_eq!(result.projective_split_deferred,0);
    assert_eq!(result.metadata.qpu_hardware_jobs,0);
    let state=result.one_shot_state.unwrap();
    assert_eq!(state.factor_count(),128);
    assert_eq!(state.factor_widths(),vec![1;128]);
    assert_eq!(state.projective_wire_splits(),127);
    assert_eq!(state.logical_state_bytes(),4096);
    assert!(state.peak_logical_bytes()<=5000);
    // Projective measurement outcomes on each fresh q correlate with the
    // corresponding trace; q0 is reset to |0> after every measurement.
    let mut selected=vec![false;128];
    for (idx,entry) in result.records[0].trace.iter().enumerate() {
        assert_eq!(entry.qubit,0);
        assert_eq!(entry.instruction,idx*4+2);
        match entry.kind {
            TraceKind::Measure{into_bit,value}=>{
                assert_eq!(into_bit,0);
                selected[idx+1]=value;
            }
            TraceKind::Reset{..}=>panic!("bus uses explicit classical feedback"),
        }
    }
    assert_eq!(result.records[0].bits[0],Some(selected[127]));
    near(state.basis_probability(&selected).unwrap(),1.0);
    assert_eq!(state.wire_probability(0,true).unwrap(),0.0);
}
#[test]
fn full_complex_phase_after_every_measured_wire_recycle_matches_dense() {
    for n in 2..=8 {
        for case in 0..12 {
            let seed=0xCAFE_9911u64+n as u64*1009+case;
            let mut split=FactorState::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let mut dense=Statevector::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            for gate in std::iter::once(Gate::H(0))
                .chain((1..n).map(|q|Gate::Cnot{control:0,target:q}))
                .chain([Gate::T(n-1),Gate::S(0)])
            {
                split.apply(gate.clone()).unwrap();
                dense.apply(gate).unwrap();
            }
            let q=case as usize %n;
            let mut a=SeededRng::new(seed);
            let mut b=SeededRng::new(seed);
            let expected=dense.measure(q,&mut b).unwrap();
            let actual=split.measure(q,&mut a).unwrap();
            assert_eq!(actual,expected);
            assert_eq!(split.projective_wire_splits(),1);
            assert_eq!(split.projective_split_deferred(),0);
            assert_eq!(split.factor_count(),2);
            assert_eq!(a.next_u64(),b.next_u64());
            equal_amplitudes(&split,&dense,n);
            // Rejoin the same wire to a formerly entangled factor and
            // compare the complete interference-bearing quantum state again.
            let target=(q+1)%n;
            let gate=Gate::Cnot{control:q,target};
            split.apply(gate.clone()).unwrap();
            dense.apply(gate).unwrap();
            equal_amplitudes(&split,&dense,n);
            assert_eq!(split.factor_count(),1);
        }
    }
}
#[test]
fn ten_wire_entanglement_after_measurement_frees_target_for_new_qubit() {
    let mut state=FactorState::zero(11,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut dense=Statevector::zero(11,DEFAULT_MEMORY_BUDGET).unwrap();
    for gate in std::iter::once(Gate::H(0))
        .chain((1..10).map(|q|Gate::Cnot{control:0,target:q}))
        .chain([Gate::T(4)])
    {
        state.apply(gate.clone()).unwrap();
        dense.apply(gate).unwrap();
    }
    assert_eq!(state.factor_widths(),vec![10,1]);
    let mut a=SeededRng::new(37);
    let mut b=SeededRng::new(37);
    let measured=state.measure(0,&mut a).unwrap();
    assert_eq!(measured,dense.measure(0,&mut b).unwrap());
    assert_eq!(state.factor_widths(),vec![9,1,1]);
    assert_eq!(state.projective_wire_splits(),1);
    let gate=Gate::Cnot{control:0,target:10};
    state.apply(gate.clone()).unwrap();
    dense.apply(gate).unwrap();
    assert_eq!(state.factor_widths(),vec![9,2]);
    equal_amplitudes(&state,&dense,11);
}
#[test]
fn strict_peak_budget_defers_optional_split_without_corrupting_collapsed_state() {
    // Two 2q factors can merge into a 4q factor with temp=384 bytes,
    // but splitting any wire in the new 4q factor needs temp=416.
    let mut state=FactorState::zero(4,400).unwrap();
    let mut dense=Statevector::zero(4,400).unwrap();
    for gate in [
        Gate::H(0),Gate::H(2),
        Gate::Cnot{control:0,target:1},
        Gate::Cnot{control:2,target:3},
        Gate::Cz{control:1,target:2},
    ] {
        state.apply(gate.clone()).unwrap();
        dense.apply(gate).unwrap();
    }
    assert_eq!(state.factor_count(),1);
    assert_eq!(state.peak_logical_bytes(),384);
    let mut a=SeededRng::new(2026);
    let mut b=SeededRng::new(2026);
    assert_eq!(state.measure(1,&mut a).unwrap(),dense.measure(1,&mut b).unwrap());
    assert_eq!(state.factor_count(),1);
    assert_eq!(state.projective_wire_splits(),0);
    assert_eq!(state.projective_split_deferred(),1);
    assert_eq!(state.peak_logical_bytes(),384);
    equal_amplitudes(&state,&dense,4);
}
#[test]
fn measuring_already_isolated_factor_is_a_noop_for_topology_and_preserves_rng() {
    let mut state=FactorState::zero(3,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut dense=Statevector::zero(3,DEFAULT_MEMORY_BUDGET).unwrap();
    state.apply(Gate::H(1)).unwrap();
    dense.apply(Gate::H(1)).unwrap();
    let mut a=SeededRng::new(7788);
    let mut b=SeededRng::new(7788);
    assert_eq!(state.measure(1,&mut a).unwrap(),dense.measure(1,&mut b).unwrap());
    assert_eq!(state.projective_wire_splits(),0);
    assert_eq!(state.projective_split_deferred(),0);
    assert_eq!(state.factor_count(),3);
    assert_eq!(a.next_u64(),b.next_u64());
    equal_amplitudes(&state,&dense,3);
}
#[test]
fn reset_recycles_entangled_observed_bus_on_eleven_wires() {
    let mut factor=FactorCircuit::new(11,1);
    let mut reference=Circuit::new(11,1);
    for q in 1..11 {
        for op in [
            Instruction::Gate(Gate::H(0)),
            Instruction::Gate(Gate::Cnot{control:0,target:q}),
            Instruction::Reset{qubit:0},
        ] {
            factor.push(op.clone());
            reference.push(op);
        }
    }
    let seed=4401u64;
    let a=factor.execute_shots(16,seed,DEFAULT_MEMORY_BUDGET).unwrap();
    let b=reference.execute_shots(16,seed,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(a.records,b.shots);
    assert_eq!(a.projective_wire_splits,16*10);
    assert_eq!(a.projective_split_deferred,0);
    let a1=factor.execute_shots(1,seed,DEFAULT_MEMORY_BUDGET).unwrap();
    let b1=reference.execute_once(seed,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(a1.records[0],b1.shot);
    let state=a1.one_shot_state.unwrap();
    assert_eq!(state.factor_count(),11);
    equal_amplitudes(&state,&b1.final_state,11);
}
#[test]
fn repeated_projective_measurement_of_same_wire_does_not_invent_split_or_rng() {
    let mut factor=FactorState::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut reference=Statevector::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    for g in [Gate::H(0),Gate::Cnot{control:0,target:1}] {
        factor.apply(g.clone()).unwrap();
        reference.apply(g).unwrap();
    }
    let mut ra=SeededRng::new(456);
    let mut rb=SeededRng::new(456);
    for repeat in 0..12 {
        assert_eq!(factor.measure(0,&mut ra).unwrap(),
                   reference.measure(0,&mut rb).unwrap());
        assert_eq!(factor.projective_wire_splits(),1);
        assert_eq!(factor.factor_count(),2);
        assert_eq!(factor.projective_split_deferred(),0);
        assert_eq!(ra.next_u64(),rb.next_u64());
        equal_amplitudes(&factor,&reference,2);
        assert_eq!(repeat < 12,true);
    }
}
#[test]
fn dropped_temporary_split_does_not_reinterpret_classical_result() {
    let mut factor=FactorCircuit::new(4,1);
    factor.push(Instruction::Gate(Gate::H(0)));
    factor.push(Instruction::Gate(Gate::H(2)));
    factor.push(Instruction::Gate(Gate::Cnot{control:0,target:1}));
    factor.push(Instruction::Gate(Gate::Cnot{control:2,target:3}));
    factor.push(Instruction::Gate(Gate::Cz{control:1,target:2}));
    factor.push(Instruction::Measure{qubit:1,into_bit:0});
    factor.push(Instruction::If{bit:0,equals:true,gate:Gate::X(0)});
    // Logical bytes state256 + output (ShotRecord+Option<bool>+Trace)
    // fit below 600 while the optional split still cannot fit when
    // state budget after reserving output is set carefully.
    let exact=factor.execute_shots(1,6789,600).unwrap();
    assert_eq!(exact.records[0].trace.len(),1);
    assert_eq!(exact.records[0].bits[0].is_some(),true);
    assert_eq!(exact.projective_wire_splits+exact.projective_split_deferred,1);
}
