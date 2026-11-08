//! Analytic Pauli-Hamiltonian energy and variance for pure factor states.
//! No QPU, no stochastic observable sampling, no global 2^128 state.
use crate::factorized::{
    FactorError,FactorState,PauliTerm,MAX_ABS_PAULI_COEFFICIENT,MAX_PAULI_TERMS,
};
use crate::{SimError,NORM_TOLERANCE};

pub const MAX_MOMENT_WORK_UNITS:usize=16_777_216;

#[derive(Debug,Clone,PartialEq)]
pub struct HamiltonianMoments {
    pub mean:f64,
    pub second_moment:f64,
    pub variance:f64,
    pub commuting_pairs:usize,
    pub anticommuting_pairs:usize,
    pub estimated_work_units:usize,
    /// True iff a small negative roundoff residual was replaced by zero.
    pub roundoff_clamped:bool,
}

fn compensated_add(sum:&mut f64,error:&mut f64,value:f64) {
    let y=value-*error;
    let next=*sum+y;
    *error=(next-*sum)-y;
    *sum=next;
}

// Hermitian Pauli products return a real sign and a Pauli word when they
// commute. For anti-commuting pairs the two H^2 cross terms cancel.
fn hermitian_product(a:&str,b:&str)->Result<Option<(String,f64)>,FactorError> {
    let lhs=a.as_bytes();
    let rhs=b.as_bytes();
    if lhs.len()!=rhs.len() {
        return Err(FactorError::InvalidBasisWidth{expected:lhs.len(),got:rhs.len()});
    }
    let mut result=Vec::new();
    result.try_reserve_exact(lhs.len()).map_err(|_|FactorError::AllocationFailed)?;
    let mut phase=0u8;
    for (&x,&y) in lhs.iter().zip(rhs) {
        let (z,turn)=match (x,y) {
            (b'I',other)|(other,b'I')=>(other,0),
            (p,q) if p==q=>(b'I',0),
            (b'X',b'Y')=>(b'Z',1),
            (b'Y',b'Z')=>(b'X',1),
            (b'Z',b'X')=>(b'Y',1),
            (b'Y',b'X')=>(b'Z',3),
            (b'Z',b'Y')=>(b'X',3),
            (b'X',b'Z')=>(b'Y',3),
            _=>return Err(FactorError::InvalidPauliSymbol{
                wire:result.len(),symbol:x,
            }),
        };
        phase=(phase+turn)%4;
        result.push(z);
    }
    if !phase.is_multiple_of(2) {return Ok(None);}
    let word=String::from_utf8(result).map_err(|_|FactorError::ArithmeticOverflow)?;
    Ok(Some((word,if phase==0 {1.0}else{-1.0})))
}

/// Compute E[H], E[H²], Var(H) of a real-coefficient Pauli Hamiltonian.
/// One normalized current pure-state trajectory only: after measurement
/// this is conditional, NOT a mixed-branch/channel average. No RNG draws.
/// Cost scales with actual local factor sizes, never the global Hilbert size.
pub fn evaluate_hamiltonian_moments(
    state:&FactorState,terms:&[PauliTerm],
)->Result<HamiltonianMoments,FactorError> {
    if terms.is_empty()||terms.len()>MAX_PAULI_TERMS {
        return Err(FactorError::InvalidPauliTermCount);
    }
    for t in terms {
        if !t.coefficient.is_finite()
            ||t.coefficient.abs()>MAX_ABS_PAULI_COEFFICIENT {
            return Err(FactorError::InvalidPauliCoefficient);
        }
        if t.paulis.len()!=state.qubits() {
            return Err(FactorError::InvalidBasisWidth{
                expected:state.qubits(),got:t.paulis.len(),
            });
        }
        for (wire,&symbol) in t.paulis.as_bytes().iter().enumerate() {
            if !matches!(symbol,b'I'|b'X'|b'Y'|b'Z') {
                return Err(FactorError::InvalidPauliSymbol{wire,symbol});
            }
        }
    }
    // Conservatively charge all pairs, including those that cancel.
    // Local dense Pauli work is O(sum(2^factor_width * factor_width)).
    let mut per_eval=state.qubits();
    for width in state.factor_widths() {
        let factor=(1usize<<width).checked_mul(width+2)
            .ok_or(FactorError::ArithmeticOverflow)?;
        per_eval=per_eval.checked_add(factor)
            .ok_or(FactorError::ArithmeticOverflow)?;
    }
    let evaluations=terms.len().checked_mul(terms.len()+1)
        .map(|n|n/2).ok_or(FactorError::ArithmeticOverflow)?;
    let work=evaluations.checked_mul(per_eval)
        .ok_or(FactorError::ArithmeticOverflow)?;
    if work>MAX_MOMENT_WORK_UNITS {return Err(FactorError::WorkLimit);}

    let mut mean=0.0;
    let mut mean_comp=0.0;
    let mut second=0.0;
    let mut second_comp=0.0;
    for t in terms {
        let expectation=state.expectation_pauli(&t.paulis)?;
        compensated_add(&mut mean,&mut mean_comp,t.coefficient*expectation);
        compensated_add(&mut second,&mut second_comp,
            t.coefficient*t.coefficient);
    }
    let mut commuting=0usize;
    let mut anticommuting=0usize;
    for (i,a) in terms.iter().enumerate() {
        for b in terms.iter().skip(i+1) {
            if let Some((word,sign))=hermitian_product(&a.paulis,&b.paulis)? {
                commuting+=1;
                let expectation=state.expectation_pauli(&word)?;
                compensated_add(&mut second,&mut second_comp,
                    2.0*a.coefficient*b.coefficient*sign*expectation);
            } else {
                anticommuting+=1;
            }
        }
    }
    let mean_square=mean*mean;
    let raw=second-mean_square;
    if !mean.is_finite()||!second.is_finite()||!raw.is_finite() {
        return Err(FactorError::Simulator(SimError::NonFiniteAmplitude));
    }
    let allowed_negative=NORM_TOLERANCE*
        second.abs().max(mean_square.abs()).max(1.0);
    if raw< -allowed_negative {
        return Err(FactorError::Simulator(SimError::NotNormalized));
    }
    let roundoff_clamped=raw<0.0;
    Ok(HamiltonianMoments{
        mean,second_moment:second,
        variance:if roundoff_clamped {0.0}else{raw},
        commuting_pairs:commuting,
        anticommuting_pairs:anticommuting,
        estimated_work_units:work,
        roundoff_clamped,
    })
}
