#![forbid(unsafe_code)]

mod emit;

pub use emit::{emit_module, emit_module_with_source_contract, CArtifacts, CBackendError};
