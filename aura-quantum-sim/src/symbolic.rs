//! AURA-Q28-010: exact *small* Clifford+T symbolic phase and measurement oracle.
//!
//! Amplitudes are elements of Z[sqrt(2), i] / 2^k:
//!     [a + b*sqrt(2) + i(c + d*sqrt(2))] / 2^k
//! with checked i128 coefficients and a shared, bounded denominator exponent.
//! This representation implements H, T and all reference gates exactly,
//! without f64 normalization drift. Arbitrary rotations are not supported.
//!
//! Projective Z measurements retain an UNNORMALIZED, subselected branch.
//! Branch weights remain exact in Q(sqrt(2)), even when the normalized
//! conditional vector would require roots outside this exact coefficient ring.
//! A ratio of exact branch weights specifies conditional Born probability;
//! NO RNG sampling or measurement-normalized amplitude is claimed.
//!
//! Bounded experimental CPU reference, not a formally certified proof,
//! an Aura/QIR interface, a large-scale simulator or a hardware/QPU job.
use crate::{Gate, ENGINE_VERSION};
use std::mem::size_of;

pub const MAX_SYMBOLIC_QUBITS: usize = 4;
pub const MAX_SYMBOLIC_OPERATIONS: usize = 128;
pub const MAX_SYMBOLIC_CLASSICAL_BITS: usize = 16;
pub const MAX_SYMBOLIC_DENOMINATOR_POWER: u32 = 48;
pub const SYMBOLIC_PROFILE: &str = "local-exact-root2-gaussian-phase/1";
pub const SYMBOLIC_RESULT_SCHEMA: &str = "aura.quantum.symbolic-result/1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolicError {
    UnsupportedWidth { qubits: usize, max: usize },
    InvalidQubit { index: usize, qubits: usize },
    DuplicateQubit(usize),
    InvalidBasisIndex(usize),
    MismatchedWidth,
    DepthExceeded,
    TooManyOperations,
    TooManyClassicalBits,
    InvalidClassicalBit { bit: usize, bits: usize },
    ReadBeforeWrite { bit: usize, instruction: usize },
    CoefficientOverflow,
    ArithmeticOverflow,
    BudgetExceeded { required_bytes: usize, budget_bytes: usize },
    AllocationFailed,
    ImpossibleOutcome,
}

impl std::fmt::Display for SymbolicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SymbolicError {}

fn overflow<T>(value: Option<T>) -> Result<T, SymbolicError> {
    value.ok_or(SymbolicError::CoefficientOverflow)
}

/// An *integer* pair a + b√2, not a floating-point approximation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Root2 {
    pub rational: i128,
    pub radical: i128,
}
impl Root2 {
    pub const ZERO: Self = Self { rational: 0, radical: 0 };
    pub const ONE: Self = Self { rational: 1, radical: 0 };

    pub const fn new(rational: i128, radical: i128) -> Self {
        Self { rational, radical }
    }
    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }
    fn add(self, rhs: Self) -> Result<Self, SymbolicError> {
        Ok(Self::new(
            overflow(self.rational.checked_add(rhs.rational))?,
            overflow(self.radical.checked_add(rhs.radical))?,
        ))
    }
    fn sub(self, rhs: Self) -> Result<Self, SymbolicError> {
        Ok(Self::new(
            overflow(self.rational.checked_sub(rhs.rational))?,
            overflow(self.radical.checked_sub(rhs.radical))?,
        ))
    }
    fn neg(self) -> Result<Self, SymbolicError> {
        Ok(Self::new(
            overflow(self.rational.checked_neg())?,
            overflow(self.radical.checked_neg())?,
        ))
    }
    fn double(self) -> Result<Self, SymbolicError> {
        Ok(Self::new(
            overflow(self.rational.checked_mul(2))?,
            overflow(self.radical.checked_mul(2))?,
        ))
    }
    fn times_sqrt2(self) -> Result<Self, SymbolicError> {
        Ok(Self::new(
            overflow(self.radical.checked_mul(2))?,
            self.rational,
        ))
    }
    fn mul(self, rhs: Self) -> Result<Self, SymbolicError> {
        let integer = overflow(self.rational.checked_mul(rhs.rational))?;
        let radical_product = overflow(self.radical.checked_mul(rhs.radical))?;
        let integer = overflow(integer.checked_add(
            overflow(radical_product.checked_mul(2))?,
        ))?;
        let radical = overflow(self.rational.checked_mul(rhs.radical))?;
        let radical = overflow(radical.checked_add(
            overflow(self.radical.checked_mul(rhs.rational))?,
        ))?;
        Ok(Self::new(integer, radical))
    }
    fn square(self) -> Result<Self, SymbolicError> {
        self.mul(self)
    }
    fn is_even(self) -> bool {
        self.rational % 2 == 0 && self.radical % 2 == 0
    }
    fn halve(self) -> Self {
        Self::new(self.rational / 2, self.radical / 2)
    }
    pub fn approximate(self) -> f64 {
        self.rational as f64 + (self.radical as f64) * std::f64::consts::SQRT_2
    }
}

