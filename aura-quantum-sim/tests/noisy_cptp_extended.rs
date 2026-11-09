//! C-022 real CPTP PhaseDamping, Depolarizing, PauliMixture product.
use aura_quantum_sim::density::{DensityError,DensityMatrix,NoiseChannel};
use aura_quantum_sim::noisy_circuit::{
    NoisyAction,NoisyCircuit,NoisyInstruction,
};
use aura_quantum_sim::{Complex,Gate,DEFAULT_MEMORY_BUDGET};
fn near(x:f64,y:f64){assert!((x-y).abs()<5e-11,"{x:.14e} vs {y:.14e}");}
fn plus()->DensityMatrix{
    let mut r=DensityMatrix::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    r.apply_gate(Gate::H(0)).unwrap();r
}
fn element(r:&DensityMatrix,i:usize,j:usize)->Complex{r.element(i,j).unwrap()}
#[test]
fn phase_damping_keeps_populations_but_reduces_coherence_sqrt_one_minus_gamma(){
    let mut r=plus();
    r.apply_noise(NoiseChannel::PhaseDamping{qubit:0,gamma:0.75}).unwrap();
    near(r.basis_probability(0).unwrap(),0.5);
    near(r.basis_probability(1).unwrap(),0.5);
    near(element(&r,0,1).re,0.25);
    near(r.purity().unwrap(),0.625);
    r.apply_noise(NoiseChannel::PhaseDamping{qubit:0,gamma:1.0}).unwrap();
    near(element(&r,0,1).norm_squared(),0.0);
    near(r.purity().unwrap(),0.5);
}
#[test]
fn phase_damping_zero_exactly_preserves_y_complex_phase(){
    let mut r=plus();
    r.apply_gate(Gate::S(0)).unwrap();
    let before=element(&r,0,1);
    r.apply_noise(NoiseChannel::PhaseDamping{qubit:0,gamma:0.0}).unwrap();
    assert_eq!(element(&r,0,1),before);
    near(before.im,-0.5);
}
#[test]
fn depolarizing_probability_three_quarters_is_full_single_wire_mixing(){
    let mut r=plus();
    r.apply_noise(NoiseChannel::Depolarizing{qubit:0,probability:0.75}).unwrap();
    near(r.basis_probability(0).unwrap(),0.5);
    near(r.basis_probability(1).unwrap(),0.5);
    near(element(&r,0,1).norm_squared(),0.0);
    near(r.purity().unwrap(),0.5);
}
#[test]
fn depolarizing_p_one_is_not_identity_and_has_correct_convention(){
    let mut r=DensityMatrix::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
    r.apply_noise(NoiseChannel::Depolarizing{qubit:0,probability:1.0}).unwrap();
    near(r.basis_probability(0).unwrap(),1.0/3.0);
    near(r.basis_probability(1).unwrap(),2.0/3.0);
    near(r.purity().unwrap(),5.0/9.0);
}
#[test]
fn asymmetric_pauli_mixture_contracts_x_y_z_by_distinct_factors(){
    for (prep,real,imag) in [
        (Gate::H(0),0.1,0.0),
        (Gate::X(0),0.0,0.0),
    ]{
        let mut r=DensityMatrix::zero(1,DEFAULT_MEMORY_BUDGET).unwrap();
        r.apply_gate(prep).unwrap();
        r.apply_noise(NoiseChannel::PauliMixture{
            qubit:0,p_x:0.2,p_y:0.3,p_z:0.1,
        }).unwrap();
        near(element(&r,0,1).re,real);
        near(element(&r,0,1).im,imag);
        near(r.trace(),1.0);
        if prep==Gate::X(0){
            near(r.basis_probability(0).unwrap(),0.5);
        }
    }
    let mut y=plus();
    y.apply_gate(Gate::S(0)).unwrap();
    y.apply_noise(NoiseChannel::PauliMixture{
        qubit:0,p_x:0.2,p_y:0.3,p_z:0.1,
    }).unwrap();
    near(element(&y,0,1).im,-0.2);
}
#[test]
fn general_pauli_mixture_is_equal_to_depolarizing_for_equal_components(){
    let mut a=DensityMatrix::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    a.apply_gate(Gate::H(0)).unwrap();
    a.apply_gate(Gate::Cnot{control:0,target:1}).unwrap();
    a.apply_gate(Gate::T(1)).unwrap();
    let mut b=a.clone();
    a.apply_noise(NoiseChannel::Depolarizing{qubit:0,probability:0.6}).unwrap();
    b.apply_noise(NoiseChannel::PauliMixture{
        qubit:0,p_x:0.2,p_y:0.2,p_z:0.2,
    }).unwrap();
    for i in 0..4{
        for j in 0..4{
            near(element(&a,i,j).re,element(&b,i,j).re);
            near(element(&a,i,j).im,element(&b,i,j).im);
        }
    }
}
#[test]
fn entangled_bell_dephasing_destroys_pair_coherence_not_marginal_population(){
    let mut r=DensityMatrix::zero(2,DEFAULT_MEMORY_BUDGET).unwrap();
    r.apply_gate(Gate::H(0)).unwrap();
    r.apply_gate(Gate::Cnot{control:0,target:1}).unwrap();
    r.apply_noise(NoiseChannel::PhaseDamping{qubit:0,gamma:1.0}).unwrap();
    near(element(&r,0,3).norm_squared(),0.0);
    near(r.basis_probability(0).unwrap(),0.5);
    near(r.basis_probability(3).unwrap(),0.5);
    near(r.purity().unwrap(),0.5);
    for q in 0..2{
        let m=r.reduced_qubit(q).unwrap();
        near(m[0][0].re,0.5);
        near(m[1][1].re,0.5);
    }
}
#[test]
fn invalid_pauli_sum_and_nonfinite_parameters_do_not_mutate_density(){
    for channel in [
        NoiseChannel::PauliMixture{qubit:0,p_x:0.6,p_y:0.4,p_z:0.1},
        NoiseChannel::PauliMixture{qubit:0,p_x:-0.1,p_y:0.0,p_z:0.0},
        NoiseChannel::PauliMixture{qubit:0,p_x:f64::NAN,p_y:0.0,p_z:0.0},
        NoiseChannel::PauliMixture{qubit:0,p_x:0.0,p_y:f64::INFINITY,p_z:0.0},
        NoiseChannel::Depolarizing{qubit:0,probability:1.1},
        NoiseChannel::Depolarizing{qubit:0,probability:-0.1},
        NoiseChannel::PhaseDamping{qubit:0,gamma:f64::NAN},
        NoiseChannel::PhaseDamping{qubit:0,gamma:-0.1},
    ]{
        let mut r=plus();
        let old=element(&r,0,1);
        assert_eq!(r.apply_noise(channel).unwrap_err(),DensityError::InvalidProbability);
        assert_eq!(element(&r,0,1),old);
    }
    let mut r=plus();
    assert_eq!(r.apply_noise(NoiseChannel::Depolarizing{
        qubit:2,probability:0.4,
    }).unwrap_err(),DensityError::InvalidQubit{index:2,qubits:1});
}
#[test]
fn dynamically_conditioned_new_noise_uses_existing_classical_feedforward(){
    let mut c=NoisyCircuit::new(2,1);
    c.push(NoisyInstruction::Gate(Gate::H(0)));
    c.push(NoisyInstruction::Gate(Gate::H(1)));
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    c.push(NoisyInstruction::If{
        bit:0,equals:true,
        action:NoisyAction::Noise(NoiseChannel::Depolarizing{
            qubit:1,probability:0.75,
        }),
    });
    let mut yes=false;
    let mut no=false;
    for seed in 0..96{
        let out=c.execute_once(seed,DEFAULT_MEMORY_BUDGET).unwrap();
        let m=out.final_density.reduced_qubit(1).unwrap();
        match out.shot.bits[0]{
            Some(true)=>{yes=true;near(m[0][1].norm_squared(),0.0);}
            Some(false)=>{no=true;near(m[0][1].re,0.5);}
            None=>panic!("measure must write classical bit"),
        }
    }
    assert!(yes&&no);
}
#[test]
fn untaken_classical_noise_still_fails_preflight_and_no_result(){
    let mut c=NoisyCircuit::new(1,1);
    c.push(NoisyInstruction::Measure{qubit:0,into_bit:0});
    c.push(NoisyInstruction::If{
        bit:0,equals:true,
        action:NoisyAction::Noise(NoiseChannel::PauliMixture{
            qubit:0,p_x:0.5,p_y:0.5,p_z:0.5,
        }),
    });
    let err=c.execute_once(0,DEFAULT_MEMORY_BUDGET).unwrap_err();
    assert!(format!("{err}").contains("InvalidProbability"));
}
