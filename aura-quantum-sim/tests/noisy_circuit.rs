//! End-to-end Q28-007/011 mixed-state instruments and adaptive noisy circuits.
use aura_quantum_sim::circuit::{Circuit, Instruction};
use aura_quantum_sim::density::{DensityError, DensityMatrix, NoiseChannel};
use aura_quantum_sim::noisy_circuit::{
    NoisyAction, NoisyCircuit, NoisyCircuitError, NoisyEventKind, NoisyInstruction,
    NOISY_DYNAMIC_PROFILE, NOISY_RESULT_SCHEMA,
};
use aura_quantum_sim::{
    Complex, Gate, SeededRng, DEFAULT_MEMORY_BUDGET, ENGINE_VERSION,
};

const EPS: f64 = 4e-11;
fn close(a: f64, b: f64) { assert!((a - b).abs() <= EPS, "{a:e} != {b:e}"); }
fn ceq(a: Complex, b: Complex) { close(a.re,b.re); close(a.im,b.im); }
fn noisy_bell() -> NoisyCircuit {
    let mut c = NoisyCircuit::new(2, 2);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Gate(Gate::Cnot {control:0,target:1}));
    c
}

#[test]
fn born_marginals_and_conditional_bell_instrument_match_expected_density() {
    let mut rho = DensityMatrix::zero(2, DEFAULT_MEMORY_BUDGET).unwrap();
    rho.apply_gate(Gate::H(0)).unwrap();
    rho.apply_gate(Gate::Cnot {control:0,target:1}).unwrap();
    close(rho.wire_probability(0,true).unwrap(),0.5);
    close(rho.wire_probability(1,false).unwrap(),0.5);
    let mut selected = rho.clone();
    selected.postselect(0,true).unwrap();
    close(selected.basis_probability(3).unwrap(),1.0);
    close(selected.purity().unwrap(),1.0);
    selected.postselect(1,true).unwrap();
    close(selected.basis_probability(3).unwrap(),1.0);
    assert_eq!(selected.postselect(0,false).unwrap_err(),DensityError::ImpossibleOutcome);
    close(selected.basis_probability(3).unwrap(),1.0);
    let mut alternate=rho.clone();
    alternate.postselect(1,false).unwrap();
    alternate.postselect(0,false).unwrap();
    close(alternate.basis_probability(0).unwrap(),1.0);
    ceq(rho.element(0,3).unwrap(),Complex::new(0.5,0.0));
}

#[test]
fn unobserved_reset_is_nonselective_cptp_and_retains_other_wire_reduced_state() {
    let mut rho = DensityMatrix::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    rho.apply_gate(Gate::H(0)).unwrap();
    rho.apply_gate(Gate::Cnot{control:0,target:1}).unwrap();
    let before = rho.reduced_qubit(1).unwrap();
    rho.reset_unobserved(0).unwrap();
    for (i,row) in before.iter().enumerate() {
        for (j,value) in row.iter().enumerate() {
            ceq(*value,rho.reduced_qubit(1).unwrap()[i][j]);
        }
    }
    close(rho.basis_probability(0).unwrap(),0.5);
    close(rho.basis_probability(2).unwrap(),0.5);
    close(rho.basis_probability(1).unwrap(),0.0);
    close(rho.basis_probability(3).unwrap(),0.0);
    close(rho.purity().unwrap(),0.5);
    close(rho.trace(),1.0);
    // Idempotent and does not create a random outcome.
    let prev = (0..4).flat_map(|r| (0..4).map(move |c| (r,c)))
        .map(|(r,c)|rho.element(r,c).unwrap()).collect::<Vec<_>>();
    rho.reset_unobserved(0).unwrap();
    let after = (0..4).flat_map(|r| (0..4).map(move |c| (r,c)))
        .map(|(r,c)|rho.element(r,c).unwrap()).collect::<Vec<_>>();
    assert_eq!(prev,after);
}