/// An integer Gaussian extension over Z[√2].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Root2Complex {
    pub real: Root2,
    pub imag: Root2,
}
impl Root2Complex {
    pub const ZERO: Self = Self { real: Root2::ZERO, imag: Root2::ZERO };
    pub const ONE: Self = Self { real: Root2::ONE, imag: Root2::ZERO };

    pub const fn new(real: Root2, imag: Root2) -> Self {
        Self { real, imag }
    }
    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }
    fn add(self, rhs: Self) -> Result<Self, SymbolicError> {
        Ok(Self::new(self.real.add(rhs.real)?, self.imag.add(rhs.imag)?))
    }
    fn sub(self, rhs: Self) -> Result<Self, SymbolicError> {
        Ok(Self::new(self.real.sub(rhs.real)?, self.imag.sub(rhs.imag)?))
    }
    fn neg(self) -> Result<Self, SymbolicError> {
        Ok(Self::new(self.real.neg()?, self.imag.neg()?))
    }
    fn times_i(self) -> Result<Self, SymbolicError> {
        Ok(Self::new(self.imag.neg()?, self.real))
    }
    fn times_neg_i(self) -> Result<Self, SymbolicError> {
        Ok(Self::new(self.imag, self.real.neg()?))
    }
    fn double(self) -> Result<Self, SymbolicError> {
        Ok(Self::new(self.real.double()?, self.imag.double()?))
    }
    fn times_sqrt2(self) -> Result<Self, SymbolicError> {
        Ok(Self::new(self.real.times_sqrt2()?, self.imag.times_sqrt2()?))
    }
    fn times_t_numerator(self) -> Result<Self, SymbolicError> {
        // T|1> = exp(i*pi/4)|1> = sqrt(2)(1+i)/2 |1>.
        self.add(self.times_i()?)?.times_sqrt2()
    }
    fn mul(self, rhs: Self) -> Result<Self, SymbolicError> {
        let re = self.real.mul(rhs.real)?.sub(self.imag.mul(rhs.imag)?)?;
        let im = self.real.mul(rhs.imag)?.add(self.imag.mul(rhs.real)?)?;
        Ok(Self::new(re, im))
    }
    fn conjugate(self) -> Result<Self, SymbolicError> {
        Ok(Self::new(self.real, self.imag.neg()?))
    }
    fn norm_numerator(self) -> Result<Root2, SymbolicError> {
        self.real.square()?.add(self.imag.square()?)
    }
    fn is_even(self) -> bool {
        self.real.is_even() && self.imag.is_even()
    }
    fn halve(self) -> Self {
        Self::new(self.real.halve(), self.imag.halve())
    }
    pub fn approximate(self) -> (f64, f64) {
        (self.real.approximate(), self.imag.approximate())
    }
}

/// A complete exact expression of an amplitude with common denominator 2^k.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactAmplitude {
    pub numerator: Root2Complex,
    pub denominator_power_of_two: u32,
}
impl ExactAmplitude {
    pub fn approximate(self) -> (f64, f64) {
        let scale = 2f64.powi(-(self.denominator_power_of_two as i32));
        let (re, im) = self.numerator.approximate();
        (re * scale, im * scale)
    }
}

