//! Standalone, bounded CPU-only statevector reference engine (Q28 C-001).
//!
//! This is numerical simulation, NOT a quantum equivalence proof or hardware run.
//! q0 is the least-significant bit of each basis index; |q[n-1] ... q0>.
//! Matrix operations act on one pure state and have no classical control/IR ABI.
#![forbid(unsafe_code)]

pub mod circuit;
pub mod json_protocol;
pub mod density;
pub mod noisy_circuit;
pub mod noisy_json;
pub mod stabilizer;
pub mod stabilizer_json;
pub mod symbolic;
pub mod symbolic_json;
pub mod dispatch_json;
pub mod factorized;
pub mod factorized_json;
pub mod factorized_counts_json;

use std::mem::size_of;

pub const ENGINE_VERSION: &str = "aura-quantum-sim/0.1.0";
pub const DEFAULT_MEMORY_BUDGET: usize = 16 * 1024 * 1024;
/// Runtime finite-precision invariant check, not a certified numerical error bound.
pub const NORM_TOLERANCE: f64 = 1e-10;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };
    pub const I: Self = Self { re: 0.0, im: 1.0 };
    pub const NEG_I: Self = Self { re: 0.0, im: -1.0 };

    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn norm_squared(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    fn finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

impl std::ops::Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl std::ops::Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError {
    DimensionOverflow,
    DimensionMismatch,
    BudgetExceeded,
    AllocationFailed,
    InvalidQubit { index: usize, qubits: usize },
    InvalidBasisIndex(usize),
    DuplicateQubit(usize),
    NonFiniteAmplitude,
    NotNormalized,
    ImpossibleOutcome,
    UnsupportedGate(String),
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SimError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    H(usize),
    X(usize),
    Y(usize),
    Z(usize),
    S(usize),
    T(usize),
    Cnot { control: usize, target: usize },
    Cz { control: usize, target: usize },
    Swap { a: usize, b: usize },
}

impl Gate {
    /// Convenience gate-name parsing for the basic reference surface only.
    /// Full parameterized/dynamic IR parsing is deliberately unsupported.
    pub fn from_name(name: &str) -> Result<Self, SimError> {
        match name {
            "H" => Ok(Self::H(0)),
            "X" => Ok(Self::X(0)),
            "Y" => Ok(Self::Y(0)),
            "Z" => Ok(Self::Z(0)),
            "S" => Ok(Self::S(0)),
            "T" => Ok(Self::T(0)),
            "CNOT" => Ok(Self::Cnot { control: 0, target: 1 }),
            "CZ" => Ok(Self::Cz { control: 0, target: 1 }),
            "SWAP" => Ok(Self::Swap { a: 0, b: 1 }),
            _ => Err(SimError::UnsupportedGate(name.to_owned())),
        }
    }
}

/// Versioned SplitMix64 stream. Specified by wrapping 64-bit arithmetic;
/// next_f64 uses high 53 bits and maps to the half-open interval [0, 1).
#[derive(Debug, Clone)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    pub fn next_f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
    }
}

/// Pure n-qubit state, with a logical byte budget covering state and shot output.
#[derive(Debug, Clone)]
pub struct Statevector {
    qubits: usize,
    amps: Vec<Complex>,
    max_bytes: usize,
    state_bytes: usize,
}

fn dimensions(qubits: usize, max_bytes: usize) -> Result<(usize, usize), SimError> {
    if qubits >= usize::BITS as usize {
        return Err(SimError::DimensionOverflow);
    }
    let len = 1usize.checked_shl(qubits as u32).ok_or(SimError::DimensionOverflow)?;
    let bytes = len.checked_mul(size_of::<Complex>()).ok_or(SimError::DimensionOverflow)?;
    if bytes > max_bytes {
        return Err(SimError::BudgetExceeded);
    }
    Ok((len, bytes))
}

impl Statevector {
    pub fn zero(qubits: usize, max_bytes: usize) -> Result<Self, SimError> {
        let (len, bytes) = dimensions(qubits, max_bytes)?;
        let mut amps = Vec::new();
        amps.try_reserve_exact(len).map_err(|_| SimError::AllocationFailed)?;
        amps.resize(len, Complex::ZERO);
        amps[0] = Complex::ONE;
        Ok(Self { qubits, amps, max_bytes, state_bytes: bytes })
    }