#[test]
fn measurement_then_classical_feedback_then_unobserved_reset_is_real_dynamic_behavior() {
    let mut circuit=noisy_bell();
    circuit.push(NoisyInstruction::Noise(NoiseChannel::PhaseFlip{
        qubit:1,probability:0.5,
    }));
    circuit.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    circuit.push(NoisyInstruction::If{
        bit:0,equals:true,action:NoisyAction::Gate(Gate::X(1)),
    });
    circuit.push(NoisyInstruction::Reset{qubit:0});
    circuit.push(NoisyInstruction::Measure{qubit:1,into_bit:1});
    for seed in 0..128u64 {
        let output=circuit.execute_once(seed,DEFAULT_MEMORY_BUDGET).unwrap();
        assert_eq!(output.shot.bits[1],Some(false));
        close(output.final_density.basis_probability(0).unwrap(),1.0);
        assert_eq!(output.shot.trace.len(),3);
        assert_eq!(output.shot.trace[0].instruction,3);
        assert_eq!(output.shot.trace[1].kind,NoisyEventKind::ResetUnobserved);
        assert_eq!(output.shot.trace[1].instruction,5);
        assert_eq!(output.shot.trace[2].kind,
            NoisyEventKind::Measure{into_bit:1,value:false});
        assert_eq!(output.metadata.schema_version,NOISY_RESULT_SCHEMA);
        assert_eq!(output.metadata.profile,NOISY_DYNAMIC_PROFILE);
        assert_eq!(output.metadata.engine_version,ENGINE_VERSION);
        assert_eq!(output.metadata.evidence_class,"simulation");
        assert_eq!(output.metadata.qpu_hardware_jobs,0);
    }
}

#[test]
fn phase_flip_noise_is_ensemble_map_not_fake_random_sampled_phase() {
    let mut c=NoisyCircuit::new(1,0);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Noise(NoiseChannel::PhaseFlip{
        qubit:0,probability:0.5,
    }));
    let result=c.execute_once(9,DEFAULT_MEMORY_BUDGET).unwrap();
    close(result.final_density.basis_probability(0).unwrap(),0.5);
    close(result.final_density.basis_probability(1).unwrap(),0.5);
    ceq(result.final_density.element(0,1).unwrap(),Complex::ZERO);
    close(result.final_density.purity().unwrap(),0.5);
    assert!(result.shot.trace.is_empty());
    assert!(result.shot.bits.is_empty());
}

#[test]
fn conditioned_noise_is_reachable_and_nontrivial_even_without_measurement_of_target() {
    let mut c=NoisyCircuit::new(2,1);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Gate(Gate::H(1)));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    c.push(NoisyInstruction::If{bit:0,equals:true,
        action:NoisyAction::Noise(NoiseChannel::AmplitudeDamping{qubit:1,gamma:1.0})});
    let mut true_seen=false;
    let mut false_seen=false;
    for seed in 0..128 {
        let result=c.execute_once(seed,DEFAULT_MEMORY_BUDGET).unwrap();
        if result.shot.bits[0]==Some(true) {
            true_seen=true;
            close(result.final_density.wire_probability(1,false).unwrap(),1.0);
            close(result.final_density.purity().unwrap(),1.0);
        } else {
            false_seen=true;
            close(result.final_density.wire_probability(1,false).unwrap(),0.5);
        }
    }
    assert!(true_seen&&false_seen);
}

#[test]
fn noisy_no_noise_observed_reset_matches_existing_pure_runner_across_seeded_shots() {
    let mut pure=Circuit::new(2,1);
    let mut mixed=NoisyCircuit::new(2,1);
    let gates=[
        Gate::H(0),Gate::T(0),Gate::H(1),
        Gate::Cnot{control:0,target:1},
    ];
    for g in gates {
        pure.push(Instruction::Gate(g.clone()));
        mixed.push(NoisyInstruction::Gate(g));
    }
    pure.push(Instruction::Measure{qubit:0,into_bit:0});
    mixed.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    pure.push(Instruction::If{bit:0,equals:true,gate:Gate::X(1)});
    mixed.push(NoisyInstruction::If{bit:0,equals:true,
        action:NoisyAction::Gate(Gate::X(1))});
    pure.push(Instruction::Reset{qubit:0});
    mixed.push(NoisyInstruction::ResetObserved{qubit:0});
    for seed in 0..128u64 {
        let p=pure.execute_once(seed,DEFAULT_MEMORY_BUDGET).unwrap();
        let m=mixed.execute_once(seed,DEFAULT_MEMORY_BUDGET).unwrap();
        assert_eq!(p.shot.bits,m.shot.bits);
        assert_eq!(p.shot.trace.len(),m.shot.trace.len());
        if let NoisyEventKind::ResetObserved{value}=m.shot.trace[1].kind {
            if let aura_quantum_sim::circuit::TraceKind::Reset{previous_value}=
                p.shot.trace[1].kind {assert_eq!(previous_value,value)}
            else {panic!("expected pure reset trace");}
        } else {panic!("expected noisy observed reset trace");}
        for basis in 0..4 {
            close(p.final_state.basis_probability(basis).unwrap(),
                  m.final_density.basis_probability(basis).unwrap());
        }
    }
}

