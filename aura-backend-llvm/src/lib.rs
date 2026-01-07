#![forbid(unsafe_code)]

mod codegen;
pub mod debugger;
pub mod pattern_lowering;

pub use codegen::{emit_llvm_ir, LlvmBackendError, LlvmArtifacts};
pub use debugger::{DwarfDebugInfo, DebuggerIntegration, FunctionDebugInfo, SourceLocation};
pub use pattern_lowering::{MatchLowering, JumpTable, analyze_match};
