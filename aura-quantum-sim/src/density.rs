//! Bounded noisy CPU density-matrix reference model (AURA-Q28-007).
//!
//! This is a finite-f64 scientific simulator, not a physical calibration,
//! an Aura compiler/IR interface, a proof, or a QPU execution engine.
//!
//! Basis bit q0 is the least significant bit. A density matrix uses row-major
//! entries rho[row * dimension + column] and represents mixed quantum states.
//! Only states constructed from |0...0> or validated Statevector are accepted;
//! arbitrary user-supplied matrices are intentionally not a public API.
//! All supported channel operations are trace-preserving CPTP maps in exact
//! arithmetic. Tolerance checks are empirical, not certified error bounds.
use crate::{Complex, Gate, SimError, Statevector, NORM_TOLERANCE};
use std::mem::size_of;

pub const MAX_DENSITY_QUBITS: usize = 8;
pub const DENSITY_PROFILE: &str = "local-density-matrix-f64-cptp/1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DensityError {
    UnsupportedWidth { qubits: usize, max: usize },
    DimensionOverflow,
    BudgetExceeded { required_bytes: usize, budget_bytes: usize },
    AllocationFailed,
    InvalidBasisIndex(usize),
    InvalidQubit { index: usize, qubits: usize },
    DuplicateQubit(usize),
    InvalidProbability,
    InvalidDensity,
    Simulator(SimError),
}

impl std::fmt::Display for DensityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for DensityError {}

impl From<SimError> for DensityError {
    fn from(value: SimError) -> Self {
        Self::Simulator(value)
    }
}

/// Noise strength is an ideal model parameter in [0,1], not hardware
/// calibration. Both maps use the Kraus convention E(rho)=sum K rho K^dagger.
///
/// PhaseFlip(p) = (1-p) rho + p Z rho Z.
/// AmplitudeDamping(gamma) uses
/// K0 = |0><0| + sqrt(1-gamma)|1><1|,
/// K1 = sqrt(gamma)|0><1|.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoiseChannel {
    PhaseFlip { qubit: usize, probability: f64 },
    AmplitudeDamping { qubit: usize, gamma: f64 },
}

#[derive(Debug, Clone)]
pub struct DensityMatrix {
    qubits: usize,
    dimension: usize,
    elements: Vec<Complex>,
    logical_bytes: usize,
}

fn conj(z: Complex) -> Complex {
    Complex::new(z.re, -z.im)
}

fn scale(z: Complex, factor: f64) -> Complex {
    Complex::new(z.re * factor, z.im * factor)
}

fn neg(z: Complex) -> Complex {
    Complex::new(-z.re, -z.im)
}

impl DensityMatrix {
    /// Allocate rho = |0...0><0...0|. Logical memory cost is
    /// 16 * 4^qubits bytes, excluding allocator/runtime overhead.
    pub fn zero(qubits: usize, budget_bytes: usize) -> Result<Self, DensityError> {
        if !(1..=MAX_DENSITY_QUBITS).contains(&qubits) {
            return Err(DensityError::UnsupportedWidth {
                qubits,
                max: MAX_DENSITY_QUBITS,
            });
        }
        let dim = 1usize
            .checked_shl(qubits as u32)
            .ok_or(DensityError::DimensionOverflow)?;
        let elements = dim.checked_mul(dim).ok_or(DensityError::DimensionOverflow)?;
        let bytes = elements
            .checked_mul(size_of::<Complex>())
            .ok_or(DensityError::DimensionOverflow)?;
        if bytes > budget_bytes {
            return Err(DensityError::BudgetExceeded {
                required_bytes: bytes,
                budget_bytes,
            });
        }
        let mut data = Vec::new();
        data.try_reserve_exact(elements).map_err(|_| DensityError::AllocationFailed)?;
        data.resize(elements, Complex::ZERO);
        data[0] = Complex::ONE;
        Ok(Self {
            qubits,
            dimension: dim,
            elements: data,
            logical_bytes: bytes,
        })
    }

    /// Lift an already validated pure state to rho = |psi><psi|.
    /// This cannot create an arbitrary, potentially non-PSD density matrix.
    pub fn from_statevector(
        pure: &Statevector,
        budget_bytes: usize,
    ) -> Result<Self, DensityError> {
        pure.validate()?;
        let mut mixed = Self::zero(pure.qubits(), budget_bytes)?;
        for row in 0..mixed.dimension {
            let left = pure.amplitude(row)?;
            for col in 0..mixed.dimension {
                let right = pure.amplitude(col)?;
                mixed.elements[row * mixed.dimension + col] = left * conj(right);
            }
        }
        mixed.validate()?;
        Ok(mixed)
    }

    pub fn qubits(&self) -> usize {
        self.qubits
    }

