use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Only required when the real Z3 integration is enabled.
    let z3_enabled = env::var_os("CARGO_FEATURE_Z3").is_some();
    if !z3_enabled {
        return;
    }

    if !cfg!(windows) {
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("aura-verify should be a workspace member")
        .to_path_buf();

    let z3_bin = workspace_root.join("tools/z3/dist/z3-4.15.4-x64-win/bin");
    let dll_src = z3_bin.join("libz3.dll");

    println!("cargo:rerun-if-changed={}", dll_src.display());

    if !dll_src.exists() {
        panic!(
            "Z3 is enabled, but {} is missing. Re-run the Z3 download/extract step.",
            dll_src.display()
        );
    }

    // Ensure `cargo run` can locate `libz3.dll` by placing it next to the produced binaries.
    // Windows DLL search includes the executable directory.
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target_dir = workspace_root.join("target").join(profile);
    let dll_dst = target_dir.join("libz3.dll");

    if let Some(parent) = dll_dst.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Only copy when the destination is missing or out of date.
    let needs_copy = match (fs::metadata(&dll_src), fs::metadata(&dll_dst)) {
        (Ok(src), Ok(dst)) => src.modified().ok() != dst.modified().ok(),
        (Ok(_), Err(_)) => true,
        _ => true,
    };

    if needs_copy {
        fs::copy(&dll_src, &dll_dst).unwrap_or_else(|e| {
            panic!(
                "Failed to copy {} to {}: {e}",
                dll_src.display(),
                dll_dst.display()
            )
        });
    }
}
