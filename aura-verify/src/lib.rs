#![forbid(unsafe_code)]

pub mod solver;
pub mod verify;
pub mod proof_summary;
pub mod counterexample_mapper;
pub mod variable_traces;
pub mod linear_types;
pub mod region_stdlib;
#[cfg(feature = "z3")]
pub mod geometry;

pub use solver::{NoZ3Prover, Prover, SmtProfile, VerifyError};
pub use proof_summary::{ProofSummary, ProofResult, ModuleSummaryCache};
pub use counterexample_mapper::{TypedValue, CounterexampleMapper};
pub use variable_traces::{TraceCollector, VariableTrace, TraceEvent};
pub use linear_types::{OwnershipChecker, OwnershipBinding, Ownership, OwnershipError};
pub use region_stdlib::{BoundsContract, VerifiedVec, VerifiedHashMap};
#[cfg(feature = "z3")]
pub use solver::z3_prover::Z3Prover;
pub use verify::verify_program;
#[cfg(feature = "z3")]
pub use verify::verify_program_z3;
#[cfg(feature = "z3")]
pub use verify::verify_program_z3_profile;
#[cfg(feature = "z3")]
pub use verify::{verify_program_z3_report, VerificationReport, VerificationStatus};
