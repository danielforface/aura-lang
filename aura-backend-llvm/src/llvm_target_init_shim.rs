#![forbid(unsafe_code)]

use std::ffi::c_int;

// LLVM C API defines `LLVMBool` as `int` (0 = false/success, 1 = true/error).
const LLVM_BOOL_ERROR: c_int = 1;

// These are declared in LLVM's `llvm-c/Target.h`.
// Some Windows LLVM distributions omit them from LLVM-C.dll; in that case,
// inkwell still references them, and the final link fails.
//
// This shim is deliberately conservative:
// - The `*_All*` functions are no-ops.
// - The `*_Native*` functions report an error if called.
//
// If you install a full LLVM that exports these symbols, set:
// `AURA_NO_LLVM_TARGET_INIT_SHIM=1`
// to disable these definitions.

#[no_mangle]
pub extern "C" fn LLVMInitializeAllTargetInfos() {}

#[no_mangle]
pub extern "C" fn LLVMInitializeAllTargets() {}

#[no_mangle]
pub extern "C" fn LLVMInitializeAllTargetMCs() {}

#[no_mangle]
pub extern "C" fn LLVMInitializeAllAsmParsers() {}

#[no_mangle]
pub extern "C" fn LLVMInitializeAllAsmPrinters() {}

#[no_mangle]
pub extern "C" fn LLVMInitializeAllDisassemblers() {}

#[no_mangle]
pub extern "C" fn LLVMInitializeNativeTarget() -> c_int {
    LLVM_BOOL_ERROR
}

#[no_mangle]
pub extern "C" fn LLVMInitializeNativeAsmPrinter() -> c_int {
    LLVM_BOOL_ERROR
}

#[no_mangle]
pub extern "C" fn LLVMInitializeNativeAsmParser() -> c_int {
    LLVM_BOOL_ERROR
}

#[no_mangle]
pub extern "C" fn LLVMInitializeNativeDisassembler() -> c_int {
    LLVM_BOOL_ERROR
}
