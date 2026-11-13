//! C-026: real-coefficient Pauli Hamiltonian E[H], E[H^2], variance
//! for unconditional or final-register postselected NOISY dynamic channels.
//! This is mixed-state Tr(rho H^2)-Tr(rho H)^2, NOT a sampled-shot variance,
//! pure-state trajectory, quantum hardware energy, or device calibration.
use crate::factorized::{
    PauliEstimate,PauliTerm,MAX_ABS_PAULI_COEFFICIENT,MAX_PAULI_TERMS,
    PAULI_TERM_LOGICAL_OVERHEAD,
};
use crate::noisy_circuit::{NoisyCircuit,NoisyInstruction};
use crate::noisy_ensemble::{
    execute_noisy_conditioned_ensemble,execute_noisy_ensemble,
    EnsembleError,NoisyCondition,MAX_ENSEMBLE_QUBITS,
    MAX_ENSEMBLE_MEASUREMENTS,MAX_ENSEMBLE_INSTRUCTIONS,
    MAX_ENSEMBLE_WORK,
};
use crate::{Complex,DEFAULT_MEMORY_BUDGET,NORM_TOLERANCE};
use std::mem::size_of;

pub const NOISY_HAMILTONIAN_PROFILE:&str="bounded-noisy-pauli-hamiltonian-dense-f64/1";
/// Extra logical memory includes two dense H/H² matrices, terms/strings,
/// intermediate term estimates and allocator overhead. Not physical RSS.
pub const HAMILTONIAN_SCRATCH_BYTES:usize=16*1024;
pub const HAMILTONIAN_FIXED_WORK:usize=4096;

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum NoisyHamiltonianError{
    InvalidTerms,InvalidCoefficient,
    InvalidPauli{term:usize,wire:usize},
    InvalidBudget,BudgetExceeded,WorkLimit,Overflow,
    InvalidDensity,Numeric,
    Ensemble(EnsembleError),
}
impl std::fmt::Display for NoisyHamiltonianError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for NoisyHamiltonianError{}
impl From<EnsembleError> for NoisyHamiltonianError{
    fn from(e:EnsembleError)->Self{Self::Ensemble(e)}
}
#[derive(Debug,Clone)]
pub struct NoisyHamiltonian{
    pub qubits:usize,
    pub terms:Vec<PauliEstimate>,
    pub weighted_sum:f64,
    pub weighted_second_moment:f64,
    pub weighted_variance:f64,
    pub roundoff_clamped:bool,
    pub conditions:Option<Vec<NoisyCondition>>,
    pub condition_probability:Option<f64>,
    pub selected_branch_count:Option<usize>,
    pub reserved_peak_logical_bytes:usize,
    pub charged_work_units:usize,
}
fn multiply(a:usize,b:usize)->Result<usize,NoisyHamiltonianError>{
    a.checked_mul(b).ok_or(NoisyHamiltonianError::Overflow)
}
fn plus(a:usize,b:usize)->Result<usize,NoisyHamiltonianError>{
    a.checked_add(b).ok_or(NoisyHamiltonianError::Overflow)
}
fn scaled(z:Complex,value:f64)->Complex{
    Complex::new(z.re*value,z.im*value)
}
fn finite(z:Complex)->bool{z.re.is_finite()&&z.im.is_finite()}
fn valid_terms(qubits:usize,terms:&[PauliTerm])
    ->Result<(),NoisyHamiltonianError>{
    if terms.is_empty()||terms.len()>MAX_PAULI_TERMS{
        return Err(NoisyHamiltonianError::InvalidTerms);
    }
    for (i,t) in terms.iter().enumerate(){
        if !t.coefficient.is_finite()||
            t.coefficient.abs()>MAX_ABS_PAULI_COEFFICIENT{
            return Err(NoisyHamiltonianError::InvalidCoefficient);
        }
        if t.paulis.len()!=qubits{
            return Err(NoisyHamiltonianError::InvalidPauli{
                term:i,wire:qubits,
            });
        }
        for (wire,ch) in t.paulis.bytes().enumerate(){
            if !matches!(ch,b'I'|b'X'|b'Y'|b'Z'){
                return Err(NoisyHamiltonianError::InvalidPauli{term:i,wire});
            }
        }
    }
    Ok(())
}
/// The Pauli word is ASCII q0-first; the global state index is q0-LSB.
/// For an input |basis>, Y|0>=+i|1> and Y|1>=-i|0>.
/// Returns destination and complex matrix element <destination|P|basis>.
fn pauli_on_basis(paulis:&str,basis:usize)->(usize,Complex){
    let mut target=basis;
    let mut phase=Complex::ONE;
    for (wire,p) in paulis.bytes().enumerate(){
        let bit=(basis>>wire)&1;
        match p{
            b'I'=>{},
            b'X'=>target^=1usize<<wire,
            b'Y'=>{
                target^=1usize<<wire;
                phase=phase*if bit==0{Complex::I}else{Complex::NEG_I};
            }
            b'Z'=>{
                if bit==1{phase=scaled(phase,-1.0);}
            }
            _=>unreachable!("Pauli words prevalidated"),
        }
    }
    (target,phase)
}
fn expectation_word(rho:&[Complex],dim:usize,word:&str)
    ->Result<f64,NoisyHamiltonianError>{
    let mut expectation=Complex::ZERO;
    for basis in 0..dim{
        let (to,phase)=pauli_on_basis(word,basis);
        expectation=expectation+rho[basis*dim+to]*phase;
    }
    // A real-coefficient Hermitian Pauli term must have real expectation.
    if !finite(expectation)||
        expectation.im.abs()>NORM_TOLERANCE||
        expectation.re.abs()>1.0+NORM_TOLERANCE{
        return Err(NoisyHamiltonianError::Numeric);
    }
    Ok(expectation.re)
}
fn trace_product(rho:&[Complex],matrix:&[Complex],dim:usize)
    ->Result<Complex,NoisyHamiltonianError>{
    let mut total=Complex::ZERO;
    for (row,rho_row) in rho.chunks_exact(dim).enumerate(){
        for (col,&z) in rho_row.iter().enumerate(){
            total=total+z*matrix[col*dim+row];
        }
    }
    if !finite(total){return Err(NoisyHamiltonianError::Numeric);}
    Ok(total)
}
fn admit(qubits:usize,program:&NoisyCircuit,terms:&[PauliTerm],
    conditions:Option<&[NoisyCondition]>,budget:usize,
)->Result<(usize,usize,usize),NoisyHamiltonianError>{
    valid_terms(qubits,terms)?;
    if budget==0||budget>DEFAULT_MEMORY_BUDGET{
        return Err(NoisyHamiltonianError::InvalidBudget);
    }
    let strings=multiply(terms.len(),plus(qubits,PAULI_TERM_LOGICAL_OVERHEAD)?)?;
    let structs=multiply(terms.len(),size_of::<PauliEstimate>())?;
    let reserve=plus(HAMILTONIAN_SCRATCH_BYTES,plus(strings,structs)?)?;
    if reserve>=budget{return Err(NoisyHamiltonianError::BudgetExceeded);}
    let dim=1usize.checked_shl(
        u32::try_from(qubits).map_err(|_|NoisyHamiltonianError::Overflow)?
    ).ok_or(NoisyHamiltonianError::Overflow)?;
    let cells=multiply(dim,dim)?;
    // Independently charged Pauli matrix fill + H^2 dense multiplication,
    // per-Pauli expectations, and output; prevents work-free observables.
    let fill=multiply(multiply(terms.len(),dim)?,plus(qubits,4)?)?;
    let eval=multiply(multiply(terms.len(),dim)?,plus(qubits,2)?)?;
    let cubic=multiply(multiply(cells,dim)?,12)?;
    let work=plus(
        plus(fill,eval)?,
        plus(cubic,plus(multiply(cells,4)?,HAMILTONIAN_FIXED_WORK)?)?
    )?;
    // Reproduce only the exact C-023 conservative *admission* arithmetic
    // to reject total-work overflows BEFORE executing a physical circuit.
    // Actual circuit legality and instruments still use C-023 preflight.
    if (1..=MAX_ENSEMBLE_QUBITS).contains(&qubits)
        &&program.instructions().len()<=MAX_ENSEMBLE_INSTRUCTIONS{
        let measures=program.instructions().iter().filter(|x|
            matches!(x,NoisyInstruction::Measure{..})).count();
        if measures<=MAX_ENSEMBLE_MEASUREMENTS{
            let charged=multiply(
                multiply(cells,program.instructions().len().max(1))?,
                multiply(1usize<<measures,8)?
            )?;
            let selection=if let Some(bits)=conditions{
                multiply(bits.len(),64)?
            }else{0};
            let total=plus(plus(charged,selection)?,work)?;
            if total>MAX_ENSEMBLE_WORK{
                return Err(NoisyHamiltonianError::WorkLimit);
            }
        }
    }
    Ok((reserve,work,dim))
}
/// A single expectation of H for a true NOISY mixed density matrix.
/// All existing C-023/C-024 constraints stay authoritative; caller cannot
/// ask for final unselected register histories through this result.
/// Real coefficients, ASCII q0-first Pauli words, at most 64 terms.
pub fn analyze_noisy_hamiltonian(
    program:&NoisyCircuit,terms:&[PauliTerm],
    conditions:Option<&[NoisyCondition]>,budget:usize,
)->Result<NoisyHamiltonian,NoisyHamiltonianError>{
    let qubits=program.qubits();
    let (reserve,work,dim)=admit(qubits,program,terms,conditions,budget)?;
    let physical_budget=budget-reserve;
    let (rho,physical_peak,physical_work,event,selected_count)=match conditions{
        Some(bits)=>{
            let x=execute_noisy_conditioned_ensemble(
                program,bits,physical_budget)?;
            (x.final_density,x.reserved_peak_logical_bytes,
                x.charged_work_units,Some((x.conditions,x.condition_probability)),
                Some(x.selected_branch_count))
        }
        None=>{
            let x=execute_noisy_ensemble(program,physical_budget)?;
            (x.final_density,x.reserved_peak_logical_bytes,
                x.charged_work_units,None,None)
        }
    };
    let peak=plus(physical_peak,reserve)?;
    let total_work=plus(physical_work,work)?;
    if peak>budget{return Err(NoisyHamiltonianError::BudgetExceeded);}
    if total_work>MAX_ENSEMBLE_WORK{
        return Err(NoisyHamiltonianError::WorkLimit);
    }
    if rho.len()!=multiply(dim,dim)?{
        return Err(NoisyHamiltonianError::InvalidDensity);
    }
    let mut estimates=Vec::new();
    estimates.try_reserve_exact(terms.len())
        .map_err(|_|NoisyHamiltonianError::BudgetExceeded)?;
    let mut h=vec![Complex::ZERO;dim*dim];
    for t in terms{
        let expectation=expectation_word(&rho,dim,&t.paulis)?;
        let contribution=t.coefficient*expectation;
        if !contribution.is_finite(){return Err(NoisyHamiltonianError::Numeric);}
        estimates.push(PauliEstimate{
            paulis:t.paulis.clone(),coefficient:t.coefficient,
            expectation,contribution,
        });
        for ket in 0..dim{
            let (bra,phase)=pauli_on_basis(&t.paulis,ket);
            let index=bra*dim+ket;
            h[index]=h[index]+scaled(phase,t.coefficient);
        }
    }
    // H is Hermitian even when Pauli words contain complex Y factors.
    for (row,r) in h.chunks_exact(dim).enumerate(){
        for (col,&z) in r.iter().enumerate(){
            let other=h[col*dim+row];
            if !finite(z)||
                (z.re-other.re).abs()>NORM_TOLERANCE||
                (z.im+other.im).abs()>NORM_TOLERANCE{
                return Err(NoisyHamiltonianError::Numeric);
            }
        }
    }
    let mean=trace_product(&rho,&h,dim)?;
    let mut squared=vec![Complex::ZERO;dim*dim];
    for (row,outrow) in squared.chunks_exact_mut(dim).enumerate(){
        let hrow=&h[row*dim..(row+1)*dim];
        for (col,cell) in outrow.iter_mut().enumerate(){
            let mut z=Complex::ZERO;
            for (k,&first) in hrow.iter().enumerate(){
                z=z+first*h[k*dim+col];
            }
            *cell=z;
        }
    }
    let second=trace_product(&rho,&squared,dim)?;
    let bound=terms.iter().map(|x|x.coefficient.abs()).sum::<f64>();
    let first_tolerance=NORM_TOLERANCE*(1.0+bound);
    let second_tolerance=NORM_TOLERANCE*(1.0+bound*bound);
    if !finite(mean)||!finite(second)||
        mean.im.abs()>first_tolerance||
        second.im.abs()>second_tolerance||
        second.re< -second_tolerance{
        return Err(NoisyHamiltonianError::Numeric);
    }
    let variance_raw=second.re-mean.re*mean.re;
    if !variance_raw.is_finite(){
        return Err(NoisyHamiltonianError::Numeric);
    }
    let allowance=NORM_TOLERANCE*
        (1.0+second.re.abs()+mean.re*mean.re);
    if variance_raw< -allowance{
        return Err(NoisyHamiltonianError::Numeric);
    }
    let roundoff_clamped=variance_raw<0.0;
    let (event_conditions,event_probability)=match event{
        Some((c,p))=>(Some(c),Some(p)),
        None=>(None,None),
    };
    Ok(NoisyHamiltonian{
        qubits,terms:estimates,
        weighted_sum:mean.re,weighted_second_moment:second.re,
        weighted_variance:variance_raw.max(0.0),
        roundoff_clamped,conditions:event_conditions,
        condition_probability:event_probability,selected_branch_count:selected_count,
        reserved_peak_logical_bytes:peak,charged_work_units:total_work,
    })
}
