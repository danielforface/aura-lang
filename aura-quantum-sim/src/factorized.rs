//! AURA-Q28-008 — untruncated *disconnected-factor tensor network* for pure CPU circuits.
//!
//! A state is an explicit tensor product of small fully complex Statevectors.
//! One/two-qubit gates on the same component use the already validated dense
//! oracle. Cross-component CNOT/CZ join components **unless** a mathematically
//! exact local Pauli eigenstate proof permits an equivalent local operation
//! without entanglement. SWAP relabels disconnected factor wire identities.
//! No SVD, truncation, approximation of entanglement, rank compression or claim
//! to arbitrary-width globally entangled simulation. Arithmetic remains f64.
//!
//! The component cap is strict. Projective Z measurement makes the measured
//! coordinate *provably separable*; a budgeted tensor factorization can then
//! free its wire for subsequent non-Clifford gates without truncation. On
//! allocation denial the collapsed original component remains physically valid.
//! This is standalone simulation, not QIR/Aura syntax or a QPU job.
use crate::circuit::{Instruction, ShotRecord, TraceEntry, TraceKind};
use crate::{Complex, Gate, SeededRng, SimError, Statevector, ENGINE_VERSION, NORM_TOLERANCE};
use std::collections::BTreeMap;
use std::mem::size_of;