    /// Accept a caller-owned state iff dimensions, finite values and norm agree.
    pub fn from_amplitudes(
        qubits: usize,
        amps: Vec<Complex>,
        max_bytes: usize,
    ) -> Result<Self, SimError> {
        let (len, bytes) = dimensions(qubits, max_bytes)?;
        if amps.len() != len {
            return Err(SimError::DimensionMismatch);
        }
        let state = Self { qubits, amps, max_bytes, state_bytes: bytes };
        state.validate()?;
        Ok(state)
    }

    pub fn qubits(&self) -> usize {
        self.qubits
    }

    pub fn state_bytes(&self) -> usize {
        self.state_bytes
    }

    pub fn amplitude(&self, basis_index: usize) -> Result<Complex, SimError> {
        self.amps.get(basis_index).copied().ok_or(SimError::InvalidBasisIndex(basis_index))
    }

    pub fn basis_probability(&self, basis_index: usize) -> Result<f64, SimError> {
        Ok(self.amplitude(basis_index)?.norm_squared())
    }

    pub fn norm_squared(&self) -> Result<f64, SimError> {
        if !self.amps.iter().all(|a| a.finite()) {
            return Err(SimError::NonFiniteAmplitude);
        }
        let total: f64 = self.amps.iter().map(|a| a.norm_squared()).sum();
        if !total.is_finite() {
            return Err(SimError::NonFiniteAmplitude);
        }
        Ok(total)
    }

    pub fn validate(&self) -> Result<(), SimError> {
        let norm = self.norm_squared()?;
        if (norm - 1.0).abs() > NORM_TOLERANCE {
            return Err(SimError::NotNormalized);
        }
        Ok(())
    }

    fn mask(&self, qubit: usize) -> Result<usize, SimError> {
        if qubit >= self.qubits {
            return Err(SimError::InvalidQubit { index: qubit, qubits: self.qubits });
        }
        Ok(1usize << qubit)
    }

    fn masks(&self, a: usize, b: usize) -> Result<(usize, usize), SimError> {
        let ma = self.mask(a)?;
        let mb = self.mask(b)?;
        if a == b {
            return Err(SimError::DuplicateQubit(a));
        }
        Ok((ma, mb))
    }

    fn single(&mut self, qubit: usize, m: [[Complex; 2]; 2]) -> Result<(), SimError> {
        let mask = self.mask(qubit)?;
        for i in 0..self.amps.len() {
            if i & mask == 0 {
                let j = i | mask;
                let a = self.amps[i];
                let b = self.amps[j];
                self.amps[i] = m[0][0] * a + m[0][1] * b;
                self.amps[j] = m[1][0] * a + m[1][1] * b;
            }
        }
        Ok(())
    }

    pub fn apply(&mut self, gate: Gate) -> Result<(), SimError> {
        self.validate()?;
        let o = Complex::ZERO;
        let one = Complex::ONE;
        let minus = Complex::new(-1.0, 0.0);
        let i = Complex::I;
        let neg_i = Complex::NEG_I;
        let a = std::f64::consts::FRAC_1_SQRT_2;
        let plus = Complex::new(a, 0.0);
        let neg = Complex::new(-a, 0.0);
        match gate {
            Gate::H(q) => self.single(q, [[plus, plus], [plus, neg]])?,
            Gate::X(q) => self.single(q, [[o, one], [one, o]])?,
            Gate::Y(q) => self.single(q, [[o, neg_i], [i, o]])?,
            Gate::Z(q) => self.single(q, [[one, o], [o, minus]])?,
            Gate::S(q) => self.single(q, [[one, o], [o, i]])?,
            Gate::T(q) => self.single(q, [[one, o], [o, Complex::new(a, a)]])?,
            Gate::Cnot { control, target } => {
                let (mc, mt) = self.masks(control, target)?;
                for basis in 0..self.amps.len() {
                    if basis & mc != 0 && basis & mt == 0 {
                        self.amps.swap(basis, basis | mt);
                    }
                }
            }
            Gate::Cz { control, target } => {
                let (mc, mt) = self.masks(control, target)?;
                for (basis, amp) in self.amps.iter_mut().enumerate() {
                    if basis & mc != 0 && basis & mt != 0 {
                        amp.re = -amp.re;
                        amp.im = -amp.im;
                    }
                }
            }
            Gate::Swap { a, b } => {
                let (ma, mb) = self.masks(a, b)?;
                for basis in 0..self.amps.len() {
                    if basis & ma != 0 && basis & mb == 0 {
                        self.amps.swap(basis, (basis ^ ma) | mb);
                    }
                }
            }
        }
        self.validate()
    }