/// Exact a + b√2 over 2^k. Coefficients are signed i128; JSON adapters
/// must serialize them as decimal *strings*, not unsafe JS numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactProbability {
    pub numerator: Root2,
    pub denominator_power_of_two: u32,
}
impl ExactProbability {
    fn canonical(mut numerator: Root2, mut exponent: u32) -> Self {
        while exponent > 0 && numerator.is_even() {
            numerator = numerator.halve();
            exponent -= 1;
        }
        Self { numerator, denominator_power_of_two: exponent }
    }
    pub fn is_zero(self) -> bool {
        self.numerator.is_zero()
    }
    pub fn approximate(self) -> f64 {
        self.numerator.approximate()
            * 2f64.powi(-(self.denominator_power_of_two as i32))
    }
}

/// A physically normalized input vector evolves unitarily until selective
/// measurement. Afterwards this state is intentionally *subnormalized*.
#[derive(Debug, Clone)]
pub struct SymbolicState {
    qubits: usize,
    denominator_power_of_two: u32,
    numerators: Vec<Root2Complex>,
    logical_bytes: usize,
}

impl SymbolicState {
    pub fn zero(qubits: usize, budget_bytes: usize) -> Result<Self, SymbolicError> {
        if !(1..=MAX_SYMBOLIC_QUBITS).contains(&qubits) {
            return Err(SymbolicError::UnsupportedWidth {
                qubits, max: MAX_SYMBOLIC_QUBITS,
            });
        }
        let count = 1usize << qubits;
        let bytes = count.checked_mul(size_of::<Root2Complex>())
            .ok_or(SymbolicError::ArithmeticOverflow)?;
        if bytes > budget_bytes {
            return Err(SymbolicError::BudgetExceeded {
                required_bytes: bytes, budget_bytes,
            });
        }
        let mut numerators = Vec::new();
        numerators.try_reserve_exact(count)
            .map_err(|_| SymbolicError::AllocationFailed)?;
        numerators.resize(count, Root2Complex::ZERO);
        numerators[0] = Root2Complex::ONE;
        Ok(Self {
            qubits, denominator_power_of_two: 0,
            numerators, logical_bytes: bytes,
        })
    }
    pub fn qubits(&self) -> usize { self.qubits }
    pub fn logical_bytes(&self) -> usize { self.logical_bytes }
    pub fn dimension(&self) -> usize { self.numerators.len() }
    pub fn denominator_power_of_two(&self) -> u32 { self.denominator_power_of_two }

    pub fn amplitude(&self, index: usize) -> Result<ExactAmplitude, SymbolicError> {
        let numerator = *self.numerators.get(index)
            .ok_or(SymbolicError::InvalidBasisIndex(index))?;
        Ok(ExactAmplitude {
            numerator,
            denominator_power_of_two: self.denominator_power_of_two,
        })
    }