#[test]
fn two_modes_of_reset_differ_in_observability_but_both_preserve_required_marginals() {
    let mut u=noisy_bell();
    u.push(NoisyInstruction::Reset{qubit:0});
    let a=u.execute_once(42,DEFAULT_MEMORY_BUDGET).unwrap();
    close(a.final_density.purity().unwrap(),0.5);
    assert_eq!(a.shot.trace.len(),1);
    assert_eq!(a.shot.trace[0].kind,NoisyEventKind::ResetUnobserved);
    let mut v=noisy_bell();
    v.push(NoisyInstruction::ResetObserved{qubit:0});
    let b=v.execute_once(42,DEFAULT_MEMORY_BUDGET).unwrap();
    close(b.final_density.purity().unwrap(),1.0);
    assert!(matches!(b.shot.trace[0].kind,NoisyEventKind::ResetObserved{..}));
    close(b.final_density.wire_probability(0,false).unwrap(),1.0);
    // Both versions reset the selected qubit, but expose different evidence.
    close(a.final_density.wire_probability(0,false).unwrap(),1.0);
}

#[test]
fn invalid_unreachable_conditional_noise_and_qubit_fail_preflight() {
    let mut c=NoisyCircuit::new(2,1);
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0}); // always false
    c.push(NoisyInstruction::If{bit:0,equals:true,
        action:NoisyAction::Noise(NoiseChannel::PhaseFlip{
            qubit:1,probability:f64::NAN,
        })});
    assert_eq!(c.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        NoisyCircuitError::Density(DensityError::InvalidProbability));
    let mut d=NoisyCircuit::new(2,1);
    d.push(NoisyInstruction::If{bit:0,equals:false,
        action:NoisyAction::Gate(Gate::X(0))});
    assert_eq!(d.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        NoisyCircuitError::ReadBeforeWrite{bit:0,instruction:0});
    let mut bad=NoisyCircuit::new(2,0);
    bad.push(NoisyInstruction::Gate(Gate::Swap{a:1,b:1}));
    assert_eq!(bad.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        NoisyCircuitError::Density(DensityError::DuplicateQubit(1)));
}

#[test]
fn density_resource_budget_and_negative_dimensions_are_explicit() {
    let c=NoisyCircuit::new(9,0);
    assert_eq!(c.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        NoisyCircuitError::InvalidWidth{qubits:9,max:8});
    let mut d=NoisyCircuit::new(2,0);
    d.push(NoisyInstruction::Reset{qubit:0});
    assert!(matches!(d.execute_shots(4096,1,128).unwrap_err(),
        NoisyCircuitError::BudgetExceeded{..}));
    assert_eq!(d.execute_shots(0,1,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        NoisyCircuitError::ZeroShots);
    let empty=NoisyCircuit::new(0,0);
    assert_eq!(empty.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err(),
        NoisyCircuitError::InvalidWidth{qubits:0,max:8});
}

#[test]
fn measurement_seed_replay_and_noisy_statistics_are_within_declared_six_sigma() {
    let mut c=NoisyCircuit::new(1,1);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Noise(NoiseChannel::AmplitudeDamping{qubit:0,gamma:0.36}));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    let a=c.execute_shots(4096,0xabcd,DEFAULT_MEMORY_BUDGET).unwrap();
    let b=c.execute_shots(4096,0xabcd,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_eq!(a,b);
    let n_true=a.records.iter().filter(|r|r.bits[0]==Some(true)).count();
    let expected=4096.0*0.32;
    let sigma=(4096.0*0.32*0.68_f64).sqrt();
    assert!((n_true as f64-expected).abs() < 6.0*sigma,
        "{n_true} sampled |1>, expected {expected}");
    let c2=c.execute_shots(4096,0xabce,DEFAULT_MEMORY_BUDGET).unwrap();
    assert_ne!(a.records,c2.records);
}

#[test]
fn pure_and_density_postselection_are_equivalent_for_arbitrary_entangled_phase() {
    let mut p=aura_quantum_sim::Statevector::zero(3,DEFAULT_MEMORY_BUDGET).unwrap();
    for g in [Gate::H(0),Gate::T(0),Gate::H(1),
        Gate::Cnot{control:0,target:2},Gate::S(1),Gate::H(2)] {
        p.apply(g).unwrap();
    }
    let mut d=DensityMatrix::from_statevector(&p,DEFAULT_MEMORY_BUDGET).unwrap();
    let mut rng0=SeededRng::new(0xbadcafe);
    let mut rng1=SeededRng::new(0xbadcafe);
    for q in [0,2,1,0] {
        let out0=p.measure(q,&mut rng0).unwrap();
        let out1=d.measure(q,&mut rng1).unwrap();
        assert_eq!(out0,out1);
        for r in 0..8 {for c in 0..8 {
            let a=p.amplitude(r).unwrap();
            let b=p.amplitude(c).unwrap();
            let expected=a*Complex::new(b.re,-b.im);
            ceq(d.element(r,c).unwrap(),expected);
        }}
    }
}
