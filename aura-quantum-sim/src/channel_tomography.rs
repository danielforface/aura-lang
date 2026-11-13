//! C-020 local ideal-channel tomography on bounded final instrument ensemble.
//! True reduced subsystem density matrices, no sampled trajectory or QPU.
//! A mixed local state does NOT by itself establish entanglement.
use crate::channel_moments::{
    enumerate_channel,Branch,ChannelError,MAX_CHANNEL_QUBITS,
};
use crate::circuit::{Circuit,Instruction};
use crate::{Complex,NORM_TOLERANCE};
use std::collections::{BTreeMap,BTreeSet};

pub const TOMOGRAPHY_PROFILE:&str="ideal-dynamic-channel-reduced-densities-f64/1";
pub const MAX_TOMOGRAPHY_PAIRS:usize=16;
#[derive(Debug,Clone)]
pub struct OneWireDensity{
    pub wire:usize,
    /// Row-major computational-basis matrix, q0 is global LSB.
    pub density:[[Complex;2];2],
    /// Tr(rho X), Tr(rho Y), Tr(rho Z), in that order.
    pub bloch:[f64;3],
    /// Tr(rho_reduced²), 0.5..1; not an entanglement witness.
    pub purity:f64,
    /// Weighted mean of local reduced purity on each unobserved branch.
    pub mean_branch_purity:f64,
    /// mean_branch_purity - purity; mixture of instrument histories.
    pub branch_mixing_gap:f64,
}
#[derive(Debug,Clone)]
pub struct TwoWireDensity{
    pub wires:[usize;2],
    /// 4x4 row-major, pair basis = q[a] + 2*q[b], a < b.
    pub density:[[Complex;4];4],
    pub purity:f64,
    pub mean_branch_purity:f64,
    pub branch_mixing_gap:f64,
}
#[derive(Debug,Clone)]
pub struct ChannelTomography{
    pub wires:Vec<OneWireDensity>,
    pub pairs:Vec<TwoWireDensity>,
    pub branch_count:usize,
    pub instrument_count:usize,
    pub probability_mass:f64,
    pub classical_probabilities:BTreeMap<String,f64>,
    pub reserved_peak_logical_bytes:usize,
    pub work_units:usize,
}
fn conjugate(z:Complex)->Complex{Complex::new(z.re,-z.im)}
fn weighted(z:Complex,w:f64)->Complex{Complex::new(z.re*w,z.im*w)}
fn single_reduced(state:&crate::Statevector,wire:usize)
    ->Result<[[Complex;2];2],ChannelError>{
    let mut out=[[Complex::ZERO;2];2];
    let mask=1usize<<wire;
    for basis in 0..(1usize<<state.qubits()){
        if basis&mask!=0{continue;}
        let low=state.amplitude(basis)?;
        let high=state.amplitude(basis|mask)?;
        out[0][0]=out[0][0]+low*conjugate(low);
        out[0][1]=out[0][1]+low*conjugate(high);
        out[1][0]=out[1][0]+high*conjugate(low);
        out[1][1]=out[1][1]+high*conjugate(high);
    }
    Ok(out)
}
fn pair_reduced(state:&crate::Statevector,wires:[usize;2])
    ->Result<[[Complex;4];4],ChannelError>{
    let a=1usize<<wires[0];
    let b=1usize<<wires[1];
    let mut out=[[Complex::ZERO;4];4];
    for basis in 0..(1usize<<state.qubits()){
        if basis&(a|b)!=0{continue;}
        let amps=[
            state.amplitude(basis)?,
            state.amplitude(basis|a)?,
            state.amplitude(basis|b)?,
            state.amplitude(basis|a|b)?,
        ];
        for row in 0..4{
            for col in 0..4{
                out[row][col]=out[row][col]+amps[row]*conjugate(amps[col]);
            }
        }
    }
    Ok(out)
}
fn purity<const N:usize>(density:&[[Complex;N];N])
    ->Result<f64,ChannelError>{
    let mut trace=0.0;
    let mut p=0.0;
    for (i,row) in density.iter().enumerate(){
        trace+=row[i].re;
        if !row[i].re.is_finite()||row[i].im.abs()>NORM_TOLERANCE{
            return Err(ChannelError::Numeric);
        }
        for (j,z) in row.iter().enumerate(){
            let opposite=density[j][i];
            if !z.re.is_finite()||!z.im.is_finite()
                ||(z.re-opposite.re).abs()>NORM_TOLERANCE
                ||(z.im+opposite.im).abs()>NORM_TOLERANCE{
                return Err(ChannelError::Numeric);
            }
            p+=z.norm_squared();
        }
    }
    if !trace.is_finite()||(trace-1.0).abs()>NORM_TOLERANCE
        ||!p.is_finite()||p>1.0+NORM_TOLERANCE
        ||p<1.0/(N as f64)-NORM_TOLERANCE{
        return Err(ChannelError::Numeric);
    }
    Ok(p.clamp(1.0/N as f64,1.0))
}
fn mixing_gap(mean:f64,mixed:f64)->Result<f64,ChannelError>{
    let delta=mean-mixed;
    if !delta.is_finite()||delta< -NORM_TOLERANCE{
        return Err(ChannelError::Numeric);
    }
    Ok(delta.max(0.0))
}
/// A final-register filter, NOT a past instrument-history selector.
/// Canonical ascending bit indices. Only explicitly measured bits are valid.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct ClassicalCondition{
    pub bit:usize,
    pub equals:bool,
}
/// Conditioned reduced state, with NO unselected classical histogram or
/// individual instrument histories in the public result.
#[derive(Debug,Clone)]
pub struct ConditionedTomography{
    pub conditions:Vec<ClassicalCondition>,
    pub condition_probability:f64,
    pub selected_branch_count:usize,
    pub instrument_count:usize,
    pub wires:Vec<OneWireDensity>,
    pub pairs:Vec<TwoWireDensity>,
    pub reserved_peak_logical_bytes:usize,
    pub work_units:usize,
}
/// Conditional probabilities below this absolute floor cannot be certified
/// as reliable in the current double-precision numerical profile. Fail closed.
pub const MIN_CONDITION_PROBABILITY:f64=1e-12;
fn preflight_reductions(circuit:&Circuit,selected_pairs:&[[usize;2]])
    ->Result<(usize,usize),ChannelError>{
    let n=circuit.qubits();
    if !(1..=MAX_CHANNEL_QUBITS).contains(&n){
        return Err(ChannelError::InvalidWidth);
    }
    if selected_pairs.len()>MAX_TOMOGRAPHY_PAIRS{
        return Err(ChannelError::InvalidTomographyPairs);
    }
    let mut seen=BTreeSet::new();
    for pair in selected_pairs{
        if pair[0]>=pair[1]||pair[1]>=n||!seen.insert(*pair){
            return Err(ChannelError::InvalidTomographyPairs);
        }
    }
    // Conservative extra memory reservation beyond the common channel
    // branch buffer, and CPU-work factor for each 2x2/4x4 reduction.
    let work=n.checked_add(selected_pairs.len().checked_mul(4)
        .ok_or(ChannelError::Overflow)?).ok_or(ChannelError::Overflow)?;
    let reserve=n.checked_mul(256).and_then(|v|v.checked_add(
        selected_pairs.len().checked_mul(512)?))
        .ok_or(ChannelError::Overflow)?;
    Ok((work,reserve))
}
fn validate_conditions(circuit:&Circuit,conditions:&[ClassicalCondition])
    ->Result<(),ChannelError>{
    if conditions.is_empty()||conditions.len()>64{
        return Err(ChannelError::InvalidCondition);
    }
    if circuit.classical_bits()>64{
        return Err(ChannelError::InvalidCondition);
    }
    let mut measured=vec![false;circuit.classical_bits()];
    for op in circuit.instructions(){
        if let Instruction::Measure{into_bit,..}=op{
            if *into_bit<measured.len(){measured[*into_bit]=true;}
        }
    }
    let mut previous=None;
    for c in conditions{
        if c.bit>=measured.len()||!measured[c.bit]
            ||previous.is_some_and(|b|c.bit<=b){
            return Err(ChannelError::InvalidCondition);
        }
        previous=Some(c.bit);
    }
    Ok(())
}
/// One numerical reduction routine for unconditional and postselected
/// tomography. Caller decides the normalization and which branches enter.
fn summarize_reductions<'a>(
    n:usize,selected_pairs:&[[usize;2]],
    branches:impl Iterator<Item=&'a Branch>,normalizer:f64,
)->Result<(Vec<OneWireDensity>,Vec<TwoWireDensity>),ChannelError>{
    if normalizer<=0.0||!normalizer.is_finite(){
        return Err(ChannelError::Numeric);
    }
    let mut singles=vec![[[Complex::ZERO;2];2];n];
    let mut pairs=vec![[[Complex::ZERO;4];4];selected_pairs.len()];
    let mut single_branch_purity=vec![0.0;n];
    let mut pair_branch_purity=vec![0.0;selected_pairs.len()];
    for b in branches{
        let weight=b.weight/normalizer;
        if weight<=0.0||!weight.is_finite(){
            return Err(ChannelError::Numeric);
        }
        for q in 0..n{
            let local=single_reduced(&b.state,q)?;
            single_branch_purity[q]+=weight*purity(&local)?;
            for row in 0..2{
                for col in 0..2{
                    singles[q][row][col]=singles[q][row][col]+
                        weighted(local[row][col],weight);
                }
            }
        }
        for (i,&wires) in selected_pairs.iter().enumerate(){
            let local=pair_reduced(&b.state,wires)?;
            pair_branch_purity[i]+=weight*purity(&local)?;
            for row in 0..4{
                for col in 0..4{
                    pairs[i][row][col]=pairs[i][row][col]+
                        weighted(local[row][col],weight);
                }
            }
        }
    }
    let wires=singles.into_iter().enumerate()
        .map(|(wire,density)|{
            let p=purity(&density)?;
            let rho01=density[0][1];
            Ok(OneWireDensity{
                wire,density,
                bloch:[2.0*rho01.re,-2.0*rho01.im,
                    density[0][0].re-density[1][1].re],
                purity:p,
                mean_branch_purity:single_branch_purity[wire],
                branch_mixing_gap:mixing_gap(single_branch_purity[wire],p)?,
            })
        }).collect::<Result<Vec<_>,ChannelError>>()?;
    let pairs=pairs.into_iter().enumerate()
        .map(|(i,density)|{
            let p=purity(&density)?;
            Ok(TwoWireDensity{
                wires:selected_pairs[i],density,purity:p,
                mean_branch_purity:pair_branch_purity[i],
                branch_mixing_gap:mixing_gap(pair_branch_purity[i],p)?,
            })
        }).collect::<Result<Vec<_>,ChannelError>>()?;
    Ok((wires,pairs))
}
/// Original C-020 unconditioned channel-reduced tomography, API unchanged.
pub fn execute_channel_tomography(
    circuit:&Circuit,selected_pairs:&[[usize;2]],budget:usize,
)->Result<ChannelTomography,ChannelError>{
    let (work_factor,reserve)=preflight_reductions(circuit,selected_pairs)?;
    let (branches,instruments,peak,work,mass)=
        enumerate_channel(circuit,budget,work_factor,reserve)?;
    // Preserve C-020 arithmetic semantics exactly: unconditioned ensemble
    // weights are already globally normalized, so normalizer is 1.
    let (wires,pairs)=summarize_reductions(
        circuit.qubits(),selected_pairs,branches.iter(),1.0)?;
    let mut histogram=BTreeMap::new();
    for b in &branches{
        let key:String=b.bits.iter().rev().map(|bit|match bit{
            Some(true)=>'1',Some(false)=>'0',None=>'?',
        }).collect();
        *histogram.entry(key).or_insert(0.0)+=b.weight;
    }
    Ok(ChannelTomography{
        wires,pairs,branch_count:branches.len(),
        instrument_count:instruments,probability_mass:mass,
        classical_probabilities:histogram,
        reserved_peak_logical_bytes:peak,work_units:work,
    })
}
/// Postselect ONLY by final explicit classical register values.
/// Returns conditional reduced densities and Born probability of selection.
/// Unrequested branch histories and registers are not emitted or persisted.
pub fn execute_conditioned_tomography(
    circuit:&Circuit,selected_pairs:&[[usize;2]],
    conditions:&[ClassicalCondition],budget:usize,
)->Result<ConditionedTomography,ChannelError>{
    let (work_factor,reserve)=preflight_reductions(circuit,selected_pairs)?;
    validate_conditions(circuit,conditions)?;
    let work_factor=work_factor.checked_add(conditions.len())
        .ok_or(ChannelError::Overflow)?;
    let reserve=reserve.checked_add(
        conditions.len().checked_mul(std::mem::size_of::<ClassicalCondition>())
            .ok_or(ChannelError::Overflow)?
    ).ok_or(ChannelError::Overflow)?;
    let (branches,instrument_count,peak,work,_mass)=
        enumerate_channel(circuit,budget,work_factor,reserve)?;
    let is_selected=|b:&Branch|conditions.iter().all(|c|
        b.bits[c.bit]==Some(c.equals));
    let mut probability=0.0;
    let mut count=0usize;
    for b in &branches{
        if is_selected(b){
            probability+=b.weight;
            count+=1;
        }
    }
    if count==0||probability==0.0{
        return Err(ChannelError::ImpossibleCondition);
    }
    if !probability.is_finite()||probability>1.0+NORM_TOLERANCE{
        return Err(ChannelError::ProbabilityMass);
    }
    if probability<MIN_CONDITION_PROBABILITY{
        return Err(ChannelError::ConditionProbabilityTooSmall);
    }
    let (wires,pairs)=summarize_reductions(
        circuit.qubits(),selected_pairs,
        branches.iter().filter(|b|is_selected(b)),probability)?;
    Ok(ConditionedTomography{
        conditions:conditions.to_vec(),
        condition_probability:probability,
        selected_branch_count:count,instrument_count,
        wires,pairs,reserved_peak_logical_bytes:peak,work_units:work,
    })
}
