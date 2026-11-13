//! AURA-Q28-010 exact symbolic amplitude, relative phase and measurement
//! acceptance tests against analytic algebra and independent dense f64 oracle.
use aura_quantum_sim::symbolic::{
    ExactProbability, Root2, Root2Complex, SymbolicCircuit, SymbolicError,
    SymbolicInstruction, SymbolicState, SYMBOLIC_PROFILE, SYMBOLIC_RESULT_SCHEMA,
    MAX_SYMBOLIC_DENOMINATOR_POWER, MAX_SYMBOLIC_OPERATIONS,
};
use aura_quantum_sim::{Gate, Statevector, DEFAULT_MEMORY_BUDGET};

const EPS: f64 = 8e-12;
fn near(got: f64, expected: f64) {
    assert!((got - expected).abs() < EPS,
        "got {got:.17e}, expected {expected:.17e}");
}
fn next(s:&mut u64)->u64 {
    *s ^= *s >> 12; *s ^= *s << 25; *s ^= *s >> 27;
    s.wrapping_mul(0x2545f4914f6cdd1d)
}
fn gate(n:usize,s:&mut u64)->Gate {
    let choice = (next(s)%if n>1 {9} else {6}) as usize;
    let a = next(s) as usize % n;
    let b = if n>1 {(a + 1 + next(s) as usize % (n-1))%n} else {0};
    match choice {
        0=>Gate::H(a),1=>Gate::T(a),2=>Gate::S(a),
        3=>Gate::X(a),4=>Gate::Y(a),5=>Gate::Z(a),
        6=>Gate::Cnot{control:a,target:b},
        7=>Gate::Cz{control:a,target:b},
        _=>Gate::Swap{a,b},
    }
}

#[test]
fn hth_exact_nonclifford_interference_is_2_plus_or_minus_root2_over_four() {
    let mut s=SymbolicState::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    for g in [Gate::H(0),Gate::T(0),Gate::H(0)] {
        s.apply(g).unwrap();
    }
    assert_eq!(s.total_mass().unwrap(),
        ExactProbability{numerator:Root2::ONE,denominator_power_of_two:0});
    assert_eq!(s.basis_weight(0).unwrap(),
        ExactProbability{numerator:Root2::new(2,1),denominator_power_of_two:2});
    assert_eq!(s.basis_weight(1).unwrap(),
        ExactProbability{numerator:Root2::new(2,-1),denominator_power_of_two:2});
    near(s.basis_weight(0).unwrap().approximate(),
         (2.0+2.0_f64.sqrt())/4.0);
    near(s.basis_weight(1).unwrap().approximate(),
         (2.0-2.0_f64.sqrt())/4.0);
}

#[test]
fn randomized_all_nine_gate_families_match_dense_amplitudes_and_probabilities(){
    let mut checks=0usize;
    for n in 1..=4 {
        for case in 0..24 {
            let mut seed=0xc010_u64+case*17+n as u64*1001;
            let mut exact=SymbolicState::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let mut dense=Statevector::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            for _ in 0..30 {
                let g=gate(n,&mut seed);
                exact.apply(g.clone()).unwrap();
                dense.apply(g).unwrap();
            }
            assert_eq!(exact.total_mass().unwrap(),
                ExactProbability{numerator:Root2::ONE,denominator_power_of_two:0});
            for i in 0..(1usize<<n) {
                let (re,im)=exact.amplitude(i).unwrap().approximate();
                let oracle=dense.amplitude(i).unwrap();
                near(re,oracle.re);
                near(im,oracle.im);
                near(exact.basis_weight(i).unwrap().approximate(),
                     dense.basis_probability(i).unwrap());
                checks+=1;
            }
        }
    }
    assert_eq!(checks,24*(2+4+8+16));
}

#[test]
fn exact_relative_coherence_includes_non_clifford_complex_phase(){
    let mut s=SymbolicState::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    s.apply(Gate::H(0)).unwrap();
    s.apply(Gate::T(0)).unwrap();
    let c=s.coherence(0,1).unwrap();
    // |+_T> = [sqrt2,1+i]/2; coherence = (sqrt2 - i sqrt2)/4.
    assert_eq!(c.denominator_power_of_two,2);
    assert_eq!(c.numerator,Root2Complex::new(
        Root2::new(0,1),Root2::new(0,-1)));
    near(c.approximate().0,2f64.sqrt()/4.0);
    near(c.approximate().1,-2f64.sqrt()/4.0);
    let conjugate=s.coherence(1,0).unwrap();
    near(conjugate.approximate().0,c.approximate().0);
    near(conjugate.approximate().1,-c.approximate().1);
}