    fn mask(&self, q: usize) -> Result<usize, SymbolicError> {
        if q >= self.qubits {
            Err(SymbolicError::InvalidQubit { index: q, qubits: self.qubits })
        } else { Ok(1usize << q) }
    }
    fn pair(&self, a: usize, b: usize) -> Result<(usize, usize), SymbolicError> {
        let x = self.mask(a)?;
        let y = self.mask(b)?;
        if a == b { return Err(SymbolicError::DuplicateQubit(a)); }
        Ok((x, y))
    }
    pub fn check_gate(&self, gate: &Gate) -> Result<(), SymbolicError> {
        match gate {
            Gate::H(q) | Gate::X(q) | Gate::Y(q) | Gate::Z(q)
            | Gate::S(q) | Gate::T(q) => { self.mask(*q)?; }
            Gate::Cnot { control, target } | Gate::Cz { control, target } => {
                self.pair(*control, *target)?;
            }
            Gate::Swap { a, b } => { self.pair(*a, *b)?; }
        }
        Ok(())
    }
    fn reduce(mut terms: Vec<Root2Complex>, mut exponent: u32)
        -> (Vec<Root2Complex>, u32)
    {
        while exponent > 0 && terms.iter().all(|term| term.is_even()) {
            for term in &mut terms { *term = term.halve(); }
            exponent -= 1;
        }
        (terms, exponent)
    }
    /// Atomic checked update; arithmetic overflow never leaves a half-applied
    /// gate or changes the previously exposed state.
    pub fn apply(&mut self, gate: Gate) -> Result<(), SymbolicError> {
        self.check_gate(&gate)?;
        let increment = matches!(gate, Gate::H(_) | Gate::T(_));
        if increment && self.denominator_power_of_two >= MAX_SYMBOLIC_DENOMINATOR_POWER {
            return Err(SymbolicError::DepthExceeded);
        }
        let mut next = Vec::new();
        next.try_reserve_exact(self.numerators.len())
            .map_err(|_| SymbolicError::AllocationFailed)?;
        next.extend_from_slice(&self.numerators);

        match gate {
            Gate::H(q) => {
                let m = 1usize << q;
                for row in 0..self.dimension() {
                    if row & m == 0 {
                        let other = row | m;
                        let a = self.numerators[row];
                        let b = self.numerators[other];
                        next[row] = a.add(b)?.times_sqrt2()?;
                        next[other] = a.sub(b)?.times_sqrt2()?;
                    }
                }
            }
            Gate::T(q) => {
                let m = 1usize << q;
                for (index, value) in next.iter_mut().enumerate() {
                    *value = if index & m == 0 {
                        value.double()?
                    } else {
                        value.times_t_numerator()?
                    };
                }
            }
            Gate::S(q) => {
                let m = 1usize << q;
                for (i, value) in next.iter_mut().enumerate() {
                    if i & m != 0 { *value = value.times_i()?; }
                }
            }
            Gate::Z(q) => {
                let m = 1usize << q;
                for (i, value) in next.iter_mut().enumerate() {
                    if i & m != 0 { *value = value.neg()?; }
                }
            }
            Gate::X(q) | Gate::Y(q) => {
                let m = 1usize << q;
                for i in 0..self.dimension() {
                    if i & m == 0 {
                        let other = i | m;
                        let a = self.numerators[i];
                        let b = self.numerators[other];
                        if matches!(gate, Gate::Y(_)) {
                            next[i] = b.times_neg_i()?;
                            next[other] = a.times_i()?;
                        } else {
                            next[i] = b;
                            next[other] = a;
                        }
                    }
                }
            }
            Gate::Cnot { control, target } => {
                let mc = 1usize << control;
                let mt = 1usize << target;
                for i in 0..self.dimension() {
                    if i & mc != 0 && i & mt == 0 {
                        next.swap(i, i | mt);
                    }
                }
            }
            Gate::Cz { control, target } => {
                let mask = (1usize << control) | (1usize << target);
                for (i, value) in next.iter_mut().enumerate() {
                    if i & mask == mask { *value = value.neg()?; }
                }
            }
            Gate::Swap { a, b } => {
                let ma = 1usize << a;
                let mb = 1usize << b;
                for i in 0..self.dimension() {
                    if i & ma != 0 && i & mb == 0 {
                        next.swap(i, i ^ ma ^ mb);
                    }
                }
            }
        }
        let exponent = self.denominator_power_of_two + u32::from(increment);
        let (next, exponent) = Self::reduce(next, exponent);
        self.numerators = next;
        self.denominator_power_of_two = exponent;
        Ok(())
    }

    fn summed_weight(&self, selected: impl Fn(usize) -> bool)
        -> Result<ExactProbability, SymbolicError>
    {
        let mut total = Root2::ZERO;
        for (index, z) in self.numerators.iter().copied().enumerate() {
            if selected(index) {
                total = total.add(z.norm_numerator()?)?;
            }
        }
        let exponent = self.denominator_power_of_two
            .checked_mul(2).ok_or(SymbolicError::ArithmeticOverflow)?;
        Ok(ExactProbability::canonical(total, exponent))
    }
    pub fn total_mass(&self) -> Result<ExactProbability, SymbolicError> {
        self.summed_weight(|_| true)
    }
    pub fn basis_weight(&self, basis: usize)
        -> Result<ExactProbability, SymbolicError>
    {
        if basis >= self.dimension() {
            return Err(SymbolicError::InvalidBasisIndex(basis));
        }
        self.summed_weight(|index| index == basis)
    }
    pub fn wire_weight(&self, qubit: usize, outcome: bool)
        -> Result<ExactProbability, SymbolicError>
    {
        let mask = self.mask(qubit)?;
        self.summed_weight(|i| (i & mask != 0) == outcome)
    }
    /// Return exact *joint* Born weight after selecting the specified Z
    /// outcome. For a previously selected branch this is NOT the normalized
    /// conditional probability: use after_mass / before_mass as an exact ratio.
    pub fn postselect(&mut self, qubit: usize, outcome: bool)
        -> Result<ExactProbability, SymbolicError>
    {
        let mask = self.mask(qubit)?;
        let weight = self.wire_weight(qubit, outcome)?;
        if weight.is_zero() {
            return Err(SymbolicError::ImpossibleOutcome);
        }
        for (i, amp) in self.numerators.iter_mut().enumerate() {
            if (i & mask != 0) != outcome { *amp = Root2Complex::ZERO; }
        }
        Ok(weight)
    }