pub const MAX_FACTOR_QUBITS: usize = 10;
pub const MAX_FACTOR_GLOBAL_QUBITS: usize = 128;
pub const MAX_FACTOR_INSTRUCTIONS: usize = 512;
pub const MAX_FACTOR_SHOTS: usize = 1024;
/// The trace-less measurement counts API is separate from the 1024-shot
/// detailed-records API, with a conservative finite global CPU work cap.
pub const MAX_FACTOR_COUNT_SHOTS:usize=65_536;
pub const MAX_FACTOR_COUNT_BINS:usize=4_096;
pub const MAX_FACTOR_COUNT_WORK_UNITS:usize=268_435_456;
/// Only a logical accounting allowance, not a guarantee of allocator RSS.
pub const FACTOR_COUNT_BIN_LOGICAL_OVERHEAD:usize=128;
pub const FACTORIZED_COUNTS_RESULT_SCHEMA:&str="aura.quantum.factorized-counts-result/1";
pub const MAX_FACTOR_WORK_UNITS: usize = 128 * 1024 * 1024;
pub const FACTORIZED_PROFILE: &str = "local-disconnected-factor-statevector-f64-untruncated/1";
pub const FACTORIZED_RESULT_SCHEMA: &str = "aura.quantum.factorized-result/1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactorError {
    UnsupportedWidth { qubits: usize, max: usize },
    FactorWidthExceeded { required: usize, max: usize },
    InvalidQubit { qubit: usize, qubits: usize },
    DuplicateQubit(usize),
    InvalidBasisWidth { expected: usize, got: usize },
    TooManyInstructions,
    ZeroShots,
    TooManyShots,
    HistogramBinLimit { max_bins:usize },
    TooManyClassicalBits,
    InvalidClassicalBit { bit: usize, bits: usize },
    ReadBeforeWrite { bit: usize, instruction: usize },
    ArithmeticOverflow,
    WorkLimit,
    AllocationFailed,
    BudgetExceeded { required_bytes: usize, budget_bytes: usize },
    Simulator(SimError),
}
impl std::fmt::Display for FactorError {
    fn fmt(&self, f:&mut std::fmt::Formatter<'_>)->std::fmt::Result { write!(f,"{self:?}") }
}
impl std::error::Error for FactorError {}
impl From<SimError> for FactorError {
    fn from(value:SimError)->Self { Self::Simulator(value) }
}

#[derive(Debug,Clone)]
struct Factor {
    /// The i-th vector coordinate refers to global wire wires[i].
    /// Order is arbitrary, unique across all factors and preserved across
    /// cross-component SWAP by replacing labels without touching amplitudes.
    wires:Vec<usize>,
    state:Statevector,
}
#[derive(Debug,Clone)]
pub struct FactorState {
    qubits:usize,
    factors:Vec<Factor>,
    budget_bytes:usize,
    peak_logical_bytes:usize,
    projective_wire_splits:usize,
    projective_split_deferred:usize,
    product_preserving_two_wire_gates:usize,
}
impl FactorState {
    pub fn zero(qubits:usize,budget_bytes:usize)->Result<Self,FactorError> {
        if !(1..=MAX_FACTOR_GLOBAL_QUBITS).contains(&qubits) {
            return Err(FactorError::UnsupportedWidth{qubits,max:MAX_FACTOR_GLOBAL_QUBITS});
        }
        // 2 complex amplitudes per single-qubit factor. No 2^n allocation.
        let bytes=qubits.checked_mul(2 * size_of::<Complex>())
            .ok_or(FactorError::ArithmeticOverflow)?;
        if bytes>budget_bytes {
            return Err(FactorError::BudgetExceeded {
                required_bytes:bytes,budget_bytes,
            });
        }
        let mut factors=Vec::new();
        factors.try_reserve_exact(qubits).map_err(|_|FactorError::AllocationFailed)?;
        for qubit in 0..qubits {
            factors.push(Factor {
                wires:vec![qubit],
                state:Statevector::zero(1,budget_bytes)?,
            });
        }
        Ok(Self{
            qubits,factors,budget_bytes,peak_logical_bytes:bytes,
            projective_wire_splits:0,projective_split_deferred:0,
            product_preserving_two_wire_gates:0,
        })
    }
    pub fn qubits(&self)->usize {self.qubits}
    pub fn factor_count(&self)->usize {self.factors.len()}
    pub fn factor_widths(&self)->Vec<usize> {
        self.factors.iter().map(|f|f.wires.len()).collect()
    }
    pub fn factor_wires(&self)->Vec<Vec<usize>> {
        self.factors.iter().map(|f|f.wires.clone()).collect()
    }
    pub fn logical_state_bytes(&self)->usize {
        self.factors.iter().map(|f|f.state.state_bytes()).sum()
    }
    pub fn peak_logical_bytes(&self)->usize {self.peak_logical_bytes}
    /// Number of nontrivial component splits from actual projective Z outcomes.
    pub fn projective_wire_splits(&self)->usize {self.projective_wire_splits}
    /// Eligible splits skipped due to logical memory or fallible allocation.
    /// The state remains collapsed and physically equivalent when deferred.
    pub fn projective_split_deferred(&self)->usize {self.projective_split_deferred}
    /// Only provably non-entangling cross-factor CX/CZ operations. Includes
    /// true identity operations; no amplitude tolerance or SVD is involved.
    pub fn product_preserving_two_wire_gates(&self)->usize {
        self.product_preserving_two_wire_gates
    }
    fn check_wire(&self,q:usize)->Result<(),FactorError> {
        if q>=self.qubits {Err(FactorError::InvalidQubit{qubit:q,qubits:self.qubits})}
        else {Ok(())}
    }
    fn check_gate(&self,g:&Gate)->Result<(),FactorError> {
        match g {
            Gate::H(q)|Gate::X(q)|Gate::Y(q)|Gate::Z(q)|
            Gate::S(q)|Gate::T(q)=>self.check_wire(*q),
            Gate::Cnot{control,target}|Gate::Cz{control,target}=>{
                self.check_wire(*control)?;self.check_wire(*target)?;
                if control==target {return Err(FactorError::DuplicateQubit(*control));}
                Ok(())
            }
            Gate::Swap{a,b}=>{
                self.check_wire(*a)?;self.check_wire(*b)?;
                if a==b {return Err(FactorError::DuplicateQubit(*a));}
                Ok(())
            }
        }
    }
    fn locate(&self,wire:usize)->(usize,usize) {
        // This is only called after bounds checks. Every wire belongs to
        // exactly one factor, including after nonlocal SWAP.
        self.factors.iter().enumerate().find_map(|(factor,block)|
            block.wires.iter().position(|&q|q==wire).map(|local|(factor,local))
        ).expect("validated factor partition invariant")
    }
    /// A qubit can be used as an exact computational-basis control iff
    /// one of the two disjoint supports is EMPTY. Inspect raw complex
    /// coefficients, never compare rounded Born probabilities to 0 or 1:
    /// a probability such as 1-1e-13 does *not* prove separability.
    fn exact_z_eigenvalue(
        &self, owner:usize, local:usize,
    )->Result<Option<bool>,FactorError> {
        let f=&self.factors[owner];
        let mask=1usize<<local;
        let mut has_zero=false;
        let mut has_one=false;
        for i in 0..(1usize<<f.wires.len()) {
            let a=f.state.amplitude(i)?;
            if a!=Complex::ZERO {
                if i&mask==0 {has_zero=true;} else {has_one=true;}
                if has_zero&&has_one {return Ok(None);}
            }
        }
        // Normalized source vectors cannot have empty total support.
        if has_zero==has_one {return Ok(None);}
        Ok(Some(has_one))
    }

