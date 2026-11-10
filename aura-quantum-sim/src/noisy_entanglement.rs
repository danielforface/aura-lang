//! C-025: two-wire mixed-state entanglement analysis, not a purity proxy.
//! Real density-matrix simulation -> q0-LSB partial trace -> 4x4 PPT spectrum.
//! Fully bounded, CPU-only; no hardware, QIR, or formal verification claim.
use crate::noisy_ensemble::{
    execute_noisy_ensemble,execute_noisy_conditioned_ensemble,EnsembleError,
    NoisyCondition,MAX_ENSEMBLE_WORK,
};
use crate::noisy_circuit::NoisyCircuit;
use crate::{Complex,NORM_TOLERANCE};
pub const ENTANGLEMENT_PROFILE:&str="noisy-two-qubit-ppt-density-f64/1";
pub const ENTANGLEMENT_EXTRA_BYTES:usize=4096;
pub const ENTANGLEMENT_EXTRA_WORK:usize=4096;
pub const EIGEN_ABS_TOLERANCE:f64=2e-13;
pub const PPT_WITNESS_TOLERANCE:f64=1e-10;
pub const MAX_JACOBI_SWEEPS:usize=48;

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum EntanglementError{
    InvalidPair,InvalidBudget,Overflow,InvalidDensity,
    EigensolverConvergence,Ensemble(EnsembleError),
}
impl std::fmt::Display for EntanglementError{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"{self:?}")
    }
}
impl std::error::Error for EntanglementError{}
impl From<EnsembleError> for EntanglementError{
    fn from(x:EnsembleError)->Self{Self::Ensemble(x)}
}
#[derive(Debug,Clone)]
pub struct NoisyPairEntanglement{
    pub selected_pair:[usize;2],
    /// q0-LSB: local index bit 0 is pair[0], bit 1 is pair[1].
    pub reduced_density:[[Complex;4];4],
    pub reduced_purity:f64,
    pub single_wire_purities:[f64;2],
    pub mutual_information_bits:f64,
    pub partial_transpose_eigenvalues:[f64;4],
    /// Sum of absolute negative PT eigenvalues, not a general witness of all
    /// entanglement in dimensions above 2x2.
    pub negativity:f64,
    pub logarithmic_negativity_bits:f64,
    /// ONLY says a strictly negative 2x2 partial-transpose eigenvalue was
    /// detected beyond the declared numerical threshold.
    pub npt_witness_detected:bool,
    pub numerical_clamp_applied:bool,
    pub selected_conditions:Option<Vec<NoisyCondition>>,
    pub condition_probability:Option<f64>,
    pub selected_branch_count:Option<usize>,
    pub reserved_peak_logical_bytes:usize,
    pub charged_work_units:usize,
}
fn scale(z:Complex,t:f64)->Complex{
    Complex::new(z.re*t,z.im*t)
}
fn neg(z:Complex)->Complex{scale(z,-1.0)}
fn conj(z:Complex)->Complex{Complex::new(z.re,-z.im)}
fn closed_finite(z:Complex)->bool{z.re.is_finite()&&z.im.is_finite()}
fn add(a:Complex,b:Complex)->Complex{a+b}
fn validate_pair(qubits:usize,pair:[usize;2])->Result<(),EntanglementError>{
    if pair[0]>=pair[1]||pair[1]>=qubits{
        return Err(EntanglementError::InvalidPair);
    }
    Ok(())
}
/// Partial trace over all wires outside (a,b), including nonadjacent
/// wires and coherences. Do not confuse q0-LSB row order with tensor
/// factor order in an external library.
pub fn reduced_pair(
    density:&[Complex],qubits:usize,pair:[usize;2],
)->Result<[[Complex;4];4],EntanglementError>{
    validate_pair(qubits,pair)?;
    let dim=1usize.checked_shl(
        u32::try_from(qubits).map_err(|_|EntanglementError::Overflow)?
    ).ok_or(EntanglementError::Overflow)?;
    if density.len()!=dim.checked_mul(dim).ok_or(EntanglementError::Overflow)?{
        return Err(EntanglementError::InvalidDensity);
    }
    let mask=(1usize<<pair[0])|(1usize<<pair[1]);
    let mut out=[[Complex::ZERO;4];4];
    for row in 0..dim{
        let r=((row>>pair[0])&1)|(((row>>pair[1])&1)<<1);
        for col in 0..dim{
            if (row&!mask)!=(col&!mask){continue;}
            let c=((col>>pair[0])&1)|(((col>>pair[1])&1)<<1);
            let z=density[row*dim+col];
            if !closed_finite(z){return Err(EntanglementError::InvalidDensity);}
            out[r][c]=out[r][c]+z;
        }
    }
    Ok(out)
}
fn purity<const N:usize>(rho:&[[Complex;N];N])->Result<f64,EntanglementError>{
    let mut trace=0.0;
    let mut p=0.0;
    for (i,row) in rho.iter().enumerate(){
        let d=row[i];
        if !closed_finite(d)||d.im.abs()>NORM_TOLERANCE
            ||d.re< -NORM_TOLERANCE{
            return Err(EntanglementError::InvalidDensity);
        }
        trace+=d.re;
        for (j,&z) in row.iter().enumerate(){
            let other=rho[j][i];
            if !closed_finite(z)||
                (z.re-other.re).abs()>NORM_TOLERANCE||
                (z.im+other.im).abs()>NORM_TOLERANCE{
                return Err(EntanglementError::InvalidDensity);
            }
            p+=z.norm_squared();
        }
    }
    if !trace.is_finite()||(trace-1.0).abs()>NORM_TOLERANCE
        ||!p.is_finite()||p<1.0/N as f64-NORM_TOLERANCE
        ||p>1.0+NORM_TOLERANCE{
        return Err(EntanglementError::InvalidDensity);
    }
    Ok(p)
}
fn reduced_single(pair:&[[Complex;4];4],wire:usize)->[[Complex;2];2]{
    let mut out=[[Complex::ZERO;2];2];
    for (r,row) in pair.iter().enumerate(){
        for (c,&z) in row.iter().enumerate(){
            let ra=(r>>wire)&1;
            let ca=(c>>wire)&1;
            let other=1-wire;
            if ((r>>other)&1)==((c>>other)&1){
                out[ra][ca]=add(out[ra][ca],z);
            }
        }
    }
    out
}
fn ent2(rho:&[[Complex;2];2])->Result<f64,EntanglementError>{
    let a=rho[0][0].re;
    let d=rho[1][1].re;
    let off=rho[0][1].norm_squared();
    let gap=((a-d)*(a-d)+4.0*off).sqrt();
    let eig=[(a+d-gap)*0.5,(a+d+gap)*0.5];
    entropy(&eig)
}
fn entropy(values:&[f64])->Result<f64,EntanglementError>{
    let mut sum=0.0;
    let mut s=0.0;
    for &x in values{
        if !x.is_finite()||x< -NORM_TOLERANCE||x>1.0+NORM_TOLERANCE{
            return Err(EntanglementError::InvalidDensity);
        }
        let v=x.max(0.0);
        sum+=v;
        if v>0.0{s-=v*v.log2();}
    }
    if (sum-1.0).abs()>NORM_TOLERANCE{
        return Err(EntanglementError::InvalidDensity);
    }
    Ok(s)
}
/// Qubit B partial transpose: (a,b;c,d) -> (a,d;c,b).
fn transpose_second(rho:&[[Complex;4];4])->[[Complex;4];4]{
    let mut pt=[[Complex::ZERO;4];4];
    for r in 0..4{
        for c in 0..4{
            pt[r][c]=rho[(r&1)|(c&2)][(c&1)|(r&2)];
        }
    }
    pt
}
/// Numerically bounded cyclic complex-Hermitian Jacobi diagonalization:
/// each rotation removes a complex off-diagonal with an explicit unitary
/// phase followed by a REAL 2x2 Jacobi rotation. No 4x4 eigen crate needed.
/// Eigenvalues are returned in ascending order.
pub fn hermitian_eigenvalues_4(
    input:[[Complex;4];4],
)->Result<[f64;4],EntanglementError>{
    let mut a=input;
    for (i,row) in a.iter().enumerate(){
        if !closed_finite(row[i])||row[i].im.abs()>NORM_TOLERANCE{
            return Err(EntanglementError::InvalidDensity);
        }
        for (j,&z) in row.iter().enumerate().skip(i+1){
            if !closed_finite(z)||!closed_finite(a[j][i])||
                (z.re-a[j][i].re).abs()>NORM_TOLERANCE||
                (z.im+a[j][i].im).abs()>NORM_TOLERANCE{
                return Err(EntanglementError::InvalidDensity);
            }
        }
    }
    for _sweep in 0..MAX_JACOBI_SWEEPS{
        let mut maximum=0.0_f64;
        for p in 0..3{
            for q in (p+1)..4{
                let b=a[p][q];
                let magnitude=b.norm_squared().sqrt();
                if !magnitude.is_finite(){
                    return Err(EntanglementError::InvalidDensity);
                }
                maximum=maximum.max(magnitude);
                if magnitude<=4e-16{continue;}
                // D† A D; D_qq=conj(A_pq)/|A_pq| ensures A_pq real+.
                let phase=scale(conj(b),1.0/magnitude);
                // Apply phase to column q, then restore Hermitian row q
                // in a separate pass to avoid aliasing mutable matrix rows.
                let mut conjugate_column=[Complex::ZERO;4];
                for (k,row) in a.iter_mut().enumerate(){
                    if k==q{continue;}
                    row[q]=row[q]*phase;
                    conjugate_column[k]=conj(row[q]);
                }
                for (k,value) in a[q].iter_mut().enumerate(){
                    if k!=q{*value=conjugate_column[k];}
                }
                let app=a[p][p].re;
                let aqq=a[q][q].re;
                let tau=(aqq-app)/(2.0*magnitude);
                let t=if tau>=0.0{
                    1.0/(tau+tau.hypot(1.0))
                }else{
                    -1.0/(-tau+tau.hypot(1.0))
                };
                let c=1.0/(1.0+t*t).sqrt();
                let s=t*c;
                // Two-phase column/row update is the exact same
                // unitary real Jacobi rotation without simultaneous
                // mutable borrowing of rows p and q.
                let mut new_p=[Complex::ZERO;4];
                let mut new_q=[Complex::ZERO;4];
                for (k,row) in a.iter_mut().enumerate(){
                    if k==p||k==q{continue;}
                    let x=row[p];
                    let y=row[q];
                    new_p[k]=add(scale(x,c),neg(scale(y,s)));
                    new_q[k]=add(scale(x,s),scale(y,c));
                    row[p]=new_p[k];
                    row[q]=new_q[k];
                }
                for (k,&value) in new_p.iter().enumerate(){
                    if k==p||k==q{continue;}
                    a[p][k]=conj(value);
                    a[q][k]=conj(new_q[k]);
                }
                a[p][p]=Complex::new(app-t*magnitude,0.0);
                a[q][q]=Complex::new(aqq+t*magnitude,0.0);
                a[p][q]=Complex::ZERO;
                a[q][p]=Complex::ZERO;
            }
        }
        if maximum<=EIGEN_ABS_TOLERANCE{
            let mut eig=std::array::from_fn(|i|a[i][i].re);
            eig.sort_by(|x,y|x.total_cmp(y));
            if eig.iter().any(|x|!x.is_finite()){
                return Err(EntanglementError::InvalidDensity);
            }
            return Ok(eig);
        }
    }
    Err(EntanglementError::EigensolverConvergence)
}
fn calculate(
    density:&[Complex],qubits:usize,pair:[usize;2],
    selection:Option<(Vec<NoisyCondition>,f64,usize)>,
    peak:usize,work:usize,
)->Result<NoisyPairEntanglement,EntanglementError>{
    let reduced=reduced_pair(density,qubits,pair)?;
    let reduced_purity=purity(&reduced)?;
    let wa=reduced_single(&reduced,0);
    let wb=reduced_single(&reduced,1);
    let pa=purity(&wa)?;
    let pb=purity(&wb)?;
    let eig=hermitian_eigenvalues_4(reduced)?;
    let s_pair=entropy(&eig)?;
    let s_a=ent2(&wa)?;
    let s_b=ent2(&wb)?;
    let mut mutual_information=s_a+s_b-s_pair;
    if mutual_information< -NORM_TOLERANCE{
        return Err(EntanglementError::InvalidDensity);
    }
    if mutual_information<0.0{mutual_information=0.0;}
    let pt_eig=hermitian_eigenvalues_4(transpose_second(&reduced))?;
    let mut negativity=0.0;
    let mut clamped=false;
    for &x in &pt_eig{
        if x< -PPT_WITNESS_TOLERANCE{negativity-=x;}
        else if x<0.0{clamped=true;}
    }
    if !(0.0..=0.5+NORM_TOLERANCE).contains(&negativity){
        return Err(EntanglementError::InvalidDensity);
    }
    let (selected_conditions,condition_probability,selected_branch_count)=
        match selection{
            Some((bits,p,count))=>(Some(bits),Some(p),Some(count)),
            None=>(None,None,None),
        };
    Ok(NoisyPairEntanglement{
        selected_pair:pair,reduced_density:reduced,reduced_purity,
        single_wire_purities:[pa,pb],
        mutual_information_bits:mutual_information,
        partial_transpose_eigenvalues:pt_eig,
        negativity,logarithmic_negativity_bits:(1.0+2.0*negativity).log2(),
        npt_witness_detected:pt_eig[0]< -PPT_WITNESS_TOLERANCE,
        numerical_clamp_applied:clamped,
        selected_conditions,condition_probability,selected_branch_count,
        reserved_peak_logical_bytes:peak,charged_work_units:work,
    })
}
/// A 2×2 PPT violation is a valid mixed-state entanglement *witness* for
/// the selected two-qubit reduced state, not a claim of global separability.
/// No selector: unconditional noisy channel. Some: final-classical-bit
/// postselection. Strict pair order a<b is mandatory. No raw state/histogram
/// of unselected outcomes is emitted.
pub fn analyze_noisy_pair(
    program:&NoisyCircuit,pair:[usize;2],
    conditions:Option<&[NoisyCondition]>,budget:usize,
)->Result<NoisyPairEntanglement,EntanglementError>{
    validate_pair(program.qubits(),pair)?;
    if budget<=ENTANGLEMENT_EXTRA_BYTES{
        return Err(EntanglementError::InvalidBudget);
    }
    // Charge extra 4x4 scratch matrices and bounded Jacobi work before
    // invoking the existing C-023/C-024 heavy channel calculation.
    let adjusted_budget=budget-ENTANGLEMENT_EXTRA_BYTES;
    let (density,peak,work,selection)=match conditions{
        Some(bits)=>{
            let result=execute_noisy_conditioned_ensemble(
                program,bits,adjusted_budget)?;
            (result.final_density,
             result.reserved_peak_logical_bytes,
             result.charged_work_units,
             Some((result.conditions,result.condition_probability,
                   result.selected_branch_count)))
        }
        None=>{
            let result=execute_noisy_ensemble(program,adjusted_budget)?;
            (result.final_density,
             result.reserved_peak_logical_bytes,
             result.charged_work_units,None)
        }
    };
    let peak=peak.checked_add(ENTANGLEMENT_EXTRA_BYTES)
        .ok_or(EntanglementError::Overflow)?;
    let work=work.checked_add(ENTANGLEMENT_EXTRA_WORK)
        .ok_or(EntanglementError::Overflow)?;
    if work>MAX_ENSEMBLE_WORK{
        return Err(EntanglementError::Ensemble(EnsembleError::WorkLimit));
    }
    if peak>budget{return Err(EntanglementError::InvalidBudget);}
    calculate(&density,program.qubits(),pair,selection,peak,work)
}
