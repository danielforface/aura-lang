#![forbid(unsafe_code)]

mod error;
mod capability;
mod lower;
mod sema;
mod types;
mod verifier;
pub mod net_verifier;
pub mod ownership_enforcement;
pub mod move_tracking;
pub mod control_flow;
pub mod function_signature;
pub mod diagnostics;
pub mod capability_enforcement;
pub mod capability_validator;
pub mod capability_diagnostics;
pub mod race_detector;
pub mod explanation_engine;

pub use error::SemanticError;
pub use capability::CapabilityGraph;
pub use lower::lower_program;
pub use sema::Checker;
pub use types::Type;
pub use verifier::{DummySolver, Verifier};
pub use net_verifier::{NetworkVerifier, NetworkProperty, SocketState};
pub use ownership_enforcement::{OwnershipContext, OwnershipState, OwnershipBinding, OwnershipViolation, ViolationKind};
pub use move_tracking::{LinearTypeKind, classify_type, MoveTracker, LinearTypeRules, LinearTypeViolationDiagnostic};
pub use control_flow::{ControlFlowGraph, ControlFlowPath, OwnershipFlowAnalyzer};
pub use function_signature::{LinearFunctionSignature, LinearParam, LinearReturn, ParamMode, ReturnMode, SignatureValidator, SignatureContext};
pub use diagnostics::{LinearTypeDiagnostic, DiagnosticFactory, DiagnosticReporter, Severity, Location, CodeSnippet, DiagnosticBuilder};
pub use capability_enforcement::{CapabilityKind, CapabilityState, CapabilityViolation, CapabilityBinding, CapabilityContext};
pub use capability_validator::CapabilityValidator;
pub use capability_diagnostics::{CapabilityDiagnostic, CapabilitySeverity, CapabilityLocation, CapabilityDiagnosticFactory, CapabilityDiagnosticReporter, CodeSnippet as CapabilityCodeSnippet};
pub use race_detector::{RaceDetector, RaceViolation, MemoryAccess, AccessType, SynchronizationInfo, LockInfo};
pub use explanation_engine::{ExplanationEngine, Explanation, ProofStep, Counterexample, VariableBinding};