    /// True only if the ENTIRE local factor is a +1 or -1 eigenvector
    /// of Pauli X on this wire. This includes a wire correlated to
    /// other wires, but only when the *whole* factor obeys X|ψ>=±|ψ>.
    /// Compare both real and imaginary components EXACTLY. Ambiguous
    /// almost-eigenvectors fall back to full checked tensor joins.
    fn exact_x_eigenvalue(
        &self, owner:usize, local:usize,
    )->Result<Option<bool>,FactorError> {
        let f=&self.factors[owner];
        let mask=1usize<<local;
        let mut plus=true;
        let mut minus=true;
        for i in 0..(1usize<<f.wires.len()) {
            if i&mask!=0 {continue;}
            let a=f.state.amplitude(i)?;
            let b=f.state.amplitude(i|mask)?;
            if a!=b {plus=false;}
            if a.re!=-b.re || a.im!=-b.im {minus=false;}
            if !plus&&!minus {return Ok(None);}
        }
        if plus {Ok(Some(false))} // X eigenvalue +1.
        else if minus {Ok(Some(true))} // X eigenvalue -1.
        else {Ok(None)}
    }

    /// A cross-factor controlled gate is product-preserving in these
    /// independently sufficient, *exact* algebraic cases:
    ///   CX: Z-eigen control -> apply X on target iff control is |1>;
    ///       X-eigen target -> apply Z on control iff eigenvalue is -1.
    ///   CZ: either end in a Z basis eigenstate -> apply Z to the
    ///       opposite factor iff the eigenvalue is |1>.
    /// Global phase and control/target entanglement are preserved.
    /// Does not touch factor topology, consume RNG, or allocate amplitudes.
    fn apply_product_preserving_controlled(
        &mut self, gate:&Gate, ia:usize, la:usize, ib:usize, lb:usize,
    )->Result<bool,FactorError> {
        match gate {
            Gate::Cnot{..}=>{
                if let Some(high)=self.exact_z_eigenvalue(ia,la)? {
                    if high {self.factors[ib].state.apply(Gate::X(lb))?;}
                } else if let Some(minus)=self.exact_x_eigenvalue(ib,lb)? {
                    if minus {self.factors[ia].state.apply(Gate::Z(la))?;}
                } else {
                    return Ok(false);
                }
            }
            Gate::Cz{..}=>{
                if let Some(high)=self.exact_z_eigenvalue(ia,la)? {
                    if high {self.factors[ib].state.apply(Gate::Z(lb))?;}
                } else if let Some(high)=self.exact_z_eigenvalue(ib,lb)? {
                    if high {self.factors[ia].state.apply(Gate::Z(la))?;}
                } else {
                    return Ok(false);
                }
            }
            _=>return Ok(false),
        }
        self.product_preserving_two_wire_gates+=1;
        Ok(true)
    }
    pub fn apply(&mut self,gate:Gate)->Result<(),FactorError> {
        self.check_gate(&gate)?;
        match gate {
            Gate::H(q)|Gate::X(q)|Gate::Y(q)|Gate::Z(q)|
            Gate::S(q)|Gate::T(q)=>{
                let (block,local)=self.locate(q);
                let mapped=match gate {
                    Gate::H(_)=>Gate::H(local),
                    Gate::X(_)=>Gate::X(local),
                    Gate::Y(_)=>Gate::Y(local),
                    Gate::Z(_)=>Gate::Z(local),
                    Gate::S(_)=>Gate::S(local),
                    _=>Gate::T(local),
                };
                self.factors[block].state.apply(mapped)?;
                Ok(())
            }
            Gate::Swap{a,b}=>{
                let (ia,la)=self.locate(a);
                let (ib,lb)=self.locate(b);
                if ia==ib {
                    self.factors[ia].state.apply(Gate::Swap{a:la,b:lb})?;
                } else {
                    // Swap exchanges wire OWNERSHIP; no entanglement is created
                    // by relabeling the two tensor-factor coordinates.
                    self.factors[ia].wires[la]=b;
                    self.factors[ib].wires[lb]=a;
                }
                Ok(())
            }
            Gate::Cnot{control,target}|Gate::Cz{control,target}=>{
                let (ia,la)=self.locate(control);
                let (ib,lb)=self.locate(target);
                if ia==ib {
                    let mapped=if matches!(gate,Gate::Cnot{..}) {
                        Gate::Cnot{control:la,target:lb}
                    } else {Gate::Cz{control:la,target:lb}};
                    self.factors[ia].state.apply(mapped)?;
                    return Ok(());
                }
                // Before attempting an expensive, width-limited join,
                // execute only mathematically PROVEN non-entangling cross
                // interactions locally, preserving exact factor topology.
                if self.apply_product_preserving_controlled(&gate,ia,la,ib,lb)? {
                    return Ok(());
                }
                let left=&self.factors[ia];
                let right=&self.factors[ib];
                let width=left.wires.len().checked_add(right.wires.len())
                    .ok_or(FactorError::ArithmeticOverflow)?;
                if width>MAX_FACTOR_QUBITS {
                    return Err(FactorError::FactorWidthExceeded{
                        required:width,max:MAX_FACTOR_QUBITS,
                    });
                }
                let count=1usize.checked_shl(width as u32)
                    .ok_or(FactorError::ArithmeticOverflow)?;
                let new_bytes=count.checked_mul(size_of::<Complex>())
                    .ok_or(FactorError::ArithmeticOverflow)?;
                // Both old factors and new factor coexist until commit.
                // Constrain this temporary peak, not only final logical state.
                let temp=self.logical_state_bytes().checked_add(new_bytes)
                    .ok_or(FactorError::ArithmeticOverflow)?;
                if temp>self.budget_bytes {
                    return Err(FactorError::BudgetExceeded{
                        required_bytes:temp,budget_bytes:self.budget_bytes,
                    });
                }
                let left_dim=1usize<<left.wires.len();
                let right_dim=1usize<<right.wires.len();
                let mut amplitudes=Vec::new();
                amplitudes.try_reserve_exact(count)
                    .map_err(|_|FactorError::AllocationFailed)?;
                for r in 0..right_dim {
                    let ar=right.state.amplitude(r)?;
                    for l in 0..left_dim {
                        amplitudes.push(left.state.amplitude(l)?*ar);
                    }
                }
                let mut merged=Statevector::from_amplitudes(
                    width,amplitudes,self.budget_bytes)?;
                let mapped=if matches!(gate,Gate::Cnot{..}){
                    Gate::Cnot{control:la,target:left.wires.len()+lb}
                } else {
                    Gate::Cz{control:la,target:left.wires.len()+lb}
                };
                // Complete two-wire gate before touching either old factor.
                merged.apply(mapped)?;
                let mut wires=left.wires.clone();
                wires.extend_from_slice(&right.wires);
                let factor=Factor{wires,state:merged};
                let low=ia.min(ib);
                let high=ia.max(ib);
                self.factors.remove(high);
                self.factors[low]=factor;
                self.peak_logical_bytes=self.peak_logical_bytes.max(temp);
                Ok(())
            }
        }
    }
    pub fn wire_probability(&self,qubit:usize,outcome:bool)
        ->Result<f64,FactorError>
    {
        self.check_wire(qubit)?;
        let (owner,local)=self.locate(qubit);
        Ok(self.factors[owner].state.wire_probability(local,outcome)?)
    }
    /// Projective Z yields |b> ⊗ |rest_b>; this is guaranteed rank-one,
    /// independent of Schmidt tolerance. The conditional rest amplitudes
    /// retain their entire complex phase, and original wire order is preserved
    /// except for the extracted coordinate.
    ///
    /// The old factor and both replacement vectors coexist until a complete
    /// atomic commit. If temporary allocation is unavailable, retaining the
    /// collapsed but joined original component is always scientifically valid.
    fn try_release_measured_wire(
        &mut self, owner:usize, local:usize, outcome:bool,
    )->Result<bool,FactorError> {
        let width=self.factors[owner].wires.len();
        if width<=1 {return Ok(false);}
        let remaining_dim=1usize.checked_shl((width-1) as u32)
            .ok_or(FactorError::ArithmeticOverflow)?;
        let new_bytes=remaining_dim.checked_mul(size_of::<Complex>())
            .and_then(|v|v.checked_add(2*size_of::<Complex>()))
            .ok_or(FactorError::ArithmeticOverflow)?;
        let temporary=self.logical_state_bytes().checked_add(new_bytes)
            .ok_or(FactorError::ArithmeticOverflow)?;
        if temporary>self.budget_bytes {return Ok(false);}
        if self.factors.try_reserve(1).is_err() {return Ok(false);}

        let mut remaining_wires=Vec::new();
        if remaining_wires.try_reserve_exact(width-1).is_err() {return Ok(false);}
        let mut measured_wires=Vec::new();
        if measured_wires.try_reserve_exact(1).is_err() {return Ok(false);}
        let measured_wire=self.factors[owner].wires[local];
        for (i,&q) in self.factors[owner].wires.iter().enumerate() {
            if i!=local {remaining_wires.push(q);}
        }
        measured_wires.push(measured_wire);

        let mut amplitudes=Vec::new();
        if amplitudes.try_reserve_exact(remaining_dim).is_err() {return Ok(false);}
        let low_mask=(1usize<<local)-1;
        for i in 0..remaining_dim {
            let original_basis=(i&low_mask)|((i>>local)<<(local+1))
                |((outcome as usize)<<local);
            amplitudes.push(self.factors[owner].state.amplitude(original_basis)?);
        }
        let remaining=match Statevector::from_amplitudes(
            width-1,amplitudes,self.budget_bytes,
        ) {
            Ok(v)=>v,
            Err(SimError::AllocationFailed|SimError::BudgetExceeded)=>return Ok(false),
            Err(e)=>return Err(e.into()),
        };
        let mut measured=match Statevector::zero(1,self.budget_bytes) {
            Ok(v)=>v,
            Err(SimError::AllocationFailed|SimError::BudgetExceeded)=>return Ok(false),
            Err(e)=>return Err(e.into()),
        };
        if outcome {measured.apply(Gate::X(0))?;}

        self.factors[owner]=Factor{wires:remaining_wires,state:remaining};
        self.factors.push(Factor{wires:measured_wires,state:measured});
        self.peak_logical_bytes=self.peak_logical_bytes.max(temporary);
        self.projective_wire_splits+=1; // bounded by real projective operations
        Ok(true)
    }