    /// Exact phase-sensitive coherence amp_i * conj(amp_j), not a rounded
    /// probability. Denominator is 2^(2*k) and may exceed i64 precision.
    pub fn coherence(&self, row: usize, col: usize)
        -> Result<ExactAmplitude, SymbolicError>
    {
        let a = self.amplitude(row)?;
        let b = self.amplitude(col)?;
        Ok(ExactAmplitude {
            numerator: a.numerator.mul(b.numerator.conjugate()?)?,
            denominator_power_of_two: self.denominator_power_of_two
                .checked_mul(2).ok_or(SymbolicError::ArithmeticOverflow)?,
        })
    }

    /// Compare physical rays exactly using cross products, allowing only
    /// global nonzero scalar/phase differences. Does NOT certify the entire
    /// evolution law or arbitrary circuits beyond the bounded ring.
    pub fn same_ray(&self, rhs: &Self) -> Result<bool, SymbolicError> {
        if self.qubits != rhs.qubits { return Err(SymbolicError::MismatchedWidth); }
        let reference = (0..self.dimension()).find(|&i| {
            !self.numerators[i].is_zero() || !rhs.numerators[i].is_zero()
        });
        let Some(pivot) = reference else { return Ok(false); };
        let a = self.numerators[pivot];
        let b = rhs.numerators[pivot];
        if a.is_zero() || b.is_zero() { return Ok(false); }
        for i in 0..self.dimension() {
            if self.numerators[i].mul(b)? != rhs.numerators[i].mul(a)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolicInstruction {
    Gate(Gate),
    /// Explicit postselected branch. No nondeterministic sampling.
    Measure { qubit: usize, into_bit: usize, outcome: bool },
    If { bit: usize, equals: bool, gate: Gate },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactMeasureEvent {
    pub instruction: usize,
    pub qubit: usize,
    pub into_bit: usize,
    pub outcome: bool,
    /// Absolute joint branch weight immediately BEFORE the projection.
    pub before_mass: ExactProbability,
    /// Absolute joint branch weight immediately AFTER projection.
    pub after_mass: ExactProbability,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicMetadata {
    pub engine_version: &'static str,
    pub profile: &'static str,
    pub schema: &'static str,
    pub exact_ring: &'static str,
    pub measurement_model: &'static str,
    pub evidence_class: &'static str,
    pub qpu_hardware_jobs: u8,
}
impl Default for SymbolicMetadata {
    fn default() -> Self {
        Self {
            engine_version: ENGINE_VERSION,
            profile: SYMBOLIC_PROFILE,
            schema: SYMBOLIC_RESULT_SCHEMA,
            exact_ring: "Z[sqrt(2),i]/2^k,checked-i128",
            measurement_model: "explicit-outcome-subnormalized-branches",
            evidence_class: "simulation",
            qpu_hardware_jobs: 0,
        }
    }
}
#[derive(Debug, Clone)]
pub struct SymbolicResult {
    pub metadata: SymbolicMetadata,
    pub classical_bits: Vec<Option<bool>>,
    pub trace: Vec<ExactMeasureEvent>,
    pub final_state: SymbolicState,
}
#[derive(Debug, Clone)]
pub struct SymbolicCircuit {
    qubits: usize,
    classical_bits: usize,
    instructions: Vec<SymbolicInstruction>,
}
impl SymbolicCircuit {
    pub fn new(qubits: usize, classical_bits: usize) -> Self {
        Self { qubits, classical_bits, instructions: Vec::new() }
    }
    pub fn qubits(&self) -> usize { self.qubits }
    pub fn classical_bits(&self) -> usize { self.classical_bits }
    pub fn instructions(&self) -> &[SymbolicInstruction] { &self.instructions }
    pub fn push(&mut self, operation: SymbolicInstruction) {
        self.instructions.push(operation);
    }
    fn preflight(&self, budget: usize) -> Result<usize, SymbolicError> {
        if self.instructions.len() > MAX_SYMBOLIC_OPERATIONS {
            return Err(SymbolicError::TooManyOperations);
        }
        if self.classical_bits > MAX_SYMBOLIC_CLASSICAL_BITS {
            return Err(SymbolicError::TooManyClassicalBits);
        }
        let state = SymbolicState::zero(self.qubits, budget)?;
        let mut written = vec![false; self.classical_bits];
        let mut measure_count = 0usize;
        let mut costly_gates = 0usize;
        for (index, op) in self.instructions.iter().enumerate() {
            let gate = match op {
                SymbolicInstruction::Gate(g) => Some(g),
                SymbolicInstruction::If { bit, gate, .. } => {
                    if *bit >= self.classical_bits {
                        return Err(SymbolicError::InvalidClassicalBit {
                            bit: *bit, bits: self.classical_bits,
                        });
                    }
                    if !written[*bit] {
                        return Err(SymbolicError::ReadBeforeWrite { bit: *bit, instruction: index });
                    }
                    Some(gate)
                }
                SymbolicInstruction::Measure { qubit, into_bit, .. } => {
                    state.mask(*qubit)?;
                    if *into_bit >= self.classical_bits {
                        return Err(SymbolicError::InvalidClassicalBit {
                            bit: *into_bit, bits: self.classical_bits,
                        });
                    }
                    written[*into_bit] = true;
                    measure_count += 1;
                    None
                }
            };
            if let Some(gate) = gate {
                state.check_gate(gate)?;
                if matches!(gate, Gate::H(_) | Gate::T(_)) {
                    costly_gates += 1;
                    if costly_gates > MAX_SYMBOLIC_DENOMINATOR_POWER as usize {
                        return Err(SymbolicError::DepthExceeded);
                    }
                }
            }
        }
        let bytes = self.classical_bits
            .checked_mul(size_of::<Option<bool>>())
            .and_then(|v| v.checked_add(
                measure_count.checked_mul(size_of::<ExactMeasureEvent>())?,
            ))
            .and_then(|v| v.checked_add(state.logical_bytes()))
            .ok_or(SymbolicError::ArithmeticOverflow)?;
        if bytes > budget {
            return Err(SymbolicError::BudgetExceeded {
                required_bytes: bytes, budget_bytes: budget,
            });
        }
        Ok(measure_count)
    }
    pub fn execute(&self, budget: usize) -> Result<SymbolicResult, SymbolicError> {
        let count = self.preflight(budget)?;
        let mut state = SymbolicState::zero(self.qubits, budget)?;
        let mut classical_bits = vec![None; self.classical_bits];
        let mut trace = Vec::new();
        trace.try_reserve_exact(count).map_err(|_| SymbolicError::AllocationFailed)?;
        for (index, operation) in self.instructions.iter().enumerate() {
            match operation {
                SymbolicInstruction::Gate(gate) => state.apply(gate.clone())?,
                SymbolicInstruction::Measure { qubit, into_bit, outcome } => {
                    let before_mass = state.total_mass()?;
                    let after_mass = state.postselect(*qubit, *outcome)?;
                    classical_bits[*into_bit] = Some(*outcome);
                    trace.push(ExactMeasureEvent {
                        instruction: index, qubit: *qubit,
                        into_bit: *into_bit, outcome: *outcome,
                        before_mass, after_mass,
                    });
                }
                SymbolicInstruction::If { bit, equals, gate } => {
                    let value = classical_bits[*bit].ok_or(
                        SymbolicError::ReadBeforeWrite { bit: *bit, instruction: index },
                    )?;
                    if value == *equals { state.apply(gate.clone())?; }
                }
            }
        }
        Ok(SymbolicResult {
            metadata: SymbolicMetadata::default(),
            classical_bits, trace, final_state: state,
        })
    }
}
