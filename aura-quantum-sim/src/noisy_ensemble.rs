//! C-023: exact-in-model, seed-free *unconditional* noisy dynamic channel.
//! Distinct from sampled noisy shots and from pure ideal channel C-018.
//! Reuses the existing qualified DensityMatrix Kraus/gate/preflight engine.
use crate::density::{DensityError,DensityMatrix};
use crate::noisy_circuit::{NoisyAction,NoisyCircuit,NoisyCircuitError,NoisyInstruction};
use crate::{Complex,DEFAULT_MEMORY_BUDGET,NORM_TOLERANCE};
use std::collections::BTreeMap;
use std::mem::size_of;

pub const NOISY_ENSEMBLE_PROFILE:&str="bounded-noisy-channel-ensemble-dense-f64/1";
pub const MAX_ENSEMBLE_QUBITS:usize=4;
pub const MAX_ENSEMBLE_CLASSICAL_BITS:usize=8;
pub const MAX_ENSEMBLE_MEASUREMENTS:usize=6;
pub const MAX_ENSEMBLE_BRANCHES:usize=64;
pub const MAX_ENSEMBLE_INSTRUCTIONS:usize=128;
/// Conservative charged matrix-cell visits; not measured CPU instructions.
pub const MAX_ENSEMBLE_WORK:usize=16_777_216;
/// Unresolved positive Born probabilities cannot be safely ignored.
pub const MIN_ENSEMBLE_BRANCH_WEIGHT:f64=1e-12;

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum EnsembleError{
    InvalidQubits,InvalidClassicalBits,InvalidInstructions,
    InvalidMeasurements,UnsupportedResetObserved,InvalidBudget,
    BudgetExceeded,WorkLimit,Allocation,Overflow,
    BranchResolution,BranchLimit,ProbabilityMass,InvalidDensity,
    InvalidCondition,ImpossibleCondition,ConditionProbabilityTooSmall,
    Circuit(NoisyCircuitError),Density(DensityError),
}
impl std::fmt::Display for EnsembleError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for EnsembleError{}
impl From<NoisyCircuitError> for EnsembleError{
    fn from(value:NoisyCircuitError)->Self{Self::Circuit(value)}
}
impl From<DensityError> for EnsembleError{
    fn from(value:DensityError)->Self{Self::Density(value)}
}
#[derive(Debug,Clone)]
pub struct NoisyEnsemble{
    pub qubits:usize,
    pub dimension:usize,
    /// Row-major complex array; q0 is global basis least-significant bit.
    pub final_density:Vec<Complex>,
    pub trace:f64,
    pub purity:f64,
    pub classical_probabilities:BTreeMap<String,f64>,
    pub final_branch_count:usize,
    pub measurement_count:usize,
    pub probability_mass:f64,
    pub reserved_peak_logical_bytes:usize,
    pub charged_work_units:usize,
}
struct Branch{
    density:DensityMatrix,
    bits:Vec<Option<bool>>,
    weight:f64,
}
fn mul(a:usize,b:usize)->Result<usize,EnsembleError>{
    a.checked_mul(b).ok_or(EnsembleError::Overflow)
}
fn add(a:usize,b:usize)->Result<usize,EnsembleError>{
    a.checked_add(b).ok_or(EnsembleError::Overflow)
}
fn preflight(program:&NoisyCircuit,budget:usize)
    ->Result<(usize,usize,usize,usize,usize),EnsembleError>{
    let n=program.qubits();
    if !(1..=MAX_ENSEMBLE_QUBITS).contains(&n){
        return Err(EnsembleError::InvalidQubits);
    }
    if program.classical_bits()>MAX_ENSEMBLE_CLASSICAL_BITS{
        return Err(EnsembleError::InvalidClassicalBits);
    }
    let count=program.instructions().len();
    if count>MAX_ENSEMBLE_INSTRUCTIONS{
        return Err(EnsembleError::InvalidInstructions);
    }
    if budget==0||budget>DEFAULT_MEMORY_BUDGET{
        return Err(EnsembleError::InvalidBudget);
    }
    let mut measures=0usize;
    for op in program.instructions(){
        match op{
            NoisyInstruction::Measure{..}=>measures+=1,
            NoisyInstruction::ResetObserved{..}=>{
                // Observed reset reveals an internal outcome in old shot trace.
                // Unconditioned v1 intentionally refuses to silently discard it.
                return Err(EnsembleError::UnsupportedResetObserved);
            }
            _=>{}
        }
    }
    if measures>MAX_ENSEMBLE_MEASUREMENTS{
        return Err(EnsembleError::InvalidMeasurements);
    }
    let limit=1usize<<measures;
    let dimension=1usize<<n;
    let cells=mul(dimension,dimension)?;
    let matrix_bytes=mul(cells,size_of::<Complex>())?;
    // Two complete branch buffers during a measurement split (old+new),
    // one final aggregate, and classical bit arrays per branch.
    let branch_matrix_bytes=mul(matrix_bytes,add(mul(limit,2)?,1)?)?;
    let per_branch=add(size_of::<Branch>(),program.classical_bits())?;
    let peak=add(branch_matrix_bytes,mul(per_branch,mul(limit,2)?)?)?;
    if peak>budget{return Err(EnsembleError::BudgetExceeded);}
    let work=mul(mul(cells,count.max(1))?,mul(limit,8)?)?;
    if work>MAX_ENSEMBLE_WORK{return Err(EnsembleError::WorkLimit);}
    // Validates ALL operations including untaken conditional gate/noise
    // before a single density matrix is allocated or instrument branches.
    program.preflight(1,budget)?;
    Ok((measures,dimension,peak,work,limit))
}
fn apply_action(branch:&mut Branch,action:&NoisyAction)->Result<(),EnsembleError>{
    match action{
        NoisyAction::Gate(g)=>branch.density.apply_gate(g.clone())?,
        NoisyAction::Noise(n)=>branch.density.apply_noise(*n)?,
    }
    Ok(())
}
/// Final register bit condition; never an internal noise Kraus or reset
/// outcome. The input list MUST have strictly ascending, distinct bit indices.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct NoisyCondition{
    pub bit:usize,
    pub equals:bool,
}
/// Only event-specific statistics are exposed. No other classical histogram,
/// individual branch probabilities or internal Kraus histories are returned.
#[derive(Debug,Clone)]
pub struct NoisyConditionalEnsemble{
    pub conditions:Vec<NoisyCondition>,
    pub condition_probability:f64,
    pub selected_branch_count:usize,
    pub measurement_count:usize,
    pub qubits:usize,
    pub dimension:usize,
    pub final_density:Vec<Complex>,
    pub trace:f64,
    pub purity:f64,
    pub reserved_peak_logical_bytes:usize,
    pub charged_work_units:usize,
}
/// Independent of minimum PER-INSTRUMENT branch weight.
pub const MIN_NOISY_CONDITION_PROBABILITY:f64=1e-12;
fn validate_conditions(program:&NoisyCircuit,conditions:&[NoisyCondition])
    ->Result<(),EnsembleError>{
    if conditions.is_empty()||conditions.len()>MAX_ENSEMBLE_CLASSICAL_BITS
        ||program.classical_bits()>MAX_ENSEMBLE_CLASSICAL_BITS{
        return Err(EnsembleError::InvalidCondition);
    }
    let mut written=[false;MAX_ENSEMBLE_CLASSICAL_BITS];
    for ins in program.instructions(){
        if let NoisyInstruction::Measure{into_bit,..}=ins{
            if *into_bit<MAX_ENSEMBLE_CLASSICAL_BITS{
                written[*into_bit]=true;
            }
        }
    }
    let mut last=None;
    for c in conditions{
        if c.bit>=program.classical_bits()||!written[c.bit]
            ||last.is_some_and(|previous|c.bit<=previous){
            return Err(EnsembleError::InvalidCondition);
        }
        last=Some(c.bit);
    }
    Ok(())
}
fn matches_conditions(b:&Branch,conditions:&[NoisyCondition])->bool{
    conditions.iter().all(|c|b.bits[c.bit]==Some(c.equals))
}
struct Enumerated{
    branches:Vec<Branch>,
    measurements:usize,
    dim:usize,
    peak:usize,
    work:usize,
    mass:f64,
}
/// Reuse the one C-owned physical instrument enumerator for both
/// unconditional and final-classical-register conditioned noisy channels.
/// Charge conditional-event selection bytes and work BEFORE any allocation.
fn enumerate(
    program:&NoisyCircuit,budget:usize,extra_bytes:usize,extra_work:usize,
)->Result<Enumerated,EnsembleError>{
    let (measurements,dim,peak,work,max_branches)=preflight(program,budget)?;
    let peak=add(peak,extra_bytes)?;
    let work=add(work,extra_work)?;
    if peak>budget{return Err(EnsembleError::BudgetExceeded);}
    if work>MAX_ENSEMBLE_WORK{return Err(EnsembleError::WorkLimit);}
    let mut branches=vec![Branch{
        density:DensityMatrix::zero(program.qubits(),budget)?,
        bits:vec![None;program.classical_bits()],
        weight:1.0,
    }];
    for (instruction,op) in program.instructions().iter().enumerate(){
        match op{
            NoisyInstruction::Gate(g)=>{
                for b in &mut branches{b.density.apply_gate(g.clone())?;}
            }
            NoisyInstruction::Noise(n)=>{
                for b in &mut branches{b.density.apply_noise(*n)?;}
            }
            NoisyInstruction::Reset{qubit}=>{
                for b in &mut branches{b.density.reset_unobserved(*qubit)?;}
            }
            NoisyInstruction::ResetObserved{..}=>{
                return Err(EnsembleError::UnsupportedResetObserved);
            }
            NoisyInstruction::If{bit,equals,action}=>{
                for b in &mut branches{
                    let actual=b.bits[*bit].ok_or(
                        NoisyCircuitError::ReadBeforeWrite{
                            bit:*bit,instruction,
                        })?;
                    if actual==*equals{apply_action(b,action)?;}
                }
            }
            NoisyInstruction::Measure{qubit,into_bit}=>{
                let mut next=Vec::new();
                next.try_reserve_exact(max_branches)
                    .map_err(|_|EnsembleError::Allocation)?;
                for b in branches{
                    let p0=b.density.wire_probability(*qubit,false)?;
                    let p1=b.density.wire_probability(*qubit,true)?;
                    if !p0.is_finite()||!p1.is_finite()||
                        (p0+p1-1.0).abs()>NORM_TOLERANCE{
                        return Err(EnsembleError::ProbabilityMass);
                    }
                    for (value,p) in [(false,p0),(true,p1)]{
                        if p==0.0{continue;}
                        let weight=b.weight*p;
                        if !weight.is_finite()||weight<MIN_ENSEMBLE_BRANCH_WEIGHT{
                            return Err(EnsembleError::BranchResolution);
                        }
                        let mut state=b.density.clone();
                        state.postselect(*qubit,value)?;
                        let mut bits=b.bits.clone();
                        bits[*into_bit]=Some(value);
                        next.push(Branch{density:state,bits,weight});
                    }
                }
                if next.is_empty()||next.len()>MAX_ENSEMBLE_BRANCHES{
                    return Err(EnsembleError::BranchLimit);
                }
                branches=next;
            }
        }
    }
    let mass:f64=branches.iter().map(|b|b.weight).sum();
    if !mass.is_finite()||(mass-1.0).abs()>NORM_TOLERANCE{
        return Err(EnsembleError::ProbabilityMass);
    }
    Ok(Enumerated{branches,measurements,dim,peak,work,mass})
}
/// Shared matrix assembly/validation for unconditioned and postselected
/// states. Normalization must be actual selected Born event probability;
/// no renormalization by counted branches or assumed fair coin.
fn weighted_density<'a>(
    dim:usize,branches:impl Iterator<Item=&'a Branch>,normalizer:f64,
)->Result<(Vec<Complex>,f64,f64),EnsembleError>{
    if !normalizer.is_finite()||normalizer<=0.0{
        return Err(EnsembleError::ProbabilityMass);
    }
    let mut matrix=vec![Complex::ZERO;mul(dim,dim)?];
    for b in branches{
        let w=b.weight/normalizer;
        if !w.is_finite()||w<=0.0{
            return Err(EnsembleError::ProbabilityMass);
        }
        for row in 0..dim{
            for col in 0..dim{
                let z=b.density.element(row,col)?;
                let i=row*dim+col;
                matrix[i].re+=z.re*w;
                matrix[i].im+=z.im*w;
            }
        }
    }
    let mut trace=0.0;
    let mut purity=0.0;
    for row in 0..dim{
        let diagonal=matrix[row*dim+row];
        if !diagonal.re.is_finite()||!diagonal.im.is_finite()||
            diagonal.re< -NORM_TOLERANCE||
            diagonal.im.abs()>NORM_TOLERANCE{
            return Err(EnsembleError::InvalidDensity);
        }
        trace+=diagonal.re;
        for col in 0..dim{
            let z=matrix[row*dim+col];
            let counterpart=matrix[col*dim+row];
            if !z.re.is_finite()||!z.im.is_finite()||
                (z.re-counterpart.re).abs()>NORM_TOLERANCE||
                (z.im+counterpart.im).abs()>NORM_TOLERANCE{
                return Err(EnsembleError::InvalidDensity);
            }
            purity+=z.norm_squared();
        }
    }
    if !trace.is_finite()||(trace-1.0).abs()>NORM_TOLERANCE||
       !purity.is_finite()||purity>1.0+NORM_TOLERANCE||
       purity<1.0/dim as f64-NORM_TOLERANCE{
        return Err(EnsembleError::InvalidDensity);
    }
    Ok((matrix,trace,purity))
}
/// C-023 exact original unconditional public API (unchanged signature and
/// histogram semantics), now shares the C-024 physical channel internals.
pub fn execute_noisy_ensemble(
    program:&NoisyCircuit,budget:usize,
)->Result<NoisyEnsemble,EnsembleError>{
    let run=enumerate(program,budget,0,0)?;
    let (matrix,trace,purity)=weighted_density(
        run.dim,run.branches.iter(),1.0)?;
    let mut histogram=BTreeMap::new();
    for b in &run.branches{
        let key:String=b.bits.iter().rev().map(|bit|match bit{
            None=>'?',Some(false)=>'0',Some(true)=>'1',
        }).collect();
        *histogram.entry(key).or_insert(0.0)+=b.weight;
    }
    Ok(NoisyEnsemble{
        qubits:program.qubits(),dimension:run.dim,final_density:matrix,
        trace,purity,classical_probabilities:histogram,
        final_branch_count:run.branches.len(),measurement_count:run.measurements,
        probability_mass:run.mass,reserved_peak_logical_bytes:run.peak,
        charged_work_units:run.work,
    })
}
/// Postselection on FINAL explicitly written classical bits, including
/// last-write-wins after repeat measurements. Noise and intermediate
/// feed-forward are fully applied BEFORE final event selection.
/// Emits no histogram or unselected instrument traces.
pub fn execute_noisy_conditioned_ensemble(
    program:&NoisyCircuit,conditions:&[NoisyCondition],budget:usize,
)->Result<NoisyConditionalEnsemble,EnsembleError>{
    validate_conditions(program,conditions)?;
    let extra_bytes=mul(conditions.len(),size_of::<NoisyCondition>())?;
    let extra_work=mul(conditions.len(),MAX_ENSEMBLE_BRANCHES)?;
    let run=enumerate(program,budget,extra_bytes,extra_work)?;
    let probability:f64=run.branches.iter()
        .filter(|b|matches_conditions(b,conditions))
        .map(|b|b.weight).sum();
    let count=run.branches.iter()
        .filter(|b|matches_conditions(b,conditions)).count();
    if count==0||probability==0.0{
        return Err(EnsembleError::ImpossibleCondition);
    }
    if !probability.is_finite()||probability>1.0+NORM_TOLERANCE{
        return Err(EnsembleError::ProbabilityMass);
    }
    if probability<MIN_NOISY_CONDITION_PROBABILITY{
        return Err(EnsembleError::ConditionProbabilityTooSmall);
    }
    let (matrix,trace,purity)=weighted_density(
        run.dim,run.branches.iter()
            .filter(|b|matches_conditions(b,conditions)),probability)?;
    Ok(NoisyConditionalEnsemble{
        conditions:conditions.to_vec(),
        condition_probability:probability,
        selected_branch_count:count,measurement_count:run.measurements,
        qubits:program.qubits(),dimension:run.dim,final_density:matrix,
        trace,purity,reserved_peak_logical_bytes:run.peak,
        charged_work_units:run.work,
    })
}
