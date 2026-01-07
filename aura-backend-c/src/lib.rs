#![forbid(unsafe_code)]

mod emit;

pub use emit::{emit_module, CArtifacts, CBackendError};
