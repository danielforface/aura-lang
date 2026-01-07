/// Aura Language Server Protocol (LSP) Library
/// 
/// This module provides LSP support for the Aura programming language,
/// including differential testing for proof verification, CI gating,
/// and comprehensive verification capabilities.

pub mod ci_gate;
pub mod ci_gate_driver;
pub mod counterexample_lsp;
pub mod differential_test_runner;
pub mod linear_type_debugger;
pub mod merkle_cache;
pub mod performance_tuning;
pub mod profiling;
pub mod telemetry_collector;
pub mod gdb_mi_protocol;
pub mod lldb_mi_protocol;
pub mod lsp_ci_gate_integration;
pub mod proof_result_extractor;
pub mod performance_cache;

pub use ci_gate::{CIGate, CIGateConfig, CIGateResult};
pub use ci_gate_driver::CIGateDriver;
pub use differential_test_runner::{
    DifferentialTest, DifferentialTestRunner, DifferentialTestComparison,
    GDBRunner, LLDBRunner
};
pub use gdb_mi_protocol::GDBMIProtocol;
pub use lldb_mi_protocol::LLDBMIProtocol;
pub use performance_cache::ResponseCache;

/// Version information
pub const VERSION: &str = "2.0.0";
pub const LSP_VERSION: &str = "3.17";