#[test]
fn exact_global_phase_ray_comparison_does_not_collapse_relative_phase(){
    let mut a=SymbolicState::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    a.apply(Gate::H(0)).unwrap();
    let mut b=a.clone();
    // XZX = -Z. The two states below differ by a global negative sign.
    a.apply(Gate::Z(0)).unwrap();
    b.apply(Gate::X(0)).unwrap();
    b.apply(Gate::Z(0)).unwrap();
    b.apply(Gate::X(0)).unwrap();
    assert!(a.same_ray(&b).unwrap());
    assert_ne!(a.amplitude(0).unwrap(),b.amplitude(0).unwrap());
    let mut c=SymbolicState::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    c.apply(Gate::H(0)).unwrap();
    assert!(!a.same_ray(&c).unwrap());
    assert_eq!(a.same_ray(&SymbolicState::zero(2,DEFAULT_MEMORY_BUDGET).unwrap())
        .unwrap_err(),SymbolicError::MismatchedWidth);
}

#[test]
fn forced_measurement_preserves_exact_subnormalized_branch_joint_weight(){
    let mut s=SymbolicState::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    for g in [Gate::H(0),Gate::T(0),Gate::H(0)] {
        s.apply(g).unwrap();
    }
    let selected=s.postselect(0,true).unwrap();
    assert_eq!(selected,ExactProbability{
        numerator:Root2::new(2,-1),denominator_power_of_two:2
    });
    assert_eq!(s.total_mass().unwrap(),selected);
    assert_eq!(s.basis_weight(0).unwrap().numerator,Root2::ZERO);
    s.apply(Gate::X(0)).unwrap();
    assert_eq!(s.total_mass().unwrap(),selected);
    assert_eq!(s.basis_weight(0).unwrap(),selected);
    assert_eq!(s.basis_weight(1).unwrap().numerator,Root2::ZERO);
}

#[test]
fn conditional_bell_postselection_yields_exact_joint_and_conditional_weights(){
    let mut s=SymbolicState::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    s.apply(Gate::H(0)).unwrap();
    s.apply(Gate::Cnot{control:0,target:1}).unwrap();
    let first=s.postselect(0,true).unwrap();
    assert_eq!(first,ExactProbability{
        numerator:Root2::new(1,0),denominator_power_of_two:1
    });
    let second=s.postselect(1,true).unwrap();
    assert_eq!(second,first);
    // Conditional Born probability for second bit is second / first = 1.
    near(second.approximate()/first.approximate(),1.0);
    assert_eq!(s.postselect(1,false).unwrap_err(),SymbolicError::ImpossibleOutcome);
    assert_eq!(s.total_mass().unwrap(),first);
}

