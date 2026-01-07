#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use miette::{Diagnostic, IntoDiagnostic};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("linker error: {message}")]
#[diagnostic(code(aura::linker))]
pub struct LinkerError {
    pub message: String,
}

pub fn find_clang() -> Option<PathBuf> {
    // Prefer PATH.
    if let Ok(out) = Command::new("where").arg("clang").output() {
        if out.status.success() {
            if let Ok(s) = String::from_utf8(out.stdout) {
                if let Some(first) = s.lines().next() {
                    let p = PathBuf::from(first.trim());
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
    }

    // Fallback: default LLVM install location.
    let fallback = PathBuf::from(r"C:\Program Files\LLVM\bin\clang.exe");
    if fallback.exists() {
        return Some(fallback);
    }

    None
}

pub fn link_with_clang(
    module_ll: &Path,
    out_exe: &Path,
    lib_dirs: &[PathBuf],
    libs: &[String],
    c_sources: &[PathBuf],
    runtime_dlls: &[PathBuf],
) -> Result<(), LinkerError> {
    let clang = find_clang().ok_or_else(|| LinkerError {
        message: "could not locate clang.exe (install LLVM or put clang in PATH)".to_string(),
    })?;

    let runtime_c = aura_rt::runtime_c_path();
    let runtime_h = aura_rt::runtime_h_path();
    let include_dir = runtime_h
        .parent()
        .ok_or_else(|| LinkerError {
            message: "runtime include dir missing".to_string(),
        })?
        .to_path_buf();

    let stdlib_c = aura_stdlib::stdlib_c_path();
    let stdlib_h = aura_stdlib::stdlib_h_path();
    let stdlib_include_dir = stdlib_h
        .parent()
        .ok_or_else(|| LinkerError {
            message: "stdlib include dir missing".to_string(),
        })?
        .to_path_buf();

    // Spec target command (adapted): clang module.ll -I<rt/include> <rt/runtime.c> -o main.exe -O3 -march=native -flto
    // We statically link the runtime by compiling it into the final binary.
    let verbose = std::env::var_os("AURA_LINK_VERBOSE").is_some();

    let discovered_libs = discover_libs_in_dirs(lib_dirs, libs).map_err(|e| LinkerError {
        message: format!("failed to discover .lib files: {e}"),
    })?;
    let libs = merge_libs(libs, &discovered_libs);

    let mut args: Vec<String> = Vec::new();
    args.push(module_ll.display().to_string());
    args.push(runtime_c.display().to_string());
    args.push(format!("-I{}", include_dir.display()));
    args.push(stdlib_c.display().to_string());
    args.push(format!("-I{}", stdlib_include_dir.display()));

    // Optional runtime allocator modes.
    // This is intentionally env-driven to avoid adding new CLI UX.
    // Supported values:
    // - AURA_ALLOC_MODE=region  => compile stdlib/runtime with a bump arena allocator
    // - AURA_ARENA_BYTES=<n>     => override arena size (bytes) for region mode
    if let Ok(mode) = std::env::var("AURA_ALLOC_MODE") {
        let mode = mode.trim().to_ascii_lowercase();
        match mode.as_str() {
            "region" => {
                args.push("-DAURA_ALLOC_REGION=1".to_string());
                if let Ok(bytes) = std::env::var("AURA_ARENA_BYTES") {
                    let bytes = bytes.trim();
                    if !bytes.is_empty() {
                        args.push(format!("-DAURA_ARENA_BYTES={bytes}"));
                    }
                }
            }
            "" => {}
            other => {
                return Err(LinkerError {
                    message: format!(
                        "unknown AURA_ALLOC_MODE='{other}' (supported: 'region')"
                    ),
                });
            }
        }
    }

    // Compile additional shim sources (e.g., Raylib ABI adapters).
    for src in c_sources {
        args.push(src.display().to_string());
    }
    args.push("-std=c2x".to_string());
    args.push("-O3".to_string());
    args.push("-march=native".to_string());
    args.push("-flto".to_string());
    args.push("-fuse-ld=lld".to_string());

    for dir in lib_dirs {
        args.push(format!("-L{}", dir.display()));
    }

    for lib in &libs {
        let has_path = lib.contains('\\') || lib.contains('/');

        // If the user supplied a full/relative path, pass it through as an input.
        if has_path {
            args.push(lib.to_string());
            continue;
        }

        // On Windows (MSVC flavor), prefer `-l<name>` which maps to `name.lib`
        // and participates in the linker's search paths.
        if let Some(stem) = lib.strip_suffix(".lib") {
            args.push(format!("-l{}", stem));
            continue;
        }
        if let Some(stem) = lib.strip_suffix(".a") {
            args.push(format!("-l{}", stem));
            continue;
        }

        args.push(format!("-l{}", lib));
    }

    if verbose {
        args.push("-v".to_string());
    }

    args.push("-o".to_string());
    args.push(out_exe.display().to_string());

    let mut cmd = Command::new(clang);
    cmd.args(&args);

    let out = cmd.output().into_diagnostic().map_err(|e| LinkerError {
        message: e.to_string(),
    })?;

    if !out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(LinkerError {
            message: format!(
                "clang failed (exit {})\ncommand:\n  clang {}\nstdout:\n{}\nstderr:\n{}",
                out.status,
                args.join(" "),
                stdout,
                stderr
            ),
        });
    }

    post_link_copy_dlls(out_exe, lib_dirs, runtime_dlls)?;

    Ok(())
}

fn discover_libs_in_dirs(dirs: &[PathBuf], explicit: &[String]) -> io::Result<Vec<String>> {
    // If no explicit libs were requested, do not auto-add any.
    // This avoids accidentally linking unrelated packages that happen to be present in ./deps.
    if explicit.is_empty() {
        return Ok(Vec::new());
    }

    let mut prefixes: Vec<String> = Vec::new();
    for s in explicit {
        let lower = s.to_ascii_lowercase();
        let stem = lower
            .strip_suffix(".lib")
            .or_else(|| lower.strip_suffix(".a"))
            .unwrap_or(&lower);
        // Ignore obvious path-y things.
        if stem.contains('\\') || stem.contains('/') {
            continue;
        }
        prefixes.push(stem.to_string());
    }

    let mut out = Vec::new();
    for d in dirs {
        let Ok(rd) = fs::read_dir(d) else { continue };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("lib")) {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    let stem = name
                        .to_ascii_lowercase()
                        .strip_suffix(".lib")
                        .unwrap_or(&name.to_ascii_lowercase())
                        .to_string();

                    // Only include discovered libs that look related to an explicitly requested one.
                    // Example: explicit onnxruntime.lib => include onnxruntime_providers_shared.lib.
                    let include = prefixes.iter().any(|pfx| stem.starts_with(pfx));
                    if include {
                        out.push(name.to_string());
                    }
                }
            }
        }
    }
    // Stable ordering
    out.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Ok(out)
}