    pub fn measure(&mut self,qubit:usize,rng:&mut SeededRng)
        ->Result<bool,FactorError>
    {
        self.check_wire(qubit)?;
        let (owner,local)=self.locate(qubit);
        let eligible=self.factors[owner].wires.len()>1;
        // Match the reference simulator: one RNG draw per physical Z
        // measurement, including deterministic outcomes. Factorization
        // never samples a second time or changes the measured bit.
        let outcome=self.factors[owner].state.measure(local,rng)?;
        if eligible && !self.try_release_measured_wire(owner,local,outcome)? {
            self.projective_split_deferred+=1;
        }
        Ok(outcome)
    }
    pub fn amplitude_bits(&self,bits:&[bool])->Result<Complex,FactorError> {
        if bits.len()!=self.qubits {
            return Err(FactorError::InvalidBasisWidth{
                expected:self.qubits,got:bits.len(),
            });
        }
        let mut result=Complex::ONE;
        for f in &self.factors {
            let mut index=0;
            for (local,&global) in f.wires.iter().enumerate() {
                if bits[global] {index|=1usize<<local;}
            }
            result=result*f.state.amplitude(index)?;
        }
        Ok(result)
    }
    pub fn basis_probability(&self,bits:&[bool])->Result<f64,FactorError> {
        Ok(self.amplitude_bits(bits)?.norm_squared())
    }
}
#[derive(Debug,Clone,PartialEq)]
pub struct FactorMetadata {
    pub schema:&'static str,
    pub version:&'static str,
    pub profile:&'static str,
    pub precision:&'static str,
    pub truncation:&'static str,
    pub seed:u64,
    pub shots:usize,
    pub norm_tolerance:f64,
    pub qpu_hardware_jobs:u8,
    pub evidence_class:&'static str,
}
impl FactorMetadata {
    fn new(seed:u64,shots:usize)->Self {
        Self {
            schema:FACTORIZED_RESULT_SCHEMA,
            version:ENGINE_VERSION,
            profile:FACTORIZED_PROFILE,
            precision:"f64-complex",
            truncation:"none;factor-width-hard-limit",
            seed,shots,norm_tolerance:NORM_TOLERANCE,
            qpu_hardware_jobs:0,evidence_class:"simulation",
        }
    }
}
#[derive(Debug,Clone)]
pub struct FactorBatch {
    pub metadata:FactorMetadata,
    pub records:Vec<ShotRecord>,
    /// Final state intentionally kept only for single-shot inspection.
    pub one_shot_state:Option<FactorState>,
    pub peak_logical_state_bytes:usize,
    /// Cumulative successful measured-wire releases across all shots.
    pub projective_wire_splits:usize,
    /// Cumulative eligible releases deferred because allocations were denied.
    pub projective_split_deferred:usize,
    /// Sum of real CX/CZ calls provably executed without factor joins.
    pub product_preserving_two_wire_gates:usize,
}
/// Aggregated scientific CPU sampling result; unlike FactorBatch it stores
/// no per-shot trace, register or final state. Sample ordering still consumes
/// one global deterministic SeededRng across all shots.
#[derive(Debug,Clone)]
pub struct FactorCounts {
    pub metadata:FactorMetadata,
    /// Histogram keys are reverse-register order with '?' for never-written
    /// bits, identical to the detailed factorized JSON v1 convention.
    pub histogram:BTreeMap<String,usize>,
    pub max_histogram_bins:usize,
    /// Logical accounting charge for the distinct histogram entries,
    /// separate from peak temporary quantum state bytes and allocator RSS.
    pub logical_histogram_reserved_bytes:usize,
    pub peak_logical_state_bytes:usize,
    pub projective_wire_splits:usize,
    pub projective_split_deferred:usize,
    pub product_preserving_two_wire_gates:usize,
    /// Always zero; this API does not retain records or traces.
    pub retained_shot_records:usize,
}
#[derive(Debug,Clone)]
pub struct FactorCircuit {
    qubits:usize,
    classical_bits:usize,
    instructions:Vec<Instruction>,
}
impl FactorCircuit {
    pub fn new(qubits:usize,classical_bits:usize)->Self {
        Self{qubits,classical_bits,instructions:Vec::new()}
    }
    pub fn push(&mut self,op:Instruction) {self.instructions.push(op);}
    pub fn instructions(&self)->&[Instruction] {&self.instructions}
    fn preflight(&self,shots:usize,budget:usize)->Result<(usize,usize),FactorError> {
        if !(1..=MAX_FACTOR_GLOBAL_QUBITS).contains(&self.qubits) {
            return Err(FactorError::UnsupportedWidth{
                qubits:self.qubits,max:MAX_FACTOR_GLOBAL_QUBITS,
            });
        }
        if shots==0 {return Err(FactorError::ZeroShots);}
        if shots>MAX_FACTOR_SHOTS {return Err(FactorError::TooManyShots);}
        if self.instructions.len()>MAX_FACTOR_INSTRUCTIONS {
            return Err(FactorError::TooManyInstructions);
        }
        if self.classical_bits>64 {return Err(FactorError::TooManyClassicalBits);}
        let work=shots.checked_mul(self.instructions.len().max(1))
            .and_then(|x|x.checked_mul(1usize<<MAX_FACTOR_QUBITS))
            .ok_or(FactorError::ArithmeticOverflow)?;
        if work>MAX_FACTOR_WORK_UNITS {return Err(FactorError::WorkLimit);}
        let state=FactorState::zero(self.qubits,budget)?;
        let mut written=vec![false;self.classical_bits];
        let mut traces=0usize;
        for (index,op) in self.instructions.iter().enumerate(){
            match op {
                Instruction::Gate(g)=>state.check_gate(g)?,
                Instruction::If{bit,gate,..}=>{
                    if *bit>=self.classical_bits {
                        return Err(FactorError::InvalidClassicalBit{
                            bit:*bit,bits:self.classical_bits,
                        });
                    }
                    state.check_gate(gate)?;
                    if !written[*bit] {
                        return Err(FactorError::ReadBeforeWrite{
                            bit:*bit,instruction:index,
                        });
                    }
                }
                Instruction::Measure{qubit,into_bit}=>{
                    state.check_wire(*qubit)?;
                    if *into_bit>=self.classical_bits {
                        return Err(FactorError::InvalidClassicalBit{
                            bit:*into_bit,bits:self.classical_bits,
                        });
                    }
                    written[*into_bit]=true;
                    traces+=1;
                }
                Instruction::Reset{qubit}=>{
                    state.check_wire(*qubit)?;
                    traces+=1;
                }
            }
        }
        let per_shot=self.classical_bits.checked_mul(size_of::<Option<bool>>())
            .and_then(|x|x.checked_add(size_of::<ShotRecord>()))
            .and_then(|x|x.checked_add(traces.checked_mul(size_of::<TraceEntry>())?))
            .ok_or(FactorError::ArithmeticOverflow)?;
        let output=shots.checked_mul(per_shot)
            .ok_or(FactorError::ArithmeticOverflow)?;
        let required=output.checked_add(state.logical_state_bytes())
            .ok_or(FactorError::ArithmeticOverflow)?;
        if required>budget {
            return Err(FactorError::BudgetExceeded{
                required_bytes:required,budget_bytes:budget,
            });
        }
        Ok((traces,budget-output))
    }
    /// Shared execution kernel for detailed shots and bounded histogram
    /// sampling. Suppressing trace creation NEVER changes RNG draws,
    /// measurement collapse, reset behavior or classical feed-forward.
    fn run_one(
        &self, rng:&mut SeededRng, state_budget:usize,
        with_trace:bool, trace_cap:usize,
    )->Result<(FactorState,Vec<Option<bool>>,Vec<TraceEntry>),FactorError> {
        let mut state=FactorState::zero(self.qubits,state_budget)?;
        let mut bits=vec![None;self.classical_bits];
        let mut trace=Vec::new();
        if with_trace {
            trace.try_reserve_exact(trace_cap)
                .map_err(|_|FactorError::AllocationFailed)?;
        }
        for (instruction,op) in self.instructions.iter().enumerate(){
            match op {
                Instruction::Gate(g)=>state.apply(g.clone())?,
                Instruction::Measure{qubit,into_bit}=>{
                    let value=state.measure(*qubit,rng)?;
                    bits[*into_bit]=Some(value);
                    if with_trace {
                        trace.push(TraceEntry{instruction,qubit:*qubit,
                            kind:TraceKind::Measure{into_bit:*into_bit,value}});
                    }
                }
                Instruction::Reset{qubit}=>{
                    let previous_value=state.measure(*qubit,rng)?;
                    if previous_value {state.apply(Gate::X(*qubit))?;}
                    if with_trace {
                        trace.push(TraceEntry{instruction,qubit:*qubit,
                            kind:TraceKind::Reset{previous_value}});
                    }
                }
                Instruction::If{bit,equals,gate}=>{
                    let value=bits[*bit].ok_or(FactorError::ReadBeforeWrite{
                        bit:*bit,instruction,
                    })?;
                    if value==*equals {state.apply(gate.clone())?;}
                }
            }
        }
        Ok((state,bits,trace))
    }
    pub fn execute_shots(&self,shots:usize,seed:u64,budget:usize)
        ->Result<FactorBatch,FactorError>
    {
        let (trace_cap,state_budget)=self.preflight(shots,budget)?;
        let mut rng=SeededRng::new(seed);
        let mut records=Vec::new();
        records.try_reserve_exact(shots)
            .map_err(|_|FactorError::AllocationFailed)?;
        let mut single=None;
        let mut peak=0usize;
        let mut total_splits=0usize;
        let mut total_deferred=0usize;
        let mut total_product_gates=0usize;
        for _ in 0..shots {
            let (state,bits,trace)=
                self.run_one(&mut rng,state_budget,true,trace_cap)?;
            peak=peak.max(state.peak_logical_bytes());
            total_splits=total_splits.checked_add(state.projective_wire_splits())
                .ok_or(FactorError::ArithmeticOverflow)?;
            total_deferred=total_deferred.checked_add(state.projective_split_deferred())
                .ok_or(FactorError::ArithmeticOverflow)?;
            total_product_gates=total_product_gates
                .checked_add(state.product_preserving_two_wire_gates())
                .ok_or(FactorError::ArithmeticOverflow)?;
            records.push(ShotRecord{bits,trace});
            if shots==1 {single=Some(state);}
        }
        Ok(FactorBatch{
            metadata:FactorMetadata::new(seed,shots),
            records,
            one_shot_state:single,
            peak_logical_state_bytes:peak,
            projective_wire_splits:total_splits,
            projective_split_deferred:total_deferred,
            product_preserving_two_wire_gates:total_product_gates,
        })
    }
    /// Up to 65,536 seeded observations, without storing shot records.
    ///
    /// Admission includes strict instruction/branch validation even if an
    /// If condition will never be taken. CPU work is conservatively charged
    /// at a 2^10 local state dimension for *each* operation and shot.
    /// A bounded histogram grows only after each completed shot, with its
    /// logical memory charge removed from the next shot's state budget.
    /// A failed run returns Err; no partial histogram is ever published.
    pub fn execute_counts(
        &self,shots:usize,seed:u64,budget:usize,
    )->Result<FactorCounts,FactorError> {
        if shots==0 {return Err(FactorError::ZeroShots);}
        if shots>MAX_FACTOR_COUNT_SHOTS {return Err(FactorError::TooManyShots);}
        let work=shots.checked_mul(self.instructions.len().max(1))
            .and_then(|v|v.checked_mul(1usize<<MAX_FACTOR_QUBITS))
            .ok_or(FactorError::ArithmeticOverflow)?;
        if work>MAX_FACTOR_COUNT_WORK_UNITS {return Err(FactorError::WorkLimit);}
        // Reuse all existing graph, resource, branch, and read-before-write
        // preflight (including unreachable conditional gates). One-shot
        // detailed output is a conservative fixed preflight allowance only.
        self.preflight(1,budget)?;
        let min_state=self.qubits.checked_mul(2*size_of::<Complex>())
            .ok_or(FactorError::ArithmeticOverflow)?;
        let bytes_per_bin=self.classical_bits
            .checked_add(FACTOR_COUNT_BIN_LOGICAL_OVERHEAD)
            .ok_or(FactorError::ArithmeticOverflow)?;

        let mut rng=SeededRng::new(seed);
        let mut histogram=BTreeMap::<String,usize>::new();
        let mut hist_charge=0usize;
        let mut peak=0usize;
        let mut splits=0usize;
        let mut deferred=0usize;
        let mut product_gates=0usize;
        for _ in 0..shots {
            // The histogram survives; no prior shot's quantum state or
            // trace does. Existing factor joins enforce this state budget.
            let state_budget=budget.checked_sub(hist_charge)
                .ok_or(FactorError::ArithmeticOverflow)?;
            let (state,bits,_)=self.run_one(&mut rng,state_budget,false,0)?;
            peak=peak.max(state.peak_logical_bytes());
            splits=splits.checked_add(state.projective_wire_splits())
                .ok_or(FactorError::ArithmeticOverflow)?;
            deferred=deferred.checked_add(state.projective_split_deferred())
                .ok_or(FactorError::ArithmeticOverflow)?;
            product_gates=product_gates
                .checked_add(state.product_preserving_two_wire_gates())
                .ok_or(FactorError::ArithmeticOverflow)?;
            // Release temporary old quantum factor before adding a new
            // histogram entry, so peak counts are not double-counted.
            drop(state);
            let key:String=bits.iter().rev().map(|bit|match bit {
                Some(true)=>'1',Some(false)=>'0',None=>'?'
            }).collect();
            if !histogram.contains_key(&key) {
                if histogram.len()>=MAX_FACTOR_COUNT_BINS {
                    return Err(FactorError::HistogramBinLimit{
                        max_bins:MAX_FACTOR_COUNT_BINS,
                    });
                }
                let charge=hist_charge.checked_add(bytes_per_bin)
                    .ok_or(FactorError::ArithmeticOverflow)?;
                let required=charge.checked_add(min_state)
                    .ok_or(FactorError::ArithmeticOverflow)?;
                if required>budget {
                    return Err(FactorError::BudgetExceeded{
                        required_bytes:required,budget_bytes:budget,
                    });
                }
                hist_charge=charge;
            }
            let entry=histogram.entry(key).or_insert(0);
            *entry=entry.checked_add(1).ok_or(FactorError::ArithmeticOverflow)?;
        }
        let mut metadata=FactorMetadata::new(seed,shots);
        metadata.schema=FACTORIZED_COUNTS_RESULT_SCHEMA;
        Ok(FactorCounts{
            metadata,histogram,max_histogram_bins:MAX_FACTOR_COUNT_BINS,
            logical_histogram_reserved_bytes:hist_charge,
            peak_logical_state_bytes:peak,
            projective_wire_splits:splits,
            projective_split_deferred:deferred,
            product_preserving_two_wire_gates:product_gates,
            retained_shot_records:0,
        })
    }
}
