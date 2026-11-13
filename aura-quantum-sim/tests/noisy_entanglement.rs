//! C-025 mathematical PPT/NPT on real dynamic noisy physical channels.
use aura_quantum_sim::noisy_entanglement::{
    analyze_noisy_pair,hermitian_eigenvalues_4,reduced_pair,
    EntanglementError,
};
use aura_quantum_sim::noisy_circuit::{NoisyAction,NoisyCircuit,NoisyInstruction};
use aura_quantum_sim::noisy_ensemble::NoisyCondition;
use aura_quantum_sim::density::NoiseChannel;
use aura_quantum_sim::{Complex,Gate,DEFAULT_MEMORY_BUDGET};
fn near(a:f64,b:f64){assert!((a-b).abs()<2e-10,"got {a:.16e}, expected {b:.16e}")}
fn bell()->NoisyCircuit{
    let mut q=NoisyCircuit::new(2,0);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    q
}
#[test]
fn bell_is_npt_entangled_with_half_negativity_and_two_bits_of_mutual_info(){
    let x=analyze_noisy_pair(&bell(),[0,1],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(x.negativity,0.5);
    near(x.logarithmic_negativity_bits,1.0);
    near(x.mutual_information_bits,2.0);
    near(x.reduced_purity,1.0);
    near(x.single_wire_purities[0],0.5);
    near(x.single_wire_purities[1],0.5);
    near(x.partial_transpose_eigenvalues[0],-0.5);
    assert!(x.npt_witness_detected);
    assert!(x.condition_probability.is_none());
}
#[test]
fn complete_dephasing_leaves_classical_bell_correlations_not_entanglement(){
    let mut q=bell();
    q.push(NoisyInstruction::Noise(NoiseChannel::PhaseDamping{
        qubit:0,gamma:1.0,
    }));
    let x=analyze_noisy_pair(&q,[0,1],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(x.negativity,0.0);
    near(x.logarithmic_negativity_bits,0.0);
    near(x.reduced_purity,0.5);
    near(x.mutual_information_bits,1.0);
    assert!(!x.npt_witness_detected);
}
#[test]
fn full_depolarization_breaks_entanglement_and_all_correlations(){
    let mut q=bell();
    q.push(NoisyInstruction::Noise(NoiseChannel::Depolarizing{
        qubit:0,probability:0.75,
    }));
    let x=analyze_noisy_pair(&q,[0,1],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(x.reduced_purity,0.25);
    near(x.negativity,0.0);
    near(x.mutual_information_bits,0.0);
    for (i,row) in x.reduced_density.iter().enumerate(){near(row[i].re,0.25);}
}
#[test]
fn amplitude_damping_half_has_quarter_negativity_not_purity_proxy(){
    let mut q=bell();
    q.push(NoisyInstruction::Noise(NoiseChannel::AmplitudeDamping{
        qubit:1,gamma:0.5,
    }));
    let x=analyze_noisy_pair(&q,[0,1],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(x.negativity,0.25);
    assert!(x.npt_witness_detected);
    assert!(x.reduced_purity<1.0);
}
#[test]
fn nonadjacent_qubits_in_pure_four_qubit_ghz_are_classically_correlated(){
    let mut q=NoisyCircuit::new(4,0);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    for (a,b) in [(0,1),(1,2),(2,3)]{
        q.push(NoisyInstruction::Gate(Gate::Cnot{control:a,target:b}));
    }
    let x=analyze_noisy_pair(&q,[0,3],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(x.reduced_purity,0.5);
    near(x.mutual_information_bits,1.0);
    near(x.negativity,0.0);
    near(x.reduced_density[0][0].re,0.5);
    near(x.reduced_density[3][3].re,0.5);
}
#[test]
fn postselecting_independent_ancilla_preserves_or_destroys_bell_npt(){
    let mut q=NoisyCircuit::new(3,1);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    q.push(NoisyInstruction::Gate(Gate::H(2)));
    q.push(NoisyInstruction::Measure{qubit:2,into_bit:0});
    q.push(NoisyInstruction::If{
        bit:0,equals:true,
        action:NoisyAction::Noise(NoiseChannel::PhaseDamping{
            qubit:0,gamma:1.0,
        }),
    });
    for (value,neg,mi) in [(false,0.5,2.0),(true,0.0,1.0)]{
        let c=[NoisyCondition{bit:0,equals:value}];
        let x=analyze_noisy_pair(&q,[0,1],Some(&c),DEFAULT_MEMORY_BUDGET).unwrap();
        near(x.condition_probability.unwrap(),0.5);
        near(x.negativity,neg);
        near(x.mutual_information_bits,mi);
        assert_eq!(x.selected_branch_count,Some(1));
        assert_eq!(x.selected_conditions,Some(c.to_vec()));
    }
    let avg=analyze_noisy_pair(&q,[0,1],None,DEFAULT_MEMORY_BUDGET).unwrap();
    assert!(avg.negativity>0.0&&avg.negativity<0.5);
}
#[test]
fn measured_bell_pair_has_no_two_wire_entanglement_even_when_single_outcome_selected(){
    let mut q=NoisyCircuit::new(2,1);
    q.push(NoisyInstruction::Gate(Gate::H(0)));
    q.push(NoisyInstruction::Gate(Gate::Cnot{control:0,target:1}));
    q.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    for bit in [false,true]{
        let c=[NoisyCondition{bit:0,equals:bit}];
        let x=analyze_noisy_pair(&q,[0,1],Some(&c),DEFAULT_MEMORY_BUDGET).unwrap();
        near(x.negativity,0.0);
        near(x.reduced_purity,1.0);
        near(x.condition_probability.unwrap(),0.5);
    }
}
#[test]
fn complex_nonclifford_local_unitary_cannot_change_entanglement(){
    let mut q=bell();
    q.push(NoisyInstruction::Gate(Gate::S(0)));
    q.push(NoisyInstruction::Gate(Gate::T(1)));
    q.push(NoisyInstruction::Gate(Gate::H(1)));
    let x=analyze_noisy_pair(&q,[0,1],None,DEFAULT_MEMORY_BUDGET).unwrap();
    near(x.negativity,0.5);
    near(x.mutual_information_bits,2.0);
}
#[test]
fn hermitian_jacobi_handles_complex_4x4_and_rejects_forgery(){
    let mut herm=[[Complex::ZERO;4];4];
    for (i,v) in [0.4,0.3,0.2,0.1].iter().enumerate(){
        herm[i][i]=Complex::new(*v,0.0);
    }
    let eigen=hermitian_eigenvalues_4(herm).unwrap();
    for (i,v) in [0.1,0.2,0.3,0.4].iter().enumerate(){near(eigen[i],*v);}
    herm[0][1]=Complex::new(0.0,0.5);
    assert_eq!(hermitian_eigenvalues_4(herm).unwrap_err(),
        EntanglementError::InvalidDensity);
}
#[test]
fn direct_reduced_pair_rejects_untrusted_shape_and_pair_order(){
    let density=vec![Complex::ZERO;16];
    assert_eq!(reduced_pair(&density,2,[1,0]).unwrap_err(),
        EntanglementError::InvalidPair);
    assert_eq!(reduced_pair(&density,2,[0,0]).unwrap_err(),
        EntanglementError::InvalidPair);
    assert_eq!(reduced_pair(&density,3,[0,2]).unwrap_err(),
        EntanglementError::InvalidDensity);
    assert_eq!(analyze_noisy_pair(&bell(),[0,1],None,1).unwrap_err(),
        EntanglementError::InvalidBudget);
}