    pub fn logical_bytes(&self) -> usize {
        self.logical_bytes
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// The complex density matrix entry, including relative coherence phase.
    pub fn element(&self, row: usize, col: usize) -> Result<Complex, DensityError> {
        if row >= self.dimension {
            return Err(DensityError::InvalidBasisIndex(row));
        }
        if col >= self.dimension {
            return Err(DensityError::InvalidBasisIndex(col));
        }
        Ok(self.elements[row * self.dimension + col])
    }

    pub fn trace(&self) -> f64 {
        (0..self.dimension)
            .map(|i| self.elements[i * self.dimension + i].re)
            .sum()
    }

    /// Trace(rho^2) for a constructed Hermitian rho, in [0,1] ideally.
    /// A value below 1 signals mixture; this is NOT entanglement entropy.
    pub fn purity(&self) -> Result<f64, DensityError> {
        self.validate()?;
        Ok(self.elements.iter().map(|z| z.norm_squared()).sum())
    }

    pub fn basis_probability(&self, index: usize) -> Result<f64, DensityError> {
        let z = self.element(index, index)?;
        Ok(z.re)
    }

    pub fn reduced_qubit(&self, qubit: usize) -> Result<[[Complex; 2]; 2], DensityError> {
        let mask = self.mask(qubit)?;
        let mut reduced = [[Complex::ZERO; 2]; 2];
        for base in 0..self.dimension {
            if base & mask == 0 {
                let other = base | mask;
                reduced[0][0] = reduced[0][0] + self.elements[base * self.dimension + base];
                reduced[0][1] = reduced[0][1] + self.elements[base * self.dimension + other];
                reduced[1][0] = reduced[1][0] + self.elements[other * self.dimension + base];
                reduced[1][1] = reduced[1][1] + self.elements[other * self.dimension + other];
            }
        }
        Ok(reduced)
    }

    /// Necessary trace, diagonal and Hermiticity invariants. These checks do
    /// not certify positivity of arbitrary input matrices (which the public
    /// API intentionally forbids). Supported constructors/channels preserve
    /// positivity in exact arithmetic.
    pub fn validate(&self) -> Result<(), DensityError> {
        let mut trace = 0.0;
        for row in 0..self.dimension {
            let diagonal = self.elements[row * self.dimension + row];
            if !diagonal.re.is_finite()
                || !diagonal.im.is_finite()
                || diagonal.im.abs() > NORM_TOLERANCE
                || diagonal.re < -NORM_TOLERANCE
                || diagonal.re > 1.0 + NORM_TOLERANCE
            {
                return Err(DensityError::InvalidDensity);
            }
            trace += diagonal.re;
            for col in 0..row {
                let a = self.elements[row * self.dimension + col];
                let b = self.elements[col * self.dimension + row];
                if !a.re.is_finite()
                    || !a.im.is_finite()
                    || !b.re.is_finite()
                    || !b.im.is_finite()
                    || (a.re - b.re).abs() > NORM_TOLERANCE
                    || (a.im + b.im).abs() > NORM_TOLERANCE
                {
                    return Err(DensityError::InvalidDensity);
                }
            }
        }
        if !trace.is_finite() || (trace - 1.0).abs() > NORM_TOLERANCE {
            return Err(DensityError::InvalidDensity);
        }
        Ok(())
    }

    fn mask(&self, qubit: usize) -> Result<usize, DensityError> {
        if qubit >= self.qubits {
            return Err(DensityError::InvalidQubit {
                index: qubit,
                qubits: self.qubits,
            });
        }
        Ok(1usize << qubit)
    }

    fn masks(&self, a: usize, b: usize) -> Result<(usize, usize), DensityError> {
        let left = self.mask(a)?;
        let right = self.mask(b)?;
        if a == b {
            return Err(DensityError::DuplicateQubit(a));
        }
        Ok((left, right))
    }

    /// In-place two-sided unitary action rho' = U rho U^dagger, with NO
    /// second full matrix allocation. First transform row pairs by U, then
    /// column pairs by conjugate transpose U^dagger.
    fn single(&mut self, qubit: usize, u: [[Complex; 2]; 2]) -> Result<(), DensityError> {
        let mask = self.mask(qubit)?;
        for row in 0..self.dimension {
            if row & mask == 0 {
                let other = row | mask;
                for col in 0..self.dimension {
                    let idx0 = row * self.dimension + col;
                    let idx1 = other * self.dimension + col;
                    let x = self.elements[idx0];
                    let y = self.elements[idx1];
                    self.elements[idx0] = u[0][0] * x + u[0][1] * y;
                    self.elements[idx1] = u[1][0] * x + u[1][1] * y;
                }
            }
        }
        for col in 0..self.dimension {
            if col & mask == 0 {
                let other = col | mask;
                for row in 0..self.dimension {
                    let idx0 = row * self.dimension + col;
                    let idx1 = row * self.dimension + other;
                    let x = self.elements[idx0];
                    let y = self.elements[idx1];
                    self.elements[idx0] = x * conj(u[0][0]) + y * conj(u[0][1]);
                    self.elements[idx1] = x * conj(u[1][0]) + y * conj(u[1][1]);
                }
            }
        }
        Ok(())
    }

    fn permute(&mut self, swaps: &[(usize, usize)]) {
        for &(a, b) in swaps {
            for col in 0..self.dimension {
                self.elements.swap(a * self.dimension + col, b * self.dimension + col);
            }
        }
        for &(a, b) in swaps {
            for row in 0..self.dimension {
                self.elements.swap(row * self.dimension + a, row * self.dimension + b);
            }
        }
    }

    /// Apply one of the exact reference library's 9 unitary gate families.
    /// The gate signature is read-only shared with this same Agent C crate.
    pub fn apply_gate(&mut self, gate: Gate) -> Result<(), DensityError> {
        self.validate()?;
        let o = Complex::ZERO;
        let one = Complex::ONE;
        let minus = Complex::new(-1.0, 0.0);
        let a = std::f64::consts::FRAC_1_SQRT_2;
        let plus = Complex::new(a, 0.0);
        let neg_a = Complex::new(-a, 0.0);
        match gate {
            Gate::H(q) => self.single(q, [[plus, plus], [plus, neg_a]])?,
            Gate::X(q) => self.single(q, [[o, one], [one, o]])?,
            Gate::Y(q) => self.single(q, [[o, Complex::NEG_I], [Complex::I, o]])?,
            Gate::Z(q) => self.single(q, [[one, o], [o, minus]])?,
            Gate::S(q) => self.single(q, [[one, o], [o, Complex::I]])?,
            Gate::T(q) => self.single(q, [[one, o], [o, Complex::new(a, a)]])?,
            Gate::Cnot { control, target } => {
                let (mc, mt) = self.masks(control, target)?;
                let mut swaps = Vec::new();
                for row in 0..self.dimension {
                    if row & mc != 0 && row & mt == 0 {
                        swaps.push((row, row | mt));
                    }
                }
                self.permute(&swaps);
            }
            Gate::Cz { control, target } => {
                let (mc, mt) = self.masks(control, target)?;
                for row in 0..self.dimension {
                    let r = row & mc != 0 && row & mt != 0;
                    for col in 0..self.dimension {
                        let c = col & mc != 0 && col & mt != 0;
                        if r != c {
                            let idx = row * self.dimension + col;
                            self.elements[idx] = neg(self.elements[idx]);
                        }
                    }
                }
            }
            Gate::Swap { a, b } => {
                let (ma, mb) = self.masks(a, b)?;
                let mut swaps = Vec::new();
                for row in 0..self.dimension {
                    if row & ma == 0 && row & mb != 0 {
                        swaps.push((row, row ^ ma ^ mb));
                    }
                }
                self.permute(&swaps);
            }
        }
        self.validate()
    }

    /// Apply one *idealized* single-qubit Kraus noise channel. All parameter
    /// validation happens before mutation; invalid requests are fail-closed.
    pub fn apply_noise(&mut self, channel: NoiseChannel) -> Result<(), DensityError> {
        self.validate()?;
        let (qubit, strength) = match channel {
            NoiseChannel::PhaseFlip { qubit, probability } => (qubit, probability),
            NoiseChannel::AmplitudeDamping { qubit, gamma } => (qubit, gamma),
        };
        let mask = self.mask(qubit)?;
        if !strength.is_finite() || !(0.0..=1.0).contains(&strength) {
            return Err(DensityError::InvalidProbability);
        }
        match channel {
            NoiseChannel::PhaseFlip { .. } => {
                // Kraus {sqrt(1-p)I, sqrt(p)Z}. Opposite target-bit
                // coherences acquire factor (1-2p); diagonals are unchanged.
                let multiplier = 1.0 - 2.0 * strength;
                for row in 0..self.dimension {
                    for col in 0..self.dimension {
                        if (row & mask != 0) != (col & mask != 0) {
                            let idx = row * self.dimension + col;
                            self.elements[idx] = scale(self.elements[idx], multiplier);
                        }
                    }
                }
            }
            NoiseChannel::AmplitudeDamping { .. } => {
                // Kraus:
                // K0=diag(1,sqrt(1-gamma)); K1=sqrt(gamma)|0><1|.
                // Four entries in each (environment_row,environment_col)
                // block are disjoint from all other blocks.
                let retention = 1.0 - strength;
                let coherence = retention.sqrt();
                for r0 in 0..self.dimension {
                    if r0 & mask != 0 {
                        continue;
                    }
                    let r1 = r0 | mask;
                    for c0 in 0..self.dimension {
                        if c0 & mask != 0 {
                            continue;
                        }
                        let c1 = c0 | mask;
                        let i00 = r0 * self.dimension + c0;
                        let i01 = r0 * self.dimension + c1;
                        let i10 = r1 * self.dimension + c0;
                        let i11 = r1 * self.dimension + c1;
                        let a = self.elements[i00];
                        let b = self.elements[i01];
                        let c = self.elements[i10];
                        let d = self.elements[i11];
                        self.elements[i00] = a + scale(d, strength);
                        self.elements[i01] = scale(b, coherence);
                        self.elements[i10] = scale(c, coherence);
                        self.elements[i11] = scale(d, retention);
                    }
                }
            }
        }
        self.validate()
    }
}