#[test]
fn real_symbolic_classical_feedback_tracks_known_selected_branch(){
    let mut circuit=SymbolicCircuit::new(2,2);
    circuit.push(SymbolicInstruction::Gate(Gate::H(0)));
    circuit.push(SymbolicInstruction::Gate(Gate::Cnot{control:0,target:1}));
    circuit.push(SymbolicInstruction::Measure{qubit:0,into_bit:0,outcome:true});
    circuit.push(SymbolicInstruction::If{
        bit:0,equals:true,gate:Gate::X(1),
    });
    circuit.push(SymbolicInstruction::Measure{qubit:1,into_bit:1,outcome:false});
    let out=circuit.execute(DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(out.classical_bits,vec![Some(true),Some(false)]);
    assert_eq!(out.trace.len(),2);
    assert_eq!(out.trace[0].instruction,2);
    assert_eq!(out.trace[1].instruction,4);
    assert_eq!(out.trace[0].before_mass,ExactProbability{
        numerator:Root2::ONE,denominator_power_of_two:0
    });
    assert_eq!(out.trace[0].after_mass,ExactProbability{
        numerator:Root2::ONE,denominator_power_of_two:1
    });
    assert_eq!(out.trace[1].after_mass,out.trace[0].after_mass);
    assert_eq!(out.metadata.profile,SYMBOLIC_PROFILE);
    assert_eq!(out.metadata.schema,SYMBOLIC_RESULT_SCHEMA);
    assert_eq!(out.metadata.qpu_hardware_jobs,0);
    assert_eq!(out.metadata.evidence_class,"simulation");
}

#[test]
fn symbolic_selected_branch_probability_matches_dense_pure_reference(){
    for n in 1..=4 {
        let mut dense=Statevector::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
        let mut exact=SymbolicState::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
        for q in 0..n {
            dense.apply(Gate::H(q)).unwrap();
            exact.apply(Gate::H(q)).unwrap();
            dense.apply(Gate::T(q)).unwrap();
            exact.apply(Gate::T(q)).unwrap();
        }
        let dense_p=dense.wire_probability(0,true).unwrap();
        let p=exact.wire_weight(0,true).unwrap();
        near(p.approximate(),dense_p);
        exact.postselect(0,true).unwrap();
        dense.postselect(0,true).unwrap();
        for index in 0..(1usize<<n) {
            let expected=dense.basis_probability(index).unwrap();
            let exact_cond=exact.basis_weight(index).unwrap().approximate()
                /exact.total_mass().unwrap().approximate();
            near(exact_cond,expected);
        }
    }
}

#[test]
fn impossible_outcome_and_failed_validation_never_change_prior_state(){
    let mut s=SymbolicState::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    let before=(0..4).map(|i|s.amplitude(i).unwrap()).collect::<Vec<_>>();
    assert_eq!(s.postselect(0,true).unwrap_err(),SymbolicError::ImpossibleOutcome);
    assert_eq!(s.apply(Gate::Cnot{control:0,target:0}).unwrap_err(),
        SymbolicError::DuplicateQubit(0));
    assert_eq!(s.apply(Gate::T(4)).unwrap_err(),
        SymbolicError::InvalidQubit{index:4,qubits:2});
    let after=(0..4).map(|i|s.amplitude(i).unwrap()).collect::<Vec<_>>();
    assert_eq!(before,after);
}

#[test]
fn invalid_unreachable_gate_and_classical_use_before_write_fail_preflight(){
    let mut p=SymbolicCircuit::new(1,1);
    p.push(SymbolicInstruction::If{
        bit:0,equals:false,gate:Gate::X(0),
    });
    assert_eq!(p.execute(DEFAULT_MEMORY_BUDGET).unwrap_err(),
        SymbolicError::ReadBeforeWrite{bit:0,instruction:0});
    p=SymbolicCircuit::new(1,1);
    p.push(SymbolicInstruction::Measure{qubit:0,into_bit:0,outcome:false});
    p.push(SymbolicInstruction::If{
        bit:0,equals:true,gate:Gate::H(10),
    });
    assert_eq!(p.execute(DEFAULT_MEMORY_BUDGET).unwrap_err(),
        SymbolicError::InvalidQubit{index:10,qubits:1});
}

#[test]
fn width_memory_depth_and_instruction_limit_are_explicit(){
    assert!(matches!(SymbolicState::zero(0,DEFAULT_MEMORY_BUDGET),
        Err(SymbolicError::UnsupportedWidth{..})));
    assert!(matches!(SymbolicState::zero(5,DEFAULT_MEMORY_BUDGET),
        Err(SymbolicError::UnsupportedWidth{..})));
    let s=SymbolicState::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(SymbolicState::zero(2,s.logical_bytes()-1).unwrap_err(),
        SymbolicError::BudgetExceeded{
            required_bytes:s.logical_bytes(),budget_bytes:s.logical_bytes()-1,
        });
    let mut p=SymbolicCircuit::new(1,0);
    for _ in 0..=MAX_SYMBOLIC_DENOMINATOR_POWER {
        p.push(SymbolicInstruction::Gate(Gate::H(0)));
    }
    assert_eq!(p.execute(DEFAULT_MEMORY_BUDGET).unwrap_err(),
        SymbolicError::DepthExceeded);
    p=SymbolicCircuit::new(1,0);
    for _ in 0..=MAX_SYMBOLIC_OPERATIONS {
        p.push(SymbolicInstruction::Gate(Gate::X(0)));
    }
    assert_eq!(p.execute(DEFAULT_MEMORY_BUDGET).unwrap_err(),
        SymbolicError::TooManyOperations);
}
