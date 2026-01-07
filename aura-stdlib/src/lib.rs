#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

/// Absolute path to the bundled stdlib C source (`stdlib.c`).
pub fn stdlib_c_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("stdlib.c")
}

/// Absolute path to the bundled stdlib header (`aura_stdlib.h`).
pub fn stdlib_h_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("include").join("aura_stdlib.h")
}
