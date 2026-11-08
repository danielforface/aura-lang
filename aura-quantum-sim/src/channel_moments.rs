//! Bounded ideal measurement-channel moments: enumerate all physical outcomes.
//! Distinct from seeded conditional moments, noisy density and quantum hardware.
use crate::circuit::{Circuit,Instruction};
use crate::factorized::{PauliTerm,MAX_PAULI_TERMS,MAX_ABS_PAULI_COEFFICIENT};
use crate::{Complex,Gate,Statevector,SimError,DEFAULT_MEMORY_BUDGET,NORM_TOLERANCE};
use std::collections::BTreeMap;
use std::mem::size_of;
pub const CHANNEL_PROFILE:&str="ideal-dynamic-channel-dense-f64-enumerated/1";
pub const MAX_CHANNEL_QUBITS:usize=8;
pub const MAX_CHANNEL_INSTRUMENTS:usize=8;
pub const MAX_CHANNEL_BRANCHES:usize=256;
pub const MAX_CHANNEL_INSTRUCTIONS:usize=512;
pub const MAX_CHANNEL_WORK:usize=67_108_864;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ChannelError{
    InvalidWidth,InvalidClassicalBits,InvalidInstructions,InvalidInstruments,
    InvalidTerms,InvalidCoefficient,InvalidPauli{term:usize,wire:usize},
    InvalidBit,ReadBeforeWrite{instruction:usize,bit:usize},
    InvalidBudget,BudgetExceeded,WorkLimit,BranchLimit,ProbabilityUnderflow,
    ProbabilityMass,Numeric,Overflow,Allocation,Simulator(SimError),
}
impl From<SimError> for ChannelError{
    fn from(e:SimError)->Self{Self::Simulator(e)}
}
impl std::fmt::Display for ChannelError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for ChannelError{}
#[derive(Debug,Clone)]
pub struct ChannelMoments{
    pub mean:f64,pub second_moment:f64,pub variance:f64,
    pub within_branch_variance:f64,pub between_branch_variance:f64,
    pub branch_count:usize,pub instrument_count:usize,
    pub probability_mass:f64,pub classical_probabilities:BTreeMap<String,f64>,
    pub reserved_peak_logical_bytes:usize,pub work_units:usize,
    pub roundoff_clamped:bool,
}
struct Branch{state:Statevector,bits:Vec<Option<bool>>,weight:f64}
fn plus(a:usize,b:usize)->Result<usize,ChannelError>{
    a.checked_add(b).ok_or(ChannelError::Overflow)
}
fn times(a:usize,b:usize)->Result<usize,ChannelError>{
    a.checked_mul(b).ok_or(ChannelError::Overflow)
}
fn gate_valid(g:&Gate,n:usize)->Result<(),ChannelError>{
    let valid=|q:usize|if q<n{Ok(())}else{
        Err(ChannelError::Simulator(SimError::InvalidQubit{index:q,qubits:n}))
    };
    match g{
        Gate::H(q)|Gate::X(q)|Gate::Y(q)|Gate::Z(q)|
        Gate::S(q)|Gate::T(q)=>valid(*q),
        Gate::Cnot{control,target}|Gate::Cz{control,target}=>{
            valid(*control)?;valid(*target)?;
            if control==target{
                return Err(ChannelError::Simulator(SimError::DuplicateQubit(*control)));
            }
            Ok(())
        }
        Gate::Swap{a,b}=>{
            valid(*a)?;valid(*b)?;
            if a==b{return Err(ChannelError::Simulator(SimError::DuplicateQubit(*a)));}
            Ok(())
        }
    }
}
fn admit(c:&Circuit,terms:&[PauliTerm],budget:usize)
    ->Result<(usize,usize,usize),ChannelError>{
    let n=c.qubits();
    if !(1..=MAX_CHANNEL_QUBITS).contains(&n){return Err(ChannelError::InvalidWidth);}
    if c.classical_bits()>64{return Err(ChannelError::InvalidClassicalBits);}
    if c.instructions().len()>MAX_CHANNEL_INSTRUCTIONS{return Err(ChannelError::InvalidInstructions);}
    if !(1..=MAX_PAULI_TERMS).contains(&terms.len()){return Err(ChannelError::InvalidTerms);}
    if budget==0||budget>DEFAULT_MEMORY_BUDGET{return Err(ChannelError::InvalidBudget);}
    for (i,t) in terms.iter().enumerate(){
        if !t.coefficient.is_finite()||t.coefficient.abs()>MAX_ABS_PAULI_COEFFICIENT{
            return Err(ChannelError::InvalidCoefficient);
        }
        if t.paulis.len()!=n{return Err(ChannelError::InvalidPauli{term:i,wire:n});}
        for (q,ch) in t.paulis.bytes().enumerate(){
            if !matches!(ch,b'I'|b'X'|b'Y'|b'Z'){
                return Err(ChannelError::InvalidPauli{term:i,wire:q});
            }
        }
    }
    let mut written=vec![false;c.classical_bits()];
    let mut instruments=0usize;
    for (index,op) in c.instructions().iter().enumerate(){
        match op{
            Instruction::Gate(g)=>gate_valid(g,n)?,
            Instruction::If{bit,gate,..}=>{
                if *bit>=written.len(){return Err(ChannelError::InvalidBit);}
                gate_valid(gate,n)?; // check even an untaken branch
                if !written[*bit]{
                    return Err(ChannelError::ReadBeforeWrite{instruction:index,bit:*bit});
                }
            }
            Instruction::Measure{qubit,into_bit}=>{
                gate_valid(&Gate::X(*qubit),n)?;
                if *into_bit>=written.len(){return Err(ChannelError::InvalidBit);}
                written[*into_bit]=true;
                instruments=plus(instruments,1)?;
            }
            Instruction::Reset{qubit}=>{
                gate_valid(&Gate::X(*qubit),n)?;
                instruments=plus(instruments,1)?;
            }
        }
    }
    if instruments>MAX_CHANNEL_INSTRUMENTS{return Err(ChannelError::InvalidInstruments);}
    let branches=1usize<<instruments;
    let dim=1usize<<n;
    let state_bytes=times(dim,size_of::<Complex>())?;
    let branch_bytes=plus(
        plus(state_bytes,times(c.classical_bits(),size_of::<Option<bool>>())?)?,
        plus(size_of::<Branch>(),64)?,
    )?;
    // Both generations coexist during branching; two dense H|psi> scratch
    // states + serialized classical outcome histogram also conservatively held.
    let required=plus(
        times(times(branches,branch_bytes)?,3)?,
        plus(times(state_bytes,2)?,times(branches,plus(c.classical_bits(),128)?)?)?,
    )?;
    if required>budget{return Err(ChannelError::BudgetExceeded);}
    let operations=plus(c.instructions().len().max(1),times(terms.len(),n+1)?)?;
    let work=times(times(branches,dim)?,operations)?;
    if work>MAX_CHANNEL_WORK{return Err(ChannelError::WorkLimit);}
    Ok((instruments,required,work))
}
fn supported_amplitude(s:&Statevector,q:usize,outcome:bool)->Result<bool,ChannelError>{
    for i in 0..(1usize<<s.qubits()){
        if ((i>>q)&1!=0)==outcome&&s.amplitude(i)?!=Complex::ZERO{return Ok(true);}
    }
    Ok(false)
}
fn instrument(old:Vec<Branch>,q:usize,measure_bit:Option<usize>)
    ->Result<Vec<Branch>,ChannelError>{
    let cap=times(old.len(),2)?.min(MAX_CHANNEL_BRANCHES);
    let mut result=Vec::new();
    result.try_reserve_exact(cap).map_err(|_|ChannelError::Allocation)?;
    for b in old{
        let p0=b.state.wire_probability(q,false)?;
        let p1=b.state.wire_probability(q,true)?;
        if !p0.is_finite()||!p1.is_finite()||p0<0.0||p1<0.0
            ||(p0+p1-1.0).abs()>NORM_TOLERANCE{
            return Err(ChannelError::ProbabilityMass);
        }
        for (outcome,p) in [(false,p0),(true,p1)]{
            if p==0.0{
                if supported_amplitude(&b.state,q,outcome)?{
                    return Err(ChannelError::ProbabilityUnderflow);
                }
                continue;
            }
            let weight=b.weight*p;
            if weight==0.0||!weight.is_finite(){
                return Err(ChannelError::ProbabilityUnderflow);
            }
            if result.len()>=MAX_CHANNEL_BRANCHES{return Err(ChannelError::BranchLimit);}
            let mut state=b.state.clone();
            state.postselect(q,outcome)?;
            // Reset is unobserved Z measurement followed by conditional X.
            if measure_bit.is_none()&&outcome{state.apply(Gate::X(q))?;}
            let mut bits=b.bits.clone();
            if let Some(index)=measure_bit{bits[index]=Some(outcome);}
            result.push(Branch{state,bits,weight});
        }
    }
    if result.is_empty(){return Err(ChannelError::ProbabilityMass);}
    Ok(result)
}
// A *different* numerical route from Pauli-product C-017: construct H|psi>
// and evaluate its norm and inner product. Supports Y complex phases.
fn pure_moments(state:&Statevector,terms:&[PauliTerm])
    ->Result<(f64,f64,f64,bool),ChannelError>{
    let dim=1usize<<state.qubits();
    let mut hpsi=Vec::<Complex>::new();
    hpsi.try_reserve_exact(dim).map_err(|_|ChannelError::Allocation)?;
    hpsi.resize(dim,Complex::ZERO);
    for term in terms{
        let mut transformed=state.clone();
        for (q,ch) in term.paulis.bytes().enumerate(){
            let g=match ch{
                b'I'=>continue,b'X'=>Gate::X(q),b'Y'=>Gate::Y(q),
                b'Z'=>Gate::Z(q),_=>unreachable!("Pauli prevalidated"),
            };
            transformed.apply(g)?;
        }
        for (index,dst) in hpsi.iter_mut().enumerate(){
            *dst=*dst+Complex::new(term.coefficient,0.0)*transformed.amplitude(index)?;
        }
    }
    let mut mean=Complex::ZERO;
    let mut second=0.0;
    for (i,v) in hpsi.iter().enumerate(){
        let a=state.amplitude(i)?;
        mean=mean+Complex::new(a.re,-a.im)* *v;
        second+=v.norm_squared();
    }
    let square=mean.re*mean.re;
    let scale=second.abs().max(square.abs()).max(1.0);
    let variance=second-square;
    if !mean.re.is_finite()||!mean.im.is_finite()||!second.is_finite()
        ||!variance.is_finite()||mean.im.abs()>NORM_TOLERANCE*scale
        ||variance< -NORM_TOLERANCE*scale{
        return Err(ChannelError::Numeric);
    }
    Ok((mean.re,second,variance.max(0.0),variance<0.0))
}
/// Unconditioned moments after ALL representable ideal instrument outcomes.
/// Does not take an RNG seed. A dense <=8q profile, NOT arbitrary 128q.
pub fn execute_channel_moments(c:&Circuit,terms:&[PauliTerm],budget:usize)
    ->Result<ChannelMoments,ChannelError>{
    let (instrument_count,peak,work)=admit(c,terms,budget)?;
    let mut branches=vec![Branch{
        state:Statevector::zero(c.qubits(),budget)?,
        bits:vec![None;c.classical_bits()],weight:1.0,
    }];
    for op in c.instructions(){
        match op{
            Instruction::Gate(g)=>{
                for b in &mut branches{b.state.apply(g.clone())?;}
            }
            Instruction::If{bit,equals,gate}=>{
                for b in &mut branches{
                    if b.bits[*bit]==Some(*equals){b.state.apply(gate.clone())?;}
                }
            }
            Instruction::Measure{qubit,into_bit}=>{
                branches=instrument(branches,*qubit,Some(*into_bit))?;
            }
            Instruction::Reset{qubit}=>{
                branches=instrument(branches,*qubit,None)?;
            }
        }
    }
    let mass=branches.iter().map(|b|b.weight).sum::<f64>();
    if !mass.is_finite()||(mass-1.0).abs()>NORM_TOLERANCE{
        return Err(ChannelError::ProbabilityMass);
    }
    let mut mean=0.0;let mut second=0.0;let mut within=0.0;
    let mut roundoff_clamped=false;
    let mut conditional=Vec::<(f64,f64)>::new();
    conditional.try_reserve_exact(branches.len()).map_err(|_|ChannelError::Allocation)?;
    let mut histogram=BTreeMap::<String,f64>::new();
    for b in &branches{
        let (m,s,v,clamped)=pure_moments(&b.state,terms)?;
        mean+=b.weight*m;second+=b.weight*s;within+=b.weight*v;
        roundoff_clamped|=clamped;
        conditional.push((b.weight,m));
        let key:String=b.bits.iter().rev().map(|bit|match bit{
            Some(true)=>'1',Some(false)=>'0',None=>'?',
        }).collect();
        *histogram.entry(key).or_insert(0.0)+=b.weight;
    }
    let between=conditional.iter().map(|(w,m)|w*(m-mean)*(m-mean)).sum::<f64>();
    let variance=second-mean*mean;
    let scale=second.abs().max((mean*mean).abs()).max(1.0);
    if !mean.is_finite()||!second.is_finite()||!within.is_finite()
        ||!between.is_finite()||!variance.is_finite()
        ||variance< -NORM_TOLERANCE*scale{
        return Err(ChannelError::Numeric);
    }
    Ok(ChannelMoments{
        mean,second_moment:second,variance:variance.max(0.0),
        within_branch_variance:within,between_branch_variance:between,
        branch_count:branches.len(),instrument_count,probability_mass:mass,
        classical_probabilities:histogram,reserved_peak_logical_bytes:peak,
        work_units:work,roundoff_clamped:roundoff_clamped||variance<0.0,
    })
}
