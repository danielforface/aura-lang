#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

/// Absolute path to the bundled C23 runtime source (`runtime.c`).
pub fn runtime_c_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("runtime.c")
}

/// Absolute path to the bundled runtime header (`aura_rt.h`).
pub fn runtime_h_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("include").join("aura_rt.h")
}
