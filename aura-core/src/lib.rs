#![forbid(unsafe_code)]

mod error;
mod capability;
mod lower;
mod sema;
mod types;
mod verifier;

pub use error::SemanticError;
pub use capability::{CapabilityGraph, CapabilityKind};
pub use lower::lower_program;
pub use sema::Checker;
pub use types::Type;
pub use verifier::{DummySolver, Verifier};