    /// Analytical probability of measuring one wire as false/true.
    pub fn wire_probability(&self, qubit: usize, outcome: bool) -> Result<f64, SimError> {
        self.validate()?;
        let mask = self.mask(qubit)?;
        let mut probability = 0.0;
        for (basis, amp) in self.amps.iter().enumerate() {
            if (basis & mask != 0) == outcome {
                probability += amp.norm_squared();
            }
        }
        Ok(probability)
    }

    /// Conditional state update. Impossible outcomes fail without modifying the state.
    ///
    /// Normalize from selected *amplitudes*, not f64 Born probabilities:
    /// squaring a rare but finite amplitude can underflow to zero or lose
    /// relative precision in the subnormal range. The scale-first norm also
    /// avoids constructing a potentially overflowing reciprocal of the
    /// unscaled amplitude. Measurement probabilities remain f64 estimates.
    pub fn postselect(&mut self, qubit: usize, outcome: bool) -> Result<(), SimError> {
        self.validate()?;
        let mask = self.mask(qubit)?;
        let selected = |basis: usize| (basis & mask != 0) == outcome;

        // No mutation until the selected support and normalization are valid.
        let mut scale = 0.0_f64;
        for (basis, amp) in self.amps.iter().enumerate() {
            if selected(basis) {
                scale = scale.max(amp.re.abs()).max(amp.im.abs());
            }
        }
        if scale == 0.0 {
            return Err(SimError::ImpossibleOutcome);
        }
        let mut scaled_norm_squared = 0.0_f64;
        for (basis, amp) in self.amps.iter().enumerate() {
            if selected(basis) {
                let re = amp.re / scale;
                let im = amp.im / scale;
                scaled_norm_squared += re * re + im * im;
            }
        }
        if !scaled_norm_squared.is_finite() || scaled_norm_squared <= 0.0 {
            return Err(SimError::NotNormalized);
        }
        let denominator = scaled_norm_squared.sqrt();

        for (basis, amp) in self.amps.iter_mut().enumerate() {
            if selected(basis) {
                amp.re = (amp.re / scale) / denominator;
                amp.im = (amp.im / scale) / denominator;
            } else {
                *amp = Complex::ZERO;
            }
        }
        self.validate()
    }

    /// A single, state-collapsing Z-basis measurement of one wire.
    /// The state handle remains usable; destructive ownership is not modeled here.
    pub fn measure(&mut self, qubit: usize, rng: &mut SeededRng) -> Result<bool, SimError> {
        let p_true = self.wire_probability(qubit, true)?;
        let outcome = rng.next_f64() < p_true;
        self.postselect(qubit, outcome)?;
        Ok(outcome)
    }

    /// Independent Z-basis shots from the same prepared state, not repeated
    /// measurements of an already collapsed state. Original state is unchanged.
    pub fn sample_basis(&self, shots: usize, seed: u64) -> Result<Vec<usize>, SimError> {
        self.validate()?;
        let output_bytes = shots.checked_mul(size_of::<usize>()).ok_or(SimError::DimensionOverflow)?;
        let total = self.state_bytes.checked_add(output_bytes).ok_or(SimError::DimensionOverflow)?;
        if total > self.max_bytes {
            return Err(SimError::BudgetExceeded);
        }
        let mut samples = Vec::new();
        samples.try_reserve_exact(shots).map_err(|_| SimError::AllocationFailed)?;
        let mut rng = SeededRng::new(seed);
        for _ in 0..shots {
            let draw = rng.next_f64();
            let mut cumulative = 0.0;
            let mut result = 0;
            for (basis, amp) in self.amps.iter().enumerate() {
                let p = amp.norm_squared();
                if p > 0.0 {
                    result = basis; // numerical tail fallback to last reachable outcome
                    cumulative += p;
                    if draw < cumulative {
                        break;
                    }
                }
            }
            samples.push(result);
        }
        Ok(samples)
    }
}
