#![forbid(unsafe_code)]

mod debug;
mod vm;

pub use debug::{
	DebugBreakpoint, DebugCommand, DebugEvent, DebugHandle, DebugSession, DebugWatch, PerfReport,
};
pub use vm::{Avm, AvmConfig, AvmEvent, AvmTerminated, AvmValue, ExecOutcome};