fn merge_libs(explicit: &[String], discovered: &[String]) -> Vec<String> {
    // Union(discovered, explicit), with explicit taking precedence in ordering.
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for s in explicit {
        let k = s.to_ascii_lowercase();
        if seen.insert(k) {
            out.push(s.clone());
        }
    }

    for s in discovered {
        let k = s.to_ascii_lowercase();
        if seen.insert(k) {
            out.push(s.clone());
        }
    }

    out
}

fn post_link_copy_dlls(
    out_exe: &Path,
    lib_dirs: &[PathBuf],
    runtime_dlls: &[PathBuf],
) -> Result<(), LinkerError> {
    let out_dir = out_exe.parent().ok_or_else(|| LinkerError {
        message: "output exe has no parent directory".to_string(),
    })?;

    // 1) Copy DLLs explicitly provided by the bridge (best-effort)
    for dll in runtime_dlls {
        if let Some(name) = dll.file_name() {
            let dst = out_dir.join(name);
            let _ = fs::copy(dll, &dst);
        }
    }

    // 2) Copy any DLLs in lib search dirs (best-effort)
    for d in lib_dirs {
        let Ok(rd) = fs::read_dir(d) else { continue };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("dll")) {
                if let Some(name) = p.file_name() {
                    let dst = out_dir.join(name);
                    // Avoid spamming writes if identical file is already there.
                    if dst.exists() {
                        continue;
                    }
                    let _ = fs::copy(&p, &dst);
                }
            }
        }
    }

    Ok(())
}
