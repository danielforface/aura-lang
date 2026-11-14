//! AURA-Q28-006: public Clifford tableau correctness, seed replay and
//! deterministic failure controls; independent statevector for small widths.
use aura_quantum_sim::circuit::{Circuit,Instruction};
use aura_quantum_sim::stabilizer::{
    StabilizerCircuit,StabilizerError,StabilizerEventKind,StabilizerInstruction,
    StabilizerTableau,STABILIZER_PROFILE,STABILIZER_RESULT_SCHEMA,
};
use aura_quantum_sim::{Gate,Statevector,SeededRng,DEFAULT_MEMORY_BUDGET};

const EPS:f64=2e-11;
fn close(a:f64,b:f64){assert!((a-b).abs()<=EPS,"{a:.16e} != {b:.16e}");}
fn bits(n:usize,index:usize)->Vec<bool>{
    (0..n).map(|q|index&(1usize<<q)!=0).collect()
}
fn random_u64(seed:&mut u64)->u64 {
    *seed^=*seed>>12;
    *seed^=*seed<<25;
    *seed^=*seed>>27;
    seed.wrapping_mul(0x2545f4914f6cdd1d)
}
fn gate(n:usize,seed:&mut u64)->Gate {
    let k=random_u64(seed)%if n>1 {8}else{5};
    let a=(random_u64(seed) as usize)%n;
    let b=if n>1 { (a+1+(random_u64(seed) as usize)%(n-1))%n }else{0};
    match k {
        0=>Gate::H(a),1=>Gate::S(a),2=>Gate::X(a),3=>Gate::Y(a),
        4=>Gate::Z(a),5=>Gate::Cnot{control:a,target:b},
        6=>Gate::Cz{control:a,target:b},_=>Gate::Swap{a,b},
    }
}
#[test]
fn randomized_9_family_clifford_reference_matches_1000_plus_dense_born_probabilities(){
    let mut compared=0usize;
    for n in 1..=5 {
        for case in 0..20 {
            let mut seed=0xc001_1001_u64^(n as u64*1000+case as u64);
            let mut exact=StabilizerTableau::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            let mut dense=Statevector::zero(n,DEFAULT_MEMORY_BUDGET).unwrap();
            for _ in 0..40 {
                let g=gate(n,&mut seed);
                exact.apply(g.clone()).unwrap();
                dense.apply(g).unwrap();
            }
            for index in 0..(1usize<<n) {
                close(exact.basis_probability(&bits(n,index)).unwrap(),
                      dense.basis_probability(index).unwrap());
                compared+=1;
            }
        }
    }
    assert!(compared>=1200, "insufficient differential checks {compared}");
}
#[test]
fn stabilizer_generators_are_signed_pauli_strings_in_msb_order(){
    let mut t=StabilizerTableau::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(t.stabilizer_generators(),vec!["+IZ","+ZI"]);
    t.apply(Gate::H(0)).unwrap();
    t.apply(Gate::Cnot{control:0,target:1}).unwrap();
    assert_eq!(t.stabilizer_generators(),vec!["+XX","+ZZ"]);
    close(t.basis_probability(&[false,false]).unwrap(),0.5);
    close(t.basis_probability(&[true,true]).unwrap(),0.5);
    close(t.basis_probability(&[false,true]).unwrap(),0.0);
    close(t.basis_probability(&[true,false]).unwrap(),0.0);
}
#[test]
fn ghz_interference_and_projective_branching_are_exact(){
    let mut t=StabilizerTableau::zero(5,DEFAULT_MEMORY_BUDGET).unwrap();
    t.apply(Gate::H(0)).unwrap();
    for q in 1..5{t.apply(Gate::Cnot{control:q-1,target:q}).unwrap();}
    close(t.basis_probability(&[false;5]).unwrap(),0.5);
    close(t.basis_probability(&[true;5]).unwrap(),0.5);
    let mut s=t.clone();
    close(s.postselect(0,true).unwrap(),0.5);
    close(s.postselect(4,true).unwrap(),1.0);
    assert_eq!(s.postselect(3,false).unwrap_err(),StabilizerError::ImpossibleOutcome);
    close(s.basis_probability(&[true;5]).unwrap(),1.0);
    // Postselecting a successful stabilizer eigenvalue is idempotent.
    close(s.postselect(4,true).unwrap(),1.0);
}
#[test]
fn phase_and_sign_tables_cover_y_and_clifford_inverse_identities(){
    let mut t=StabilizerTableau::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    t.apply(Gate::H(0)).unwrap();
    t.apply(Gate::S(0)).unwrap();
    assert_eq!(t.stabilizer_generators(),vec!["+Y"]);
    t.apply(Gate::H(0)).unwrap();
    assert_eq!(t.stabilizer_generators(),vec!["-Y"]);
    for _ in 0..4{t.apply(Gate::S(0)).unwrap();}
    assert_eq!(t.stabilizer_generators(),vec!["-Y"]);
    t.apply(Gate::Y(0)).unwrap();
    assert_eq!(t.stabilizer_generators(),vec!["-Y"]);
    t.apply(Gate::X(0)).unwrap();
    assert_eq!(t.stabilizer_generators(),vec!["+Y"]);
    t.apply(Gate::Z(0)).unwrap();
    assert_eq!(t.stabilizer_generators(),vec!["-Y"]);
    let mut x=StabilizerTableau::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    x.apply(Gate::H(0)).unwrap();
    x.apply(Gate::Cz{control:0,target:1}).unwrap();
    x.apply(Gate::Cz{control:0,target:1}).unwrap();
    x.apply(Gate::H(0)).unwrap();
    assert_eq!(x.stabilizer_generators(),vec!["+IZ","+ZI"]);
}
#[test]
fn pure_reference_and_stabilizer_shots_agree_on_seeded_adaptive_bell_feedback(){
    let mut ref_pure=Circuit::new(2,2);
    let mut tableau=StabilizerCircuit::new(2,2);
    let gates=[Gate::H(0),Gate::Cnot{control:0,target:1},Gate::S(1)];
    for gate in gates {
        ref_pure.push(Instruction::Gate(gate.clone()));
        tableau.push(StabilizerInstruction::Gate(gate));
    }
    ref_pure.push(Instruction::Measure{qubit:0,into_bit:0});
    tableau.push(StabilizerInstruction::Measure{qubit:0,into_bit:0});
    ref_pure.push(Instruction::If{bit:0,equals:true,gate:Gate::X(1)});
    tableau.push(StabilizerInstruction::If{bit:0,equals:true,gate:Gate::X(1)});
    ref_pure.push(Instruction::Reset{qubit:0});
    tableau.push(StabilizerInstruction::Reset{qubit:0});
    ref_pure.push(Instruction::Measure{qubit:1,into_bit:1});
    tableau.push(StabilizerInstruction::Measure{qubit:1,into_bit:1});
    for seed in 0..256 {
        let p=ref_pure.execute_once(seed,DEFAULT_MEMORY_BUDGET).unwrap();
        let s=tableau.execute_once(seed,DEFAULT_MEMORY_BUDGET).unwrap();
        assert_eq!(p.shot.bits,s.shot.bits);
        assert_eq!(p.shot.trace.len(),s.shot.trace.len());
        for (a,b) in p.shot.trace.iter().zip(&s.shot.trace) {
            assert_eq!(a.instruction,b.instruction);
            assert_eq!(a.qubit,b.qubit);
            match (&a.kind,&b.kind) {
                (aura_quantum_sim::circuit::TraceKind::Measure{into_bit:i,value:v},
                 StabilizerEventKind::Measure{into_bit:j,value:w})=>
                    assert_eq!((*i,*v),(*j,*w)),
                (aura_quantum_sim::circuit::TraceKind::Reset{previous_value:a},
                 StabilizerEventKind::Reset{previous_value:b})=>
                    assert_eq!(a,b),
                _=>panic!("event ordering or kind drift"),
            }
        }
        for basis in 0..4 {
            close(p.final_state.basis_probability(basis).unwrap(),
                  s.final_tableau.basis_probability(&bits(2,basis)).unwrap());
        }
        assert_eq!(s.metadata.profile,STABILIZER_PROFILE);
        assert_eq!(s.metadata.schema_version,STABILIZER_RESULT_SCHEMA);
        assert_eq!(s.metadata.evidence_class,"simulation");
        assert_eq!(s.metadata.qpu_hardware_jobs,0);
    }
}
#[test]
fn seeded_4096_shot_bell_marginal_is_reproducible_and_statistically_bounded(){
    let mut c=StabilizerCircuit::new(2,1);
    c.push(StabilizerInstruction::Gate(Gate::H(0)));
    c.push(StabilizerInstruction::Gate(Gate::Cnot{control:0,target:1}));
    c.push(StabilizerInstruction::Measure{qubit:0,into_bit:0});
    let a=c.execute_shots(4096,0xc009,DEFAULT_MEMORY_BUDGET).unwrap();
    let b=c.execute_shots(4096,0xc009,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(a,b);
    let yes=a.records.iter().filter(|s|s.bits[0]==Some(true)).count();
    assert!((yes as f64-2048.0).abs()<6.0*(1024.0_f64).sqrt());
    assert_ne!(a.records,c.execute_shots(4096,0xc00a,DEFAULT_MEMORY_BUDGET).unwrap().records);
}
#[test]
fn unsupported_non_clifford_even_in_never_taken_branch_fails_before_execution(){
    let mut c=StabilizerCircuit::new(1,1);
    c.push(StabilizerInstruction::Measure{qubit:0,into_bit:0});
    c.push(StabilizerInstruction::If{bit:0,equals:true,gate:Gate::T(0)});
    assert_eq!(c.execute_once(4,DEFAULT_MEMORY_BUDGET).unwrap_err(),
               StabilizerError::UnsupportedNonClifford);
    let mut t=StabilizerTableau::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    let before=t.stabilizer_generators();
    assert_eq!(t.apply(Gate::T(0)).unwrap_err(),StabilizerError::UnsupportedNonClifford);
    assert_eq!(t.stabilizer_generators(),before);
}
#[test]
fn invalid_target_alias_and_uninitialized_classical_read_fail_closed(){
    let mut c=StabilizerCircuit::new(2,1);
    c.push(StabilizerInstruction::If{bit:0,equals:false,gate:Gate::X(0)});
    assert_eq!(c.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
               StabilizerError::ReadBeforeWrite{bit:0,instruction:0});
    c=StabilizerCircuit::new(2,1);
    c.push(StabilizerInstruction::Gate(Gate::Swap{a:0,b:0}));
    assert_eq!(c.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
               StabilizerError::DuplicateQubit(0));
    c=StabilizerCircuit::new(2,1);
    c.push(StabilizerInstruction::Gate(Gate::Cnot{control:0,target:2}));
    assert_eq!(c.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
               StabilizerError::InvalidQubit{qubit:2,qubits:2});
    c=StabilizerCircuit::new(2,1);
    c.push(StabilizerInstruction::Measure{qubit:0,into_bit:1});
    assert_eq!(c.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
               StabilizerError::InvalidClassicalBit{bit:1,bits:1});
}
#[test]
fn logical_memory_is_polynomial_at_96_qubits_and_state_is_still_exact(){
    let mut t=StabilizerTableau::zero(96,DEFAULT_MEMORY_BUDGET).unwrap();
    assert!(t.logical_bytes()<64*1024,
        "tableau unexpectedly exponential: {}",t.logical_bytes());
    t.apply(Gate::H(0)).unwrap();
    for q in 1..96 {
        t.apply(Gate::Cnot{control:q-1,target:q}).unwrap();
    }
    let all_false=vec![false;96];
    let all_true=vec![true;96];
    close(t.basis_probability(&all_false).unwrap(),0.5);
    close(t.basis_probability(&all_true).unwrap(),0.5);
    let mut mixed=all_true.clone();
    mixed[48]=false;
    close(t.basis_probability(&mixed).unwrap(),0.0);
    assert_eq!(t.stabilizer_generators().len(),96);
}
#[test]
fn budget_shots_width_and_bit_width_rejections_are_structured(){
    assert!(matches!(StabilizerTableau::zero(257,DEFAULT_MEMORY_BUDGET),
        Err(StabilizerError::UnsupportedWidth{qubits:257,max:256})));
    assert!(matches!(StabilizerTableau::zero(0,DEFAULT_MEMORY_BUDGET),
        Err(StabilizerError::UnsupportedWidth{qubits:0,max:256})));
    let t=StabilizerTableau::zero(4,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(StabilizerTableau::zero(4,t.logical_bytes()-1).unwrap_err(),
               StabilizerError::BudgetExceeded{
                 required_bytes:t.logical_bytes(),budget_bytes:t.logical_bytes()-1
               });
    assert_eq!(t.basis_probability(&[false]).unwrap_err(),
               StabilizerError::InvalidBasisWidth{expected:4,got:1});
    let c=StabilizerCircuit::new(2,0);
    assert_eq!(c.execute_shots(0,1,DEFAULT_MEMORY_BUDGET).unwrap_err(),
               StabilizerError::ZeroShots);
    assert_eq!(c.execute_shots(4097,1,DEFAULT_MEMORY_BUDGET).unwrap_err(),
               StabilizerError::TooManyShots);
}
#[test]
fn repeated_projective_measurements_consume_rng_without_randomizing_eigenstates(){
    let mut x=StabilizerTableau::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    x.apply(Gate::H(0)).unwrap();
    let mut rng=SeededRng::new(0x90);
    let first=x.measure(0,&mut rng).unwrap();
    for _ in 0..8 {assert_eq!(x.measure(0,&mut rng).unwrap(),first);}
    assert_eq!(x.reset_observed(0,&mut rng).unwrap(),first);
    close(x.basis_probability(&[false]).unwrap(),1.0);
    let source=StabilizerTableau::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut a=SeededRng::new(99);
    let mut b=SeededRng::new(99);
    let mut s=source.clone();
    assert!(!s.measure(0,&mut a).unwrap());
    let _=b.next_f64();
    assert_eq!(a.next_u64(),b.next_u64());
}
