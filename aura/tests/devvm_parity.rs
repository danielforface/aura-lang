use std::{path::PathBuf, process::Command};

use aura_ir::DebugSource;
use miette::IntoDiagnostic;

fn find_c_compiler() -> Option<String> {
    if Command::new("clang").arg("--version").output().is_ok() {
        return Some("clang".to_string());
    }
    if Command::new("gcc").arg("--version").output().is_ok() {
        return Some("gcc".to_string());
    }
    None
}

#[test]
fn devvm_and_c_backend_match_stdout_for_println_smoke() -> miette::Result<()> {
    let Some(cc) = find_c_compiler() else {
        eprintln!("skipping parity smoke: no C compiler in PATH");
        return Ok(());
    };

    let src = r#"
import aura::io

cell main() ->:
  val x: u32 = 2 + 3
  io::println("x = {x}")
"#;

    // --- Dev-VM (AVM) ---
    let src_aug = aura_sdk::augment_source_with_default_std(src).into_diagnostic()?;
    let mut avm = aura_interpret::Avm::new(aura_interpret::AvmConfig { enable_z3_gate: false, ..Default::default() });
    let out = avm.exec_entry_cell(&src_aug, "main")?;
    let dev_stdout = out.stdout;

    // --- C backend (compile + run) ---
    let program = aura_parse::parse_source(&src_aug)?;
    let mut checker = aura_core::Checker::new();
    checker.set_defer_range_proofs(true);
    checker.check_program(&program)?;

    let module_ir = aura_core::lower_program(&program)?;
    let debug = DebugSource::new("parity.aura".to_string(), &src_aug);
    let artifacts = aura_backend_c::emit_module(&module_ir, Some(&debug))?;

    let out_dir = PathBuf::from("build").join("parity_test");
    std::fs::create_dir_all(&out_dir).into_diagnostic()?;
    let runtime_h = out_dir.join("aura_runtime.h");
    let module_c = out_dir.join("module.c");
    std::fs::write(&runtime_h, artifacts.runtime_h).into_diagnostic()?;
    std::fs::write(&module_c, artifacts.module_c).into_diagnostic()?;

    let exe = out_dir.join("parity_test.exe");
    let stdlib_c = aura_stdlib::stdlib_c_path();
    let stdlib_h = aura_stdlib::stdlib_h_path();
    let stdlib_include_dir = stdlib_h.parent().unwrap();

    let status = Command::new(&cc)
        .arg("-std=c2x")
        .arg("-O2")
        .arg(format!("-I{}", stdlib_include_dir.display()))
        .arg(&module_c)
        .arg(stdlib_c)
        .arg("-o")
        .arg(&exe)
        .current_dir(&out_dir)
        .status()
        .into_diagnostic()?;

    if !status.success() {
        return Err(miette::miette!("C compilation failed: {status}"));
    }

    let output = Command::new(&exe).output().into_diagnostic()?;
    if !output.status.success() {
        return Err(miette::miette!("C program failed: {}", output.status));
    }
    let c_stdout = String::from_utf8_lossy(&output.stdout).to_string();

    assert_eq!(dev_stdout, c_stdout);
    Ok(())
}
