#![forbid(unsafe_code)]

mod codegen;

pub use codegen::{emit_llvm_ir, LlvmBackendError, LlvmArtifacts};
