use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
};

use aura_ir::DebugSource;
use miette::{IntoDiagnostic, Result};

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("corpus")
}

pub fn find_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for ent in std::fs::read_dir(dir).into_diagnostic()? {
        let ent = ent.into_diagnostic()?;
        let p = ent.path();
        if p.is_dir() {
            continue;
        }
        if p.extension() == Some(OsStr::new("aura")) {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

pub fn find_c_compiler() -> Option<String> {
    if Command::new("clang").arg("--version").output().is_ok() {
        return Some("clang".to_string());
    }
    if Command::new("gcc").arg("--version").output().is_ok() {
        return Some("gcc".to_string());
    }
    None
}

pub fn ensure_required_tooling_available() -> Result<()> {
    if std::env::var("AURA_DIFFERENTIAL_REQUIRE_C").ok().as_deref() == Some("1")
        && find_c_compiler().is_none()
    {
        return Err(miette::miette!(
            "AURA_DIFFERENTIAL_REQUIRE_C=1 but no C compiler found (clang/gcc)"
        ));
    }
    Ok(())
}

pub fn run_avm(src: &str) -> Result<RunOutcome> {
    let src_aug = aura_sdk::augment_source_with_default_std(src).into_diagnostic()?;

    let mut avm = aura_interpret::Avm::new(aura_interpret::AvmConfig {
        enable_z3_gate: false,
        ..Default::default()
    });

    match avm.exec_entry_cell(&src_aug, "main") {
        Ok(out) => Ok(RunOutcome {
            ok: out.verified,
            stdout: out.stdout,
            stderr: String::new(),
        }),
        Err(e) => Ok(RunOutcome {
            ok: false,
            stdout: String::new(),
            stderr: format!("{e:?}"),
        }),
    }
}

pub fn compile_and_run_c(case_name: &str, src: &str, out_dir: &Path) -> Result<Option<RunOutcome>> {
    let Some(cc) = find_c_compiler() else {
        return Ok(None);
    };

    let src_aug = match aura_sdk::augment_source_with_default_std(src).into_diagnostic() {
        Ok(s) => s,
        Err(e) => {
            return Ok(Some(RunOutcome {
                ok: false,
                stdout: String::new(),
                stderr: format!("{case_name}: std augmentation failed: {e:?}"),
            }));
        }
    };

    let program = match aura_parse::parse_source(&src_aug) {
        Ok(p) => p,
        Err(e) => {
            return Ok(Some(RunOutcome {
                ok: false,
                stdout: String::new(),
                stderr: format!("{case_name}: parse failed: {e:?}"),
            }));
        }
    };

    let mut checker = aura_core::Checker::new();
    checker.set_defer_range_proofs(true);
    if let Err(e) = checker.check_program(&program) {
        return Ok(Some(RunOutcome {
            ok: false,
            stdout: String::new(),
            stderr: format!("{case_name}: sema failed: {e:?}"),
        }));
    }

    let module_ir = match aura_core::lower_program(&program) {
        Ok(m) => m,
        Err(e) => {
            return Ok(Some(RunOutcome {
                ok: false,
                stdout: String::new(),
                stderr: format!("{case_name}: lowering failed: {e:?}"),
            }));
        }
    };
    let debug = DebugSource::new("differential.aura".to_string(), &src_aug);
    let artifacts = aura_backend_c::emit_module(&module_ir, Some(&debug))?;

    std::fs::create_dir_all(out_dir).into_diagnostic()?;
    let module_c = out_dir.join("module.c");
    std::fs::write(out_dir.join("aura_runtime.h"), artifacts.runtime_h).into_diagnostic()?;
    std::fs::write(&module_c, artifacts.module_c).into_diagnostic()?;

    let exe = out_dir.join("run_c.exe");

    let stdlib_c = aura_stdlib::stdlib_c_path();
    let stdlib_h = aura_stdlib::stdlib_h_path();
    let stdlib_include_dir = stdlib_h
        .parent()
        .ok_or_else(|| miette::miette!("stdlib include dir missing"))?;

    let status = Command::new(&cc)
        .arg("-std=c2x")
        .arg("-O2")
        .arg(format!("-I{}", stdlib_include_dir.display()))
        .arg(&module_c)
        .arg(stdlib_c)
        .arg("-o")
        .arg(&exe)
        .current_dir(out_dir)
        .status()
        .into_diagnostic()?;

    if !status.success() {
        return Ok(Some(RunOutcome {
            ok: false,
            stdout: String::new(),
            stderr: format!("C compilation failed: {status}"),
        }));
    }

    let output = Command::new(&exe).output().into_diagnostic()?;
    Ok(Some(RunOutcome {
        ok: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }))
}

fn find_clang_windows() -> Option<PathBuf> {
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

    let fallback = PathBuf::from(r"C:\Program Files\LLVM\bin\clang.exe");
    if fallback.exists() {
        return Some(fallback);
    }

    None
}

fn find_clang_unix() -> Option<PathBuf> {
    if Command::new("clang").arg("--version").output().is_ok() {
        return Some(PathBuf::from("clang"));
    }
    None
}

pub fn compile_and_run_llvm(case_name: &str, src: &str, out_dir: &Path) -> Result<Option<RunOutcome>> {
    let clang = if cfg!(windows) {
        find_clang_windows()
    } else {
        find_clang_unix()
    };
    let Some(clang) = clang else {
        return Ok(None);
    };

    let src_aug = match aura_sdk::augment_source_with_default_std(src).into_diagnostic() {
        Ok(s) => s,
        Err(e) => {
            return Ok(Some(RunOutcome {
                ok: false,
                stdout: String::new(),
                stderr: format!("{case_name}: std augmentation failed: {e:?}"),
            }));
        }
    };

    let program = match aura_parse::parse_source(&src_aug) {
        Ok(p) => p,
        Err(e) => {
            return Ok(Some(RunOutcome {
                ok: false,
                stdout: String::new(),
                stderr: format!("{case_name}: parse failed: {e:?}"),
            }));
        }
    };

    let mut checker = aura_core::Checker::new();
    checker.set_defer_range_proofs(true);
    if let Err(e) = checker.check_program(&program) {
        return Ok(Some(RunOutcome {
            ok: false,
            stdout: String::new(),
            stderr: format!("{case_name}: sema failed: {e:?}"),
        }));
    }

    let module_ir = match aura_core::lower_program(&program) {
        Ok(m) => m,
        Err(e) => {
            return Ok(Some(RunOutcome {
                ok: false,
                stdout: String::new(),
                stderr: format!("{case_name}: lowering failed: {e:?}"),
            }));
        }
    };
    let debug = DebugSource::new("differential.aura".to_string(), &src_aug);

    let llvm_ir = match aura_backend_llvm::emit_llvm_ir(&module_ir, Some(&debug)) {
        Ok(a) => a.llvm_ir,
        Err(e) => {
            // LLVM backend may be disabled (feature-gated); treat as unavailable.
            if e.message.contains("not enabled") {
                return Ok(None);
            }
            return Ok(Some(RunOutcome {
                ok: false,
                stdout: String::new(),
                stderr: format!("{case_name}: llvm emit failed: {e:?}"),
            }));
        }
    };

    std::fs::create_dir_all(out_dir).into_diagnostic()?;
    let ll_path = out_dir.join("module.ll");
    std::fs::write(&ll_path, llvm_ir).into_diagnostic()?;

    let runtime_c = aura_rt::runtime_c_path();
    let runtime_h = aura_rt::runtime_h_path();
    let include_dir = runtime_h
        .parent()
        .ok_or_else(|| miette::miette!("runtime include dir missing"))?;

    let stdlib_c = aura_stdlib::stdlib_c_path();
    let stdlib_h = aura_stdlib::stdlib_h_path();
    let stdlib_include_dir = stdlib_h
        .parent()
        .ok_or_else(|| miette::miette!("stdlib include dir missing"))?;

    let exe = out_dir.join("run_llvm.exe");

    let status = Command::new(&clang)
        .arg("-O2")
        .arg(format!("-I{}", include_dir.display()))
        .arg(format!("-I{}", stdlib_include_dir.display()))
        .arg(&ll_path)
        .arg(runtime_c)
        .arg(stdlib_c)
        .arg("-o")
        .arg(&exe)
        .current_dir(out_dir)
        .status()
        .into_diagnostic()?;

    if !status.success() {
        return Ok(Some(RunOutcome {
            ok: false,
            stdout: String::new(),
            stderr: format!("LLVM compilation failed: {status}"),
        }));
    }

    let output = Command::new(&exe).output().into_diagnostic()?;
    Ok(Some(RunOutcome {
        ok: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }))
}

pub fn failures_root() -> PathBuf {
    PathBuf::from(".aura").join("differential_failures")
}

pub fn fixture_write_dir() -> Option<PathBuf> {
    if std::env::var("AURA_DIFFERENTIAL_WRITE_FIXTURES")
        .ok()
        .as_deref()
        != Some("1")
    {
        return None;
    }

    Some(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("differential"),
    )
}
